// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.deriveBits()` body in Rust.
//!
//! Per-algorithm `AlgorithmIdentifier` parameter parsing (PBKDF2 /
//! HKDF / ECDH / X25519 / X448), the spec-mandated key-type / curve /
//! length validation, and dispatch into the crate-internal helpers
//! ([`crate::derive_bits_sync`], [`crate::x25519::x25519_derive_bits`],
//! [`crate::x448::x448_derive_bits`]).

use std::borrow::Cow;

use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlErrorKind;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::KeyData;
use crate::crypto_key::CryptoKey;
use crate::crypto_key::CryptoKeyType;
use crate::derive_bits_sync;
use crate::key::Algorithm;
use crate::key::CryptoHash;
use crate::key::CryptoNamedCurve;
use crate::shared::ShaHash;
use crate::subtle_encrypt::extract_name_and_obj;
use crate::subtle_encrypt::v8_str;
use crate::subtle_key::SubtleKey;
use crate::subtle_sign::read_optional_buffer_source;
use crate::subtle_sign::read_required_hash;
use crate::subtle_sign::read_required_u32;
use crate::x448::x448_derive_bits;
use crate::x25519::x25519_derive_bits;

pub enum SubtleDeriveBitsParams {
  Pbkdf2 {
    hash: ShaHash,
    salt: Vec<u8>,
    iterations: u32,
  },
  Hkdf {
    hash: ShaHash,
    salt: Vec<u8>,
    info: Vec<u8>,
  },
  Ecdh {
    public: Box<SubtleKey>,
  },
  X25519 {
    public: Box<SubtleKey>,
  },
  X448 {
    public: Box<SubtleKey>,
  },
  Unknown(String),
}

impl SubtleDeriveBitsParams {
  pub fn canonical_name(&self) -> &str {
    match self {
      Self::Pbkdf2 { .. } => "PBKDF2",
      Self::Hkdf { .. } => "HKDF",
      Self::Ecdh { .. } => "ECDH",
      Self::X25519 { .. } => "X25519",
      Self::X448 { .. } => "X448",
      Self::Unknown(n) => n,
    }
  }
}

impl<'a> WebIdlConverter<'a> for SubtleDeriveBitsParams {
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
    let Some(canonical) = canonical_derive_name(&name_str) else {
      return Ok(Self::Unknown(name_str));
    };
    let obj =
      maybe_obj.ok_or_else(|| missing_dict(prefix.clone(), &context))?;

    match canonical {
      "PBKDF2" => {
        let hash =
          read_required_hash(scope, obj, "hash", prefix.clone(), &context)?;
        let salt = read_required_buffer_source(
          scope,
          obj,
          "salt",
          prefix.clone(),
          &context,
        )?;
        let iterations = read_required_u32(
          scope,
          obj,
          "iterations",
          prefix.clone(),
          &context,
        )?;
        Ok(Self::Pbkdf2 {
          hash,
          salt,
          iterations,
        })
      }
      "HKDF" => {
        let hash =
          read_required_hash(scope, obj, "hash", prefix.clone(), &context)?;
        let salt = read_required_buffer_source(
          scope,
          obj,
          "salt",
          prefix.clone(),
          &context,
        )?;
        let info = read_required_buffer_source(
          scope,
          obj,
          "info",
          prefix.clone(),
          &context,
        )?;
        Ok(Self::Hkdf { hash, salt, info })
      }
      "ECDH" => {
        let public = read_required_public_key(
          scope,
          obj,
          "public",
          prefix.clone(),
          &context,
        )?;
        Ok(Self::Ecdh {
          public: Box::new(public),
        })
      }
      "X25519" => {
        let public = read_required_public_key(
          scope,
          obj,
          "public",
          prefix.clone(),
          &context,
        )?;
        Ok(Self::X25519 {
          public: Box::new(public),
        })
      }
      "X448" => {
        let public = read_required_public_key(
          scope,
          obj,
          "public",
          prefix.clone(),
          &context,
        )?;
        Ok(Self::X448 {
          public: Box::new(public),
        })
      }
      _ => unreachable!(),
    }
  }
}

fn canonical_derive_name(name: &str) -> Option<&'static str> {
  const NAMES: &[&str] = &["PBKDF2", "HKDF", "ECDH", "X25519", "X448"];
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

fn read_required_buffer_source<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  field: &'static str,
  prefix: Cow<'static, str>,
  context: &ContextFn<'b>,
) -> Result<Vec<u8>, WebIdlError> {
  read_optional_buffer_source(scope, obj, field).ok_or_else(|| {
    WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error(format!("required dictionary member '{field}'")),
    )
  })
}

fn read_required_public_key<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  field: &'static str,
  prefix: Cow<'static, str>,
  context: &ContextFn<'b>,
) -> Result<SubtleKey, WebIdlError> {
  let key = v8_str(scope, field);
  let val = obj
    .get(scope, key.into())
    .unwrap_or_else(|| v8::undefined(scope).into());
  if val.is_undefined() || val.is_null() {
    return Err(WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error(format!("required dictionary member '{field}'")),
    ));
  }
  let Some(key_ptr) =
    deno_core::cppgc::try_unwrap_cppgc_object::<CryptoKey>(scope, val)
  else {
    return Err(WebIdlError::new(
      prefix,
      context.borrowed(),
      WebIdlErrorKind::ConvertToConverterType("CryptoKey"),
    ));
  };
  let cryptokey: &CryptoKey = &key_ptr;

  let algorithm_name = cryptokey.algorithm_name(scope).ok_or_else(|| {
    WebIdlError::other(
      prefix.clone(),
      context.borrowed(),
      JsErrorBox::type_error("CryptoKey.algorithm.name is not a string"),
    )
  })?;

  let usages = cryptokey.usages_as_vec(scope).unwrap_or_default();
  let Some(handle_ptr) = cryptokey.key_handle(scope) else {
    return Err(WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error("CryptoKey handle has been tampered with"),
    ));
  };
  let raw = handle_ptr.data().clone();
  let algorithm_named_curve = read_algorithm_named_curve(scope, cryptokey);

  Ok(SubtleKey {
    algorithm_name,
    algorithm_length: None,
    algorithm_hash: None,
    algorithm_named_curve,
    usages,
    key_type: cryptokey.key_type(),
    extractable: cryptokey.extractable_(),
    raw,
  })
}

fn read_algorithm_named_curve<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: &CryptoKey,
) -> Option<String> {
  let alg = key.algorithm_local(scope)?;
  let key_v8 = v8::String::new_from_one_byte(
    scope,
    b"namedCurve",
    v8::NewStringType::Internalized,
  )?;
  let val = alg.get(scope, key_v8.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  let s = val.to_string(scope)?;
  Some(s.to_rust_string_lossy(scope))
}

const SUPPORTED_NAMED_CURVES: &[&str] = &["P-256", "P-384", "P-521"];

pub fn run(
  params: SubtleDeriveBitsParams,
  key: SubtleKey,
  length: Option<u32>,
) -> Result<Vec<u8>, CryptoError> {
  // Steps 4-7 of the WebCrypto spec are interleaved with algorithm-
  // specific validation below. Spec steps 7/8 (algorithm name + usage
  // check on baseKey) are reproduced first since they apply across all
  // algorithms.
  if params.canonical_name() != key.algorithm_name {
    return Err(invalid_access("Invalid algorithm name".to_string()));
  }
  if !key.has_usage("deriveBits") {
    return Err(invalid_access(
      "'baseKey' usages does not contain 'deriveBits'".to_string(),
    ));
  }

  match params {
    SubtleDeriveBitsParams::Pbkdf2 {
      hash,
      salt,
      iterations,
    } => {
      let length =
        length.ok_or_else(|| op_error("Invalid length".to_string()))?;
      if length == 0 || length % 8 != 0 {
        return Err(op_error("Invalid length".to_string()));
      }
      if iterations == 0 {
        return Err(op_error("iterations must not be zero".to_string()));
      }
      let key_data: KeyData = (&key.raw).into();
      derive_bits_sync(
        key_data,
        None,
        Algorithm::Pbkdf2,
        Some(sha_to_crypto_hash(hash)),
        length as usize,
        Some(iterations),
        None,
        None,
        Some(salt),
      )
    }
    SubtleDeriveBitsParams::Hkdf { hash, salt, info } => {
      let length =
        length.ok_or_else(|| op_error("Invalid length".to_string()))?;
      if length == 0 || length % 8 != 0 {
        return Err(op_error("Invalid length".to_string()));
      }
      let key_data: KeyData = (&key.raw).into();
      derive_bits_sync(
        key_data,
        None,
        Algorithm::Hkdf,
        Some(sha_to_crypto_hash(hash)),
        length as usize,
        None,
        None,
        Some(info),
        Some(salt),
      )
    }
    SubtleDeriveBitsParams::Ecdh { public } => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access("Invalid key type".to_string()));
      }
      if public.key_type != CryptoKeyType::Public {
        return Err(invalid_access("Invalid key type".to_string()));
      }
      if public.algorithm_name != key.algorithm_name {
        return Err(invalid_access("Algorithm mismatch".to_string()));
      }
      if public.algorithm_named_curve != key.algorithm_named_curve {
        return Err(invalid_access("'namedCurve' mismatch".to_string()));
      }
      let curve_name =
        key.algorithm_named_curve.as_deref().ok_or_else(|| {
          op_error("ECDH key is missing 'namedCurve'".to_string())
        })?;
      if !SUPPORTED_NAMED_CURVES.contains(&curve_name) {
        return Err(not_supported("Not implemented".to_string()));
      }
      let named_curve = parse_named_curve(curve_name)
        .ok_or_else(|| not_supported("Not implemented".to_string()))?;
      let key_data: KeyData = (&key.raw).into();
      let public_data: KeyData = (&public.raw).into();
      let bits = derive_bits_sync(
        key_data,
        Some(public_data),
        Algorithm::Ecdh,
        None,
        length.unwrap_or(0) as usize,
        None,
        Some(named_curve),
        None,
        None,
      )?;
      truncate_to_length(bits, length)
    }
    SubtleDeriveBitsParams::X25519 { public } => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access("Invalid key type".to_string()));
      }
      if public.key_type != CryptoKeyType::Public {
        return Err(invalid_access("Invalid key type".to_string()));
      }
      if public.algorithm_name != key.algorithm_name {
        return Err(invalid_access("Algorithm mismatch".to_string()));
      }
      let mut secret = [0u8; 32];
      let is_identity =
        x25519_derive_bits(key.raw.bytes(), public.raw.bytes(), &mut secret)
          .map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))?;
      if is_identity {
        return Err(op_error("Invalid key".to_string()));
      }
      truncate_to_length(secret.to_vec(), length)
    }
    SubtleDeriveBitsParams::X448 { public } => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access("Invalid key type".to_string()));
      }
      if public.key_type != CryptoKeyType::Public {
        return Err(invalid_access("Invalid key type".to_string()));
      }
      if public.algorithm_name != key.algorithm_name {
        return Err(invalid_access("Algorithm mismatch".to_string()));
      }
      let mut secret = [0u8; 56];
      let is_identity =
        x448_derive_bits(key.raw.bytes(), public.raw.bytes(), &mut secret)
          .map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))?;
      if is_identity {
        return Err(op_error("Invalid key".to_string()));
      }
      truncate_to_length(secret.to_vec(), length)
    }
    SubtleDeriveBitsParams::Unknown(name) => Err(not_supported(format!(
      "Algorithm '{name}' is not supported"
    ))),
  }
}

fn truncate_to_length(
  bytes: Vec<u8>,
  length: Option<u32>,
) -> Result<Vec<u8>, CryptoError> {
  let Some(length) = length else {
    return Ok(bytes);
  };
  if (bytes.len() * 8) < length as usize {
    return Err(op_error("Invalid length".to_string()));
  }
  let n = length.div_ceil(8) as usize;
  Ok(bytes[..n].to_vec())
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
