// Copyright 2018-2026 the Deno authors. MIT license.

//! WebCrypto `SubtleCrypto` as a cppgc-wrapped Rust object.
//!
//! Registered on the extension via `objects = [SubtleCrypto]` so the class
//! identity lives in Rust. The static `supports(operation, algorithm)`
//! feature-detection entry point has been ported here from
//! `00_crypto.js`; the other WebCrypto methods are still defined on the
//! prototype from JS while the per-algorithm dispatch is being lifted to
//! Rust in follow-up sessions.

use std::ffi::CStr;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;

use crate::algorithm::check_support_for_algorithm;
use crate::shared::SharedError;

pub struct SubtleCrypto;

impl WebIdlInterfaceConverter for SubtleCrypto {
  const NAME: &'static str = "SubtleCrypto";
}

// SAFETY: zero-sized payload.
unsafe impl GarbageCollected for SubtleCrypto {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static CStr {
    c"SubtleCrypto"
  }
}

#[op2]
impl SubtleCrypto {
  /// `new SubtleCrypto()` is illegal per the WebCrypto spec.
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<SubtleCrypto, SharedError> {
    Err(SharedError::IllegalConstructor)
  }

  /// `SubtleCrypto.supports(operation, algorithm, lengthOrHash?)` from the
  /// WICG modern-algos spec. The single-argument-name case is handled here
  /// in Rust; the two-argument-name overload (where `lengthOrHash` is an
  /// `AlgorithmIdentifier`) is still dispatched from the JS shim, which
  /// owns the `deriveKey` / `wrapKey` paperwork it requires.
  #[fast]
  #[static_method]
  fn supports(
    #[string] operation: &str,
    #[string] algorithm_name: &str,
  ) -> bool {
    check_support_for_algorithm(operation, algorithm_name)
  }
}
