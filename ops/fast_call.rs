/// Code generation for V8 fast calls.
use crate::optimizer::Optimizer;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
  parse_quote, punctuated::Punctuated, Fields, Ident, ItemFn, ItemImpl,
  ItemStruct, Path, PathArguments, PathSegment, Token, Type, TypePath,
  Visibility,
};

pub(crate) fn generate(
  optimizer: &mut Optimizer,
  item_fn: &ItemFn,
) -> Result<TokenStream, ()> {
  /// impl <A> fast_api::FastFunction for T <A> where A: B {
  ///   fn function(&self) -> *const ::std::ffi::c_void  {
  ///     f as *const ::std::ffi::c_void
  ///   }
  ///   fn args(&self) -> &'static [fast_api::Type] {
  ///     &[ CType::T, CType::U ]
  ///   }
  ///   fn return_type(&self) -> fast_api::CType {
  ///     CType::T
  ///   }
  /// }
  let item: ItemImpl = ItemImpl {
    attrs: vec![],
    defaultness: None,
    unsafety: None,
    impl_token: Default::default(),
    generics: Default::default(),
    trait_: None,
    self_ty: Box::new(Type::Path(TypePath {
      qself: None,
      path: Path {
        leading_colon: None,
        segments,
      },
    })),
    brace_token: Default::default(),
    items: vec![],
  };

  /// struct T <A> {
  ///   _phantom: ::std::marker::PhantomData<A>,
  /// }
  let fast_ty: ItemStruct = parse_quote! {
    struct #ident {
      _phantom: ::std::marker::PhantomData<()>,
    }
  };

  let ident = Ident::new("FastCall", Span::call_site());
  let mut segments = Punctuated::new();
  segments.push_value(PathSegment {
    ident: ident.clone(),
    arguments: PathArguments::None,
  });

  Ok(quote! {})
}

#[cfg(test)]
mod tests {
  use super::*;
}
