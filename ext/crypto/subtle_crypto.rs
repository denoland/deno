// Copyright 2018-2026 the Deno authors. MIT license.

//! WebCrypto `SubtleCrypto` as a cppgc-wrapped Rust object.
//!
//! Registered on the extension via `objects = [SubtleCrypto]` so the class
//! identity lives in Rust. The singleton instance reachable as
//! `globalThis.crypto.subtle` is minted by [`op_create_subtle_crypto`].

use std::ffi::CStr;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::unsync::spawn_blocking;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;

use crate::CryptoError;
use crate::algorithm::check_support_for_algorithm;
use crate::digest::BufferSource;
use crate::digest::DigestAlgorithm;
use crate::digest::run as run_digest;
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

  /// `SubtleCrypto.digest(algorithm, data)` — compute a one-shot
  /// cryptographic hash. The `WebIdlConverter` for `DigestAlgorithm`
  /// performs the `AlgorithmIdentifier` coercion + canonical name lookup
  /// that the JS body used to do via `normalizeAlgorithm`, and
  /// `BufferSource` copies the input bytes upfront so we satisfy the
  /// spec "get a copy of the bytes" before any async work.
  #[required(2)]
  #[arraybuffer]
  async fn digest(
    &self,
    #[webidl] algorithm: DigestAlgorithm,
    #[webidl] data: BufferSource,
  ) -> Result<Vec<u8>, CryptoError> {
    spawn_blocking(move || run_digest(algorithm, data.0)).await?
  }
}

/// Mint the singleton `SubtleCrypto` instance reachable as
/// `globalThis.crypto.subtle`.
#[op2]
#[cppgc]
pub fn op_create_subtle_crypto() -> SubtleCrypto {
  SubtleCrypto
}
