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
use crate::subtle_decrypt::SubtleDecryptParams;
use crate::subtle_decrypt::run as run_decrypt;
use crate::subtle_derive_bits::SubtleDeriveBitsParams;
use crate::subtle_derive_bits::run as run_derive_bits;
use crate::subtle_encrypt::SubtleEncryptParams;
use crate::subtle_encrypt::run as run_encrypt;
use crate::subtle_key::SubtleKey;
use crate::subtle_sign::SubtleSignParams;
use crate::subtle_sign::run as run_sign;
use crate::subtle_verify::SubtleVerifyParams;
use crate::subtle_verify::run as run_verify;

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

  /// `SubtleCrypto.encrypt(algorithm, key, data)` — apply the requested
  /// encryption algorithm to `data` using `key`. The per-algorithm
  /// dictionary parsing (`label`, `iv`, `counter`, `length`, `tagLength`,
  /// `additionalData`) is done by the `SubtleEncryptParams`
  /// `WebIdlConverter`; the `SubtleKey` `WebIdlConverter` snapshots the
  /// `CryptoKey` slots (`algorithm.name`, `algorithm.length`,
  /// `algorithm.hash`, `usages`, `type`, and the underlying
  /// [`crate::key_store::CryptoKeyHandle`] data) so the dispatch can run
  /// off the v8 stack inside `spawn_blocking`.
  #[required(3)]
  #[arraybuffer]
  async fn encrypt(
    &self,
    #[webidl] algorithm: SubtleEncryptParams,
    #[webidl] key: SubtleKey,
    #[webidl] data: BufferSource,
  ) -> Result<Vec<u8>, CryptoError> {
    spawn_blocking(move || run_encrypt(algorithm, key, data.0)).await?
  }

  /// `SubtleCrypto.decrypt(algorithm, key, data)` — inverse of
  /// [`encrypt`](Self::encrypt). Same converter/dispatch shape; the JS
  /// shim used to enforce that an algorithm-name/key mismatch raises
  /// `OperationError` (whereas `encrypt` raised `InvalidAccessError`),
  /// which [`crate::subtle_decrypt::run`] preserves.
  #[required(3)]
  #[arraybuffer]
  async fn decrypt(
    &self,
    #[webidl] algorithm: SubtleDecryptParams,
    #[webidl] key: SubtleKey,
    #[webidl] data: BufferSource,
  ) -> Result<Vec<u8>, CryptoError> {
    spawn_blocking(move || run_decrypt(algorithm, key, data.0)).await?
  }

  /// `SubtleCrypto.sign(algorithm, key, data)` — produce a signature
  /// of `data` under `key`. The `SubtleSignParams` `WebIdlConverter`
  /// handles the per-algorithm parameter dictionary (`saltLength` for
  /// RSA-PSS, `hash` for ECDSA, optional `context` for ML-DSA), and
  /// `SubtleKey` snapshots the `CryptoKey` slots so the dispatch runs
  /// off the v8 stack inside `spawn_blocking`. Ed25519 stays a sync
  /// fastcall in the spawned closure.
  #[required(3)]
  #[arraybuffer]
  async fn sign(
    &self,
    #[webidl] algorithm: SubtleSignParams,
    #[webidl] key: SubtleKey,
    #[webidl] data: BufferSource,
  ) -> Result<Vec<u8>, CryptoError> {
    spawn_blocking(move || run_sign(algorithm, key, data.0)).await?
  }

  /// `SubtleCrypto.verify(algorithm, key, signature, data)` — mirror of
  /// [`sign`](Self::sign). The signature `BufferSource` is copied
  /// upfront so the spawned closure sees stable bytes; per-algorithm
  /// dispatch lives in [`crate::subtle_verify::run`].
  #[required(4)]
  async fn verify(
    &self,
    #[webidl] algorithm: SubtleVerifyParams,
    #[webidl] key: SubtleKey,
    #[webidl] signature: BufferSource,
    #[webidl] data: BufferSource,
  ) -> Result<bool, CryptoError> {
    spawn_blocking(move || run_verify(algorithm, key, signature.0, data.0))
      .await?
  }

  /// `SubtleCrypto.deriveBits(algorithm, baseKey, length?)` —
  /// algorithm-specific key derivation (PBKDF2 / HKDF / ECDH /
  /// X25519 / X448). The `SubtleDeriveBitsParams` `WebIdlConverter`
  /// pulls the algorithm dictionary (`hash` / `salt` / `iterations`
  /// for PBKDF2; `hash` / `salt` / `info` for HKDF; peer `public`
  /// `CryptoKey` for the ECDH / X-curve variants) and `SubtleKey`
  /// snapshots `baseKey`; dispatch lives in
  /// [`crate::subtle_derive_bits::run`].
  #[arraybuffer]
  #[rename("deriveBits")]
  async fn derive_bits(
    &self,
    #[webidl] algorithm: SubtleDeriveBitsParams,
    #[webidl] base_key: SubtleKey,
    length: Option<f64>,
  ) -> Result<Vec<u8>, CryptoError> {
    if !base_key.has_usage("deriveBits") {
      return Err(CryptoError::Other(deno_error::JsErrorBox::new(
        "DOMExceptionInvalidAccessError",
        "'baseKey' usages does not contain 'deriveBits'",
      )));
    }
    spawn_blocking(move || run_derive_bits(algorithm, base_key, length)).await?
  }

  /// Internal entry point used by `SubtleCrypto.prototype.deriveKey`
  /// (still in JS) to derive raw bits without re-checking the
  /// `deriveBits` usage on the base key. The JS shim has already
  /// enforced the `deriveKey` usage and algorithm match before calling
  /// this method.
  #[arraybuffer]
  #[rename("__deriveBitsInternal")]
  async fn derive_bits_internal(
    &self,
    #[webidl] algorithm: SubtleDeriveBitsParams,
    #[webidl] base_key: SubtleKey,
    length: Option<f64>,
  ) -> Result<Vec<u8>, CryptoError> {
    spawn_blocking(move || run_derive_bits(algorithm, base_key, length)).await?
  }
}

/// Mint the singleton `SubtleCrypto` instance reachable as
/// `globalThis.crypto.subtle`.
#[op2]
#[cppgc]
pub fn op_create_subtle_crypto() -> SubtleCrypto {
  SubtleCrypto
}
