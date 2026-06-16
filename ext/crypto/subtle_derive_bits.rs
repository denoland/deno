// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.deriveBits()` body in Rust.
//!
//! Per-algorithm `AlgorithmIdentifier` parameter parsing (PBKDF2 /
//! HKDF / ECDH / X25519 / X448), the spec-mandated key-type / curve /
//! length validation, and dispatch into the crate-internal helpers
//! ([`crate::derive_bits_sync`], [`crate::x25519::x25519_derive_bits`],
//! [`crate::x448::x448_derive_bits`]).

use std::borrow::Cow;

use argon2::Argon2;
use argon2::ParamsBuilder;
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
use crate::subtle_sign::read_required_u32;
use crate::x448::x448_derive_bits;
use crate::x25519::x25519_derive_bits;

pub enum SubtleDeriveBitsParams {
  Pbkdf2 {
    hash_name: String,
    salt: Vec<u8>,
    iterations: u32,
  },
  Hkdf {
    hash_name: String,
    salt: Vec<u8>,
    info: Vec<u8>,
  },
  Argon2 {
    name: &'static str,
    memory: u32,
    passes: u32,
    parallelism: u32,
    nonce: Vec<u8>,
    secret_value: Option<Vec<u8>>,
    associated_data: Option<Vec<u8>>,
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
      Self::Argon2 { name, .. } => name,
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
        let hash_name = read_required_hash_name(
          scope,
          obj,
          "hash",
          prefix.clone(),
          &context,
        )?;
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
          hash_name,
          salt,
          iterations,
        })
      }
      "HKDF" => {
        let hash_name = read_required_hash_name(
          scope,
          obj,
          "hash",
          prefix.clone(),
          &context,
        )?;
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
        Ok(Self::Hkdf {
          hash_name,
          salt,
          info,
        })
      }
      "Argon2i" | "Argon2d" | "Argon2id" => {
        let memory =
          read_required_u32(scope, obj, "memory", prefix.clone(), &context)?;
        let passes =
          read_required_u32(scope, obj, "passes", prefix.clone(), &context)?;
        let parallelism = read_required_u32(
          scope,
          obj,
          "parallelism",
          prefix.clone(),
          &context,
        )?;
        let nonce = read_required_buffer_source(
          scope,
          obj,
          "nonce",
          prefix.clone(),
          &context,
        )?;
        let secret_value = read_optional_buffer_source(
          scope,
          obj,
          "secretValue",
          prefix.clone(),
          &context,
        )?;
        let associated_data = read_optional_buffer_source(
          scope,
          obj,
          "associatedData",
          prefix.clone(),
          &context,
        )?;
        Ok(Self::Argon2 {
          name: canonical,
          memory,
          passes,
          parallelism,
          nonce,
          secret_value,
          associated_data,
        })
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
  const NAMES: &[&str] = &[
    "PBKDF2", "HKDF", "Argon2i", "Argon2d", "Argon2id", "ECDH", "X25519",
    "X448",
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

/// Read the `hash` field as an `AlgorithmIdentifier` (string or `{name}` dict)
/// and return its raw name. Unlike [`crate::subtle_sign::read_required_hash`],
/// no digest-registry lookup is performed here: that lookup is deferred to
/// [`run`] so an unrecognized name maps to a spec-mandated `NotSupportedError`
/// `DOMException` rather than the `TypeError` op2 produces when a
/// `WebIdlConverter` returns `Err`.
fn read_required_hash_name<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  field: &'static str,
  prefix: Cow<'static, str>,
  context: &ContextFn<'b>,
) -> Result<String, WebIdlError> {
  let key = v8_str(scope, field);
  let val = obj
    .get(scope, key.into())
    .unwrap_or_else(|| v8::undefined(scope).into());
  if val.is_undefined() {
    return Err(WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error(format!("required dictionary member '{field}'")),
    ));
  }
  if val.is_string() {
    return Ok(val.to_rust_string_lossy(scope));
  }
  if let Ok(o) = v8::Local::<v8::Object>::try_from(val) {
    let name_key = v8_str(scope, "name");
    let name_val = o
      .get(scope, name_key.into())
      .unwrap_or_else(|| v8::undefined(scope).into());
    let s = name_val.to_string(scope).ok_or_else(|| {
      WebIdlError::other(
        prefix.clone(),
        context.borrowed(),
        JsErrorBox::type_error(format!("'{field}.name' is not a DOMString")),
      )
    })?;
    return Ok(s.to_rust_string_lossy(scope));
  }
  Err(WebIdlError::other(
    prefix,
    context.borrowed(),
    JsErrorBox::type_error(format!(
      "'{field}' must be a HashAlgorithmIdentifier"
    )),
  ))
}

fn read_required_buffer_source<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  field: &'static str,
  prefix: Cow<'static, str>,
  context: &ContextFn<'b>,
) -> Result<Vec<u8>, WebIdlError> {
  match read_optional_buffer_source(scope, obj, field, prefix.clone(), context)?
  {
    Some(v) => Ok(v),
    None => Err(WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error(format!("required dictionary member '{field}'")),
    )),
  }
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

  let usages = cryptokey.usages_as_vec(scope).ok_or_else(|| {
    WebIdlError::other(
      prefix.clone(),
      context.borrowed(),
      JsErrorBox::type_error("CryptoKey.usages has been tampered with"),
    )
  })?;
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
  length: Option<f64>,
) -> Result<Vec<u8>, CryptoError> {
  // Spec step 1: `normalizeAlgorithm` rejects unknown algorithm names with
  // `NotSupportedError` BEFORE the algorithm/key match. Match the spec order
  // so a stray Kelvin-sign `"H<U+212A>DF"` (or any other unregistered name)
  // produces `NotSupportedError`, not `InvalidAccessError`.
  if let SubtleDeriveBitsParams::Unknown(name) = &params {
    return Err(not_supported(format!(
      "Algorithm '{name}' is not supported"
    )));
  }
  // Spec step 7 (algorithm name match on baseKey). The `deriveBits`
  // usage check is the caller's responsibility -- the cppgc method
  // body enforces it for the external `SubtleCrypto.deriveBits` path,
  // while the internal `__deriveBitsInternal` path used by
  // `deriveKey` skips it (since `deriveKey` requires `deriveKey`
  // usage on the base key, not `deriveBits`).
  if params.canonical_name() != key.algorithm_name {
    return Err(invalid_access("Invalid algorithm name".to_string()));
  }

  let length_u32 = length.map(|l| l as u32);
  match params {
    SubtleDeriveBitsParams::Pbkdf2 {
      hash_name,
      salt,
      iterations,
    } => {
      let hash = parse_sha_hash(&hash_name)
        .ok_or_else(|| not_supported("Unrecognized algorithm name".into()))?;
      let length =
        length_u32.ok_or_else(|| op_error("Invalid length".to_string()))?;
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
    SubtleDeriveBitsParams::Hkdf {
      hash_name,
      salt,
      info,
    } => {
      let hash = parse_sha_hash(&hash_name)
        .ok_or_else(|| not_supported("Unrecognized algorithm name".into()))?;
      let length =
        length_u32.ok_or_else(|| op_error("Invalid length".to_string()))?;
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
    SubtleDeriveBitsParams::Argon2 {
      name,
      memory,
      passes,
      parallelism,
      nonce,
      secret_value,
      associated_data,
    } => {
      let length =
        length_u32.ok_or_else(|| op_error("Invalid length".to_string()))?;
      if length == 0 || length % 8 != 0 {
        return Err(op_error("Invalid length".to_string()));
      }
      let mut builder = ParamsBuilder::new();
      builder
        .m_cost(memory)
        .t_cost(passes)
        .p_cost(parallelism)
        .output_len((length / 8) as usize);
      if let Some(associated_data) = associated_data {
        let data = argon2::AssociatedData::new(&associated_data)
          .map_err(|_| op_error("Invalid associatedData".to_string()))?;
        builder.data(data);
      }
      let params = builder
        .build()
        .map_err(|_| op_error("Invalid Argon2 parameters".to_string()))?;
      let algorithm = match name {
        "Argon2i" => argon2::Algorithm::Argon2i,
        "Argon2d" => argon2::Algorithm::Argon2d,
        "Argon2id" => argon2::Algorithm::Argon2id,
        _ => unreachable!(),
      };
      let argon2 = match secret_value.as_deref() {
        Some(secret_value) => Argon2::new_with_secret(
          secret_value,
          algorithm,
          argon2::Version::V0x13,
          params,
        )
        .map_err(|_| op_error("Invalid secretValue".to_string()))?,
        None => Argon2::new(algorithm, argon2::Version::V0x13, params),
      };
      let mut out = vec![0; (length / 8) as usize];
      argon2
        .hash_password_into(key.raw.bytes(), &nonce, &mut out)
        .map_err(|_| op_error("Argon2 derivation failed".to_string()))?;
      Ok(out)
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
        length_u32.unwrap_or(0) as usize,
        None,
        Some(named_curve),
        None,
        None,
      )?;
      truncate_to_length(bits, length_u32)
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
      truncate_to_length(secret.to_vec(), length_u32)
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
      truncate_to_length(secret.to_vec(), length_u32)
    }
    // `Unknown` is rejected with `NotSupportedError` above before we even
    // check the key-algorithm match (matches the spec normalize step).
    SubtleDeriveBitsParams::Unknown(_) => unreachable!(),
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

/// Map a `HashAlgorithmIdentifier.name` string onto the digest-registry
/// `ShaHash` enum. The lookup is case-sensitive and requires the exact
/// canonical spelling per the spec ("SHA-256", "SHA3-256", etc.), so a stray
/// "SHA384" or non-digest name like "PBKDF2" yields `None`.
fn parse_sha_hash(name: &str) -> Option<ShaHash> {
  match name {
    "SHA-1" => Some(ShaHash::Sha1),
    "SHA-256" => Some(ShaHash::Sha256),
    "SHA-384" => Some(ShaHash::Sha384),
    "SHA-512" => Some(ShaHash::Sha512),
    "SHA3-256" => Some(ShaHash::Sha3_256),
    "SHA3-384" => Some(ShaHash::Sha3_384),
    "SHA3-512" => Some(ShaHash::Sha3_512),
    _ => None,
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
