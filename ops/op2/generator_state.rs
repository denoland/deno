// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use proc_macro2::Ident;
use proc_macro2::TokenStream;

pub struct GeneratorState {
  pub deno_core: TokenStream,

  pub call: Ident,
  pub scope: Ident,
  pub info: Ident,
  pub fn_args: Ident,
  pub retval: Ident,
  pub result: Ident,
  pub slow_function: Ident,
  pub fast_function: Ident,
  pub args: Vec<Ident>,

  pub needs_args: bool,
  pub needs_retval: bool,
  pub needs_scope: bool,
}
