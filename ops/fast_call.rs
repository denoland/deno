/// Code generation for V8 fast calls.
use crate::optimizer::FastValue;
use crate::optimizer::Optimizer;
use pmutil::{q, Quote, ToTokensExt};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
  parse_quote, punctuated::Punctuated, token::Comma, GenericParam, Generics,
  Ident, ItemFn, ItemImpl, Path, PathArguments, PathSegment, Type, TypePath,
};

pub(crate) struct FastImplItems {
  pub(crate) impl_and_fn: TokenStream,
  pub(crate) decl: TokenStream,
  pub(crate) active: bool,
}

pub(crate) fn generate(
  core: &TokenStream,
  optimizer: &mut Optimizer,
  item_fn: &ItemFn,
) -> FastImplItems {
  if !optimizer.fast_compatible {
    return FastImplItems {
      impl_and_fn: TokenStream::new(),
      decl: quote! { None },
      active: false,
    };
  }

  // TODO(@littledivy): Use `let..else` on 1.65.0
  let output_ty = match &optimizer.fast_result {
    // Assert that the optimizer did not set a return type.
    //
    // @littledivy: This *could* potentially be used to optimize resolving
    // promises but knowing the return type at compile time instead of
    // serde_v8 serialization.
    Some(_) if optimizer.is_async => &FastValue::Void,
    Some(ty) => ty,
    None if optimizer.is_async => &FastValue::Void,
    None => {
      return FastImplItems {
        impl_and_fn: TokenStream::new(),
        decl: quote! { None },
        active: false,
      }
    }
  };

  // We've got 3 idents.
  //
  // - op_foo, the public op declaration contains the user function.
  // - op_foo_fast, the fast call type.
  // - op_foo_fast_fn, the fast call function.
  let ident = item_fn.sig.ident.clone();
  let fast_ident = Ident::new(&format!("{}_fast", ident), Span::call_site());
  let fast_fn_ident =
    Ident::new(&format!("{}_fast_fn", ident), Span::call_site());

  // Deal with generics.
  let generics = &item_fn.sig.generics;
  let (impl_generics, _, where_clause) = generics.split_for_impl();

  // struct op_foo_fast <T, U> { ... }
  let struct_generics = exclude_lifetime_params(&generics.params);
  // std::marker::PhantomData <A>
  let phantom_generics: Quote = match struct_generics {
    Some(ref params) => q!(Vars { params }, { params }),
    None => q!({ <()> }),
  };
  // op_foo_fast_fn :: <T>
  let caller_generics: Quote = match struct_generics {
    Some(ref params) => q!(Vars { params }, { ::params }),
    None => q!({}),
  };

  // This goes in the FastFunction impl block.
  let mut segments = Punctuated::new();
  {
    let mut arguments = PathArguments::None;
    if let Some(ref struct_generics) = struct_generics {
      arguments = PathArguments::AngleBracketed(parse_quote! {
        #struct_generics
      });
    }
    segments.push_value(PathSegment {
      ident: fast_ident.clone(),
      arguments,
    });
  }

  // struct T <A> {
  //   _phantom: ::std::marker::PhantomData<A>,
  // }
  let fast_ty: Quote = q!(Vars { Type: &fast_ident, generics: &struct_generics, phantom_generics }, {
    struct Type generics {
      _phantom: ::std::marker::PhantomData phantom_generics,
    }
  });

  // Original inputs.
  let mut inputs = item_fn.sig.inputs.clone();
  let mut transforms = q!({});
  let mut pre_transforms = q!({});

  // Apply parameter transforms
  for (index, input) in inputs.iter_mut().enumerate() {
    if let Some(transform) = optimizer.transforms.get(&index) {
      let quo: Quote = transform.apply_for_fast_call(core, input);
      transforms.push_tokens(&quo);
    }
  }

  // Collect idents to be passed into function call, we can now freely
  // modify the inputs.
  let idents = inputs
    .iter()
    .map(|input| match input {
      syn::FnArg::Typed(pat_type) => match &*pat_type.pat {
        syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
        _ => panic!("unexpected pattern"),
      },
      _ => panic!("unexpected argument"),
    })
    .collect::<Punctuated<_, Comma>>();

  // Retain only *pure* parameters.
  let mut fast_fn_inputs = if optimizer.has_opstate_in_parameters() {
    inputs.iter().skip(1).cloned().collect()
  } else {
    inputs.clone()
  };

  let mut input_variants = optimizer
    .fast_parameters
    .iter()
    .map(q_fast_ty_variant)
    .collect::<Punctuated<_, Comma>>();

  // Apply *hard* optimizer hints.
  if optimizer.has_fast_callback_option
    || optimizer.has_wasm_memory
    || optimizer.needs_opstate()
    || optimizer.is_async
    || optimizer.needs_fast_callback_option
  {
    let decl = parse_quote! {
      fast_api_callback_options: *mut #core::v8::fast_api::FastApiCallbackOptions
    };

    if optimizer.has_fast_callback_option || optimizer.has_wasm_memory {
      // Replace last parameter.
      assert!(fast_fn_inputs.pop().is_some());
      fast_fn_inputs.push(decl);
    } else {
      fast_fn_inputs.push(decl);
    }

    input_variants.push(q!({ CallbackOptions }));
  }

  // (recv, p_id, ...)
  //
  // Optimizer has already set it in the fast parameter variant list.
  if optimizer.is_async {
    if fast_fn_inputs.is_empty() {
      fast_fn_inputs.push(parse_quote! { __promise_id: i32 });
    } else {
      fast_fn_inputs.insert(0, parse_quote! { __promise_id: i32 });
    }
  }

  let mut output_transforms = q!({});

  if optimizer.needs_opstate()
    || optimizer.is_async
    || optimizer.has_fast_callback_option
    || optimizer.has_wasm_memory
  {
    // Dark arts 🪄 ✨
    //
    // - V8 calling convention guarantees that the callback options pointer is non-null.
    // - `data` union is always initialized as the `v8::Local<v8::Value>` variant.
    // - deno_core guarantees that `data` is a v8 External pointing to an OpCtx for the
    //   isolate's lifetime.
    let prelude = q!({
      let __opts: &mut v8::fast_api::FastApiCallbackOptions =
        unsafe { &mut *fast_api_callback_options };
    });

    pre_transforms.push_tokens(&prelude);
  }

  if optimizer.needs_opstate() || optimizer.is_async {
    // Grab the op_state identifier, the first one. ¯\_(ツ)_/¯
    let op_state = match idents.first() {
      Some(ident) if optimizer.has_opstate_in_parameters() => ident.clone(),
      // fn op_foo() -> Result<...>
      _ => Ident::new("op_state", Span::call_site()),
    };

    let ctx = q!({
      let __ctx = unsafe {
        &*(v8::Local::<v8::External>::cast(unsafe { __opts.data.data }).value()
          as *const _ops::OpCtx)
      };
    });

    pre_transforms.push_tokens(&ctx);
    pre_transforms.push_tokens(&match optimizer.is_async {
      false => q!(
        Vars {
          op_state: &op_state
        },
        {
          let op_state = &mut ::std::cell::RefCell::borrow_mut(&__ctx.state);
        }
      ),
      true => q!(
        Vars {
          op_state: &op_state
        },
        {
          let op_state = __ctx.state.clone();
        }
      ),
    });

    if optimizer.returns_result && !optimizer.is_async {
      // Magic fallback 🪄
      //
      // If Result<T, E> is Ok(T), return T as fast value.
      //
      // Err(E) gets put into `last_fast_op_error` slot and
      //
      // V8 calls the slow path so we can take the slot
      // value and throw.
      let result_wrap = q!(Vars { op_state }, {
        match result {
          Ok(result) => result,
          Err(err) => {
            op_state.last_fast_op_error.replace(err);
            __opts.fallback = true;
            Default::default()
          }
        }
      });

      output_transforms.push_tokens(&result_wrap);
    }
  }

  if optimizer.is_async {
    // Referenced variables are declared in parent block.
    let track_async = q!({
      let __op_id = __ctx.id;
      let __state = ::std::cell::RefCell::borrow(&__ctx.state);
      __state.tracker.track_async(__op_id);
    });

    output_transforms.push_tokens(&track_async);

    let queue_future = if optimizer.returns_result {
      q!({
        let __get_class = __state.get_error_class_fn;
        let result = _ops::queue_fast_async_op(__ctx, async move {
          let result = result.await;
          (
            __promise_id,
            __op_id,
            _ops::to_op_result(__get_class, result),
          )
        });
      })
    } else {
      q!({
        let result = _ops::queue_fast_async_op(__ctx, async move {
          let result = result.await;
          (__promise_id, __op_id, _ops::OpResult::Ok(result.into()))
        });
      })
    };

    output_transforms.push_tokens(&queue_future);
  }

  if !optimizer.returns_result {
    let default_output = q!({ result });
    output_transforms.push_tokens(&default_output);
  }

  let output = q_fast_ty(output_ty);
  // Generate the function body.
  //
  // fn f <S> (_: Local<Object>, a: T, b: U) -> R {
  //   /* Transforms */
  //   let a = a.into();
  //   let b = b.into();
  //
  //   let r = op::call(a, b);
  //
  //   /* Return transform */
  //   r.into()
  // }
  let fast_fn = q!(
    Vars { core, pre_transforms, op_name_fast: &fast_fn_ident, op_name: &ident, fast_fn_inputs, generics, call_generics: &caller_generics, where_clause, idents, transforms, output_transforms, output: &output },
    {
      fn op_name_fast generics (_: core::v8::Local<core::v8::Object>, fast_fn_inputs) -> output where_clause {
        use core::v8;
        use core::_ops;
        pre_transforms
        transforms
        let result = op_name::call call_generics (idents);
        output_transforms
      }
    }
  );

  let output_variant = q_fast_ty_variant(output_ty);
  let mut generics: Generics = parse_quote! { #impl_generics };
  generics.where_clause = where_clause.cloned();

  // impl <A> fast_api::FastFunction for T <A> where A: B {
  //   fn function(&self) -> *const ::std::ffi::c_void  {
  //     f as *const ::std::ffi::c_void
  //   }
  //   fn args(&self) -> &'static [fast_api::Type] {
  //     &[ CType::T, CType::U ]
  //   }
  //   fn return_type(&self) -> fast_api::CType {
  //     CType::T
  //   }
  // }
  let item: ItemImpl = ItemImpl {
    attrs: vec![],
    defaultness: None,
    unsafety: None,
    impl_token: Default::default(),
    generics,
    trait_: Some((
      None,
      parse_quote!(#core::v8::fast_api::FastFunction),
      Default::default(),
    )),
    self_ty: Box::new(Type::Path(TypePath {
      qself: None,
      path: Path {
        leading_colon: None,
        segments,
      },
    })),
    brace_token: Default::default(),
    items: vec![
      parse_quote! {
        fn function(&self) -> *const ::std::ffi::c_void {
          #fast_fn_ident #caller_generics as *const ::std::ffi::c_void
        }
      },
      parse_quote! {
        fn args(&self) -> &'static [#core::v8::fast_api::Type] {
          use #core::v8::fast_api::Type::*;
          use #core::v8::fast_api::CType;
          &[ #input_variants ]
        }
      },
      parse_quote! {
        fn return_type(&self) -> #core::v8::fast_api::CType {
          #core::v8::fast_api::CType::#output_variant
        }
      },
    ],
  };

  let mut tts = q!({});
  tts.push_tokens(&fast_ty);
  tts.push_tokens(&item);
  tts.push_tokens(&fast_fn);

  let impl_and_fn = tts.dump();
  let decl = q!(
    Vars { fast_ident, caller_generics },
    {
      Some(Box::new(fast_ident caller_generics { _phantom: ::std::marker::PhantomData }))
    }
  ).dump();

  FastImplItems {
    impl_and_fn,
    decl,
    active: true,
  }
}

/// Quote fast value type.
fn q_fast_ty(v: &FastValue) -> Quote {
  match v {
    FastValue::Void => q!({ () }),
    FastValue::U32 => q!({ u32 }),
    FastValue::I32 => q!({ i32 }),
    FastValue::U64 => q!({ u64 }),
    FastValue::I64 => q!({ i64 }),
    FastValue::F32 => q!({ f32 }),
    FastValue::F64 => q!({ f64 }),
    FastValue::Bool => q!({ bool }),
    FastValue::V8Value => q!({ v8::Local<v8::Value> }),
    FastValue::Uint8Array | FastValue::Uint32Array => unreachable!(),
  }
}

/// Quote fast value type's variant.
fn q_fast_ty_variant(v: &FastValue) -> Quote {
  match v {
    FastValue::Void => q!({ Void }),
    FastValue::U32 => q!({ Uint32 }),
    FastValue::I32 => q!({ Int32 }),
    FastValue::U64 => q!({ Uint64 }),
    FastValue::I64 => q!({ Int64 }),
    FastValue::F32 => q!({ Float32 }),
    FastValue::F64 => q!({ Float64 }),
    FastValue::Bool => q!({ Bool }),
    FastValue::V8Value => q!({ V8Value }),
    FastValue::Uint8Array => q!({ TypedArray(CType::Uint8) }),
    FastValue::Uint32Array => q!({ TypedArray(CType::Uint32) }),
  }
}

fn exclude_lifetime_params(
  generic_params: &Punctuated<GenericParam, Comma>,
) -> Option<Generics> {
  let params = generic_params
    .iter()
    .filter(|t| !matches!(t, GenericParam::Lifetime(_)))
    .cloned()
    .collect::<Punctuated<GenericParam, Comma>>();
  if params.is_empty() {
    // <()>
    return None;
  }
  Some(Generics {
    lt_token: Some(Default::default()),
    params,
    gt_token: Some(Default::default()),
    where_clause: None,
  })
}
