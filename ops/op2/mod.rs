// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use deno_proc_macro_rules::rules;
use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::ToTokens;
use std::iter::zip;
use syn2::parse2;
use syn2::FnArg;
use syn2::ItemFn;
use syn2::Path;
use thiserror::Error;

use self::dispatch_fast::generate_dispatch_fast;
use self::dispatch_slow::generate_dispatch_slow;
use self::generator_state::GeneratorState;
use self::signature::parse_signature;
use self::signature::Arg;
use self::signature::SignatureError;

pub mod dispatch_fast;
pub mod dispatch_slow;
pub mod generator_state;
pub mod signature;

#[derive(Debug, Error)]
pub enum Op2Error {
  #[error("Failed to match a pattern for '{0}': (input was '{1}')")]
  PatternMatchFailed(&'static str, String),
  #[error("Invalid attribute: '{0}'")]
  InvalidAttribute(String),
  #[error("Failed to parse syntax tree")]
  ParseError(#[from] syn2::Error),
  #[error("Failed to map a parsed signature to a V8 call")]
  V8MappingError(#[from] V8MappingError),
  #[error("Failed to parse signature")]
  SignatureError(#[from] SignatureError),
  #[error("This op is fast-compatible and should be marked as (fast)")]
  ShouldBeFast,
  #[error("This op is not fast-compatible and should not be marked as (fast)")]
  ShouldNotBeFast,
}

#[derive(Debug, Error)]
pub enum V8MappingError {
  #[error("Unable to map {1:?} to {0}")]
  NoMapping(&'static str, Arg),
}

#[derive(Default)]
pub(crate) struct MacroConfig {
  pub core: bool,
  pub fast: bool,
}

impl MacroConfig {
  pub fn from_flags(flags: Vec<Ident>) -> Result<Self, Op2Error> {
    let mut config: MacroConfig = Self::default();
    for flag in flags {
      if flag == "core" {
        config.core = true;
      } else if flag == "fast" {
        config.fast = true;
      } else {
        return Err(Op2Error::InvalidAttribute(flag.to_string()));
      }
    }
    Ok(config)
  }

  pub fn from_tokens(tokens: TokenStream) -> Result<Self, Op2Error> {
    let attr_string = tokens.to_string();
    let config = std::panic::catch_unwind(|| {
      rules!(tokens => {
        () => {
          Ok(MacroConfig::default())
        }
        ($($flags:ident),+) => {
          Self::from_flags(flags)
        }
      })
    })
    .map_err(|_| Op2Error::PatternMatchFailed("attribute", attr_string))??;
    Ok(config)
  }
}

pub fn op2(
  attr: TokenStream,
  item: TokenStream,
) -> Result<TokenStream, Op2Error> {
  let func = parse2::<ItemFn>(item)?;
  let config = MacroConfig::from_tokens(attr)?;
  generate_op2(config, func)
}

fn generate_op2(
  config: MacroConfig,
  func: ItemFn,
) -> Result<TokenStream, Op2Error> {
  // Create a copy of the original function, named "call"
  let call = Ident::new("call", Span::call_site());
  let mut op_fn = func.clone();
  op_fn.attrs.clear();
  op_fn.sig.ident = call.clone();

  // Clear inert attributes
  // TODO(mmastrac): This should limit itself to clearing ours only
  for arg in op_fn.sig.inputs.iter_mut() {
    match arg {
      FnArg::Receiver(slf) => slf.attrs.clear(),
      FnArg::Typed(ty) => ty.attrs.clear(),
    }
  }

  let signature = parse_signature(func.attrs, func.sig.clone())?;
  let processed_args =
    zip(signature.args.iter(), &func.sig.inputs).collect::<Vec<_>>();

  let mut args = vec![];
  let mut needs_args = false;
  for (index, _) in processed_args.iter().enumerate() {
    let input = format_ident!("arg{index}");
    args.push(input);
    needs_args = true;
  }

  let retval = Ident::new("rv", Span::call_site());
  let result = Ident::new("result", Span::call_site());
  let fn_args = Ident::new("args", Span::call_site());
  let scope = Ident::new("scope", Span::call_site());
  let info = Ident::new("info", Span::call_site());
  let opctx = Ident::new("opctx", Span::call_site());
  let slow_function = Ident::new("slow_function", Span::call_site());
  let fast_function = Ident::new("fast_function", Span::call_site());
  let fast_api_callback_options =
    Ident::new("fast_api_callback_options", Span::call_site());

  let deno_core = if config.core {
    syn2::parse_str::<Path>("crate")
  } else {
    syn2::parse_str::<Path>("deno_core")
  }
  .expect("Parsing crate should not fail")
  .into_token_stream();

  let mut generator_state = GeneratorState {
    args,
    fn_args,
    call,
    scope,
    info,
    opctx,
    fast_api_callback_options,
    deno_core,
    result,
    retval,
    needs_args,
    slow_function,
    fast_function,
    needs_retval: false,
    needs_scope: false,
    needs_opctx: false,
    needs_opstate: false,
    needs_fast_opctx: false,
    needs_fast_api_callback_options: false,
  };

  let name = func.sig.ident;

  let slow_fn =
    generate_dispatch_slow(&config, &mut generator_state, &signature)?;
  let (fast_definition, fast_fn) =
    match generate_dispatch_fast(&mut generator_state, &signature)? {
      Some((fast_definition, fast_fn)) => {
        if !config.fast {
          return Err(Op2Error::ShouldBeFast);
        }
        (quote!(Some({#fast_definition})), fast_fn)
      }
      None => {
        if config.fast {
          return Err(Op2Error::ShouldNotBeFast);
        }
        (quote!(None), quote!())
      }
    };

  let GeneratorState {
    deno_core,
    slow_function,
    ..
  } = &generator_state;

  let arg_count: usize = generator_state.args.len();
  let vis = func.vis;

  Ok(quote! {
    #[allow(non_camel_case_types)]
    #vis struct #name {
    }

    impl #name {
      pub const fn name() -> &'static str {
        stringify!(#name)
      }

      pub const fn decl() -> #deno_core::_ops::OpDecl {
        #deno_core::_ops::OpDecl {
          name: stringify!(#name),
          v8_fn_ptr: Self::#slow_function as _,
          enabled: true,
          fast_fn: #fast_definition,
          is_async: false,
          is_unstable: false,
          is_v8: false,
          arg_count: #arg_count as u8,
        }
      }

      #slow_fn
      #fast_fn

      #[inline(always)]
      #op_fn
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;
  use std::path::PathBuf;
  use syn2::parse_str;
  use syn2::File;
  use syn2::Item;

  #[testing_macros::fixture("op2/test_cases/**/*.rs")]
  fn test_signature_parser(input: PathBuf) {
    let update_expected = std::env::var("UPDATE_EXPECTED").is_ok();

    let source =
      std::fs::read_to_string(&input).expect("Failed to read test file");
    let file = parse_str::<File>(&source).expect("Failed to parse Rust file");
    let mut expected_out = vec![];
    for item in file.items {
      if let Item::Fn(mut func) = item {
        let mut config = None;
        func.attrs.retain(|attr| {
          let tokens = attr.into_token_stream();
          let attr_string = attr.clone().into_token_stream().to_string();
          println!("{}", attr_string);
          use syn2 as syn;
          if let Some(new_config) = rules!(tokens => {
            (#[op2]) => {
              Some(MacroConfig::default())
            }
            (#[op2( $($x:ident),* )]) => {
              Some(MacroConfig::from_flags(x).expect("Failed to parse attribute"))
            }
            (#[$_attr:meta]) => {
              None
            }
          }) {
            config = Some(new_config);
            false
          } else {
            true
          }
        });
        let tokens =
          generate_op2(config.unwrap(), func).expect("Failed to generate op");
        println!("======== Raw tokens ========:\n{}", tokens.clone());
        let tree = syn::parse2(tokens).unwrap();
        let actual = prettyplease::unparse(&tree);
        println!("======== Generated ========:\n{}", actual);
        expected_out.push(actual);
      }
    }

    let expected_out = expected_out.join("\n");

    if update_expected {
      std::fs::write(input.with_extension("out"), expected_out)
        .expect("Failed to write expectation file");
    } else {
      let expected = std::fs::read_to_string(input.with_extension("out"))
        .expect("Failed to read expectation file");
      assert_eq!(
        expected, expected_out,
        "Failed to match expectation. Use UPDATE_EXPECTED=1."
      );
    }
  }
}
