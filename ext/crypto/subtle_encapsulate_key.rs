// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.encapsulateKey()` / `decapsulateKey()` bodies in Rust.
//!
//! Composes the ML-KEM ciphertext/shared-secret produced by
//! [`crate::subtle_encapsulate`] with the Rust-native
//! [`crate::subtle_import_key::run`] (`raw-secret` form) so the shared
//! `CryptoKey` is minted entirely from Rust.

use deno_core::ToV8;
use deno_core::v8;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::crypto_key::CryptoKeyType;
use crate::subtle_encapsulate::SubtleEncapsulateParams;
use crate::subtle_encapsulate::run_decapsulate_bits;
use crate::subtle_encapsulate::run_encapsulate_bits;
use crate::subtle_export_key::KeyFormat;
use crate::subtle_import_key::ImportAlgorithm;
use crate::subtle_import_key::ImportKeyData;
use crate::subtle_import_key::run as run_import_key;
use crate::subtle_key::SubtleKey;

/// Output of `SubtleCrypto.encapsulateKey()` — `{ ciphertext: ArrayBuffer,
/// sharedKey: CryptoKey }`. Hand-rolled `ToV8` so the result is a plain
/// `Object` with the spec-mandated shape and the `ciphertext` slot is an
/// `ArrayBuffer` (the modern-algos spec mandates `ArrayBuffer`).
pub struct EncapsulateKeyOutput<'s> {
  pub ciphertext: Vec<u8>,
  pub shared_key: v8::Local<'s, v8::Object>,
}

impl<'a> ToV8<'a> for EncapsulateKeyOutput<'a> {
  type Error = JsErrorBox;

  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    let obj = v8::Object::new(scope);
    let ciphertext = bytes_to_array_buffer(scope, self.ciphertext);
    let key1 = v8::String::new_from_one_byte(
      scope,
      b"ciphertext",
      v8::NewStringType::Internalized,
    )
    .ok_or_else(|| JsErrorBox::type_error("ciphertext"))?;
    obj.set(scope, key1.into(), ciphertext.into());
    let key2 = v8::String::new_from_one_byte(
      scope,
      b"sharedKey",
      v8::NewStringType::Internalized,
    )
    .ok_or_else(|| JsErrorBox::type_error("sharedKey"))?;
    obj.set(scope, key2.into(), self.shared_key.into());
    Ok(obj.into())
  }
}

fn bytes_to_array_buffer<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  bytes: Vec<u8>,
) -> v8::Local<'s, v8::ArrayBuffer> {
  if bytes.is_empty() {
    return v8::ArrayBuffer::new(scope, 0);
  }
  let backing =
    v8::ArrayBuffer::new_backing_store_from_bytes(bytes.into_boxed_slice())
      .make_shared();
  v8::ArrayBuffer::with_backing_store(scope, &backing)
}

pub fn run_encapsulate_key<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  params: SubtleEncapsulateParams,
  encapsulation_key: SubtleKey,
  shared_key_algorithm: ImportAlgorithm,
  extractable: bool,
  usages: Vec<String>,
) -> Result<EncapsulateKeyOutput<'s>, CryptoError> {
  // The encapsulateBits path enforces "key.type === public",
  // "algorithm.name match", and "usages includes encapsulateBits". For
  // encapsulateKey the WICG spec requires "encapsulateKey" instead, so
  // we re-check usage here before delegating the bytes computation.
  if !encapsulation_key.has_usage("encapsulateKey") {
    return Err(invalid_access(
      "Encapsulation key usages must include 'encapsulateKey'".into(),
    ));
  }
  // Temporarily swap `encapsulateKey` for `encapsulateBits` so the
  // shared bytes path's usage check passes without re-running validation.
  let bits_input = SubtleKey {
    usages: encapsulation_key
      .usages
      .iter()
      .map(|u| {
        if u == "encapsulateKey" {
          "encapsulateBits".to_string()
        } else {
          u.clone()
        }
      })
      .collect(),
    ..encapsulation_key
  };
  let out = run_encapsulate_bits(params, bits_input)?;
  let shared_key = run_import_key(
    scope,
    KeyFormat::RawSecret,
    &shared_key_algorithm,
    ImportKeyData::Buffer(out.shared_key),
    extractable,
    &usages,
  )?;
  Ok(EncapsulateKeyOutput {
    ciphertext: out.ciphertext,
    shared_key,
  })
}

pub fn run_decapsulate_key<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  params: SubtleEncapsulateParams,
  decapsulation_key: SubtleKey,
  shared_key_algorithm: ImportAlgorithm,
  ciphertext: Vec<u8>,
  extractable: bool,
  usages: Vec<String>,
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  if !decapsulation_key.has_usage("decapsulateKey") {
    return Err(invalid_access(
      "Decapsulation key usages must include 'decapsulateKey'".into(),
    ));
  }
  let bits_input = SubtleKey {
    usages: decapsulation_key
      .usages
      .iter()
      .map(|u| {
        if u == "decapsulateKey" {
          "decapsulateBits".to_string()
        } else {
          u.clone()
        }
      })
      .collect(),
    ..decapsulation_key
  };
  let shared_bytes = run_decapsulate_bits(params, bits_input, ciphertext)?;
  let shared_key = run_import_key(
    scope,
    KeyFormat::RawSecret,
    &shared_key_algorithm,
    ImportKeyData::Buffer(shared_bytes),
    extractable,
    &usages,
  )?;
  // Mirror the JS forwarder's "private/secret + empty usages →
  // SyntaxError" rule. Fail closed: `key_type_of` errors if the
  // freshly-imported shared key isn't a cppgc CryptoKey instead of
  // silently skipping the check.
  let key_type = key_type_of(scope, shared_key)?;
  if matches!(key_type, CryptoKeyType::Private | CryptoKeyType::Secret)
    && usages.is_empty()
  {
    return Err(syntax_error("Invalid key usage".into()));
  }
  Ok(shared_key)
}

fn key_type_of<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: v8::Local<'s, v8::Object>,
) -> Result<CryptoKeyType, CryptoError> {
  let ptr = deno_core::cppgc::try_unwrap_cppgc_object::<
    crate::crypto_key::CryptoKey,
  >(scope, key.into())
  .ok_or_else(|| {
    CryptoError::Other(JsErrorBox::type_error(
      "internal: decapsulated key is not a CryptoKey",
    ))
  })?;
  Ok(ptr.key_type())
}

fn invalid_access(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionInvalidAccessError", msg))
}

fn syntax_error(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionSyntaxError", msg))
}
