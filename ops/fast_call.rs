use crate::optimizer::FastValue;
/// Code generation for V8 fast calls.
use crate::optimizer::Optimizer;
use pmutil::{q, Quote, ToTokensExt};
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
  parse_quote, punctuated::Punctuated, token::Comma, Fields, Ident, ItemFn,
  ItemImpl, ItemStruct, Path, PathArguments, PathSegment, Token, Type,
  TypePath, Visibility,
};

pub(crate) fn generate(
  optimizer: &mut Optimizer,
  item_fn: &ItemFn,
) -> Result<TokenStream, ()> {
  // TODO(@littledivy): Use `let..else` on 1.65.0
  let output_ty = optimizer.fast_result.as_ref().ok_or(())?;

  let ident = item_fn.sig.ident.clone();
  let mut segments = Punctuated::new();
  segments.push_value(PathSegment {
    ident: ident.clone(),
    arguments: PathArguments::None,
  });

  // struct T <A> {
  //   _phantom: ::std::marker::PhantomData<A>,
  // }
  let fast_ty: Quote = q!(Vars { Type: &ident }, {
    struct Type {
      _phantom: ::std::marker::PhantomData<()>,
    }
  });

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
    generics: Default::default(),
    trait_: None,
    self_ty: Box::new(Type::Path(TypePath {
      qself: None,
      path: Path {
        leading_colon: None,
        segments,
      },
    })),
    brace_token: Default::default(),
    items: vec![],
  };

  let mut inputs = item_fn.sig.inputs.clone();
  let mut transforms = q!({});
  // Apply parameter transforms
  for (input, transform) in inputs.iter_mut().zip(optimizer.transforms.iter()) {
    let quo: Quote = transform.apply_for_fast_call(input);
    transforms.push_tokens(&quo);
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

  // Apply *hard* optimizer hints.
  if optimizer.has_fast_callback_option || optimizer.has_opstate() {
    inputs.push(parse_quote! {
      fast_api_callback_options: *mut v8::fast_api::FastApiCallbackOptions
    });
  }

  let mut output_transforms = q!({});

  if optimizer.has_opstate() {
    // Grab the op_state identifier, the first one. ¯\_(ツ)_/¯
    let op_state = idents.first().expect("This whole thing is broken");

    // Dark arts 🪄 ✨
    // 
    // - V8 calling convention guarantees that the callback options pointer is non-null.
    // - `data` union is always initialized as the `v8::Local<v8::Value>` variant.
    // - deno_core guarantees that `data` is a v8 External pointing to an OpCtx for the 
    //   isolate's lifetime.
    let prelude = q!(Vars { op_state }, {
      let opts: &mut v8::fast_api::FastApiCallbackOptions =
        unsafe { &mut *fast_api_callback_options };
      let data = unsafe { opts.data.data };
      let ctx = unsafe {
        &*(v8::Local::<v8::External>::cast(data).value() as *const _ops::OpCtx)
      };
      let op_state = &mut std::cell::RefCell::borrow_mut(&ctx.state);
    });

    transforms.push_tokens(&prelude);

    if optimizer.returns_result {
      let result_wrap = q!(Vars { op_state }, {
        match result {
          Ok(result) => result,
          Err(err) => {
            op_state.last_fast_op_error.replace(err);
            opts.fallback = true;
            Default::default()
          }
        }
      });

      output_transforms.push_tokens(&result_wrap);
    }
  }

  let output = q_fast_ty(&output_ty);

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
    Vars { op_name: &ident, inputs, idents, transforms, output },
    {
      fn op_name(_: v8::Local<v8::Object>, inputs) -> output {
        transforms
        let result = op_name::call(idents);
      }
    }
  );

  let mut tts = q!({});
  tts.push_tokens(&fast_ty);
  tts.push_tokens(&item);
  tts.push_tokens(&fast_fn);

  Ok(tts.dump().into())
}

/// Quote fast value type.
fn q_fast_ty(v: &FastValue) -> Quote {
  match v {
    Void => q!({ () }),
    U32 => q!({ u32 }),
    I32 => q!({ i32 }),
    U64 => q!({ u64 }),
    I64 => q!({ i64 }),
    F32 => q!({ f32 }),
    F64 => q!({ f64 }),
    Bool => q!({ bool }),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::attrs::Attributes;
  use crate::Op;
  use std::path::PathBuf;

  #[testing::fixture("optimizer_tests/**/*.rs")]
  fn test_fast_call_codegen(input: PathBuf) {
    let update_expected = std::env::var("UPDATE_EXPECTED").is_ok();

    let source =
      std::fs::read_to_string(&input).expect("Failed to read test file");
    let expected = std::fs::read_to_string(input.with_extension("out"))
      .expect("Failed to read expected file");

    let item = syn::parse_str(&source).expect("Failed to parse test file");
    let mut op = Op::new(item, Default::default());
    let mut optimizer = Optimizer::new();
    optimizer.analyze(&mut op).expect("Optimizer failed");

    let actual = generate(&mut optimizer, &op.item).unwrap();
    // Validate syntax tree.
    let tree = syn::parse2(actual).unwrap();
    let actual = prettyplease::unparse(&tree);
    if update_expected {
      std::fs::write(input.with_extension("out"), actual.to_string())
        .expect("Failed to write expected file");
    } else {
      assert_eq!(actual.to_string(), expected);
    }
  }
}
