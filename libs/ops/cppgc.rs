// Copyright 2018-2025 the Deno authors. MIT license.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
  Attribute, Data, DeriveInput, Error, Fields, Ident, Meta, Result, Token,
  Type, parse_macro_input, parse_quote,
};

pub fn derives_inherits(input: TokenStream) -> TokenStream {
  match inherits_inner(parse_macro_input!(input as DeriveInput)) {
    Ok(tokens) => tokens.into(),
    Err(err) => err.to_compile_error().into(),
  }
}

pub fn derives_base(input: TokenStream) -> TokenStream {
  match base_inner(parse_macro_input!(input as DeriveInput)) {
    Ok(tokens) => tokens.into(),
    Err(err) => err.to_compile_error().into(),
  }
}

fn inherits_inner(input: DeriveInput) -> Result<TokenStream2> {
  let DeriveInput {
    ident,
    generics,
    data,
    attrs,
    ..
  } = input;

  let base = parse_base_attr(&attrs)?;
  let mut impl_generics = generics.clone();
  impl_generics.params.push(parse_quote!(__TransitiveBase));
  let where_clause = impl_generics.make_where_clause();
  where_clause
    .predicates
    .push(parse_quote!(#base: deno_core::cppgc::Inherits<__TransitiveBase>));
  where_clause
    .predicates
    .push(parse_quote!(__TransitiveBase: deno_core::cppgc::Base));

  ensure_repr_c(&attrs, ident.span())?;

  let first_field = first_field(&data).ok_or_else(|| {
    Error::new(
      ident.span(),
      "cppgc inheritance requires at least one field",
    )
  })?;

  let (field_path, field_ty_span) = match &first_field.field {
    FieldRef::Named(ident, ty_span) => (quote!(#ident), *ty_span),
    FieldRef::Unnamed(idx, ty_span) => (quote!(#idx), *ty_span),
  };

  if !types_equal(&first_field.ty, &base) {
    return Err(Error::new(
      field_ty_span,
      "first field must be the base type for cppgc inheritance",
    ));
  }

  let (transitive_impl_generics, _, transitive_where_clause) =
    impl_generics.split_for_impl();
  let (base_impl_generics, base_ty_generics, base_where_clause) =
    generics.split_for_impl();

  let offset_assert = quote! {
    const _: () = {
      const OFFSET: usize = ::core::mem::offset_of!(#ident #base_ty_generics, #field_path);
      assert!(OFFSET == 0, "base field must be at offset 0");
    };
  };
  let size_align_assert = quote! {
    const _: () = {
      assert!(
        ::core::mem::size_of::<#base>() != 0,
        "zero-sized base types are not supported for inheritance between cppgc types"
      );
      assert!(
        ::core::mem::align_of::<#ident #base_ty_generics>() >= ::core::mem::align_of::<#base>(),
        "derived alignment must be >= base alignment for inheritance between cppgc types"
      );
    };
  };

  Ok(quote! {
      #offset_assert
      #size_align_assert
      #[automatically_derived]
      unsafe impl #base_impl_generics deno_core::cppgc::Inherits<#base> for #ident #base_ty_generics #base_where_clause {}
      deno_core::_ops::inventory::submit!(deno_core::cppgc::verify_inherits::<#base, #ident #base_ty_generics>());
      #[automatically_derived]
      unsafe impl #transitive_impl_generics deno_core::cppgc::Inherits<__TransitiveBase> for #ident #base_ty_generics #transitive_where_clause {}
  })
}

fn base_inner(input: DeriveInput) -> Result<TokenStream2> {
  let DeriveInput {
    ident,
    generics,
    data: _data,
    attrs,
    ..
  } = input;

  ensure_repr_c(&attrs, ident.span())?;

  let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

  let size_assert = quote! {
    const _: () = {
      assert!(
        ::core::mem::size_of::<#ident #ty_generics>() != 0,
        "zero-sized base types are not supported for cppgc inheritance"
      );
    };
  };

  Ok(quote! {
    #size_assert
    #[automatically_derived]
    unsafe impl #impl_generics deno_core::cppgc::Base for #ident #ty_generics #where_clause {
      fn __cache() -> &'static std::sync::OnceLock<Vec<std::any::TypeId>> {
        static CACHE: std::sync::OnceLock<Vec<std::any::TypeId>> = std::sync::OnceLock::new();
        &CACHE
      }
    }
  })
}

fn ensure_repr_c(attrs: &[Attribute], span: proc_macro2::Span) -> Result<()> {
  for attr in attrs {
    if !attr.path().is_ident("repr") {
      continue;
    }
    if let Meta::List(list) = &attr.meta {
      let nested: Punctuated<Meta, Token![,]> =
        list.parse_args_with(Punctuated::parse_terminated)?;
      if nested
        .iter()
        .any(|meta| matches!(meta, Meta::Path(path) if path.is_ident("C")))
      {
        return Ok(());
      }
    }
  }
  Err(Error::new(
    span,
    "cppgc inheritance requires #[repr(C)] on the type",
  ))
}

fn parse_base_attr(attrs: &[Attribute]) -> Result<Type> {
  let mut found = None;
  for attr in attrs {
    if !attr.path().is_ident("cppgc_inherits_from") {
      continue;
    }
    if found.is_some() {
      return Err(Error::new(
        attr.span(),
        "cppgc_inherits_from specified more than once",
      ));
    }
    let base = attr.parse_args::<Type>()?;
    found = Some(base);
  }
  found.ok_or_else(|| {
    Error::new(
      proc_macro2::Span::call_site(),
      "derive(CppgcInherits) requires #[cppgc_inherits_from(BaseType)]",
    )
  })
}

#[derive(Clone)]
enum FieldRef {
  Named(Ident, proc_macro2::Span),
  Unnamed(syn::Index, proc_macro2::Span),
}

fn first_field(data: &Data) -> Option<FieldRefWithType> {
  match data {
    Data::Struct(data_struct) => match &data_struct.fields {
      Fields::Named(fields) => fields.named.first().map(|f| FieldRefWithType {
        field: FieldRef::Named(f.ident.clone().unwrap(), f.ty.span()),
        ty: f.ty.clone(),
      }),
      Fields::Unnamed(fields) => {
        fields.unnamed.first().map(|f| FieldRefWithType {
          field: FieldRef::Unnamed(syn::Index::from(0), f.ty.span()),
          ty: f.ty.clone(),
        })
      }
      Fields::Unit => None,
    },
    _ => None,
  }
}

struct FieldRefWithType {
  field: FieldRef,
  ty: Type,
}

fn types_equal(a: &Type, b: &Type) -> bool {
  quote!(#a).to_string() == quote!(#b).to_string()
}
