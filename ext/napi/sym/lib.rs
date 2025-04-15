// Copyright 2018-2025 the Deno authors. MIT license.

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
    "cli/napi/sym/symbol_exports.json is out of sync!"
  );

  TokenStream::from(quote! {
    crate::napi_wrap! {
      #func
    }
  })
}
