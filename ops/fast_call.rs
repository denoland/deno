/// Code generation for V8 fast calls.
use crate::optimizer::Optimizer;
use pmutil::{q, Quote};
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
  let ident = item_fn.sig.ident.clone();
  let mut segments = Punctuated::new();
  segments.push_value(PathSegment {
    ident: ident.clone(),
    arguments: PathArguments::None,
  });

  // struct T <A> {
  //   _phantom: ::std::marker::PhantomData<A>,
  // }
  let fast_ty: Quote = q!(Vars { Type: &ident }, {
    struct Type {
      _phantom: ::std::marker::PhantomData<()>,
    }
  });

  // impl <A> fast_api::FastFunction for T <A> where A: B {
  //   fn function(&self) -> *const ::std::ffi::c_void  {
  //     f as *const ::std::ffi::c_void
  //   }
  //   fn args(&self) -> &'static [fast_api::Type] {
  //     &[ CType::T, CType::U ]
  //   }
  //   fn return_type(&self) -> fast_api::CType {
  //     CType::T
  //   }
  // }
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

  let mut inputs = item_fn.sig.inputs.clone();
  let mut transforms = q!({});
  // Apply parameter transforms
  for (input, transform) in inputs.iter_mut().zip(optimizer.transforms.iter()) {
    let quo: Quote = transform.apply_for_fast_call(input);
    transforms.push_tokens(&quo);
  }

  // Collect idents to be passed into function call, we can now freely
  // modify the inputs.
  let idents = inputs
    .iter()
    .map(|input| match input {
      syn::FnArg::Typed(pat_type) => match &*pat_type.pat {
        syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
        _ => panic!("unexpected pattern"),
      },
      _ => panic!("unexpected argument"),
    })
    .collect::<Vec<_>>();

  // Apply *hard* optimizer hints.
  if optimizer.has_fast_callback_option {
    inputs.push(parse_quote! {
      fast_api_callback_options: *mut v8::fast_api::FastApiCallbackOptions
    });
  }

  let fast_fn = q!(Vars { op_name: &ident }, {
    fn op_name(_: v8::Local<v8::Object>) {}
  });

  let mut tts = q!({});
  tts.push_tokens(&fast_ty);

  Ok(quote! {})
}

#[cfg(test)]
mod tests {
  use super::*;
}
