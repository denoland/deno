// Copyright 2018-2026 the Deno authors. MIT license.

use proc_macro2::Ident;
use proc_macro2::TokenStream;
use quote::quote;
use stringcase::camel_case;
use syn::Attribute;
use syn::DataEnum;
use syn::Error;
use syn::Fields;
use syn::LitStr;
use syn::Token;
use syn::Variant;
use syn::ext::IdentExt;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;

use crate::conversion::kw as shared_kw;

pub fn get_body(
  ident_string: String,
  attributes: Vec<Attribute>,
  data: DataEnum,
) -> Result<TokenStream, Error> {
  // For now, FromV8 only supports externally-tagged enums. Other modes
  // (internally tagged, adjacently tagged, untagged) require extra design
  // around field-tag stripping and try-each ordering, so they are deferred
  // until a caller needs them. Reject container-level attributes loudly
  // instead of silently treating them as externally-tagged.
  reject_unsupported_container_attributes(&attributes)?;

  let mut unit_arms = Vec::new();
  let mut variant_arms = Vec::new();
  let mut has_non_unit = false;

  for variant in data.variants {
    let variant_span = variant.span();
    let variant_attrs = EnumVariantAttribute::from_variant(&variant)?;
    let variant_ident = variant.ident.clone();
    let tag_name = variant_attrs
      .rename
      .clone()
      .unwrap_or_else(|| camel_case(&variant_ident.unraw().to_string()));

    match &variant.fields {
      Fields::Unit => {
        unit_arms.push(quote! {
          #tag_name => return Ok(Self::#variant_ident),
        });
      }
      Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
        has_non_unit = true;
        let converter = if variant_attrs.serde {
          quote! {
            ::deno_core::serde_v8::from_v8(__scope, __inner)
              .map_err(::deno_error::JsErrorBox::from_err)?
          }
        } else {
          quote! {
            ::deno_core::convert::FromV8::from_v8(__scope, __inner)
              .map_err(::deno_error::JsErrorBox::from_err)?
          }
        };
        let key = crate::get_internalized_string(Ident::new(
          &tag_name,
          variant_ident.span(),
        ))?;
        variant_arms.push(quote! {
          {
            let __key = #key;
            if let Some(__inner) = __obj.get(__scope, __key)
              && !__inner.is_undefined()
            {
              return Ok(Self::#variant_ident(#converter));
            }
          }
        });
      }
      _ => {
        return Err(Error::new(
          variant_span,
          "FromV8 enum derive currently supports only unit and single-element newtype variants",
        ));
      }
    }
  }

  let unit_branch = if unit_arms.is_empty() {
    quote! {}
  } else {
    quote! {
      if let Ok(__s) =
        ::deno_core::v8::Local::<::deno_core::v8::String>::try_from(__value)
      {
        let __s = __s.to_rust_string_lossy(__scope);
        match __s.as_str() {
          #(#unit_arms)*
          _ => {
            return Err(::deno_error::JsErrorBox::type_error(
              concat!("Unknown string variant for '", #ident_string, "'"),
            ));
          }
        }
      }
    }
  };

  let object_branch = if has_non_unit {
    quote! {
      let __obj: ::deno_core::v8::Local<::deno_core::v8::Object> =
        match __value.try_into() {
          Ok(obj) => obj,
          Err(err) => {
            return Err(::deno_error::JsErrorBox::from_err(
              ::deno_core::error::DataError::from(err),
            ));
          }
        };

      #(#variant_arms)*

      Err(::deno_error::JsErrorBox::type_error(
        concat!("No matching variant key found for '", #ident_string, "'"),
      ))
    }
  } else {
    quote! {
      Err(::deno_error::JsErrorBox::type_error(
        concat!("Expected string variant for '", #ident_string, "'"),
      ))
    }
  };

  Ok(quote! {
    #unit_branch
    #object_branch
  })
}

/// FromV8 only supports externally-tagged enums, so any container-level
/// `#[v8(...)]` / `#[from_v8(...)]` attribute (e.g. `tag`, `content`,
/// `untagged`) is unsupported. Reject them loudly instead of silently
/// ignoring them and falling back to externally-tagged behavior.
fn reject_unsupported_container_attributes(
  attributes: &[Attribute],
) -> Result<(), Error> {
  for attr in attributes {
    if attr.path().is_ident("v8") || attr.path().is_ident("from_v8") {
      return Err(Error::new_spanned(
        attr,
        "FromV8 enum derive does not support container-level attributes; \
         only externally-tagged enums are supported",
      ));
    }
  }

  Ok(())
}

struct EnumVariantAttribute {
  rename: Option<String>,
  serde: bool,
}

impl EnumVariantAttribute {
  fn from_variant(variant: &Variant) -> syn::Result<Self> {
    let mut rename: Option<String> = None;
    let mut serde = false;

    for attr in &variant.attrs {
      if attr.path().is_ident("v8") || attr.path().is_ident("from_v8") {
        let list = attr.meta.require_list()?;
        let args = list.parse_args_with(
          Punctuated::<EnumVariantArgument, Token![,]>::parse_terminated,
        )?;

        for arg in args {
          match arg {
            EnumVariantArgument::Rename { value, .. } => {
              rename = Some(value.value())
            }
            EnumVariantArgument::Serde { .. } => serde = true,
          }
        }
      }
    }

    Ok(Self { rename, serde })
  }
}

#[allow(dead_code, reason = "unused properties")]
enum EnumVariantArgument {
  Rename {
    name_token: shared_kw::rename,
    eq_token: Token![=],
    value: LitStr,
  },
  Serde {
    name_token: shared_kw::serde,
  },
}

impl Parse for EnumVariantArgument {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let lookahead = input.lookahead1();

    if lookahead.peek(shared_kw::rename) {
      Ok(Self::Rename {
        name_token: input.parse()?,
        eq_token: input.parse()?,
        value: input.parse()?,
      })
    } else if lookahead.peek(shared_kw::serde) {
      Ok(Self::Serde {
        name_token: input.parse()?,
      })
    } else {
      Err(lookahead.error())
    }
  }
}
