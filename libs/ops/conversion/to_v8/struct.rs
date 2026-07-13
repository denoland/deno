// Copyright 2018-2026 the Deno authors. MIT license.

use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use syn::DataStruct;
use syn::Error;
use syn::Field;
use syn::Fields;
use syn::LitStr;
use syn::Path;
use syn::Token;
use syn::Type;
use syn::ext::IdentExt;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;

use crate::conversion::kw as shared_kw;
use crate::conversion::to_v8::convert_or_serde;

pub fn get_body(span: Span, data: DataStruct) -> Result<TokenStream, Error> {
  match data.fields {
    Fields::Named(fields) => {
      let fields = fields
        .named
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<StructField>, Error>>()?;

      // A `skip_if = <predicate>` field is conditionally present, so its key set
      // is only known at runtime. The fixed `with_prototype_and_properties` fast
      // path can't express that, so fall back to pushing keys/values into vectors
      // (preserving declaration order) when any field opts in. Otherwise keep the
      // cheaper fixed-array path.
      let any_skip_if = fields.iter().any(|field| field.skip_if.is_some());

      if any_skip_if {
        let len = fields.len();
        let pushes = fields
          .into_iter()
          .map(|field| {
            let key = crate::get_internalized_string(
              &field.js_name.to_string(),
              field.js_name.span(),
            )?;
            let name = field.name.clone();
            let skip_if = field.skip_if.clone();
            let converter =
              convert_or_serde(field.serde, field.ty.span(), field.name);
            let push = quote! {
              __keys.push(#key);
              __values.push(#converter);
            };
            // Mirrors serde's `skip_serializing_if`: the predicate is called on
            // a reference to the field and the field is skipped when it returns
            // true.
            Ok(if let Some(skip_if) = skip_if {
              quote! {
                if !#skip_if(&#name) {
                  #push
                }
              }
            } else {
              push
            })
          })
          .collect::<Result<Vec<_>, Error>>()?;

        let body = quote! {
          let __null = ::deno_core::v8::null(__scope).into();
          let mut __keys: ::std::vec::Vec<::deno_core::v8::Local<::deno_core::v8::Name>> =
            ::std::vec::Vec::with_capacity(#len);
          let mut __values: ::std::vec::Vec<::deno_core::v8::Local<::deno_core::v8::Value>> =
            ::std::vec::Vec::with_capacity(#len);

          #(#pushes)*

          Ok::<_, ::deno_error::JsErrorBox>(::deno_core::v8::Object::with_prototype_and_properties(
            __scope,
            __null,
            &__keys,
            &__values,
          ).into())
        };

        return Ok(body);
      }

      let mut names = Vec::with_capacity(fields.len());
      let mut converters = Vec::with_capacity(fields.len());

      for field in fields {
        names.push(crate::get_internalized_string(
          &field.js_name.to_string(),
          field.js_name.span(),
        )?);

        converters.push(convert_or_serde(
          field.serde,
          field.ty.span(),
          field.name,
        ));
      }

      let body = quote! {
        let __null = ::deno_core::v8::null(__scope).into();
        let __keys = &[#
          (#names),
        *];
        let __converters = &[#(#converters),*];

        Ok::<_, ::deno_error::JsErrorBox>(::deno_core::v8::Object::with_prototype_and_properties(
          __scope,
          __null,
          __keys,
          __converters,
        ).into())
      };

      Ok(body)
    }
    Fields::Unnamed(fields) => {
      let fields = fields
        .unnamed
        .into_iter()
        .enumerate()
        .map(TryInto::try_into)
        .collect::<Result<Vec<StructTupleField>, Error>>()?;

      let value = if fields.len() == 1 {
        let field = fields.first().unwrap();
        let converter = convert_or_serde(field.serde, field.span, quote!(__0));
        quote!(Ok::<_, ::deno_error::JsErrorBox>(#converter))
      } else {
        let fields = fields
          .into_iter()
          .map(|field| {
            let i = format_ident!("__{}", field.i, span = field.span);
            convert_or_serde(field.serde, field.span, i)
          })
          .collect::<Vec<_>>();

        quote! {
          let __value = &[#(#fields),*];
          Ok::<_, ::deno_error::JsErrorBox>(::deno_core::v8::Array::new_with_elements(__scope, __value).into())
        }
      };

      Ok(value)
    }
    Fields::Unit => {
      Err(Error::new(span, "Unit fields are currently not supported"))
    }
  }
}

pub struct StructField {
  pub name: Ident,
  serde: bool,
  skip_if: Option<Path>,
  ty: Type,
  pub js_name: Ident,
}

impl TryFrom<Field> for StructField {
  type Error = Error;
  fn try_from(value: Field) -> Result<Self, Self::Error> {
    let span = value.span();
    let crate::conversion::SharedAttribute {
      mut rename,
      mut serde,
    } = crate::conversion::StructFieldArgumentShared::parse(&value.attrs)?;
    let mut skip_if = None;

    for attr in value.attrs {
      if attr.path().is_ident("to_v8") {
        let list = attr.meta.require_list()?;
        let args = list.parse_args_with(
          Punctuated::<StructFieldArgument, Token![,]>::parse_terminated,
        )?;

        for argument in args {
          match argument {
            StructFieldArgument::Rename { value, .. } => {
              rename = Some(value.value())
            }
            StructFieldArgument::Serde { .. } => serde = true,
            StructFieldArgument::SkipIf { value, .. } => skip_if = Some(value),
          }
        }
      }
    }

    let name = value.ident.unwrap();
    let js_name = rename
      .unwrap_or_else(|| stringcase::camel_case(&name.unraw().to_string()));
    let js_name = Ident::new(&js_name, span);

    Ok(Self {
      js_name,
      name,
      serde,
      skip_if,
      ty: value.ty,
    })
  }
}

#[allow(dead_code, reason = "unused properties")]
pub(crate) enum StructFieldArgument {
  Rename {
    name_token: shared_kw::rename,
    eq_token: Token![=],
    value: LitStr,
  },
  Serde {
    name_token: shared_kw::serde,
  },
  // `skip_if = <predicate>` — mirrors serde's `skip_serializing_if` but takes an
  // unquoted path (e.g. `Option::is_none`) rather than a string literal.
  SkipIf {
    name_token: shared_kw::skip_if,
    eq_token: Token![=],
    value: Path,
  },
}

impl Parse for StructFieldArgument {
  fn parse(input: ParseStream) -> Result<Self, Error> {
    let lookahead = input.lookahead1();
    if lookahead.peek(shared_kw::rename) {
      Ok(StructFieldArgument::Rename {
        name_token: input.parse()?,
        eq_token: input.parse()?,
        value: input.parse()?,
      })
    } else if lookahead.peek(shared_kw::serde) {
      Ok(StructFieldArgument::Serde {
        name_token: input.parse()?,
      })
    } else if lookahead.peek(shared_kw::skip_if) {
      Ok(StructFieldArgument::SkipIf {
        name_token: input.parse()?,
        eq_token: input.parse()?,
        value: input.parse()?,
      })
    } else {
      Err(lookahead.error())
    }
  }
}

struct StructTupleField {
  span: Span,
  i: usize,
  serde: bool,
}

impl TryFrom<(usize, Field)> for StructTupleField {
  type Error = Error;
  fn try_from((i, value): (usize, Field)) -> Result<Self, Self::Error> {
    let span = value.span();
    let crate::conversion::SharedTupleAttribute { mut serde } =
      crate::conversion::StructTupleFieldArgumentShared::parse(&value.attrs)?;

    for attr in value.attrs {
      if attr.path().is_ident("to_v8") {
        let list = attr.meta.require_list()?;
        let args = list.parse_args_with(
          Punctuated::<StructTupleFieldArgument, Token![,]>::parse_terminated,
        )?;

        for argument in args {
          match argument {
            StructTupleFieldArgument::Serde { .. } => serde = true,
          }
        }
      }
    }

    Ok(Self { span, i, serde })
  }
}

#[allow(dead_code, reason = "unused properties")]
enum StructTupleFieldArgument {
  Serde {
    name_token: crate::conversion::kw::serde,
  },
}

impl Parse for StructTupleFieldArgument {
  fn parse(input: ParseStream) -> Result<Self, Error> {
    let lookahead = input.lookahead1();
    if lookahead.peek(shared_kw::serde) {
      Ok(StructTupleFieldArgument::Serde {
        name_token: input.parse()?,
      })
    } else {
      Err(lookahead.error())
    }
  }
}
