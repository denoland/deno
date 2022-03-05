use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn op(_attr: TokenStream, item: TokenStream) -> TokenStream {
  let func = syn::parse::<syn::ItemFn>(item).expect("expected a function");
  let name = &func.sig.ident;
  TokenStream::from(quote! {
    pub fn #name<'s>(
      scope: &mut v8::HandleScope<'s>,
      args: v8::FunctionCallbackArguments,
      mut rv: v8::ReturnValue,
    ) {
      use crate::JsRuntime;

      let a = args.get(0);
      let b = args.get(1);

      #func

      let a = serde_v8::from_v8(scope, a).unwrap();
      let b = serde_v8::from_v8(scope, b).unwrap();
      let state_rc = JsRuntime::state(scope);
      let state = state_rc.borrow_mut();
      let result = #name(&mut state.op_state.borrow_mut(), a, b).unwrap();

      let ret = serde_v8::to_v8(scope, result).unwrap();
      rv.set(ret);
    }
  })
}

#[proc_macro_attribute]
pub fn op_async(_attr: TokenStream, item: TokenStream) -> TokenStream {
  let func = syn::parse::<syn::ItemFn>(item).expect("expected a function");
  let name = &func.sig.ident;
  TokenStream::from(quote! {
    pub fn #name<'s>(
      scope: &mut v8::HandleScope<'s>,
      args: v8::FunctionCallbackArguments,
      mut rv: v8::ReturnValue,
    ) {
      use crate::JsRuntime;
      use futures::FutureExt;
      use crate::OpCall;
      use crate::serialize_op_result;
      use crate::PromiseId;
      use crate::bindings::throw_type_error;
      
      let promise_id = args.get(0);
      let promise_id = v8::Local::<v8::Integer>::try_from(promise_id)
        .map(|l| l.value() as PromiseId)
        .map_err(Error::from);
      // Fail if promise id invalid (not an int)
      let promise_id: PromiseId = match promise_id {
        Ok(promise_id) => promise_id,
        Err(err) => {
          throw_type_error(scope, format!("invalid promise id: {}", err));
          return;
        }
      };

      let a = args.get(1);
      let b = args.get(2);

      #func

      let a = serde_v8::from_v8(scope, a).unwrap();
      let b = serde_v8::from_v8(scope, b).unwrap();
      let state_rc = JsRuntime::state(scope);
      let mut state = state_rc.borrow_mut();
      let op_state = state.op_state.clone();
      let fut = async move {
        let result = #name(op_state.clone(), a, b).await;
        (promise_id, serialize_op_result(result, op_state))
      };

      state.pending_ops.push(OpCall::eager(fut));
      state.have_unpolled_ops = true;
    }
  })
}
