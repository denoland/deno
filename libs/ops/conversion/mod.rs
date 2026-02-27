// Copyright 2018-2025 the Deno authors. MIT license.

use syn::Attribute;
use syn::Error;
use syn::LitStr;
use syn::Token;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;

pub mod from_v8;
pub mod to_v8;

mod kw {
  syn::custom_keyword!(rename);
  syn::custom_keyword!(serde);
}

#[allow(dead_code)]
enum StructFieldArgumentShared {
  Rename {
    name_token: kw::rename,
    eq_token: Token![=],
    value: LitStr,
  },
  Serde {
    name_token: kw::serde,
  },
}

impl Parse for StructFieldArgumentShared {
  fn parse(input: ParseStream) -> Result<Self, Error> {
    let lookahead = input.lookahead1();
    if lookahead.peek(kw::rename) {
      Ok(StructFieldArgumentShared::Rename {
        name_token: input.parse()?,
        eq_token: input.parse()?,
        value: input.parse()?,
      })
    } else if lookahead.peek(kw::serde) {
      Ok(StructFieldArgumentShared::Serde {
        name_token: input.parse()?,
      })
    } else {
      Err(lookahead.error())
    }
  }
}

struct SharedAttribute {
  pub rename: Option<String>,
  pub serde: bool,
}

impl StructFieldArgumentShared {
  fn parse(attrs: &[Attribute]) -> syn::Result<SharedAttribute> {
    let mut rename: Option<String> = None;
    let mut serde = false;

    for attr in attrs {
      if attr.path().is_ident("v8") {
        let list = attr.meta.require_list()?;
        let args = list.parse_args_with(
          Punctuated::<StructFieldArgumentShared, Token![,]>::parse_terminated,
        )?;

        for argument in args {
          match argument {
            StructFieldArgumentShared::Rename { value, .. } => {
              rename = Some(value.value())
            }
            StructFieldArgumentShared::Serde { .. } => serde = true,
          }
        }
      }
    }

    Ok(SharedAttribute { rename, serde })
  }
}

#[allow(dead_code)]
enum StructTupleFieldArgumentShared {
  Serde { name_token: kw::serde },
}

impl Parse for StructTupleFieldArgumentShared {
  fn parse(input: ParseStream) -> Result<Self, Error> {
    let lookahead = input.lookahead1();
    if lookahead.peek(kw::serde) {
      Ok(StructTupleFieldArgumentShared::Serde {
        name_token: input.parse()?,
      })
    } else {
      Err(lookahead.error())
    }
  }
}

struct SharedTupleAttribute {
  pub serde: bool,
}

impl StructTupleFieldArgumentShared {
  fn parse(attrs: &[Attribute]) -> syn::Result<SharedTupleAttribute> {
    let mut serde = false;

    for attr in attrs {
      if attr.path().is_ident("v8") {
        let list = attr.meta.require_list()?;
        let args = list.parse_args_with(
          Punctuated::<StructTupleFieldArgumentShared, Token![,]>::parse_terminated,
        )?;

        for argument in args {
          match argument {
            StructTupleFieldArgumentShared::Serde { .. } => serde = true,
          }
        }
      }
    }

    Ok(SharedTupleAttribute { serde })
  }
}
