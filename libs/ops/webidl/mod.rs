// Copyright 2018-2025 the Deno authors. MIT license.

mod dictionary;
mod r#enum;

use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::quote;
use syn::Attribute;
use syn::Data;
use syn::DeriveInput;
use syn::Error;
use syn::Token;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse2;
use syn::spanned::Spanned;

pub fn webidl(item: TokenStream) -> Result<TokenStream, Error> {
  let input = parse2::<DeriveInput>(item)?;
  let span = input.span();
  let ident = input.ident;
  let ident_string = ident.to_string();
  let converter = input
    .attrs
    .into_iter()
    .find_map(|attr| ConverterType::from_attribute(attr).transpose())
    .ok_or_else(|| {
      Error::new(span, "missing top-level #[webidl] attribute")
    })??;

  let out = match input.data {
    Data::Struct(data) => match converter {
      ConverterType::Dictionary => {
        create_impl(ident, dictionary::get_body(ident_string, span, data)?)
      }
      ConverterType::Enum => {
        return Err(Error::new(span, "Structs do not support enum converters"));
      }
    },
    Data::Enum(data) => match converter {
      ConverterType::Dictionary => {
        return Err(Error::new(
          span,
          "Enums currently do not support dictionary converters",
        ));
      }
      ConverterType::Enum => {
        let (body, as_str) = r#enum::get_body(ident_string, &ident, data)?;
        let implementation = create_impl(ident, body);

        quote! {
          #implementation
          #as_str
        }
      }
    },
    Data::Union(_) => return Err(Error::new(span, "Unions are not supported")),
  };

  Ok(out)
}

mod kw {
  syn::custom_keyword!(dictionary);
  syn::custom_keyword!(default);
  syn::custom_keyword!(rename);
  syn::custom_keyword!(required);
}

enum ConverterType {
  Dictionary,
  Enum,
}

impl ConverterType {
  fn from_attribute(attr: Attribute) -> Result<Option<Self>, Error> {
    if attr.path().is_ident("webidl") {
      let list = attr.meta.require_list()?;
      let value = list.parse_args::<Self>()?;
      Ok(Some(value))
    } else {
      Ok(None)
    }
  }
}

impl Parse for ConverterType {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let lookahead = input.lookahead1();

    if lookahead.peek(kw::dictionary) {
      input.parse::<kw::dictionary>()?;
      Ok(Self::Dictionary)
    } else if lookahead.peek(Token![enum]) {
      input.parse::<Token![enum]>()?;
      Ok(Self::Enum)
    } else {
      Err(lookahead.error())
    }
  }
}

fn create_impl(ident: impl ToTokens, body: TokenStream) -> TokenStream {
  quote! {
    impl<'a> ::deno_core::webidl::WebIdlConverter<'a> for #ident {
      type Options = ();

      fn convert<'b, 'i>(
        __scope: &mut ::deno_core::v8::PinScope<'a, 'i>,
        __value: ::deno_core::v8::Local<'a, ::deno_core::v8::Value>,
        __prefix: std::borrow::Cow<'static, str>,
        __context: ::deno_core::webidl::ContextFn<'b>,
        __options: &Self::Options,
      ) -> Result<Self, ::deno_core::webidl::WebIdlError>
      {
        #body
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use proc_macro2::Ident;
  use std::path::PathBuf;
  use syn::Item;
  use syn::punctuated::Punctuated;

  fn derives_webidl<'a>(
    attrs: impl IntoIterator<Item = &'a Attribute>,
  ) -> bool {
    attrs.into_iter().any(|attr| {
      attr.path().is_ident("derive") && {
        let list = attr.meta.require_list().unwrap();
        let idents = list
          .parse_args_with(Punctuated::<Ident, Token![,]>::parse_terminated)
          .unwrap();
        idents.iter().any(|ident| ident == "WebIDL")
      }
    })
  }

  fn expand_webidl(item: impl ToTokens) -> TokenStream {
    webidl(item.to_token_stream()).expect("Failed to generate WebIDL")
  }

  #[testing_macros::fixture("webidl/test_cases/*.rs")]
  fn test_proc_macro_sync(input: PathBuf) {
    crate::infra::run_macro_expansion_test(input, |file| {
      file.items.into_iter().filter_map(|item| {
        match item {
          Item::Struct(struct_item) => {
            if derives_webidl(&struct_item.attrs) {
              return Some(expand_webidl(struct_item));
            }
          }
          Item::Enum(enum_item) => {
            if derives_webidl(&enum_item.attrs) {
              return Some(expand_webidl(enum_item));
            }
          }
          _ => {}
        }

        None
      })
    })
  }
}
