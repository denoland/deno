// Copyright 2018-2026 the Deno authors. MIT license.

use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use stringcase::camel_case;
use syn::Attribute;
use syn::DataEnum;
use syn::DataStruct;
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
  span: Span,
  attributes: Vec<Attribute>,
  data: DataEnum,
) -> Result<TokenStream, Error> {
  let mode = EnumMode::from_attributes(span, attributes)?;

  let variants = data
    .variants
    .into_iter()
    .map(|variant| {
      let variant_span = variant.span();
      let variant_attrs = EnumVariantAttribute::from_variant(&variant)?;
      let variant_ident = variant.ident;

      let tag_name = variant_attrs
        .rename
        .unwrap_or_else(|| camel_case(&variant_ident.unraw().to_string()));
      let tag_value = crate::get_internalized_string(Ident::new(
        &tag_name,
        variant_ident.span(),
      ))?;

      if variant.fields == Fields::Unit {
        let tag = match &mode {
          EnumMode::ExternallyTagged => quote! {
            Ok(#tag_value)
          },
          EnumMode::InternallyTagged { tag } | EnumMode::AdjacentlyTagged { tag, .. }  => {
            quote! {
              let __null = ::deno_core::v8::null(__scope).into();
              let __keys = &[#tag];
              let __values = &[#tag_value];

              Ok(::deno_core::v8::Object::with_prototype_and_properties(
                __scope,
                __null,
                __keys,
                __values,
              ).into())
            }
          }
          EnumMode::Untagged => {
            quote! {
              Ok(::deno_core::v8::null(__scope).into())
            }
          }
        };

        Ok(quote! {
         Self::#variant_ident => {
          #tag
        }
      })
      } else {
        let fields = super::destruct_fields(&variant.fields)?;

        let conversion = if variant_attrs.serde {
          get_serde_variant_conversion(variant_span, variant.fields)?
        } else {
          super::r#struct::get_body(
            variant_span,
            DataStruct {
              struct_token: Default::default(),
              fields: variant.fields,
              semi_token: None,
            },
          )?
        };

        let tag = match &mode {
          EnumMode::ExternallyTagged => {
            quote! {
              let __null = ::deno_core::v8::null(__scope).into();
              let __keys = &[#tag_value];
              let __converters = &[__body];

              Ok(::deno_core::v8::Object::with_prototype_and_properties(
                __scope,
                __null,
                __keys,
                __converters,
              ).into())
            }
          }
          EnumMode::InternallyTagged { tag } => {
            quote! {
              if Ok(__obj_body) = __body.try_cast::<::deno_core::v8::Object>() {
                let __tag_key = #tag;
                let __tag_value = #tag_value;

                 __obj_body.set(__scope, __tag_key, __tag_value);

                Ok(__obj_body.into())
              } else {
                panic!("cannot use non-object value with an internally tag enum");
              }
            }
          }
          EnumMode::AdjacentlyTagged { tag, content } => {
            quote! {
              let __null = ::deno_core::v8::null(__scope).into();
              let __keys = &[#tag, #content];
              let __converters = &[#tag_value, __body];

              Ok(::deno_core::v8::Object::with_prototype_and_properties(
                __scope,
                __null,
                __keys,
                __converters,
              ).into())
            }
          }
          EnumMode::Untagged => {
            quote! {
              Ok(__body)
            }
          }
        };

        Ok(quote! {
           Self::#variant_ident #fields => {
            let __body = { #conversion }?;
            #tag
          }
        })
      }
    })
    .collect::<Result<Vec<_>, Error>>()?;

  let impl_body = quote! {
    match self {
      #(#variants),*
    }
  };

  Ok(impl_body)
}

fn get_serde_variant_conversion(
  span: Span,
  fields: Fields,
) -> Result<TokenStream, Error> {
  match fields {
    Fields::Named(named) => {
      let mut field_names = Vec::with_capacity(named.named.len());
      let mut field_strs = Vec::with_capacity(named.named.len());

      for field in named.named {
        let field = super::r#struct::StructField::try_from(field)?;

        field_names.push(field.name);
        field_strs.push(field.js_name);
      }

      Ok(quote! {
        let __obj = ::deno_core::v8::Object::new(__scope);
        #(
          let __key = ::deno_core::v8::String::new(__scope, stringify!(#field_strs)).unwrap().into();
          let __val = ::deno_core::serde_v8::to_v8(__scope, #field_names).map_err(::deno_error::JsErrorBox::from_err)?;
          __obj.set(__scope, __key, __val);
        )*
        Ok::<_, ::deno_error::JsErrorBox>(__obj.into())
      })
    }
    Fields::Unnamed(unnamed) => {
      if unnamed.unnamed.len() == 1 {
        Ok(quote! {
          Ok::<_, ::deno_error::JsErrorBox>(
            ::deno_core::serde_v8::to_v8(__scope, __0)
              .map_err(::deno_error::JsErrorBox::from_err)?
          )
        })
      } else {
        let len = unnamed.unnamed.len();
        let field_idents: Vec<_> = unnamed
          .unnamed
          .into_iter()
          .enumerate()
          .map(|(i, field)| format_ident!("__{}", i, span = field.span()))
          .collect();
        let indices: Vec<_> = (0..len as u32).collect();

        Ok(quote! {
          let __arr = ::deno_core::v8::Array::new(__scope, #len as i32);
          #(
            let __val = ::deno_core::serde_v8::to_v8(__scope, #field_idents)
              .map_err(::deno_error::JsErrorBox::from_err)?;
            __arr.set_index(__scope, #indices, __val);
          )*
          Ok::<_, ::deno_error::JsErrorBox>(__arr.into())
        })
      }
    }
    Fields::Unit => Err(Error::new(span, "Cannot use serde on unit variant")),
  }
}

#[derive(Default)]
enum EnumMode {
  #[default]
  ExternallyTagged,
  InternallyTagged {
    tag: TokenStream,
  },
  AdjacentlyTagged {
    tag: TokenStream,
    content: TokenStream,
  },
  Untagged,
}

impl EnumMode {
  fn from_attributes(span: Span, attrs: Vec<Attribute>) -> Result<Self, Error> {
    let mut tag: Option<String> = None;
    let mut content: Option<String> = None;
    let mut untagged = false;

    for attr in attrs {
      if attr.path().is_ident("to_v8") {
        let list = attr.meta.require_list()?;
        let args = list.parse_args_with(
          Punctuated::<EnumModeArgument, Token![,]>::parse_terminated,
        )?;

        for arg in args {
          match arg {
            EnumModeArgument::Tag { value, .. } => tag = Some(value.value()),
            EnumModeArgument::Content { value, .. } => {
              content = Some(value.value())
            }
            EnumModeArgument::Untagged { .. } => untagged = true,
          }
        }
      }
    }

    if untagged {
      if tag.is_some() || content.is_some() {
        return Err(Error::new(
          Span::call_site(),
          "Cannot combine `untagged` with `tag` or `content`",
        ));
      }
      return Ok(EnumMode::Untagged);
    }

    match (tag, content) {
      (None, None) => Ok(EnumMode::ExternallyTagged),
      (Some(tag), None) => Ok(EnumMode::InternallyTagged {
        tag: crate::get_internalized_string(Ident::new(&tag, span))?,
      }),
      (Some(tag), Some(content)) => Ok(EnumMode::AdjacentlyTagged {
        tag: crate::get_internalized_string(Ident::new(&tag, span))?,
        content: crate::get_internalized_string(Ident::new(&content, span))?,
      }),
      (None, Some(_)) => Err(Error::new(
        Span::call_site(),
        "`content` requires `tag` to be specified",
      )),
    }
  }
}

#[allow(dead_code, reason = "unused properties")]
enum EnumModeArgument {
  Tag {
    name_token: shared_kw::tag,
    eq_token: Token![=],
    value: LitStr,
  },
  Content {
    name_token: shared_kw::content,
    eq_token: Token![=],
    value: LitStr,
  },
  Untagged {
    name_token: shared_kw::untagged,
  },
}

impl Parse for EnumModeArgument {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let lookahead = input.lookahead1();

    if lookahead.peek(shared_kw::tag) {
      Ok(Self::Tag {
        name_token: input.parse()?,
        eq_token: input.parse()?,
        value: input.parse()?,
      })
    } else if lookahead.peek(shared_kw::content) {
      Ok(Self::Content {
        name_token: input.parse()?,
        eq_token: input.parse()?,
        value: input.parse()?,
      })
    } else if lookahead.peek(shared_kw::untagged) {
      Ok(Self::Untagged {
        name_token: input.parse()?,
      })
    } else {
      Err(lookahead.error())
    }
  }
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
      if attr.path().is_ident("v8") {
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

      if attr.path().is_ident("to_v8") {
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
