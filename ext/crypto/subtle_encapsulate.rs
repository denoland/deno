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
use deno_core::convert::Uint8Array;
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
#[derive(deno_core::ToV8)]
pub struct EncapsulateBitsOutput {
  pub ciphertext: Uint8Array,
  pub shared_key: Uint8Array,
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
    ciphertext: ciphertext.as_ref().to_vec().into(),
    shared_key: shared_secret.as_ref().to_vec().into(),
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
