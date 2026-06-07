// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.deriveKey()` body in Rust.
//!
//! Compose path: validates the `baseKey` algorithm match + `deriveKey`
//! usage, runs the `deriveBits` synchronously off the v8 stack via
//! [`crate::subtle_derive_bits::run`], then mints the derived `CryptoKey`
//! through the Rust `raw-secret` import path
//! ([`crate::subtle_import_key::run`]).

use deno_core::ToV8;
use deno_core::v8;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::algorithm::compute_key_length;
use crate::subtle_derive_bits::SubtleDeriveBitsParams;
use crate::subtle_export_key::KeyFormat;
use crate::subtle_import_key::ImportAlgorithm;
use crate::subtle_import_key::ImportKeyData;
use crate::subtle_import_key::run as run_import_key;
use crate::subtle_key::SubtleKey;

/// The bytes-and-import-params bundle returned from the async
/// `SubtleCrypto.deriveKey` body. The `ToV8` impl builds the derived
/// `CryptoKey` synchronously in the JS scope after the spawn_blocking
/// completes, by calling [`run_import_key`] with `KeyFormat::RawSecret`.
pub struct DerivedKey {
  pub bits: Vec<u8>,
  pub derived_algorithm: ImportAlgorithm,
  pub extractable: bool,
  pub usages: Vec<String>,
}

impl<'a> ToV8<'a> for DerivedKey {
  type Error = JsErrorBox;

  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    let DerivedKey {
      bits,
      derived_algorithm,
      extractable,
      usages,
    } = self;
    let key = run_import_key(
      scope,
      KeyFormat::RawSecret,
      &derived_algorithm,
      ImportKeyData::Buffer(bits),
      extractable,
      &usages,
    )
    .map_err(crypto_error_to_js)?;
    Ok(key.into())
  }
}

fn crypto_error_to_js(err: CryptoError) -> JsErrorBox {
  match err {
    CryptoError::Other(b) => b,
    other => JsErrorBox::from_err(other),
  }
}

/// Resolve the per-algorithm derived-key length used to feed `deriveBits`.
/// Mirrors the legacy JS `op_crypto_get_key_length(name, length, hash)`
/// call after the `normalizeAlgorithm(..., "get key length")` step.
pub fn key_length_for(
  derived: &ImportAlgorithm,
) -> Result<Option<u32>, CryptoError> {
  compute_key_length(
    &derived.name,
    derived.length,
    derived.hash_name.as_deref(),
  )
  .map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))
}

pub fn check_base_key(
  params: &SubtleDeriveBitsParams,
  base_key: &SubtleKey,
) -> Result<(), CryptoError> {
  if params.canonical_name() != base_key.algorithm_name {
    return Err(CryptoError::Other(JsErrorBox::new(
      "DOMExceptionInvalidAccessError",
      format!("Invalid algorithm name: {}", params.canonical_name()),
    )));
  }
  if !base_key.has_usage("deriveKey") {
    return Err(CryptoError::Other(JsErrorBox::new(
      "DOMExceptionInvalidAccessError",
      "'baseKey' usages does not contain 'deriveKey'",
    )));
  }
  Ok(())
}

/// Bundle the derive-bits output with the import params the cppgc method
/// will hand off to `ToV8` for the synchronous import step.
pub fn run(
  bits: Vec<u8>,
  derived_algorithm: ImportAlgorithm,
  extractable: bool,
  usages: Vec<String>,
) -> DerivedKey {
  DerivedKey {
    bits,
    derived_algorithm,
    extractable,
    usages,
  }
}
