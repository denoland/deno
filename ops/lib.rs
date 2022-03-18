// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro_crate::crate_name;
use proc_macro_crate::FoundCrate;
use quote::quote;
use quote::ToTokens;
use syn::Ident;

// Identifer to the `deno_core` crate.
//
// If macro called in deno_core, `crate` is used.
// If macro called outside deno_core, `deno_core` OR the renamed
// version from Cargo.toml is used.
fn core_import() -> TokenStream2 {
  let found_crate =
    crate_name("deno_core").expect("deno_core not present in `Cargo.toml`");

  match found_crate {
    FoundCrate::Itself => {
      // TODO(@littledivy): This won't work for `deno_core` examples
      // since `crate` does not refer to `deno_core`.
      // examples must re-export deno_core to make this work
      // until Span inspection APIs are stabalized.
      //
      // https://github.com/rust-lang/rust/issues/54725
      quote!(crate)
    }
    FoundCrate::Name(name) => {
      let ident = Ident::new(&name, Span::call_site());
      quote!(#ident)
    }
  }
}

#[proc_macro_attribute]
pub fn op(_attr: TokenStream, item: TokenStream) -> TokenStream {
  let func = syn::parse::<syn::ItemFn>(item).expect("expected a function");
  let name = &func.sig.ident;
  let generics = &func.sig.generics;
  let type_params = &func.sig.generics.params;
  let where_clause = &func.sig.generics.where_clause;

  // Preserve the original func as op_foo::call()
  let original_func = {
    let mut func = func.clone();
    func.sig.ident = quote::format_ident!("call");
    func
  };

  let core = core_import();

  let v8_body = if func.sig.asyncness.is_some() {
    codegen_v8_async(&core, &func)
  } else {
    codegen_v8_sync(&core, &func)
  };

  // Generate wrapper
  quote! {
    #[allow(non_camel_case_types)]
    pub struct #name;

    impl #name {
      pub fn name() -> &'static str {
        stringify!(#name)
      }

      pub fn v8_fn_ptr #generics () -> #core::v8::FunctionCallback #where_clause {
        use #core::v8::MapFnTo;
        Self::v8_func::<#type_params>.map_fn_to()
      }

      pub fn decl #generics () -> #core::OpDecl #where_clause {
        #core::OpDecl {
          name: Self::name(),
          v8_fn_ptr: Self::v8_fn_ptr::<#type_params>(),
        }
      }

      #[inline]
      #original_func

      pub fn v8_func #generics (
        scope: &mut #core::v8::HandleScope,
        args: #core::v8::FunctionCallbackArguments,
        mut rv: #core::v8::ReturnValue,
      ) #where_clause {
        #v8_body
      }
    }
  }.into()
}

/// Generate the body of a v8 func for an async op
fn codegen_v8_async(core: &TokenStream2, f: &syn::ItemFn) -> TokenStream2 {
  let arg0 = f.sig.inputs.first();
  let uses_opstate = arg0.map(is_rc_refcell_opstate).unwrap_or_default();
  let args_head = if uses_opstate {
    quote! { state, }
  } else {
    quote! {}
  };
  let rust_i0 = if uses_opstate { 1 } else { 0 };
  let (arg_decls, args_tail) = codegen_args(core, f, rust_i0, 2);
  let type_params = &f.sig.generics.params;

  quote! {
    use #core::futures::FutureExt;
    // SAFETY: Called from Deno.core.opAsync. Which retrieves the index using opId table.
    let op_id = unsafe {
      #core::v8::Local::<#core::v8::Integer>::cast(args.get(0))
    }.value() as usize;

    let promise_id = args.get(1);
    let promise_id = #core::v8::Local::<#core::v8::Integer>::try_from(promise_id)
      .map(|l| l.value() as #core::PromiseId)
      .map_err(#core::anyhow::Error::from);
    // Fail if promise id invalid (not an int)
    let promise_id: #core::PromiseId = match promise_id {
      Ok(promise_id) => promise_id,
      Err(err) => {
        #core::_ops::throw_type_error(scope, format!("invalid promise id: {}", err));
        return;
      }
    };

    #arg_decls

    // SAFETY: Unchecked cast to external since #core guarantees args.data() is a v8 External.
    let state_refcell_raw = unsafe {
      #core::v8::Local::<#core::v8::External>::cast(args.data().unwrap_unchecked())
    }.value();

    // SAFETY: The Rc<RefCell<OpState>> is functionally pinned and is tied to the isolate's lifetime
    let state = unsafe {
      let ptr = state_refcell_raw as *const std::cell::RefCell<#core::OpState>;
      // Increment so it will later be decremented/dropped by the underlaying func it is moved to
      std::rc::Rc::increment_strong_count(ptr);
      std::rc::Rc::from_raw(ptr)
    };
    // Track async call & get copy of get_error_class_fn
    let get_class = {
      let state = state.borrow();
      state.tracker.track_async(op_id);
      state.get_error_class_fn
    };

    #core::_ops::queue_async_op(scope, async move {
      let result = Self::call::<#type_params>(#args_head #args_tail).await;
      (promise_id, op_id, #core::_ops::to_op_result(get_class, result))
    });
  }
}

/// Generate the body of a v8 func for a sync op
fn codegen_v8_sync(core: &TokenStream2, f: &syn::ItemFn) -> TokenStream2 {
  let arg0 = f.sig.inputs.first();
  let uses_opstate = arg0.map(is_mut_ref_opstate).unwrap_or_default();
  let args_head = if uses_opstate {
    quote! { op_state, }
  } else {
    quote! {}
  };
  let rust_i0 = if uses_opstate { 1 } else { 0 };
  let (arg_decls, args_tail) = codegen_args(core, f, rust_i0, 1);
  let ret = codegen_sync_ret(core, &f.sig.output);
  let type_params = &f.sig.generics.params;

  quote! {
    // SAFETY: Called from Deno.core.opSync. Which retrieves the index using opId table.
    let op_id = unsafe {
      #core::v8::Local::<#core::v8::Integer>::cast(args.get(0)).value()
    } as usize;

    #arg_decls

    // SAFETY: Unchecked cast to external since #core guarantees args.data() is a v8 External.
    let state_refcell_raw = unsafe {
      #core::v8::Local::<#core::v8::External>::cast(args.data().unwrap_unchecked())
    }.value();

    // SAFETY: The Rc<RefCell<OpState>> is functionally pinned and is tied to the isolate's lifetime
    let state = unsafe { &*(state_refcell_raw as *const std::cell::RefCell<#core::OpState>) };

    let op_state = &mut state.borrow_mut();
    let result = Self::call::<#type_params>(#args_head #args_tail);

    op_state.tracker.track_sync(op_id);

    #ret
  }
}

fn codegen_args(
  core: &TokenStream2,
  f: &syn::ItemFn,
  rust_i0: usize, // Index of first generic arg in rust
  v8_i0: usize,   // Index of first generic arg in v8/js
) -> (TokenStream2, TokenStream2) {
  let inputs = &f.sig.inputs.iter().skip(rust_i0).enumerate();
  let ident_seq: TokenStream2 = inputs
    .clone()
    .map(|(i, _)| format!("arg_{i}"))
    .collect::<Vec<_>>()
    .join(", ")
    .parse()
    .unwrap();
  let decls: TokenStream2 = inputs
    .clone()
    .map(|(i, arg)| {
      codegen_arg(core, arg, format!("arg_{i}").as_ref(), v8_i0 + i)
    })
    .collect();
  (decls, ident_seq)
}

fn codegen_arg(
  core: &TokenStream2,
  arg: &syn::FnArg,
  name: &str,
  idx: usize,
) -> TokenStream2 {
  let ident = quote::format_ident!("{name}");
  let pat = match arg {
    syn::FnArg::Typed(pat) => &pat.pat,
    _ => unreachable!(),
  };
  // Fast path if arg should be skipped
  if matches!(**pat, syn::Pat::Wild(_)) {
    return quote! { let #ident = (); };
  }
  // Otherwise deserialize it via serde_v8
  quote! {
    let #ident = args.get(#idx as i32);
    let #ident = match #core::serde_v8::from_v8(scope, #ident) {
      Ok(v) => v,
      Err(err) => {
        let msg = format!("Error parsing args at position {}: {}", #idx, #core::anyhow::Error::from(err));
        return #core::_ops::throw_type_error(scope, msg);
      }
    };
  }
}

fn codegen_sync_ret(
  core: &TokenStream2,
  output: &syn::ReturnType,
) -> TokenStream2 {
  let ret_type = match output {
    // Func with no return no-ops
    syn::ReturnType::Default => return quote! { let ret = (); },
    // Func with a return Result<T, E>
    syn::ReturnType::Type(_, ty) => ty,
  };

  // Optimize Result<(), Err> to skip serde_v8 when Ok(...)
  let ok_block = if is_unit_result(&**ret_type) {
    quote! {}
  } else {
    quote! {
      let ret = #core::serde_v8::to_v8(scope, v).unwrap();
      rv.set(ret);
    }
  };

  quote! {
    match result {
      Ok(v) => {
        #ok_block
      },
      Err(err) => {
        let err = #core::OpError::new(op_state.get_error_class_fn, err);
        rv.set(#core::serde_v8::to_v8(scope, err).unwrap());
      },
    };
  }
}

/// Detects if a type is of the form Result<(), Err>
fn is_unit_result(ty: &syn::Type) -> bool {
  let path = match ty {
    syn::Type::Path(ref path) => path,
    _ => return false,
  };

  let maybe_result = path.path.segments.first().expect("Invalid return type.");
  if maybe_result.ident != "Result" {
    return false;
  }
  assert!(!maybe_result.arguments.is_empty());

  let args = match &maybe_result.arguments {
    syn::PathArguments::AngleBracketed(args) => args,
    _ => unreachable!(),
  };

  match args.args.first().unwrap() {
    syn::GenericArgument::Type(syn::Type::Tuple(ty)) => ty.elems.is_empty(),
    _ => false,
  }
}

fn is_mut_ref_opstate(arg: &syn::FnArg) -> bool {
  tokens(arg).ends_with(": & mut OpState")
    || tokens(arg).ends_with(": & mut deno_core :: OpState")
}

fn is_rc_refcell_opstate(arg: &syn::FnArg) -> bool {
  tokens(arg).ends_with(": Rc < RefCell < OpState > >")
    || tokens(arg).ends_with(": Rc < RefCell < deno_core :: OpState > >")
}

fn tokens(x: impl ToTokens) -> String {
  x.to_token_stream().to_string()
}
