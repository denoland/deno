// Copyright 2018-2025 the Deno authors. MIT license.

use proc_macro::TokenStream;
use quote::quote;
use syn::ItemFn;
use syn::ReturnType;
use syn::parse_macro_input;

#[proc_macro_attribute]
pub fn test(attr: TokenStream, item: TokenStream) -> TokenStream {
  // Parse the attribute to check if it's "flaky"
  let is_flaky = attr.to_string().trim() == "flaky";
  generate_test_macro(item, is_flaky)
}

fn generate_test_macro(item: TokenStream, is_flaky: bool) -> TokenStream {
  let input = parse_macro_input!(item as ItemFn);
  let fn_name = &input.sig.ident;

  // Detect if the function is async
  let is_async = input.sig.asyncness.is_some();

  // Check for #[ignore] attribute
  let is_ignored = input
    .attrs
    .iter()
    .any(|attr| attr.path().is_ident("ignore"));

  let expanded = if is_async {
    let wrapper_name =
      syn::Ident::new(&format!("{}_wrapper", fn_name), fn_name.span());

    // Check if the function returns a Result
    let returns_result = match &input.sig.output {
      ReturnType::Type(_, ty) => {
        if let syn::Type::Path(type_path) = &**ty {
          type_path
            .path
            .segments
            .last()
            .is_some_and(|seg| seg.ident == "Result")
        } else {
          false
        }
      }
      _ => false,
    };

    let await_call = if returns_result {
      quote! { #fn_name().await.unwrap(); }
    } else {
      quote! { #fn_name().await; }
    };

    quote! {
        // Keep the original async function
        #input

        // Create a sync wrapper that runs the async function
        fn #wrapper_name() {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                #await_call
            });
        }

        test_util::submit! {
            test_util::TestMacroCase {
                name: stringify!(#fn_name),
                module_name: module_path!(),
                func: #wrapper_name,
                flaky: #is_flaky,
                file: file!(),
                line: line!(),
                col: column!(),
                ignore: #is_ignored,
            }
        }
    }
  } else {
    quote! {
        #input

        test_util::submit! {
            test_util::TestMacroCase {
                name: stringify!(#fn_name),
                module_name: module_path!(),
                func: #fn_name,
                flaky: #is_flaky,
                file: file!(),
                line: line!(),
                col: column!(),
                ignore: #is_ignored,
            }
        }
    }
  };

  TokenStream::from(expanded)
}
