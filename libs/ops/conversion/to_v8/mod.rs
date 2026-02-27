// Copyright 2018-2025 the Deno authors. MIT license.

mod r#struct;

use proc_macro2::TokenStream;
use quote::quote;
use quote::{ToTokens, quote_spanned};
use syn::Data;
use syn::DeriveInput;
use syn::Error;
use syn::parse2;
use syn::spanned::Spanned;

pub fn to_v8(item: TokenStream) -> Result<TokenStream, Error> {
  let input = parse2::<DeriveInput>(item)?;
  let span = input.span();
  let ident = input.ident;

  let out = match input.data {
    Data::Struct(data) => create_impl(ident, r#struct::get_body(span, data)?),
    Data::Enum(_) => return Err(Error::new(span, "Enums are not supported")),
    Data::Union(_) => return Err(Error::new(span, "Unions are not supported")),
  };

  Ok(out)
}

fn convert_or_serde(
  serde: bool,
  span: proc_macro2::Span,
  value: TokenStream,
) -> TokenStream {
  if serde {
    quote_spanned! { span =>
      ::deno_core::serde_v8::to_v8(
        __scope,
        #value,
      ).map_err(::deno_error::JsErrorBox::from_err)?
    }
  } else {
    quote_spanned! { span =>
      ::deno_core::convert::ToV8::to_v8(
        #value,
        __scope,
      ).map_err(::deno_error::JsErrorBox::from_err)?
    }
  }
}

fn create_impl(ident: impl ToTokens, body: TokenStream) -> TokenStream {
  quote! {
    impl<'a> ::deno_core::convert::ToV8<'a> for #ident {
      type Error = ::deno_error::JsErrorBox;

      fn to_v8<'i>(
        self,
        __scope: &mut ::deno_core::v8::PinScope<'a, 'i>,
      ) -> Result<::deno_core::v8::Local<'a, ::deno_core::v8::Value>, Self::Error>
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
  use syn::Attribute;
  use syn::Item;
  use syn::Token;
  use syn::punctuated::Punctuated;

  fn derives_to_v8<'a>(attrs: impl IntoIterator<Item = &'a Attribute>) -> bool {
    attrs.into_iter().any(|attr| {
      attr.path().is_ident("derive") && {
        let list = attr.meta.require_list().unwrap();
        let idents = list
          .parse_args_with(Punctuated::<Ident, Token![,]>::parse_terminated)
          .unwrap();
        idents.iter().any(|ident| ident == "ToV8")
      }
    })
  }

  fn expand_to_v8(item: impl ToTokens) -> TokenStream {
    to_v8(item.to_token_stream()).expect("Failed to generate ToV8")
  }

  #[testing_macros::fixture("conversion/to_v8/test_cases/*.rs")]
  fn test_proc_macro_sync(input: PathBuf) {
    crate::infra::run_macro_expansion_test(input, |file| {
      file.items.into_iter().filter_map(|item| {
        match item {
          Item::Struct(struct_item) => {
            if derives_to_v8(&struct_item.attrs) {
              return Some(expand_to_v8(struct_item));
            }
          }
          Item::Enum(enum_item) => {
            if derives_to_v8(&enum_item.attrs) {
              return Some(expand_to_v8(enum_item));
            }
          }
          _ => {}
        }

        None
      })
    })
  }
}
