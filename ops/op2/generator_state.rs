// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use proc_macro2::Ident;
use proc_macro2::TokenStream;

pub struct GeneratorState {
  /// The path to the `deno_core` crate (either `deno_core` or `crate`, the latter used if the op is `(core)`).
  pub deno_core: TokenStream,

  /// Identifiers for each of the arguments of the original function
  pub args: Vec<Ident>,
  /// The new identifier for the original function's contents.
  pub call: Ident,
  /// The result of the `call` function
  pub result: Ident,

  /// The `v8::CallbackScope` used if necessary for the function.
  pub scope: Ident,
  /// The `v8::FunctionCallbackInfo` used to pass args into the slow function.
  pub info: Ident,
  /// The `v8::FunctionCallbackArguments` used to pass args into the slow function.
  pub fn_args: Ident,
  /// The `OpCtx` used for various information required for some ops.
  pub opctx: Ident,
  /// The `FastApiCallbackOptions` used in fast calls for fallback returns.
  pub fast_api_callback_options: Ident,
  /// The `v8::ReturnValue` used in the slow function
  pub retval: Ident,
  /// The "slow" function (ie: the one that isn't a fastcall)
  pub slow_function: Ident,
  /// The "fast" function (ie: a fastcall)
  pub fast_function: Ident,

  pub needs_args: bool,
  pub needs_retval: bool,
  pub needs_scope: bool,
  pub needs_opstate: bool,
  pub needs_opctx: bool,
  pub needs_fast_opctx: bool,
  pub needs_fast_api_callback_options: bool,
}
