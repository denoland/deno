use proc_macro::TokenStream;
use quote::quote;

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
    quote! { #func }
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
        let a = match deno_core::serde_v8::from_v8(scope, a) {
          Ok(v) => v,
          Err(err) => {
            // Throw TypeError
            let msg = format!("Error parsing args: {}", deno_core::anyhow::Error::from(err));
            let message = v8::String::new(scope, msg.as_ref()).unwrap();
            let exception = v8::Exception::type_error(scope, message);
            scope.throw_exception(exception);
            return;
          }
        };
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
        let b = match deno_core::serde_v8::from_v8(scope, b) {
          Ok(v) => v,
          Err(err) => {
            // Throw TypeError
            let msg = format!("Error parsing args: {}", deno_core::anyhow::Error::from(err));
            let message = v8::String::new(scope, msg.as_ref()).unwrap();
            let exception = v8::Exception::type_error(scope, message);
            scope.throw_exception(exception);
            return;
          }
        };
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
      let ret = deno_core::serialize_op_result(scope, &op_state2.borrow(), result).unwrap();
      rv.set(ret);
    },
  };

  let is_async = func.sig.asyncness.is_some();
  if is_async {
    TokenStream::from(quote! {
      #[allow(non_camel_case_types)]
      pub struct #name;

      impl #name {
        pub fn name() -> &'static str {
          stringify!(#name)
        }

        pub fn v8_cb #generics () -> deno_core::v8::FunctionCallback #where_clause {
          use deno_core::v8::MapFnTo;
          Self::v8_func::<#type_params>.map_fn_to()
        }

        pub fn decl #generics ()  -> (&'static str, deno_core::v8::FunctionCallback) #where_clause {
          (Self::name(), Self::v8_cb::<#type_params>())
        }

        #original_func

        pub fn v8_func #generics (
          scope: &mut deno_core::v8::HandleScope,
          args: deno_core::v8::FunctionCallbackArguments,
          mut rv: deno_core::v8::ReturnValue,
        ) #where_clause {
          use deno_core::JsRuntime;
          use deno_core::futures::FutureExt;
          use deno_core::OpCall;
          use deno_core::to_op_result;
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
          let op_state2 = op_state.clone();
          state.pending_ops.push(OpCall::eager(async move {
            let result = #name::<#type_params>(op_state.clone(), a, b).await;
            (promise_id, op_id, to_op_result(&op_state.borrow(), result))
          }));
        }
      }
    })
  } else {
    TokenStream::from(quote! {
      #[allow(non_camel_case_types)]
      pub struct #name;

      impl #name {
        pub fn name() -> &'static str {
          stringify!(#name)
        }

        pub fn v8_cb #generics () -> deno_core::v8::FunctionCallback #where_clause {
          use deno_core::v8::MapFnTo;
          Self::v8_func::<#type_params>.map_fn_to()
        }

        pub fn decl #generics ()  -> (&'static str, deno_core::v8::FunctionCallback) #where_clause {
          (Self::name(), Self::v8_cb::<#type_params>())
        }

        #original_func

        #[inline]
        pub fn v8_func #generics (
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
          let op_state2 = state.op_state.clone();

          let result = #name::<#type_params>(&mut op_state2.borrow_mut(), a, b);
          op_state2.borrow_mut().tracker.track_sync(op_id);

          #ret
        }
      }
    })
  }
}
