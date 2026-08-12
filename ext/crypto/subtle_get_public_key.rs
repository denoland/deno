// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.getPublicKey()` body in Rust.
//!
//! Per the WICG modern-algos spec, derives the matching public key of a
//! private CryptoKey for asymmetric algorithms (RSA-*, ECDSA, ECDH,
//! Ed25519, X25519, X448, ML-DSA, ML-KEM, SLH-DSA). For ML-KEM the public key is
//! recovered from the expanded decapsulation key bytes; for ML-DSA the
//! key pair is re-derived from the seed (always present in the seeded
//! private key material). For RSA/EC keys the path round-trips through
//! the existing SPKI export + import. For OKP keys the path computes the
//! raw public key bytes and re-imports as JWK.

use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use deno_core::v8;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::crypto_key::CryptoKeyType;
use crate::ed25519::jwk_x_ed25519;
use crate::export_key::ExportKeyAlgorithm;
use crate::export_key::ExportKeyFormat;
use crate::export_key::ExportKeyOptions;
use crate::export_key::export_key_with_raw;
use crate::make_key::AlgorithmDict;
use crate::make_key::make_crypto_key;
use crate::shared::EcNamedCurve;
use crate::shared::RawKeyData;
use crate::subtle_export_key::KeyFormat;
use crate::subtle_import_key::ImportAlgorithm;
use crate::subtle_import_key::ImportKeyData;
use crate::subtle_import_key::run as run_import_key;
use crate::subtle_key::SubtleKey;
use crate::x448::x448_public_key;
use crate::x25519::x25519_public_key;

pub fn run<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: SubtleKey,
  usages: Vec<String>,
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  let algorithm_name = key.algorithm_name.as_str();
  match algorithm_name {
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" | "RSA-OAEP" | "ECDSA" | "ECDH"
    | "Ed25519" | "X25519" | "X448" | "ML-DSA-44" | "ML-DSA-65"
    | "ML-DSA-87" | "ML-KEM-512" | "ML-KEM-768" | "ML-KEM-1024" => {}
    _ if crate::slhdsa::variant_from_name(algorithm_name).is_some() => {}
    other => {
      return Err(not_supported(format!(
        "getPublicKey() is not supported for {other}"
      )));
    }
  }
  if key.key_type != CryptoKeyType::Private {
    return Err(invalid_access(
      "Public keys can only be derived from private keys".into(),
    ));
  }

  match algorithm_name {
    "ML-KEM-512" | "ML-KEM-768" | "ML-KEM-1024" => {
      validate_public_key_usages(
        &usages,
        &["encapsulateKey", "encapsulateBits"],
      )?;
      let variant = match algorithm_name {
        "ML-KEM-512" => crate::mlkem::MlKemVariant::MlKem512,
        "ML-KEM-768" => crate::mlkem::MlKemVariant::MlKem768,
        "ML-KEM-1024" => crate::mlkem::MlKemVariant::MlKem1024,
        _ => unreachable!(),
      };
      let public_key = crate::mlkem::public_from_expanded(
        variant,
        key.raw.expanded_private_key(),
      )
      .map_err(|_| op_error("Failed to derive public key".into()))?;
      let usages_strs: Vec<&str> = usages.iter().map(String::as_str).collect();
      Ok(make_crypto_key(
        scope,
        CryptoKeyType::Public,
        true,
        &usages_strs,
        AlgorithmDict::new(algorithm_name),
        RawKeyData::Raw(public_key.into_boxed_slice()),
      ))
    }
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => {
      validate_public_key_usages(&usages, &["verify"])?;
      let variant = match algorithm_name {
        "ML-DSA-44" => 0u8,
        "ML-DSA-65" => 1,
        "ML-DSA-87" => 2,
        _ => unreachable!(),
      };
      let seed = key
        .raw
        .seed()
        .ok_or_else(|| op_error("Failed to derive public key".into()))?;
      let derived = crate::mldsa::from_seed(variant, seed)
        .map_err(|_| op_error("Failed to derive public key".into()))?;
      let usages_strs: Vec<&str> = usages.iter().map(String::as_str).collect();
      Ok(make_crypto_key(
        scope,
        CryptoKeyType::Public,
        true,
        &usages_strs,
        AlgorithmDict::new(algorithm_name),
        RawKeyData::Raw(derived.public_key.into_boxed_slice()),
      ))
    }
    _ if let Some(variant) =
      crate::slhdsa::variant_from_name(algorithm_name) =>
    {
      validate_public_key_usages(&usages, &["verify"])?;
      let public_key = crate::slhdsa::public_from_private(
        variant,
        key.raw.expanded_private_key(),
      )
      .map_err(|_| op_error("Failed to derive public key".into()))?;
      let usages_strs: Vec<&str> = usages.iter().map(String::as_str).collect();
      Ok(make_crypto_key(
        scope,
        CryptoKeyType::Public,
        true,
        &usages_strs,
        AlgorithmDict::new(algorithm_name),
        RawKeyData::Raw(public_key.into_boxed_slice()),
      ))
    }
    "RSASSA-PKCS1-v1_5" | "RSA-PSS" | "RSA-OAEP" => {
      let alg = match algorithm_name {
        "RSASSA-PKCS1-v1_5" => ExportKeyAlgorithm::RsassaPkcs1v15 {},
        "RSA-PSS" => ExportKeyAlgorithm::RsaPss {},
        "RSA-OAEP" => ExportKeyAlgorithm::RsaOaep {},
        _ => unreachable!(),
      };
      let opts = ExportKeyOptions::new(ExportKeyFormat::Spki, alg);
      let spki_bytes =
        export_to_bytes(opts, &key.raw, "Failed to derive public key")?;
      let imp_alg = ImportAlgorithm {
        name: algorithm_name.to_string(),
        hash_name: key.algorithm_hash.map(sha_hash_name).map(str::to_string),
        length: None,
        named_curve: None,
        modulus_length: None,
        public_exponent: None,
      };
      run_import_key(
        scope,
        KeyFormat::Spki,
        &imp_alg,
        ImportKeyData::Buffer(spki_bytes),
        true,
        &usages,
      )
    }
    "ECDSA" | "ECDH" => {
      let curve = key
        .algorithm_named_curve
        .as_deref()
        .and_then(parse_named_curve)
        .ok_or_else(|| op_error("Failed to derive public key".into()))?;
      let alg = match algorithm_name {
        "ECDSA" => ExportKeyAlgorithm::Ecdsa { named_curve: curve },
        "ECDH" => ExportKeyAlgorithm::Ecdh { named_curve: curve },
        _ => unreachable!(),
      };
      let opts = ExportKeyOptions::new(ExportKeyFormat::Spki, alg);
      let spki_bytes =
        export_to_bytes(opts, &key.raw, "Failed to derive public key")?;
      let imp_alg = ImportAlgorithm {
        name: algorithm_name.to_string(),
        hash_name: None,
        length: None,
        named_curve: key.algorithm_named_curve.clone(),
        modulus_length: None,
        public_exponent: None,
      };
      run_import_key(
        scope,
        KeyFormat::Spki,
        &imp_alg,
        ImportKeyData::Buffer(spki_bytes),
        true,
        &usages,
      )
    }
    "Ed25519" | "X25519" | "X448" => {
      let x = match algorithm_name {
        "Ed25519" => jwk_x_ed25519(key.raw.bytes())
          .map_err(|_| op_error("Failed to derive public key".into()))?,
        "X25519" => x25519_public_key(key.raw.bytes()),
        "X448" => x448_public_key(key.raw.bytes())
          .map_err(|_| op_error("Failed to derive public key".into()))?,
        _ => unreachable!(),
      };
      let pub_bytes = BASE64_URL_SAFE_NO_PAD
        .decode(x.trim_end_matches('='))
        .map_err(|_| op_error("Failed to derive public key".into()))?;
      let imp_alg = ImportAlgorithm {
        name: algorithm_name.to_string(),
        hash_name: None,
        length: None,
        named_curve: None,
        modulus_length: None,
        public_exponent: None,
      };
      run_import_key(
        scope,
        KeyFormat::Raw,
        &imp_alg,
        ImportKeyData::Buffer(pub_bytes),
        true,
        &usages,
      )
    }
    _ => unreachable!(),
  }
}

fn validate_public_key_usages(
  usages: &[String],
  allowed: &[&str],
) -> Result<(), CryptoError> {
  for u in usages {
    if !allowed.contains(&u.as_str()) {
      return Err(CryptoError::Other(JsErrorBox::new(
        "DOMExceptionSyntaxError",
        "Invalid key usage",
      )));
    }
  }
  Ok(())
}

fn export_to_bytes(
  opts: ExportKeyOptions,
  key_data: &RawKeyData,
  err_msg: &str,
) -> Result<Vec<u8>, CryptoError> {
  let res = export_key_with_raw(opts, key_data)
    .map_err(|_| op_error(err_msg.into()))?;
  match res {
    crate::export_key::ExportKeyResult::Spki(b)
    | crate::export_key::ExportKeyResult::Pkcs8(b)
    | crate::export_key::ExportKeyResult::Raw(b) => Ok(b.as_ref().to_vec()),
    _ => Err(op_error(err_msg.into())),
  }
}

fn parse_named_curve(name: &str) -> Option<EcNamedCurve> {
  match name {
    "P-256" => Some(EcNamedCurve::P256),
    "P-384" => Some(EcNamedCurve::P384),
    "P-521" => Some(EcNamedCurve::P521),
    _ => None,
  }
}

fn sha_hash_name(h: crate::shared::ShaHash) -> &'static str {
  use crate::shared::ShaHash::*;
  match h {
    Sha1 => "SHA-1",
    Sha256 => "SHA-256",
    Sha384 => "SHA-384",
    Sha512 => "SHA-512",
    Sha3_256 => "SHA3-256",
    Sha3_384 => "SHA3-384",
    Sha3_512 => "SHA3-512",
  }
}

fn not_supported(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionNotSupportedError", msg))
}

fn invalid_access(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionInvalidAccessError", msg))
}

fn op_error(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionOperationError", msg))
}
