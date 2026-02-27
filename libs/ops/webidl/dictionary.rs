// Copyright 2018-2025 the Deno authors. MIT license.

use super::kw;
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
use syn::MetaNameValue;
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
  let fields = match data.fields {
    Fields::Named(fields) => fields,
    Fields::Unnamed(_) => {
      return Err(Error::new(
        span,
        "Unnamed fields are currently not supported",
      ));
    }
    Fields::Unit => {
      return Err(Error::new(span, "Unit fields are currently not supported"));
    }
  };

  let mut fields = fields
    .named
    .into_iter()
    .map(TryInto::try_into)
    .collect::<Result<Vec<DictionaryField>, Error>>()?;
  fields.sort_by(|a, b| a.name.cmp(&b.name));

  let names = fields
    .iter()
    .map(|field| field.name.clone())
    .collect::<Vec<_>>();

  let fields = fields.into_iter().map(|field| {
    let field_name = field.name;
    let js_name = field.js_name.to_string();
    let key = crate::get_internalized_string(field.js_name)?;

    let options = if field.converter_options.is_empty() {
      quote!(Default::default())
    } else {
      let inner = field.converter_options
        .into_iter()
        .map(|(k, v)| quote!(#k: #v))
        .collect::<Vec<_>>();

      let ty = field.ty;

      // Type-alias to workaround https://github.com/rust-lang/rust/issues/86935
      quote! {
        {
          type Alias<'a> = <#ty as ::deno_core::webidl::WebIdlConverter<'a>>::Options;
          Alias {
            #(#inner),*,
            ..Default::default()
          }
        }
      }
    };

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

    let required_or_default = match field.default_value { Some(default) => {
      default.to_token_stream()
    } _ => {
      quote! {
        return Err(::deno_core::webidl::WebIdlError::new(
          __prefix,
          __context.borrowed(),
          ::deno_core::webidl::WebIdlErrorKind::DictionaryCannotConvertKey {
            converter: #ident_string,
            key: #js_name,
          },
        ));
      }
    }};

    Ok(quote! {
      let #field_name = {
        let __key = #key;

        if let Some(__value) = __obj.as_ref().and_then(|__obj| __obj.get(__scope, __key))#undefined_as_none {
          ::deno_core::webidl::WebIdlConverter::convert(
            __scope,
            __value,
            __prefix.clone(),
            ::deno_core::webidl::ContextFn::new_borrowed(&|| format!("'{}' of '{}' ({})", #js_name, #ident_string, __context.call()).into()),
            &#options,
          )?
        } else {
          #required_or_default
        }
      };
    })
  }).collect::<Result<Vec<_>, Error>>()?;

  let body = quote! {
    let __obj: Option<::deno_core::v8::Local<::deno_core::v8::Object>> = if __value.is_undefined() || __value.is_null() {
      None
    } else {
      if let Ok(obj) = __value.try_into() {
        Some(obj)
      } else {
        return Err(::deno_core::webidl::WebIdlError::new(
          __prefix,
          __context.borrowed(),
          ::deno_core::webidl::WebIdlErrorKind::ConvertToConverterType("dictionary")
        ));
      }
    };

    #(#fields)*

    Ok(Self { #(#names),* })
  };

  Ok(body)
}

struct DictionaryField {
  name: Ident,
  js_name: Ident,
  default_value: Option<Expr>,
  converter_options: std::collections::HashMap<Ident, Expr>,
  ty: Type,
}

impl TryFrom<Field> for DictionaryField {
  type Error = Error;
  fn try_from(value: Field) -> Result<Self, Self::Error> {
    let span = value.span();
    let mut default_value: Option<Expr> = None;
    let mut rename: Option<String> = None;
    let mut converter_options = std::collections::HashMap::new();

    for attr in value.attrs {
      if attr.path().is_ident("webidl") {
        let list = attr.meta.require_list()?;
        let args = list.parse_args_with(
          Punctuated::<DictionaryFieldArgument, Token![,]>::parse_terminated,
        )?;

        for argument in args {
          match argument {
            DictionaryFieldArgument::Default { value, .. } => {
              default_value = Some(value)
            }
            DictionaryFieldArgument::Rename { value, .. } => {
              rename = Some(value.value())
            }
          }
        }
      } else if attr.path().is_ident("options") {
        let list = attr.meta.require_list()?;
        let args = list.parse_args_with(
          Punctuated::<MetaNameValue, Token![,]>::parse_terminated,
        )?;

        let args = args
          .into_iter()
          .map(|kv| {
            let ident = kv.path.require_ident()?;
            Ok((ident.clone(), kv.value))
          })
          .collect::<Result<Vec<_>, Error>>()?;

        converter_options.extend(args);
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
      converter_options,
      ty: value.ty,
    })
  }
}

#[allow(dead_code)]
enum DictionaryFieldArgument {
  Default {
    name_token: kw::default,
    eq_token: Token![=],
    value: Expr,
  },
  Rename {
    name_token: kw::rename,
    eq_token: Token![=],
    value: LitStr,
  },
}

impl Parse for DictionaryFieldArgument {
  fn parse(input: ParseStream) -> Result<Self, Error> {
    let lookahead = input.lookahead1();
    if lookahead.peek(kw::default) {
      Ok(DictionaryFieldArgument::Default {
        name_token: input.parse()?,
        eq_token: input.parse()?,
        value: input.parse()?,
      })
    } else if lookahead.peek(kw::rename) {
      Ok(DictionaryFieldArgument::Rename {
        name_token: input.parse()?,
        eq_token: input.parse()?,
        value: input.parse()?,
      })
    } else {
      Err(lookahead.error())
    }
  }
}
