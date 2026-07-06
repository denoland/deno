// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.encapsulateBits()` / `decapsulateBits()` bodies in Rust.
//!
//! Per the WICG modern-algos spec, both methods are restricted to the
//! ML-KEM family (`ML-KEM-512` / `ML-KEM-768` / `ML-KEM-1024`); the
//! WebIDL converter rejects any other algorithm name via the
//! `Unknown` variant, which the impl methods turn into the
//! spec-mandated `NotSupportedError` `DOMException`. The compute path
//! mirrors the bytes-only logic in [`crate::mlkem::op_crypto_ml_kem_encapsulate`]
//! and [`crate::mlkem::op_crypto_ml_kem_decapsulate`], using the raw
//! public/private key bytes already captured in `SubtleKey`.

use std::borrow::Cow;

use aws_lc_rs::kem;
use deno_core::ToV8;
use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::crypto_key::CryptoKeyType;
use crate::mlkem::MlKemVariant;
use crate::subtle_encrypt::extract_name_and_obj;
use crate::subtle_key::SubtleKey;

pub enum SubtleEncapsulateParams {
  MlKem(MlKemVariant),
  Unknown(String),
}

impl<'a> WebIdlConverter<'a> for SubtleEncapsulateParams {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    let (name_str, _maybe_obj) =
      extract_name_and_obj(scope, value, prefix.clone(), context.borrowed())?;
    match canonical_name(&name_str) {
      Some("ML-KEM-512") => Ok(Self::MlKem(MlKemVariant::MlKem512)),
      Some("ML-KEM-768") => Ok(Self::MlKem(MlKemVariant::MlKem768)),
      Some("ML-KEM-1024") => Ok(Self::MlKem(MlKemVariant::MlKem1024)),
      _ => Ok(Self::Unknown(name_str)),
    }
  }
}

fn canonical_name(name: &str) -> Option<&'static str> {
  const NAMES: &[&str] = &["ML-KEM-512", "ML-KEM-768", "ML-KEM-1024"];
  NAMES.iter().copied().find(|n| n.eq_ignore_ascii_case(name))
}

/// Output of `SubtleCrypto.encapsulateBits()` — the ML-KEM ciphertext and
/// the encapsulated shared secret, both as `ArrayBuffer`.
///
/// Hand-rolled `ToV8` instead of `#[derive(ToV8)]` so the returned value is
/// a plain `Object` (default prototype, satisfies `instanceof Object` in
/// WPT) and the `ciphertext` / `sharedKey` slots are `ArrayBuffer`
/// instances (the modern-algos spec mandates `ArrayBuffer`, and the WPT
/// asserts `instanceof ArrayBuffer`). The macro-generated path would
/// produce a null-prototype object holding `Uint8Array` values, which
/// fails both checks.
pub struct EncapsulateBitsOutput {
  pub ciphertext: Vec<u8>,
  pub shared_key: Vec<u8>,
}

impl<'a> ToV8<'a> for EncapsulateBitsOutput {
  type Error = JsErrorBox;

  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    let ciphertext = bytes_to_array_buffer(scope, self.ciphertext);
    let shared_key = bytes_to_array_buffer(scope, self.shared_key);
    let obj = v8::Object::new(scope);
    let key = v8::String::new_from_one_byte(
      scope,
      b"ciphertext",
      v8::NewStringType::Internalized,
    )
    .ok_or_else(|| JsErrorBox::type_error("ciphertext key"))?;
    obj.set(scope, key.into(), ciphertext.into());
    let key = v8::String::new_from_one_byte(
      scope,
      b"sharedKey",
      v8::NewStringType::Internalized,
    )
    .ok_or_else(|| JsErrorBox::type_error("sharedKey key"))?;
    obj.set(scope, key.into(), shared_key.into());
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
    v8::ArrayBuffer::new_backing_store_from_bytes(bytes.into_boxed_slice());
  let backing_shared = backing.make_shared();
  v8::ArrayBuffer::with_backing_store(scope, &backing_shared)
}

pub fn run_encapsulate_bits(
  params: SubtleEncapsulateParams,
  key: SubtleKey,
) -> Result<EncapsulateBitsOutput, CryptoError> {
  let variant = match params {
    SubtleEncapsulateParams::MlKem(v) => v,
    SubtleEncapsulateParams::Unknown(name) => {
      return Err(not_supported(format!(
        "Encapsulation not supported for {name}"
      )));
    }
  };
  let canonical = params_canonical(variant);
  if key.algorithm_name != canonical {
    return Err(invalid_access(
      "Encapsulation key algorithm does not match".into(),
    ));
  }
  if key.key_type != CryptoKeyType::Public {
    return Err(invalid_access(
      "Encapsulation key must be a public key".into(),
    ));
  }
  if !key.has_usage("encapsulateBits") {
    return Err(invalid_access(
      "Encapsulation key usages must include 'encapsulateBits'".into(),
    ));
  }
  let public_key = key.raw.bytes();
  let alg = variant.algorithm();
  let ek = kem::EncapsulationKey::new(alg, public_key)
    .map_err(|_| op_error("Encapsulation failed".into()))?;
  let (ciphertext, shared_secret) = ek
    .encapsulate()
    .map_err(|_| op_error("Encapsulation failed".into()))?;
  Ok(EncapsulateBitsOutput {
    ciphertext: ciphertext.as_ref().to_vec(),
    shared_key: shared_secret.as_ref().to_vec(),
  })
}

pub fn run_decapsulate_bits(
  params: SubtleEncapsulateParams,
  key: SubtleKey,
  ciphertext: Vec<u8>,
) -> Result<Vec<u8>, CryptoError> {
  let variant = match params {
    SubtleEncapsulateParams::MlKem(v) => v,
    SubtleEncapsulateParams::Unknown(name) => {
      return Err(not_supported(format!(
        "Decapsulation not supported for {name}"
      )));
    }
  };
  let canonical = params_canonical(variant);
  if key.algorithm_name != canonical {
    return Err(invalid_access(
      "Decapsulation key algorithm does not match".into(),
    ));
  }
  if key.key_type != CryptoKeyType::Private {
    return Err(invalid_access(
      "Decapsulation key must be a private key".into(),
    ));
  }
  if !key.has_usage("decapsulateBits") {
    return Err(invalid_access(
      "Decapsulation key usages must include 'decapsulateBits'".into(),
    ));
  }
  let expected = variant.ciphertext_size();
  if ciphertext.len() != expected {
    return Err(op_error(format!(
      "ML-KEM {canonical} ciphertext must be {expected} bytes"
    )));
  }
  let private_key = key.raw.expanded_private_key();
  let alg = variant.algorithm();
  let dk = kem::DecapsulationKey::new(alg, private_key)
    .map_err(|_| op_error("Decapsulation failed".into()))?;
  let ct = kem::Ciphertext::from(ciphertext.as_slice());
  let shared_secret = dk
    .decapsulate(ct)
    .map_err(|_| op_error("Decapsulation failed".into()))?;
  Ok(shared_secret.as_ref().to_vec())
}

fn params_canonical(variant: MlKemVariant) -> &'static str {
  match variant {
    MlKemVariant::MlKem512 => "ML-KEM-512",
    MlKemVariant::MlKem768 => "ML-KEM-768",
    MlKemVariant::MlKem1024 => "ML-KEM-1024",
  }
}

fn invalid_access(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionInvalidAccessError", msg))
}

fn op_error(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionOperationError", msg))
}

fn not_supported(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionNotSupportedError", msg))
}
