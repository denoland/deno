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
      
      let a = args.get(1);
      let b = args.get(2);
        
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