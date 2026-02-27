// Copyright 2018-2025 the Deno authors. MIT license.

#![doc = include_str!("README.md")]
#![deny(clippy::unnecessary_wraps)]

use proc_macro::TokenStream;

mod conversion;
mod cppgc;
mod op2;
mod webidl;

#[proc_macro_derive(CppgcInherits, attributes(cppgc_inherits_from))]
pub fn cppgc_inherits(item: TokenStream) -> TokenStream {
  cppgc::derives_inherits(item)
}

#[proc_macro_derive(CppgcBase)]
pub fn cppgc_inherits_from(item: TokenStream) -> TokenStream {
  cppgc::derives_base(item)
}

/// A macro designed to provide an extremely fast V8->Rust interface layer.
#[doc = include_str!("op2/README.md")]
#[proc_macro_attribute]
pub fn op2(attr: TokenStream, item: TokenStream) -> TokenStream {
  op2_macro(attr, item)
}

fn op2_macro(attr: TokenStream, item: TokenStream) -> TokenStream {
  match op2::op2(attr.into(), item.into()) {
    Ok(output) => output.into(),
    Err(err) => syn::Error::from(err).into_compile_error().into(),
  }
}

#[proc_macro_derive(WebIDL, attributes(webidl, options))]
pub fn webidl(item: TokenStream) -> TokenStream {
  match webidl::webidl(item.into()) {
    Ok(output) => output.into(),
    Err(err) => err.into_compile_error().into(),
  }
}

#[proc_macro_derive(FromV8, attributes(from_v8, v8))]
pub fn from_v8(item: TokenStream) -> TokenStream {
  match conversion::from_v8::from_v8(item.into()) {
    Ok(output) => output.into(),
    Err(err) => err.into_compile_error().into(),
  }
}

#[proc_macro_derive(ToV8, attributes(to_v8, v8))]
pub fn to_v8(item: TokenStream) -> TokenStream {
  match conversion::to_v8::to_v8(item.into()) {
    Ok(output) => output.into(),
    Err(err) => err.into_compile_error().into(),
  }
}

fn get_internalized_string(
  name: syn::Ident,
) -> Result<proc_macro2::TokenStream, syn::Error> {
  let name_str = name.to_string();

  if !name_str.is_ascii() {
    return Err(syn::Error::new(
      name.span(),
      "Only ASCII keys are supported",
    ));
  }

  Ok(quote::quote! {
    ::deno_core::v8::String::new_from_one_byte(
      __scope,
      #name_str.as_bytes(),
      ::deno_core::v8::NewStringType::Internalized,
    )
    .unwrap()
    .into()
  })
}

#[cfg(test)]
mod infra {
  use std::path::PathBuf;
  use syn::File;

  pub fn run_macro_expansion_test<F, I>(input: PathBuf, expander: F)
  where
    F: FnOnce(File) -> I,
    I: Iterator<Item = proc_macro2::TokenStream>,
  {
    let update_expected = std::env::var("UPDATE_EXPECTED").is_ok();

    let source =
      std::fs::read_to_string(&input).expect("Failed to read test file");

    const PRELUDE: &str = r"// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();";

    if !source.starts_with(PRELUDE) {
      panic!("Source does not start with expected prelude:]n{PRELUDE}");
    }

    let file =
      syn::parse_str::<File>(&source).expect("Failed to parse Rust file");

    let expected_out = expander(file)
      .map(|tokens| {
        println!("======== Raw tokens ========:\n{}", tokens.clone());
        let tree = syn::parse2(tokens).unwrap();
        let actual = prettyplease::unparse(&tree);
        println!("======== Generated ========:\n{}", actual);
        actual
      })
      .collect::<Vec<String>>()
      .join("\n");

    if update_expected {
      std::fs::write(input.with_extension("out"), expected_out)
        .expect("Failed to write expectation file");
    } else {
      let expected = std::fs::read_to_string(input.with_extension("out"))
        .expect("Failed to read expectation file");

      pretty_assertions::assert_eq!(
        expected,
        expected_out,
        "Failed to match expectation. Use UPDATE_EXPECTED=1."
      );
    }
  }
}
