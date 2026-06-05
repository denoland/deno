// Copyright 2018-2026 the Deno authors. MIT license.

//! Rust implementation of the `SubtleCrypto` key-wrapping
//! (`wrapKey` / `unwrapKey`) and key-encapsulation
//! (`encapsulateKey` / `encapsulateBits` / `decapsulateKey` /
//! `decapsulateBits`) methods.
//!
//! These methods are *composites*: the surrounding JS still performs webidl
//! argument conversion, `normalizeAlgorithm`, and the parts that must touch the
//! `CryptoKey` JS class (calling `exportKey` / `importKey`, constructing keys,
//! JWK (de)serialization). What is ported here is the per-method validation
//! that throws `DOMException`s plus the actual crypto:
//!
//!   * `op_crypto_wrap_key_web` / `op_crypto_unwrap_key_web`: the AES-KW branch
//!     (the only "wrapKey"/"unwrapKey" registered algorithm). The shared
//!     validations that gate *both* the AES-KW branch and the encrypt/decrypt
//!     branch (algorithm-name mismatch, usage, extractable) intentionally stay
//!     in JS because they run before the branch is chosen and the
//!     encrypt/decrypt branch is handled entirely in JS via `encrypt`/
//!     `decrypt`. These ops re-validate the AES-KW preconditions (key is a
//!     secret key, data length multiple of 8, key length 16/24/32) and run the
//!     RFC 3394 wrap/unwrap.
//!
//!   * `op_crypto_encapsulate_web` / `op_crypto_decapsulate_web`: the full
//!     validation (`InvalidAccessError` for algorithm/type/usage,
//!     `OperationError` on crypto failure, `OperationError` for bad ciphertext
//!     size, `NotSupportedError` for unknown variants) plus the ML-KEM
//!     encapsulate/decapsulate. The buffer-returning `encapsulateBits` /
//!     `decapsulateBits` consume these directly; `encapsulateKey` /
//!     `decapsulateKey` use the same ops then hand the shared secret back to JS
//!     `importKey` to build the `CryptoKey`.
//!
//! All four `DOMException` class strings used here
//! (`InvalidAccessError`, `OperationError`, `DataError`, `NotSupportedError`)
//! are registered in `runtime/js/99_main.js`, so the `#[class("DOMException...")]`
//! attributes surface as real `DOMException`s.
//!
//! The AES-KW and ML-KEM compute is deliberately *replicated* from
//! `op_crypto_wrap_key` / `op_crypto_unwrap_key` (lib.rs) and
//! `op_crypto_ml_kem_encapsulate` / `op_crypto_ml_kem_decapsulate`
//! (mlkem.rs) because one op cannot call another and the task forbids editing
//! those files. The parent is expected to dedup this later.

use aes_kw::KekAes128;
use aes_kw::KekAes192;
use aes_kw::KekAes256;
use aws_lc_rs::kem;
use deno_core::JsBuffer;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use serde::Deserialize;

use crate::mlkem::MlKemVariant;
use crate::shared::V8RawKeyData;

/// Errors mirroring the `DOMException`s thrown by the JS wrap/unwrap and
/// encapsulate/decapsulate methods. Every class string used here has a
/// registered builder in `runtime/js/99_main.js`.
#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum WebWrapKemError {
  #[class("DOMExceptionInvalidAccessError")]
  #[error("{0}")]
  InvalidAccess(String),
  #[class("DOMExceptionOperationError")]
  #[error("{0}")]
  Operation(String),
  #[class("DOMExceptionNotSupportedError")]
  #[error("{0}")]
  NotSupported(String),
  #[class("DOMExceptionDataError")]
  #[error("{0}")]
  Data(String),
}

// ---------------------------------------------------------------------------
// wrapKey / unwrapKey (AES-KW branch)
// ---------------------------------------------------------------------------

/// Arguments for the AES-KW wrap/unwrap branch. `key` is the wrapping key's raw
/// material (`WeakMapPrototypeGet(KEY_STORE, wrappingKey[_handle])`), shaped as
/// `{ type, data }` exactly as the existing ops expect.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AesKwArg {
  key: V8RawKeyData,
}

/// AES-KW wrap. Mirrors `op_crypto_wrap_key`'s AES-KW arm: the wrapping key must
/// be a secret key (`as_secret_key`, TypeError if not), `data` must be a
/// multiple of 8 bytes, and the key must be 128/192/256-bit. Wrap failure maps
/// to `OperationError`, matching the original op's `EncryptionError`
/// (registered as `DOMExceptionOperationError`).
#[op2]
pub fn op_crypto_wrap_key_web(
  #[serde] arg: AesKwArg,
  #[buffer] data: JsBuffer,
) -> Result<Uint8Array, WebWrapKemError> {
  let key = arg.key.as_secret_key().map_err(|_| {
    WebWrapKemError::InvalidAccess("Key type not supported".to_string())
  })?;

  if !data.len().is_multiple_of(8) {
    return Err(WebWrapKemError::Data(
      "data length is not a multiple of 8 bytes".to_string(),
    ));
  }

  let wrapped_key = match key.len() {
    16 => KekAes128::new(key.into()).wrap_vec(&data),
    24 => KekAes192::new(key.into()).wrap_vec(&data),
    32 => KekAes256::new(key.into()).wrap_vec(&data),
    _ => {
      return Err(WebWrapKemError::Data("invalid key length".to_string()));
    }
  }
  .map_err(|_| WebWrapKemError::Operation("encryption error".to_string()))?;

  Ok(wrapped_key.into())
}

/// AES-KW unwrap. Mirrors `op_crypto_unwrap_key`'s AES-KW arm. Unwrap failure
/// maps to `OperationError`, matching the original op's `DecryptionError`.
#[op2]
pub fn op_crypto_unwrap_key_web(
  #[serde] arg: AesKwArg,
  #[buffer] data: JsBuffer,
) -> Result<Uint8Array, WebWrapKemError> {
  let key = arg.key.as_secret_key().map_err(|_| {
    WebWrapKemError::InvalidAccess("Key type not supported".to_string())
  })?;

  if !data.len().is_multiple_of(8) {
    return Err(WebWrapKemError::Data(
      "data length is not a multiple of 8 bytes".to_string(),
    ));
  }

  let unwrapped_key = match key.len() {
    16 => KekAes128::new(key.into()).unwrap_vec(&data),
    24 => KekAes192::new(key.into()).unwrap_vec(&data),
    32 => KekAes256::new(key.into()).unwrap_vec(&data),
    _ => {
      return Err(WebWrapKemError::Data("invalid key length".to_string()));
    }
  }
  .map_err(|_| WebWrapKemError::Operation("decryption error".to_string()))?;

  Ok(unwrapped_key.into())
}

// ---------------------------------------------------------------------------
// encapsulateKey / encapsulateBits / decapsulateKey / decapsulateBits
// ---------------------------------------------------------------------------

/// Returns the algorithm for an ML-KEM variant. Replicated from
/// `MlKemVariant::algorithm` (which is private to mlkem.rs).
fn mlkem_algorithm(
  variant: MlKemVariant,
) -> &'static kem::Algorithm<kem::AlgorithmId> {
  match variant {
    MlKemVariant::MlKem512 => &kem::ML_KEM_512,
    MlKemVariant::MlKem768 => &kem::ML_KEM_768,
    MlKemVariant::MlKem1024 => &kem::ML_KEM_1024,
  }
}

/// Expected ciphertext size per variant (FIPS 203). Mirrors
/// `ML_KEM_CIPHERTEXT_SIZES` in `00_crypto.js`.
fn mlkem_ciphertext_size(variant: MlKemVariant) -> usize {
  match variant {
    MlKemVariant::MlKem512 => 768,
    MlKemVariant::MlKem768 => 1088,
    MlKemVariant::MlKem1024 => 1568,
  }
}

#[derive(deno_core::ToV8)]
pub struct EncapsulateOutput {
  pub ciphertext: Uint8Array,
  pub shared_secret: Uint8Array,
}

/// Arguments common to encapsulate/decapsulate. `algorithm` is the normalized
/// algorithm name (e.g. "ML-KEM-768"). `key_type` is the `CryptoKey`'s
/// `[[type]]`, `key_usages` are its `[[usages]]`, and `key_algorithm_name` is
/// `key[_algorithm].name`. `key_data` is the raw key material from the
/// `KEY_STORE` (the encapsulation public key bytes, or the decapsulation
/// private key bytes).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncapsulateArg {
  algorithm: String,
  key_type: String,
  key_usages: Vec<String>,
  key_algorithm_name: String,
}

/// Parse + validate the ML-KEM variant from the (already normalized) algorithm
/// name. Unknown names are `NotSupportedError`, matching the `default` arm of
/// `mlKemEncapsulate`/`mlKemDecapsulate`.
fn mlkem_variant(
  name: &str,
  encapsulate: bool,
) -> Result<MlKemVariant, WebWrapKemError> {
  match name {
    "ML-KEM-512" => Ok(MlKemVariant::MlKem512),
    "ML-KEM-768" => Ok(MlKemVariant::MlKem768),
    "ML-KEM-1024" => Ok(MlKemVariant::MlKem1024),
    other => Err(WebWrapKemError::NotSupported(if encapsulate {
      format!("Encapsulation not supported for {other}")
    } else {
      format!("Decapsulation not supported for {other}")
    })),
  }
}

/// `encapsulateKey` / `encapsulateBits` crypto + validation. The JS stub passes
/// the encapsulation key's fields and raw public-key bytes; the resulting
/// `{ ciphertext, sharedSecret }` is returned to JS, which either hands
/// `sharedSecret` to `importKey` (`encapsulateKey`) or returns both buffers
/// directly (`encapsulateBits`).
///
/// `usage` is "encapsulateKey" or "encapsulateBits" depending on the caller, so
/// the usage validation uses the correct required usage.
/// Owned encapsulate/decapsulate key fields + algorithm name.
pub(crate) struct KemArg {
  pub algorithm: String,
  pub key_type: String,
  pub key_usages: Vec<String>,
  pub key_algorithm_name: String,
}

/// `(ciphertext, shared_secret)` for `encapsulate*`.
pub(crate) fn encapsulate_compute(
  arg: KemArg,
  usage: &str,
  key_data: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), WebWrapKemError> {
  if arg.key_algorithm_name != arg.algorithm {
    return Err(WebWrapKemError::InvalidAccess(
      "Encapsulation key algorithm does not match".to_string(),
    ));
  }
  if arg.key_type != "public" {
    return Err(WebWrapKemError::InvalidAccess(
      "Encapsulation key must be a public key".to_string(),
    ));
  }
  if !arg.key_usages.iter().any(|u| u == usage) {
    return Err(WebWrapKemError::InvalidAccess(format!(
      "Encapsulation key usages must include '{usage}'"
    )));
  }
  let variant = mlkem_variant(&arg.algorithm, true)?;
  let alg = mlkem_algorithm(variant);
  let ek = kem::EncapsulationKey::new(alg, key_data).map_err(|_| {
    WebWrapKemError::Operation("Encapsulation failed".to_string())
  })?;
  let (ciphertext, shared_secret) = ek.encapsulate().map_err(|_| {
    WebWrapKemError::Operation("Encapsulation failed".to_string())
  })?;
  Ok((
    ciphertext.as_ref().to_vec(),
    shared_secret.as_ref().to_vec(),
  ))
}

/// Raw shared secret for `decapsulate*`.
pub(crate) fn decapsulate_compute(
  arg: KemArg,
  usage: &str,
  key_data: &[u8],
  ciphertext: &[u8],
) -> Result<Vec<u8>, WebWrapKemError> {
  if arg.key_algorithm_name != arg.algorithm {
    return Err(WebWrapKemError::InvalidAccess(
      "Decapsulation key algorithm does not match".to_string(),
    ));
  }
  if arg.key_type != "private" {
    return Err(WebWrapKemError::InvalidAccess(
      "Decapsulation key must be a private key".to_string(),
    ));
  }
  if !arg.key_usages.iter().any(|u| u == usage) {
    return Err(WebWrapKemError::InvalidAccess(format!(
      "Decapsulation key usages must include '{usage}'"
    )));
  }
  let variant = mlkem_variant(&arg.algorithm, false)?;
  let expected = mlkem_ciphertext_size(variant);
  if ciphertext.len() != expected {
    return Err(WebWrapKemError::Operation(format!(
      "ML-KEM {} ciphertext must be {expected} bytes",
      arg.algorithm
    )));
  }
  let alg = mlkem_algorithm(variant);
  let dk = kem::DecapsulationKey::new(alg, key_data).map_err(|_| {
    WebWrapKemError::Operation("Decapsulation failed".to_string())
  })?;
  let ct = kem::Ciphertext::from(ciphertext);
  let shared_secret = dk.decapsulate(ct).map_err(|_| {
    WebWrapKemError::Operation("Decapsulation failed".to_string())
  })?;
  Ok(shared_secret.as_ref().to_vec())
}

#[op2]
pub fn op_crypto_encapsulate_web(
  #[serde] arg: EncapsulateArg,
  #[string] usage: String,
  #[buffer] key_data: JsBuffer,
) -> Result<EncapsulateOutput, WebWrapKemError> {
  // InvalidAccessError validations (mirror the JS order).
  if arg.key_algorithm_name != arg.algorithm {
    return Err(WebWrapKemError::InvalidAccess(
      "Encapsulation key algorithm does not match".to_string(),
    ));
  }
  if arg.key_type != "public" {
    return Err(WebWrapKemError::InvalidAccess(
      "Encapsulation key must be a public key".to_string(),
    ));
  }
  if !arg.key_usages.contains(&usage) {
    return Err(WebWrapKemError::InvalidAccess(format!(
      "Encapsulation key usages must include '{usage}'"
    )));
  }

  let variant = mlkem_variant(&arg.algorithm, true)?;
  let alg = mlkem_algorithm(variant);

  // Both EncapsulationKey::new and encapsulate() failures map to OperationError,
  // matching the JS `catch (_) { throw OperationError }` around the op call.
  let ek = kem::EncapsulationKey::new(alg, &key_data).map_err(|_| {
    WebWrapKemError::Operation("Encapsulation failed".to_string())
  })?;
  let (ciphertext, shared_secret) = ek.encapsulate().map_err(|_| {
    WebWrapKemError::Operation("Encapsulation failed".to_string())
  })?;

  Ok(EncapsulateOutput {
    ciphertext: ciphertext.as_ref().to_vec().into(),
    shared_secret: shared_secret.as_ref().to_vec().into(),
  })
}

/// `decapsulateKey` / `decapsulateBits` crypto + validation. Returns the raw
/// shared secret bytes; JS either hands them to `importKey` (`decapsulateKey`)
/// or returns them directly (`decapsulateBits`).
#[op2]
pub fn op_crypto_decapsulate_web(
  #[serde] arg: EncapsulateArg,
  #[string] usage: String,
  #[buffer] key_data: JsBuffer,
  #[buffer] ciphertext: JsBuffer,
) -> Result<Uint8Array, WebWrapKemError> {
  // InvalidAccessError validations (mirror the JS order).
  if arg.key_algorithm_name != arg.algorithm {
    return Err(WebWrapKemError::InvalidAccess(
      "Decapsulation key algorithm does not match".to_string(),
    ));
  }
  if arg.key_type != "private" {
    return Err(WebWrapKemError::InvalidAccess(
      "Decapsulation key must be a private key".to_string(),
    ));
  }
  if !arg.key_usages.contains(&usage) {
    return Err(WebWrapKemError::InvalidAccess(format!(
      "Decapsulation key usages must include '{usage}'"
    )));
  }

  let variant = mlkem_variant(&arg.algorithm, false)?;

  // Ciphertext size check (OperationError), matching `mlKemDecapsulate`.
  let expected = mlkem_ciphertext_size(variant);
  if ciphertext.len() != expected {
    return Err(WebWrapKemError::Operation(format!(
      "ML-KEM {} ciphertext must be {expected} bytes",
      arg.algorithm
    )));
  }

  let alg = mlkem_algorithm(variant);
  // DecapsulationKey::new and decapsulate() failures both map to OperationError,
  // matching the JS `catch (_) { throw OperationError }`.
  let dk = kem::DecapsulationKey::new(alg, &key_data).map_err(|_| {
    WebWrapKemError::Operation("Decapsulation failed".to_string())
  })?;
  let ct = kem::Ciphertext::from(&*ciphertext);
  let shared_secret = dk.decapsulate(ct).map_err(|_| {
    WebWrapKemError::Operation("Decapsulation failed".to_string())
  })?;

  Ok(shared_secret.as_ref().to_vec().into())
}
