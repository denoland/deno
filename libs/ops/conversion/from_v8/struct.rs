// Copyright 2018-2025 the Deno authors. MIT license.

use super::convert_or_serde;
use super::kw;
use crate::conversion::kw as shared_kw;
use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::quote;
use syn::DataStruct;
use syn::Error;
use syn::Expr;
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

pub fn get_body(
  ident_string: String,
  span: Span,
  data: DataStruct,
) -> Result<TokenStream, Error> {
  match data.fields {
    Fields::Named(fields) => {
      let mut fields = fields
        .named
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<StructField>, Error>>()?;
      fields.sort_by(|a, b| a.name.cmp(&b.name));

      let names = fields
        .iter()
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();

      let fields = fields.into_iter().map(|field| {
        let field_name = field.name;
        let js_name = field.js_name.to_string();
        let key = crate::get_internalized_string(field.js_name)?;

        let undefined_as_none = if field.default_value.is_some() {
          quote! {
            .and_then(|__value| {
              if __value.is_undefined() {
                None
              } else {
                Some(__value)
              }
            })
          }
        } else {
          quote!()
        };

        let required_or_default = match field.default_value {
          Some(default) => default.to_token_stream(),
          None => {
            quote! {
              return Err(::deno_error::JsErrorBox::type_error(concat!("Missing required field '", #js_name ,"' on '", #ident_string, "'")));
            }
        }};

        let converter = convert_or_serde(field.serde, field.ty.span(), quote!(__value));

        Ok(quote! {
          let #field_name = {
            let __key = #key;

            if let Some(__value) = __obj.get(__scope, __key)#undefined_as_none {
             #converter
            } else {
              #required_or_default
            }
          };
        })
      }).collect::<Result<Vec<_>, Error>>()?;

      let body = quote! {
        let __obj: ::deno_core::v8::Local<::deno_core::v8::Object> =  match __value.try_into() {
          Ok(obj) => obj,
          Err(err) => return Err(::deno_error::JsErrorBox::from_err(::deno_core::error::DataError::from(err))),
        };

        #(#fields)*

        Ok(Self { #(#names),* })
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
          convert_or_serde(field.serde, field.span, quote!(__value));
        quote!(Ok(Self(#converter)))
      } else {
        let fields = fields
          .into_iter()
          .map(|field| {
            let i = field.i as u32;
            let converter =
              convert_or_serde(field.serde, field.span, quote!(__element_value));

            quote! {
              {
                let __element_value = __array.get_index(__scope, #i).ok_or_else(|| ::deno_error::JsErrorBox::type_error(concat!("Missing element ", #i, " on '", #ident_string, "'")))?;
                #converter
              }
            }
          })
          .collect::<Vec<_>>();

        quote! {
          let __array = ::deno_core::v8::Local::<::deno_core::v8::Array>::try_from(__value)
            .map_err(|err| ::deno_error::JsErrorBox::from_err(::deno_core::error::DataError::from(err)))?;

          Ok(Self(#(#fields),*))
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
  js_name: Ident,
  default_value: Option<Expr>,
  serde: bool,
  ty: Type,
}

impl TryFrom<Field> for StructField {
  type Error = Error;
  fn try_from(value: Field) -> Result<Self, Self::Error> {
    let span = value.span();
    let mut default_value: Option<Expr> = None;
    let crate::conversion::SharedAttribute {
      mut rename,
      mut serde,
    } = crate::conversion::StructFieldArgumentShared::parse(&value.attrs)?;

    for attr in value.attrs {
      if attr.path().is_ident("from_v8") {
        let list = attr.meta.require_list()?;
        let args = list.parse_args_with(
          Punctuated::<StructFieldArgument, Token![,]>::parse_terminated,
        )?;

        for argument in args {
          match argument {
            StructFieldArgument::Default { value, .. } => {
              default_value = Some(value.unwrap_or_else(|| {
                syn::parse2(quote!(Default::default())).unwrap()
              }))
            }
            StructFieldArgument::Rename { value, .. } => {
              rename = Some(value.value())
            }
            StructFieldArgument::Serde { .. } => serde = true,
          }
        }
      }
    }

    if default_value.is_none() {
      let is_option = match &value.ty {
        Type::Path(path) => match path.path.segments.last() {
          Some(last) => last.ident == "Option",
          _ => false,
        },
        _ => false,
      };

      if is_option {
        default_value = Some(syn::parse_quote!(None));
      }
    }

    let name = value.ident.unwrap();
    let js_name = rename
      .unwrap_or_else(|| stringcase::camel_case(&name.unraw().to_string()));
    let js_name = Ident::new(&js_name, span);

    Ok(Self {
      name,
      js_name,
      default_value,
      serde,
      ty: value.ty,
    })
  }
}

#[allow(dead_code)]
enum StructFieldArgument {
  Default {
    name_token: kw::default,
    eq_token: Option<Token![=]>,
    value: Option<Expr>,
  },
  Rename {
    name_token: crate::conversion::kw::rename,
    eq_token: Token![=],
    value: LitStr,
  },
  Serde {
    name_token: crate::conversion::kw::serde,
  },
}

impl Parse for StructFieldArgument {
  fn parse(input: ParseStream) -> Result<Self, Error> {
    let lookahead = input.lookahead1();
    if lookahead.peek(kw::default) {
      let name_token: kw::default = input.parse()?;

      if input.peek(Token![=]) {
        Ok(StructFieldArgument::Default {
          name_token,
          eq_token: Some(input.parse()?),
          value: Some(input.parse()?),
        })
      } else if input.is_empty() || input.peek(Token![,]) {
        Ok(StructFieldArgument::Default {
          name_token,
          eq_token: None,
          value: None,
        })
      } else {
        Err(input.error("expected `=` or end of argument after `default`"))
      }
    } else if lookahead.peek(shared_kw::rename) {
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
      if attr.path().is_ident("from_v8") {
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
