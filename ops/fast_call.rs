/// Code generation for V8 fast calls.
use crate::optimizer::Optimizer;
use pmutil::{q, Quote, ToTokensExt};
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
  parse_quote, punctuated::Punctuated, token::Comma, Fields, Ident, ItemFn,
  ItemImpl, ItemStruct, Path, PathArguments, PathSegment, Token, Type,
  TypePath, Visibility,
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
    .collect::<Punctuated<_, Comma>>();

  // Apply *hard* optimizer hints.
  if optimizer.has_fast_callback_option {
    inputs.push(parse_quote! {
      fast_api_callback_options: *mut v8::fast_api::FastApiCallbackOptions
    });
  }

  let output = match &item_fn.sig.output {
    syn::ReturnType::Default => quote! { () },
    syn::ReturnType::Type(_, ty) => quote! { #ty },
  };

  // Generate the function body.
  //
  // fn f <S> (_: Local<Object>, a: T, b: U) -> R {
  //   /* Transforms */
  //   let a = a.into();
  //   let b = b.into();
  //
  //   let r = op::call(a, b);
  //
  //   /* Return transform */
  //   r.into()
  // }
  let fast_fn = q!(
    Vars { op_name: &ident, inputs, idents, transforms },
    {
      fn op_name(_: v8::Local<v8::Object>, inputs) {
        transforms
        let result = op_name::call(idents);
      }
    }
  );

  let mut tts = q!({});
  tts.push_tokens(&fast_ty);
  tts.push_tokens(&item);
  tts.push_tokens(&fast_fn);

  Ok(tts.dump().into())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::attrs::Attributes;
  use crate::Op;
  use std::path::PathBuf;

  #[testing::fixture("optimizer_tests/**/*.rs")]
  fn test_fast_call_codegen(input: PathBuf) {
    let update_expected = std::env::var("UPDATE_EXPECTED").is_ok();

    let source =
      std::fs::read_to_string(&input).expect("Failed to read test file");
    let expected = std::fs::read_to_string(input.with_extension("out"))
      .expect("Failed to read expected file");

    let item = syn::parse_str(&source).expect("Failed to parse test file");
    let mut op = Op::new(item, Default::default());
    let mut optimizer = Optimizer::new();
    optimizer.analyze(&mut op).expect("Optimizer failed");

    let actual = generate(&mut optimizer, &op.item).unwrap();
    // Validate syntax tree.
    let tree = syn::parse2(actual).unwrap();
    let actual = prettyplease::unparse(&tree);
    if update_expected {
      std::fs::write(input.with_extension("out"), actual.to_string())
        .expect("Failed to write expected file");
    } else {
      assert_eq!(actual.to_string(), expected);
    }
  }
}
