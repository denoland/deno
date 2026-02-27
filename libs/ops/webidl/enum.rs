// Copyright 2018-2025 the Deno authors. MIT license.

use super::kw;
use proc_macro2::Ident;
use proc_macro2::TokenStream;
use quote::quote;
use syn::DataEnum;
use syn::Error;
use syn::LitStr;
use syn::Token;
use syn::Variant;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;

pub fn get_body(
  ident_string: String,
  ident: &Ident,
  data: DataEnum,
) -> Result<(TokenStream, TokenStream), Error> {
  let variants = data
    .variants
    .into_iter()
    .map(get_variant_name)
    .collect::<Result<indexmap::IndexMap<_, _>, _>>()?;

  let names = variants.keys();
  let idents = variants.values();

  let impl_body = quote! {
    match __value.to_rust_string_lossy(__scope).as_str() {
      #(#names => Ok(Self::#idents)),*,
      s => Err(::deno_core::webidl::WebIdlError::new(__prefix, __context, ::deno_core::webidl::WebIdlErrorKind::InvalidEnumVariant { converter: #ident_string, variant: s.to_string() }))
    }
  };

  let names = variants.keys();
  let idents = variants.values();

  let as_str = quote! {
    impl #ident {
      pub fn as_str(&self) -> &'static str {
        match self {
          #(Self::#idents => #names),*,
        }
      }
    }
  };

  Ok((impl_body, as_str))
}

fn get_variant_name(value: Variant) -> Result<(String, Ident), Error> {
  let mut rename: Option<String> = None;

  if !value.fields.is_empty() {
    return Err(Error::new(
      value.fields.span(),
      "variants with fields are not allowed for enum converters",
    ));
  }

  for attr in value.attrs {
    if attr.path().is_ident("webidl") {
      let list = attr.meta.require_list()?;
      let args = list.parse_args_with(
        Punctuated::<EnumVariantArgument, Token![,]>::parse_terminated,
      )?;

      for argument in args {
        match argument {
          EnumVariantArgument::Rename { value, .. } => {
            rename = Some(value.value())
          }
        }
      }
    }
  }

  Ok((
    rename.unwrap_or_else(|| stringcase::kebab_case(&value.ident.to_string())),
    value.ident,
  ))
}

#[allow(dead_code)]
enum EnumVariantArgument {
  Rename {
    name_token: kw::rename,
    eq_token: Token![=],
    value: LitStr,
  },
}

impl Parse for EnumVariantArgument {
  fn parse(input: ParseStream) -> Result<Self, Error> {
    let lookahead = input.lookahead1();
    if lookahead.peek(kw::rename) {
      Ok(EnumVariantArgument::Rename {
        name_token: input.parse()?,
        eq_token: input.parse()?,
        value: input.parse()?,
      })
    } else {
      Err(lookahead.error())
    }
  }
}
