// Copyright 2018-2025 the Deno authors. MIT license.

use proc_macro::TokenStream;
use quote::quote;
use syn::ItemFn;
use syn::ReturnType;
use syn::parse_macro_input;

#[derive(Default)]
struct TestAttributes {
  flaky: bool,
  timeout: Option<usize>,
}

#[proc_macro_attribute]
pub fn test(attr: TokenStream, item: TokenStream) -> TokenStream {
  let attrs = parse_test_attributes(attr);
  generate_test_macro(item, attrs)
}

fn parse_test_attributes(attr: TokenStream) -> TestAttributes {
  let attr_str = attr.to_string();
  let attr_str = attr_str.trim();

  if attr_str.is_empty() {
    return TestAttributes::default();
  }

  let mut result = TestAttributes::default();

  // Split by commas to handle multiple attributes
  for part in attr_str.split(',') {
    let part = part.trim();

    if part == "flaky" {
      result.flaky = true;
    } else if let Some(timeout_value) = part.strip_prefix("timeout") {
      // Parse "timeout = 60_000" or "timeout=60_000"
      let timeout_value = timeout_value.trim();
      if let Some(value_str) = timeout_value.strip_prefix('=') {
        let value_str = value_str.trim();
        // Remove underscores from the number (e.g., "60_000" -> "60000")
        let value_str = value_str.replace('_', "");
        match value_str.parse::<usize>() {
          Ok(value) => result.timeout = Some(value),
          Err(_) => {
            panic!("Invalid timeout value: {}. Expected a number.", value_str);
          }
        }
      } else {
        panic!("Invalid timeout syntax. Expected: timeout = <number>");
      }
    } else {
      panic!(
        "Unknown test attribute: '{}'. Valid attributes are: flaky, timeout = <number>",
        part
      );
    }
  }

  result
}

fn generate_test_macro(
  item: TokenStream,
  attrs: TestAttributes,
) -> TokenStream {
  let input = parse_macro_input!(item as ItemFn);
  let fn_name = &input.sig.ident;

  // Detect if the function is async
  let is_async = input.sig.asyncness.is_some();

  // Check for #[ignore] attribute
  let is_ignored = input
    .attrs
    .iter()
    .any(|attr| attr.path().is_ident("ignore"));

  let timeout_expr = if let Some(timeout) = attrs.timeout {
    quote! { Some(#timeout) }
  } else {
    quote! { None }
  };

  let is_flaky = attrs.flaky;

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

  // Determine if we need a wrapper function
  let needs_wrapper = is_async || returns_result;

  let (test_func, func_def) = if needs_wrapper {
    let wrapper_name =
      syn::Ident::new(&format!("{}_wrapper", fn_name), fn_name.span());

    let wrapper_body = if is_async {
      let call = if returns_result {
        quote! { #fn_name().await.unwrap(); }
      } else {
        quote! { #fn_name().await; }
      };
      quote! {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            #call
        });
      }
    } else {
      // Non-async, but returns Result
      quote! {
        #fn_name().unwrap();
      }
    };

    let wrapper = quote! {
      fn #wrapper_name() {
        #wrapper_body
      }
    };

    (quote! { #wrapper_name }, wrapper)
  } else {
    (quote! { #fn_name }, quote! {})
  };

  let expanded = quote! {
    #input

    #func_def

    test_util::submit! {
        test_util::TestMacroCase {
            name: stringify!(#fn_name),
            module_name: module_path!(),
            func: #test_func,
            flaky: #is_flaky,
            file: file!(),
            line: line!(),
            col: column!(),
            ignore: #is_ignored,
            timeout: #timeout_expr,
        }
    }
  };

  TokenStream::from(expanded)
}
