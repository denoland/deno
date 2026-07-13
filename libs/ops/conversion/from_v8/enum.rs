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
  match EnumMode::from_attributes(&attributes)? {
    EnumMode::ExternallyTagged => {
      get_externally_tagged_body(ident_string, data)
    }
    EnumMode::Untagged => get_untagged_body(ident_string, data),
  }
}

fn get_externally_tagged_body(
  ident_string: String,
  data: DataEnum,
) -> Result<TokenStream, Error> {
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

/// `ExternallyTagged` is the only mode `FromV8` derives without an
/// attribute. `Untagged` probes the value's own raw v8 type against each
/// variant in declaration order and dispatches to the first match — see
/// `get_untagged_body` for why that's a raw-type probe and not a
/// try-each-`FromV8` loop. `FromV8`'s `internally tagged` and `adjacently
/// tagged` modes are still unsupported: unlike untagged (a value either
/// matches a variant's raw type or it doesn't), those need field-tag
/// stripping before the remaining fields can be parsed, which is extra
/// design nobody has needed yet.
#[derive(Default)]
enum EnumMode {
  #[default]
  ExternallyTagged,
  Untagged,
}

impl EnumMode {
  fn from_attributes(attributes: &[Attribute]) -> Result<Self, Error> {
    let mut untagged = false;

    for attr in attributes {
      if attr.path().is_ident("v8") || attr.path().is_ident("from_v8") {
        let list = attr.meta.require_list()?;
        let args = list.parse_args_with(
          Punctuated::<EnumModeArgument, Token![,]>::parse_terminated,
        )?;

        for arg in args {
          match arg {
            EnumModeArgument::Untagged { .. } => untagged = true,
          }
        }
      }
    }

    Ok(if untagged {
      EnumMode::Untagged
    } else {
      EnumMode::ExternallyTagged
    })
  }
}

#[allow(dead_code, reason = "unused properties")]
enum EnumModeArgument {
  Untagged { name_token: shared_kw::untagged },
}

impl Parse for EnumModeArgument {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let lookahead = input.lookahead1();

    if lookahead.peek(shared_kw::untagged) {
      Ok(Self::Untagged {
        name_token: input.parse()?,
      })
    } else {
      Err(lookahead.error())
    }
  }
}

/// Untagged: `__value` itself, with no wrapper, must directly be one variant's
/// payload. Each variant is tried in declaration order — a unit variant
/// matches `null`/`undefined`, a single-field newtype variant matches if its
/// inner type's `deno_core::convert::UntaggedProbe` says its raw v8 type
/// matches `__value` — and the first match wins, mirroring serde's untagged
/// deserialization.
///
/// Probing is deliberately not "try each variant's `FromV8` and keep the
/// first `Ok`": some `FromV8` impls are intentionally coercive (`String`
/// stringifies anything via `toString()`) and would wrongly claim a value
/// that should have matched an earlier, more specific variant. Once
/// `UntaggedProbe` confirms the raw type, the actual conversion is expected
/// to succeed, so its error (if any) propagates instead of falling through
/// to the next variant.
fn get_untagged_body(
  ident_string: String,
  data: DataEnum,
) -> Result<TokenStream, Error> {
  let mut attempts = Vec::new();

  for variant in data.variants {
    let variant_span = variant.span();
    let variant_attrs = EnumVariantAttribute::from_variant(&variant)?;
    let variant_ident = variant.ident.clone();

    match &variant.fields {
      Fields::Unit => {
        attempts.push((
          quote! { __value.is_null_or_undefined() },
          quote! { Ok(Self::#variant_ident) },
        ));
      }
      Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
        if variant_attrs.serde {
          return Err(Error::new(
            variant_span,
            "FromV8 untagged enum derive does not support `#[from_v8(serde)]` \
             variants: untagged matching needs a non-coercive type probe \
             before attempting conversion, and serde_v8's `from_v8` doesn't \
             expose one",
          ));
        }
        let field_ty = &fields.unnamed.first().unwrap().ty;
        attempts.push((
          quote! { <#field_ty as ::deno_core::convert::UntaggedProbe>::probe(__value) },
          quote! {
            Ok(Self::#variant_ident(
              ::deno_core::convert::FromV8::from_v8(__scope, __value)
                .map_err(::deno_error::JsErrorBox::from_err)?,
            ))
          },
        ));
      }
      _ => {
        return Err(Error::new(
          variant_span,
          "FromV8 untagged enum derive currently supports only unit and \
           single-element newtype variants",
        ));
      }
    }
  }

  let no_match = quote! {
    Err(::deno_error::JsErrorBox::type_error(
      concat!("Value did not match any variant of '", #ident_string, "'"),
    ))
  };

  // `rest` starts as a braced block (valid directly after `else`) and each
  // fold step chains `else #rest` onto the previous `if` without wrapping it
  // in another block, so the result prints as a flat `if / else if / ... /
  // else` chain instead of nested `else { if ... }`.
  let mut rest = quote! { { #no_match } };
  for (cond, body) in attempts.into_iter().rev() {
    rest = quote! { if #cond { #body } else #rest };
  }

  Ok(rest)
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
