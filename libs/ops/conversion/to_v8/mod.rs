// Copyright 2018-2026 the Deno authors. MIT license.

mod r#enum;
mod r#struct;

use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::format_ident;
use quote::quote;
use quote::quote_spanned;
use syn::Data;
use syn::DeriveInput;
use syn::Error;
use syn::Fields;
use syn::Generics;
use syn::parse2;
use syn::spanned::Spanned;

pub fn to_v8(item: TokenStream) -> Result<TokenStream, Error> {
  let input = parse2::<DeriveInput>(item)?;
  let span = input.span();
  let ident = input.ident;
  let generics = input.generics;

  let out = match input.data {
    Data::Struct(data) => {
      let fields = destruct_fields(&data.fields)?;
      let body = r#struct::get_body(span, data)?;

      create_impl(
        &ident,
        &generics,
        quote! {
          let Self #fields = self;
          #body
        },
      )
    }
    Data::Enum(data) => {
      create_impl(&ident, &generics, r#enum::get_body(span, input.attrs, data)?)
    }
    Data::Union(_) => return Err(Error::new(span, "Unions are not supported")),
  };

  Ok(out)
}

fn destruct_fields(fields: &Fields) -> Result<TokenStream, Error> {
  match fields {
    Fields::Named { 0: named } => {
      let fields = named
        .named
        .iter()
        .map(|field| field.ident.as_ref().unwrap());

      Ok(quote! {
        {
          #(#fields),*
        }
      })
    }
    Fields::Unnamed(unnamed) => {
      let fields = unnamed.unnamed.iter().enumerate().map(|(i, field)| {
        let idx = syn::Index::from(i);
        let ident = format_ident!("__{i}", span = field.span());
        quote!(#idx: #ident)
      });

      Ok(quote! {
        {
          #(#fields),*
        }
      })
    }
    Fields::Unit => Err(Error::new(
      fields.span(),
      "Unit structs cannot be destructured",
    )),
  }
}

fn convert_or_serde<T: quote::ToTokens>(
  serde: bool,
  span: proc_macro2::Span,
  value: T,
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

fn create_impl(
  ident: impl ToTokens,
  generics: &Generics,
  body: TokenStream,
) -> TokenStream {
  // Collect lifetime names already in use on the struct, to avoid E0403.
  let used: std::collections::HashSet<String> = generics
    .lifetimes()
    .map(|lt| lt.lifetime.ident.to_string())
    .collect();

  // Pick a scope lifetime ('a preferred, fall back to __scope) that doesn't shadow a struct lt.
  let scope_lt_name = ["a", "__scope", "__v8scope"]
    .iter()
    .copied()
    .find(|n| !used.contains(*n))
    .expect("no free lifetime name for ToV8 scope");
  let scope_lt = syn::Lifetime::new(
    &format!("'{}", scope_lt_name),
    proc_macro2::Span::call_site(),
  );

  // Pick an inner lifetime ('i preferred) that doesn't shadow a struct lt or == scope_lt.
  let inner_lt_name = ["i", "__inner", "__v8inner"]
    .iter()
    .copied()
    .find(|n| !used.contains(*n) && *n != scope_lt_name)
    .expect("no free lifetime name for PinScope inner lifetime");
  let inner_lt = syn::Lifetime::new(
    &format!("'{}", inner_lt_name),
    proc_macro2::Span::call_site(),
  );

  // Build impl generics: scope_lt + any generics on the struct.
  let lp = syn::LifetimeParam::new(scope_lt.clone());
  let mut all_params = generics.clone();
  all_params.params.insert(0, syn::GenericParam::Lifetime(lp));

  let (impl_generics, _, where_clause) = all_params.split_for_impl();
  let (_, ty_generics, _) = generics.split_for_impl();

  quote! {
    impl #impl_generics ::deno_core::convert::ToV8<#scope_lt> for #ident #ty_generics #where_clause {
      type Error = ::deno_error::JsErrorBox;

      fn to_v8<#inner_lt>(
        self,
        __scope: &mut ::deno_core::v8::PinScope<#scope_lt, #inner_lt>,
      ) -> Result<::deno_core::v8::Local<#scope_lt, ::deno_core::v8::Value>, Self::Error>
      {
        #body
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use proc_macro2::Ident;
  use syn::Attribute;
  use syn::Item;
  use syn::Token;
  use syn::punctuated::Punctuated;

  use super::*;

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
      file.items.into_iter().filter_map(|item| match item {
        Item::Struct(struct_item) if derives_to_v8(&struct_item.attrs) => {
          Some(expand_to_v8(struct_item))
        }
        Item::Enum(enum_item) if derives_to_v8(&enum_item.attrs) => {
          Some(expand_to_v8(enum_item))
        }
        _ => None,
      })
    })
  }
}
