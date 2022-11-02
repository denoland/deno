/// Code generation for V8 fast calls.
use crate::optimizer::Optimizer;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
  punctuated::Punctuated, Fields, Ident, ItemImpl, ItemStruct, Path,
  PathArguments, PathSegment, Token, Type, TypePath, Visibility, ItemFn, parse_quote
};

struct FastCallImpl<'s> {
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
  item: ItemImpl,

  /// struct T <A> {
  ///   _phantom: ::std::marker::PhantomData<A>,
  /// }
  fast_ty: ItemStruct,

  optimizer: &'s mut Optimizer,
}

impl<'s> FastCallImpl<'s> {
  pub(crate) fn new(optimizer: &'s mut Optimizer) -> Self {
    let ident = Ident::new("FastCall", Span::call_site());
    let mut segments = Punctuated::new();
    segments.push_value(PathSegment {
      ident: ident.clone(),
      arguments: PathArguments::None,
    });

    Self {
      item: ItemImpl {
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
      },
      fast_ty: parse_quote! {
        struct #ident {
          _phantom: ::std::marker::PhantomData<()>,
        }
      },
      optimizer,
    }
  }

  pub(crate) fn generate(
    mut self,
    item_fn: &ItemFn,
  ) -> Result<TokenStream, ()> {
    
    Ok(quote! {})
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_fast_call() {
    let _ = FastCallImpl::new();
  }
}
