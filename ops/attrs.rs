// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::Error;
use syn::Ident;
use syn::Result;
use syn::Token;

#[derive(Clone, Debug, Default)]
pub struct Attributes {
  pub is_unstable: bool,
  pub is_v8: bool,
  pub must_be_fast: bool,
  pub deferred: bool,
  pub is_wasm: bool,
  pub relation: Option<Ident>,
}

impl Parse for Attributes {
  fn parse(input: ParseStream) -> Result<Self> {
    let mut self_ = Self::default();
    let mut fast = false;
    while let Ok(v) = input.parse::<Ident>() {
      match v.to_string().as_str() {
        "unstable" => self_.is_unstable = true,
        "v8" => self_.is_v8 = true,
        "fast" => fast = true,
        "deferred" => self_.deferred = true,
        "wasm" => self_.is_wasm = true,
        "slow" => {
          if !fast {
            return Err(Error::new(
              input.span(),
              "relational attributes can only be used with fast attribute",
            ));
          }
          input.parse::<Token![=]>()?;
          self_.relation = Some(input.parse()?);
        }
        _ => {
          return Err(Error::new(
             input.span(),
            "invalid attribute, expected one of: unstable, v8, fast, deferred, wasm",
            ));
        }
      };
      let _ = input.parse::<Token![,]>();
    }

    self_.must_be_fast = self_.is_wasm || fast;

    Ok(self_)
  }
}
