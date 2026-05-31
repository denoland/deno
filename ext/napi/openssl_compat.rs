// Copyright 2018-2026 the Deno authors. MIT license.

//! OpenSSL-API compatibility shims for legacy Node.js native addons.
//!
//! Older Node.js native addons (typically NAN-based, e.g. `nodegit`)
//! reference OpenSSL symbols like `EVP_des_ede3_cbc` directly,
//! expecting them to be re-exported by the host binary. Node.js
//! statically links OpenSSL and exposes those symbols via `-rdynamic`,
//! so the addon's `dlopen` resolves them from `node` itself.
//!
//! Deno embeds AWS-LC (a BoringSSL fork) via the `aws-lc-sys` crate,
//! but AWS-LC prefixes every C symbol with `aws_lc_<version>_` to
//! avoid colliding with a system OpenSSL. As a result, even though the
//! AWS-LC implementation is statically linked into `deno`, addons
//! cannot find it by its conventional OpenSSL name.
//!
//! This module provides thin `#[no_mangle]` wrappers that re-export
//! selected AWS-LC functions under their conventional OpenSSL names.
//! The symbol names are listed in `ext/napi/sym/symbol_exports.json`
//! so they end up on the dynamic export list of the final `deno`
//! binary (matching how the `napi_*` and `uv_*` symbols are exposed).
//!
//! ### Adding more symbols
//!
//! 1. Add a `wrap!` invocation below with the matching signature from
//!    `aws-lc-sys`.
//! 2. Add the symbol name to `ext/napi/sym/symbol_exports.json`.
//! 3. Re-run `tools/napi/generate_symbols_lists.js` to regenerate the
//!    `generated_symbol_exports_list_*.def` files.
//!
//! This file deliberately keeps the surface narrow — exposing the
//! entire OpenSSL API would be a maintenance burden and risks
//! ABI-breaking changes between AWS-LC versions. Symbols are added on
//! demand as legacy addons require them.

#![allow(non_snake_case, reason = "matches OpenSSL C function names")]

use aws_lc_sys::EVP_CIPHER;

/// Defines a `#[no_mangle] extern "C"` wrapper that forwards to
/// `aws_lc_sys::<name>`. Both the wrapper and the underlying AWS-LC
/// binding share the same Rust identifier, so we disambiguate by
/// fully-qualifying the call.
macro_rules! wrap {
  ($name:ident () -> $ret:ty) => {
    #[unsafe(no_mangle)]
    unsafe extern "C" fn $name() -> $ret {
      // SAFETY: forwards to `aws-lc-sys`, which is statically linked
      // into this binary. The wrapped function takes no arguments and
      // returns an opaque pointer, so there is nothing to validate.
      unsafe { aws_lc_sys::$name() }
    }
  };
}

// Triple-DES in CBC mode. This is the specific symbol whose absence
// surfaced in https://github.com/denoland/deno/issues/31730 (loading
// the `nodegit` native addon).
wrap!(EVP_des_ede3_cbc() -> *const EVP_CIPHER);
