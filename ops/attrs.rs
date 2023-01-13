// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use syn::{
  parse::{Parse, ParseStream},
  punctuated::Punctuated,
  Error, Ident, Result, Token,
};

#[derive(Copy, Clone, Debug, Default)]
pub struct Attributes {
  pub is_unstable: bool,
  pub is_v8: bool,
  pub must_be_fast: bool,
  pub deferred: bool,
  pub is_wasm: bool,
}

impl Parse for Attributes {
  fn parse(input: ParseStream) -> Result<Self> {
    let vars = Punctuated::<Ident, Token![,]>::parse_terminated(input)?;

    let vars: Vec<_> = vars.iter().map(Ident::to_string).collect();
    let vars: Vec<_> = vars.iter().map(String::as_str).collect();
    for var in vars.iter() {
      if !["unstable", "v8", "fast", "deferred", "wasm"].contains(var) {
        return Err(Error::new(
          input.span(),
          "invalid attribute, expected one of: unstable, v8, fast, deferred, wasm",
        ));
      }
    }

    let is_wasm = vars.contains(&"wasm");

    Ok(Self {
      is_unstable: vars.contains(&"unstable"),
      is_v8: vars.contains(&"v8"),
      deferred: vars.contains(&"deferred"),
      must_be_fast: is_wasm || vars.contains(&"fast"),
      is_wasm,
    })
  }
}
