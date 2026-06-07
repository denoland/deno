// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.verify()` body in Rust — symmetric mirror of
//! [`crate::subtle_sign`]. The per-algorithm parameter dictionary
//! (`saltLength` for RSA-PSS, `hash` for ECDSA, optional `context` for
//! ML-DSA) is parsed by [`SubtleVerifyParams`], and the dispatch lands
//! in the existing per-algorithm verify helpers exported from
//! [`crate::lib`], [`crate::ed25519`], and [`crate::mldsa`].

use std::borrow::Cow;

use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::KeyData;
use crate::VerifyArg;
use crate::crypto_key::CryptoKeyType;
use crate::ed25519::ed25519_verify;
use crate::key::Algorithm;
use crate::key::CryptoHash;
use crate::key::CryptoNamedCurve;
use crate::mldsa::mldsa_verify;
use crate::shared::ShaHash;
use crate::subtle_encrypt::extract_name_and_obj;
use crate::subtle_key::SubtleKey;
use crate::subtle_sign::read_optional_buffer_source;
use crate::subtle_sign::read_required_hash;
use crate::subtle_sign::read_required_u32;
use crate::verify_key_sync;

pub enum SubtleVerifyParams {
  RsassaPkcs1v15,
  RsaPss {
    salt_length: u32,
  },
  Ecdsa {
    hash: ShaHash,
  },
  Hmac,
  Ed25519,
  MlDsa {
    variant: u8,
    context: Option<Vec<u8>>,
  },
  Unknown(String),
}

impl SubtleVerifyParams {
  pub fn canonical_name(&self) -> &str {
    match self {
      Self::RsassaPkcs1v15 => "RSASSA-PKCS1-v1_5",
      Self::RsaPss { .. } => "RSA-PSS",
      Self::Ecdsa { .. } => "ECDSA",
      Self::Hmac => "HMAC",
      Self::Ed25519 => "Ed25519",
      Self::MlDsa { variant, .. } => match variant {
        0 => "ML-DSA-44",
        1 => "ML-DSA-65",
        _ => "ML-DSA-87",
      },
      Self::Unknown(n) => n,
    }
  }
}

impl<'a> WebIdlConverter<'a> for SubtleVerifyParams {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    let (name_str, maybe_obj) =
      extract_name_and_obj(scope, value, prefix.clone(), context.borrowed())?;
    let Some(canonical) = canonical_verify_name(&name_str) else {
      return Ok(Self::Unknown(name_str));
    };
    match canonical {
      "RSASSA-PKCS1-v1_5" => Ok(Self::RsassaPkcs1v15),
      "RSA-PSS" => {
        let obj =
          maybe_obj.ok_or_else(|| missing_dict(prefix.clone(), &context))?;
        let salt_length = read_required_u32(
          scope,
          obj,
          "saltLength",
          prefix.clone(),
          &context,
        )?;
        Ok(Self::RsaPss { salt_length })
      }
      "ECDSA" => {
        let obj =
          maybe_obj.ok_or_else(|| missing_dict(prefix.clone(), &context))?;
        let hash =
          read_required_hash(scope, obj, "hash", prefix.clone(), &context)?;
        Ok(Self::Ecdsa { hash })
      }
      "HMAC" => Ok(Self::Hmac),
      "Ed25519" => Ok(Self::Ed25519),
      "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => {
        let variant = match canonical {
          "ML-DSA-44" => 0,
          "ML-DSA-65" => 1,
          _ => 2,
        };
        let context_bytes = maybe_obj
          .and_then(|o| read_optional_buffer_source(scope, o, "context"));
        Ok(Self::MlDsa {
          variant,
          context: context_bytes,
        })
      }
      _ => unreachable!(),
    }
  }
}

fn canonical_verify_name(name: &str) -> Option<&'static str> {
  const NAMES: &[&str] = &[
    "RSASSA-PKCS1-v1_5",
    "RSA-PSS",
    "ECDSA",
    "HMAC",
    "Ed25519",
    "ML-DSA-44",
    "ML-DSA-65",
    "ML-DSA-87",
  ];
  NAMES.iter().copied().find(|n| n.eq_ignore_ascii_case(name))
}

fn missing_dict(
  prefix: Cow<'static, str>,
  context: &ContextFn<'_>,
) -> WebIdlError {
  WebIdlError::other(
    prefix,
    context.borrowed(),
    JsErrorBox::type_error("Algorithm requires a parameter dictionary"),
  )
}

const SUPPORTED_NAMED_CURVES: &[&str] = &["P-256", "P-384", "P-521"];

pub fn run(
  params: SubtleVerifyParams,
  key: SubtleKey,
  signature: Vec<u8>,
  data: Vec<u8>,
) -> Result<bool, CryptoError> {
  if params.canonical_name() != key.algorithm_name {
    return Err(invalid_access(format!(
      "Verifying algorithm '{}' does not match key algorithm",
      params.canonical_name()
    )));
  }
  if !key.has_usage("verify") {
    return Err(invalid_access(
      "The requested operation is not valid for the provided key".to_string(),
    ));
  }

  match params {
    SubtleVerifyParams::RsassaPkcs1v15 => {
      if key.key_type != CryptoKeyType::Public {
        return Err(invalid_access("Key type not supported".to_string()));
      }
      let hash = key.algorithm_hash.ok_or_else(|| {
        op_error("RSASSA-PKCS1-v1_5 key is missing 'hash'".to_string())
      })?;
      let key_data: KeyData = (&key.raw).into();
      let args = VerifyArg::new(
        Algorithm::RsassaPkcs1v15,
        None,
        Some(sha_to_crypto_hash(hash)),
        signature,
        None,
      );
      verify_key_sync(key_data, args, &data)
    }
    SubtleVerifyParams::RsaPss { salt_length } => {
      if key.key_type != CryptoKeyType::Public {
        return Err(invalid_access("Key type not supported".to_string()));
      }
      let hash = key
        .algorithm_hash
        .ok_or_else(|| op_error("RSA-PSS key is missing 'hash'".to_string()))?;
      let key_data: KeyData = (&key.raw).into();
      let args = VerifyArg::new(
        Algorithm::RsaPss,
        Some(salt_length),
        Some(sha_to_crypto_hash(hash)),
        signature,
        None,
      );
      verify_key_sync(key_data, args, &data)
    }
    SubtleVerifyParams::Ecdsa { hash } => {
      if key.key_type != CryptoKeyType::Public {
        return Err(invalid_access("Key type not supported".to_string()));
      }
      let curve_name =
        key.algorithm_named_curve.as_deref().ok_or_else(|| {
          op_error("ECDSA key is missing 'namedCurve'".to_string())
        })?;
      if !SUPPORTED_NAMED_CURVES.iter().any(|c| *c == curve_name) {
        return Err(not_supported("Curve not supported".to_string()));
      }
      let named_curve = parse_named_curve(curve_name)
        .ok_or_else(|| not_supported("Curve not supported".to_string()))?;
      let key_data: KeyData = (&key.raw).into();
      let args = VerifyArg::new(
        Algorithm::Ecdsa,
        None,
        Some(sha_to_crypto_hash(hash)),
        signature,
        Some(named_curve),
      );
      verify_key_sync(key_data, args, &data)
    }
    SubtleVerifyParams::Hmac => {
      let hash = key
        .algorithm_hash
        .ok_or_else(|| op_error("HMAC key is missing 'hash'".to_string()))?;
      let key_data: KeyData = (&key.raw).into();
      let args = VerifyArg::new(
        Algorithm::Hmac,
        None,
        Some(sha_to_crypto_hash(hash)),
        signature,
        None,
      );
      verify_key_sync(key_data, args, &data)
    }
    SubtleVerifyParams::Ed25519 => {
      if key.key_type != CryptoKeyType::Public {
        return Err(invalid_access("Key type not supported".to_string()));
      }
      Ok(ed25519_verify(key.raw.bytes(), &data, &signature))
    }
    SubtleVerifyParams::MlDsa { variant, context } => {
      if key.key_type != CryptoKeyType::Public {
        return Err(invalid_access("Key type not supported".to_string()));
      }
      Ok(mldsa_verify(
        variant,
        key.raw.bytes(),
        &data,
        &signature,
        context.as_deref(),
      ))
    }
    SubtleVerifyParams::Unknown(name) => Err(not_supported(format!(
      "Algorithm '{name}' is not supported"
    ))),
  }
}

fn sha_to_crypto_hash(h: ShaHash) -> CryptoHash {
  match h {
    ShaHash::Sha1 => CryptoHash::Sha1,
    ShaHash::Sha256 => CryptoHash::Sha256,
    ShaHash::Sha384 => CryptoHash::Sha384,
    ShaHash::Sha512 => CryptoHash::Sha512,
    ShaHash::Sha3_256 => CryptoHash::Sha3_256,
    ShaHash::Sha3_384 => CryptoHash::Sha3_384,
    ShaHash::Sha3_512 => CryptoHash::Sha3_512,
  }
}

fn parse_named_curve(name: &str) -> Option<CryptoNamedCurve> {
  match name {
    "P-256" => Some(CryptoNamedCurve::P256),
    "P-384" => Some(CryptoNamedCurve::P384),
    "P-521" => Some(CryptoNamedCurve::P521),
    _ => None,
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
