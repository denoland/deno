// Copyright 2018-2026 the Deno authors. MIT license.

use proc_macro::TokenStream;
use quote::ToTokens;
use quote::quote;
use syn::ItemFn;
use syn::Meta;
use syn::ReturnType;
use syn::Token;
use syn::parse::Parser;
use syn::parse_macro_input;
use syn::punctuated::Punctuated;

#[derive(Default)]
struct TestAttributes {
  flaky: bool,
  timeout: Option<usize>,
  fork_with_suffix: Option<String>,
}

#[proc_macro_attribute]
pub fn test(attr: TokenStream, item: TokenStream) -> TokenStream {
  let attrs = parse_test_attributes(attr);
  generate_test_macro(item, attrs)
}

fn parse_test_attributes(attr: TokenStream) -> TestAttributes {
  // Parse as a comma-separated list of Meta items
  let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
  let metas = match parser.parse(attr.clone()) {
    Ok(metas) => metas,
    Err(e) => {
      panic!(
        "Failed to parse test attributes: {}. Expected format: #[test], #[test(flaky)], or #[test(timeout = 300)]",
        e
      );
    }
  };

  let mut result = TestAttributes::default();

  for meta in metas {
    match meta {
      // Handle simple path like `flaky`
      Meta::Path(path) => {
        if path.is_ident("flaky") {
          result.flaky = true;
        } else {
          let ident = path
            .get_ident()
            .map(|i| i.to_string())
            .unwrap_or_else(|| path.to_token_stream().to_string());
          panic!(
            "Unknown test attribute: '{}'. Valid attributes are:\n  - flaky\n  - timeout = <number>\n  - fork = \"<suffix>\"",
            ident
          );
        }
      }
      // Handle name-value pairs like `timeout = 60_000`
      Meta::NameValue(name_value) => {
        if name_value.path.is_ident("timeout") {
          // Extract the literal value
          match &name_value.value {
            syn::Expr::Lit(expr_lit) => {
              match &expr_lit.lit {
                syn::Lit::Int(lit_int) => {
                  // Use base10_parse to automatically handle underscores
                  match lit_int.base10_parse::<usize>() {
                    Ok(value) => result.timeout = Some(value),
                    Err(e) => {
                      panic!(
                        "Invalid timeout value: '{}'. Error: {}. Expected a positive integer (e.g., timeout = 60_000).",
                        lit_int, e
                      );
                    }
                  }
                }
                _ => {
                  panic!(
                    "Invalid timeout value type. Expected an integer literal (e.g., timeout = 60_000), got: {:?}",
                    expr_lit.lit
                  );
                }
              }
            }
            _ => {
              panic!(
                "Invalid timeout value. Expected an integer literal (e.g., timeout = 60_000), got: {}",
                quote::quote!(#name_value.value)
              );
            }
          }
        } else if name_value.path.is_ident("fork_with_suffix") {
          // Extract the string literal value
          match &name_value.value {
            syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
              syn::Lit::Str(lit_str) => {
                result.fork_with_suffix = Some(lit_str.value());
              }
              _ => {
                panic!(
                  "Invalid fork value type. Expected a string literal (e.g., fork_with_suffix = \"_suffix\"), got: {:?}",
                  expr_lit.lit
                );
              }
            },
            _ => {
              panic!(
                "Invalid fork value. Expected a string literal (e.g., fork_with_suffix = \"_suffix\"), got: {}",
                quote::quote!(#name_value.value)
              );
            }
          }
        } else {
          let ident = name_value
            .path
            .get_ident()
            .map(|i| i.to_string())
            .unwrap_or_else(|| name_value.path.to_token_stream().to_string());
          panic!(
            "Unknown test attribute: '{}'. Valid attributes are:\n  - flaky\n  - timeout = <number>\n  - fork = \"<suffix>\"",
            ident
          );
        }
      }
      // Handle other meta types (List, etc.)
      _ => {
        panic!(
          "Invalid test attribute format: '{}'. Expected format:\n  - flaky\n  - timeout = <number>\n  - fork = \"<suffix>\"",
          quote::quote!(#meta)
        );
      }
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

  // Handle fork attribute - generates two test cases
  if let Some(suffix) = attrs.fork_with_suffix {
    let forked_name =
      syn::Ident::new(&format!("{}{}", fn_name, suffix), fn_name.span());
    let wrapper_name =
      syn::Ident::new(&format!("{}_wrapper", fn_name), fn_name.span());
    let forked_wrapper_name = syn::Ident::new(
      &format!("{}{}_wrapper", fn_name, suffix),
      fn_name.span(),
    );

    let (wrapper_body_false, wrapper_body_true) = if is_async {
      let call_false = if returns_result {
        quote! { #fn_name(false).await.unwrap(); }
      } else {
        quote! { #fn_name(false).await; }
      };
      let call_true = if returns_result {
        quote! { #fn_name(true).await.unwrap(); }
      } else {
        quote! { #fn_name(true).await; }
      };
      (
        quote! {
          let rt = tokio::runtime::Builder::new_current_thread()
              .enable_all()
              .build()
              .unwrap();
          rt.block_on(async {
              #call_false
          });
        },
        quote! {
          let rt = tokio::runtime::Builder::new_current_thread()
              .enable_all()
              .build()
              .unwrap();
          rt.block_on(async {
              #call_true
          });
        },
      )
    } else if returns_result {
      (
        quote! { #fn_name(false).unwrap(); },
        quote! { #fn_name(true).unwrap(); },
      )
    } else {
      (quote! { #fn_name(false); }, quote! { #fn_name(true); })
    };

    let expanded = quote! {
      #input

      fn #wrapper_name() {
        #wrapper_body_false
      }

      fn #forked_wrapper_name() {
        #wrapper_body_true
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
              timeout: #timeout_expr,
          }
      }

      test_util::submit! {
          test_util::TestMacroCase {
              name: stringify!(#forked_name),
              module_name: module_path!(),
              func: #forked_wrapper_name,
              flaky: #is_flaky,
              file: file!(),
              line: line!(),
              col: column!(),
              ignore: #is_ignored,
              timeout: #timeout_expr,
          }
      }
    };

    return TokenStream::from(expanded);
  }

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
