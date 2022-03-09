use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn op(attr: TokenStream, item: TokenStream) -> TokenStream {
  let attr = syn::parse_macro_input!(attr as syn::AttributeArgs);
  let func = syn::parse::<syn::ItemFn>(item).expect("expected a function");
  let name = &func.sig.ident;
  let generics = &func.sig.generics;
  let type_params = &func.sig.generics.params;
  let where_clause = &func.sig.generics.where_clause;

  // Should the macro preserve the original op?
  // Note that the function is renamed to `original_<NAME>`.
  // This is useful for testing purposes.
  //
  // #[op(preserve_original)]
  let preserve_original = match attr.get(0).as_ref() {
    Some(syn::NestedMeta::Meta(syn::Meta::Path(ref attr_ident))) => {
      if attr_ident.is_ident("preserve_original") {
        let mut func = func.clone();
        func.sig.ident = quote::format_ident!("original_{}", &func.sig.ident);
        quote! { #func }
      } else {
        quote! {}
      }
    }
    _ => quote! {},
  };

  let inputs = &func.sig.inputs;
  let output = &func.sig.output;
  let a = match &inputs[1] {
    syn::FnArg::Typed(pat) => match *pat.pat {
      syn::Pat::Wild(_) => quote! {
        let a = ();
      },
      _ => quote! {
        let a = args.get(2);
        let a = deno_core::serde_v8::from_v8(scope, a).unwrap();
      },
    },
    _ => unreachable!(),
  };

  let b = match &inputs[2] {
    syn::FnArg::Typed(pat) => match *pat.pat {
      syn::Pat::Wild(_) => quote! {
        let b = ();
      },
      _ => quote! {
        let b = args.get(3);
        let b = deno_core::serde_v8::from_v8(scope, b).unwrap();
      },
    },
    _ => unreachable!(),
  };

  // TODO(@littledivy): Optimize Result<(), Err> to skip serde_v8.
  let ret = match &output {
    syn::ReturnType::Default => quote! {
      let ret = ();
    },
    _ => quote! {
      let ret = deno_core::serde_v8::to_v8(scope, result).unwrap();
      rv.set(ret);
    },
  };

  let is_async = func.sig.asyncness.is_some();
  if is_async {
    TokenStream::from(quote! {
      pub fn #name #generics (
        scope: &mut deno_core::v8::HandleScope,
        args: deno_core::v8::FunctionCallbackArguments,
        mut rv: deno_core::v8::ReturnValue,
      ) #where_clause {
        use deno_core::JsRuntime;
        use deno_core::futures::FutureExt;
        use deno_core::OpCall;
        use deno_core::serialize_op_result;
        use deno_core::PromiseId;
        use deno_core::bindings::throw_type_error;
        use deno_core::v8;
        let op_id = unsafe { v8::Local::<v8::Integer>::cast(args.get(0)) }.value() as usize;

        let promise_id = args.get(1);
        let promise_id = v8::Local::<v8::Integer>::try_from(promise_id)
          .map(|l| l.value() as PromiseId)
          .map_err(deno_core::anyhow::Error::from);
        // Fail if promise id invalid (not an int)
        let promise_id: PromiseId = match promise_id {
          Ok(promise_id) => promise_id,
          Err(err) => {
            throw_type_error(scope, format!("invalid promise id: {}", err));
            return;
          }
        };

        #a
        #b
        #func

        let state_rc = JsRuntime::state(scope);
        let mut state = state_rc.borrow_mut();
        state.op_state.borrow().tracker.track_async(op_id);
        state.have_unpolled_ops = true;
        let op_state = state.op_state.clone();
        state.pending_ops.push(OpCall::eager(async move {
          let result = #name::<#type_params>(op_state.clone(), a, b).await;
          (promise_id, op_id, serialize_op_result(result, op_state))
        }));
      }

      #preserve_original
    })
  } else {
    TokenStream::from(quote! {
      #[inline]
      pub fn #name #generics (
        scope: &mut deno_core::v8::HandleScope,
        args: deno_core::v8::FunctionCallbackArguments,
        mut rv: deno_core::v8::ReturnValue,
      ) #where_clause {
        use deno_core::JsRuntime;
        use deno_core::bindings::throw_type_error;
        use deno_core::OpsTracker;
        use deno_core::v8;

        let op_id = unsafe { v8::Local::<v8::Integer>::cast(args.get(0)) }.value() as usize;

        #a
        #b
        #func

        let state_rc = deno_core::JsRuntime::state(scope);
        let state = state_rc.borrow();
        let mut op_state = state.op_state.borrow_mut();

        let result = #name::<#type_params>(&mut op_state, a, b).unwrap();
        op_state.tracker.track_sync(op_id);

        #ret
      }

      #preserve_original
    })
  }
}
