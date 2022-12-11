// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use proc_macro::TokenStream;
use quote::quote;
use serde::Deserialize;

static NAPI_EXPORTS: &str = include_str!("./symbol_exports.json");

#[derive(Deserialize)]
struct SymbolExports {
  pub symbols: Vec<String>,
}

#[proc_macro_attribute]
pub fn napi_sym(_attr: TokenStream, item: TokenStream) -> TokenStream {
  let func = syn::parse::<syn::ItemFn>(item).expect("expected a function");

  let exports: SymbolExports =
    serde_json::from_str(NAPI_EXPORTS).expect("failed to parse exports");
  let name = &func.sig.ident;
  assert!(
    exports.symbols.contains(&name.to_string()),
    "tools/napi/sym/symbol_exports.json is out of sync!"
  );

  let block = &func.block;
  let inputs = &func.sig.inputs;
  let output = &func.sig.output;
  let generics = &func.sig.generics;
  let ret_ty = match output {
    syn::ReturnType::Default => panic!("expected a return type"),
    syn::ReturnType::Type(_, ty) => quote! { #ty },
  };
  TokenStream::from(quote! {
      // SAFETY: it's an NAPI function.
      #[no_mangle]
      pub unsafe extern "C" fn #name #generics (#inputs) -> napi_status {
        let mut inner = || -> #ret_ty {
          #block
        };
        inner()
          .map(|_| napi_ok)
          .unwrap_or_else(|e| e.into())
      }
  })
}
