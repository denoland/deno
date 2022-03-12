// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro_crate::crate_name;
use proc_macro_crate::FoundCrate;
use quote::quote;
use syn::Ident;

fn core_import() -> TokenStream2 {
  let found_crate =
    crate_name("deno_core").expect("deno_core not present in `Cargo.toml`");

  match found_crate {
    FoundCrate::Itself => {
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

  let v8_body = match func.sig.asyncness.is_some() {
    true => codegen_v8_async(&core, &func),
    false => codegen_v8_sync(&core, &func),
  };

  // Generate wrapper
  quote! {
    #[allow(non_camel_case_types)]
    pub struct #name;

    impl #name {
      pub fn name() -> &'static str {
        stringify!(#name)
      }

      pub fn v8_cb #generics () -> #core::v8::FunctionCallback #where_clause {
        use #core::v8::MapFnTo;
        Self::v8_func::<#type_params>.map_fn_to()
      }

      pub fn decl #generics ()  -> (&'static str, #core::v8::FunctionCallback) #where_clause {
        (Self::name(), Self::v8_cb::<#type_params>())
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
  let a = codegen_arg(core, &f.sig.inputs[1], "a", 2);
  let b = codegen_arg(core, &f.sig.inputs[2], "b", 3);
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
        #core::bindings::throw_type_error(scope, format!("invalid promise id: {}", err));
        return;
      }
    };

    #a
    #b

    let state_rc = #core::JsRuntime::state(scope);
    let mut state = state_rc.borrow_mut();

    {
      let mut op_state = state.op_state.borrow_mut();
      op_state.tracker.track_async(op_id);
    }

    let op_state = state.op_state.clone();
    state.pending_ops.push(#core::OpCall::eager(async move {
      let result = Self::call::<#type_params>(op_state.clone(), a, b).await;
      (promise_id, op_id, #core::to_op_result(&op_state.borrow(), result))
    }));
    state.have_unpolled_ops = true;
  }
}

/// Generate the body of a v8 func for a sync op
fn codegen_v8_sync(core: &TokenStream2, f: &syn::ItemFn) -> TokenStream2 {
  let a = codegen_arg(core, &f.sig.inputs[1], "a", 2);
  let b = codegen_arg(core, &f.sig.inputs[2], "b", 3);
  let ret = codegen_sync_ret(core, &f.sig.output);
  let type_params = &f.sig.generics.params;

  quote! {
    // SAFETY: Called from Deno.core.opSync. Which retrieves the index using opId table.
    let op_id = unsafe {
      #core::v8::Local::<#core::v8::Integer>::cast(args.get(0)).value()
    } as usize;

    #a
    #b

    // SAFETY: Unchecked cast to external since #core guarantees args.data() is a v8 External.
    let state_refcell_raw = unsafe {
      #core::v8::Local::<#core::v8::External>::cast(args.data().unwrap_unchecked())
    }.value();

    // SAFETY: The Rc<RefCell<OpState>> is functionally pinned and is tied to the isolate's lifetime
    let state = unsafe { &*(state_refcell_raw as *const std::cell::RefCell<#core::OpState>) };

    let mut op_state = state.borrow_mut();
    let result = Self::call::<#type_params>(&mut op_state, a, b);

    op_state.tracker.track_sync(op_id);

    #ret
  }
}

fn codegen_arg(
  core: &TokenStream2,
  arg: &syn::FnArg,
  name: &str,
  idx: i32,
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
    let #ident = args.get(#idx);
    let #ident = match #core::serde_v8::from_v8(scope, #ident) {
      Ok(v) => v,
      Err(err) => {
        // Throw TypeError
        let msg = format!("Error parsing args: {}", #core::anyhow::Error::from(err));
        let message = #core::v8::String::new(scope, msg.as_ref()).unwrap();
        let exception = #core::v8::Exception::type_error(scope, message);
        scope.throw_exception(exception);
        return;
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
  let ok_block = match is_unit_result(&**ret_type) {
    true => quote! {},
    false => quote! {
      let ret = #core::serde_v8::to_v8(scope, v).unwrap();
      rv.set(ret);
    },
  };

  quote! {
    match result {
      Ok(v) => {
        #ok_block
      },
      Err(err) => {
        let err = #core::serde_v8::to_v8(
          scope,
          #core::OpError {
            class_name: (op_state.get_error_class_fn)(&err),
            message: err.to_string(),
            code: #core::error_codes::get_error_code(&err),
          },
        ).unwrap();

        rv.set(err);
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
