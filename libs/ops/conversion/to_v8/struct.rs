// Copyright 2018-2025 the Deno authors. MIT license.

use crate::conversion::kw as shared_kw;
use crate::conversion::to_v8::convert_or_serde;
use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use syn::DataStruct;
use syn::Error;
use syn::Field;
use syn::Fields;
use syn::LitStr;
use syn::Token;
use syn::Type;
use syn::ext::IdentExt;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;

pub fn get_body(span: Span, data: DataStruct) -> Result<TokenStream, Error> {
  match data.fields {
    Fields::Named(fields) => {
      let mut fields = fields
        .named
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<StructField>, Error>>()?;
      fields.sort_by(|a, b| a.name.cmp(&b.name));

      let mut names = Vec::with_capacity(fields.len());
      let mut converters = Vec::with_capacity(fields.len());

      for field in fields {
        names.push(crate::get_internalized_string(field.js_name)?);

        let field_name = field.name;
        converters.push(convert_or_serde(
          field.serde,
          field.ty.span(),
          quote!(self.#field_name),
        ));
      }

      let body = quote! {
        let __null = ::deno_core::v8::null(__scope).into();
        let __keys = &[#
          (#names),
        *];
        let __converters = &[#(#converters),*];

        Ok(::deno_core::v8::Object::with_prototype_and_properties(
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
        let converter =
          convert_or_serde(field.serde, field.span, quote!(self.0));
        quote!(Ok(#converter))
      } else {
        let fields = fields
          .into_iter()
          .map(|field| {
            let i = syn::Index::from(field.i);
            convert_or_serde(field.serde, field.span, quote!(self.#i))
          })
          .collect::<Vec<_>>();

        quote! {
          let __value = &[#(#fields),*];
          Ok(::deno_core::v8::Array::new_with_elements(__scope, __value).into())
        }
      };

      Ok(value)
    }
    Fields::Unit => {
      Err(Error::new(span, "Unit fields are currently not supported"))
    }
  }
}

struct StructField {
  name: Ident,
  serde: bool,
  ty: Type,
  js_name: Ident,
}

impl TryFrom<Field> for StructField {
  type Error = Error;
  fn try_from(value: Field) -> Result<Self, Self::Error> {
    let span = value.span();
    let crate::conversion::SharedAttribute {
      mut rename,
      mut serde,
    } = crate::conversion::StructFieldArgumentShared::parse(&value.attrs)?;

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
      ty: value.ty,
    })
  }
}

#[allow(dead_code)]
enum StructFieldArgument {
  Rename {
    name_token: shared_kw::rename,
    eq_token: Token![=],
    value: LitStr,
  },
  Serde {
    name_token: shared_kw::serde,
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

#[allow(dead_code)]
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
