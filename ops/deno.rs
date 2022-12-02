// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
#![cfg(not(test))]

use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::Ident;

/// Identifier to the `deno_core` crate.
///
/// If macro called in deno_core, `crate` is used.
/// If macro called outside deno_core, `deno_core` OR the renamed
/// version from Cargo.toml is used.
pub(crate) fn import() -> TokenStream {
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
