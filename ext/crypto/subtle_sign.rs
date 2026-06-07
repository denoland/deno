// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.sign()` body in Rust.
//!
//! `WebIdlConverter` for the per-operation `AlgorithmIdentifier`
//! (`RSASSA-PKCS1-v1_5`, `RSA-PSS`, `ECDSA`, `HMAC`, `Ed25519`,
//! `ML-DSA-44` / `ML-DSA-65` / `ML-DSA-87`), the per-algorithm spec
//! validation (key type, named curve, etc.), and dispatch into the
//! existing per-algorithm sign helpers exported from [`crate::lib`],
//! [`crate::ed25519`], and [`crate::mldsa`].

use std::borrow::Cow;

use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::KeyData;
use crate::SignArg;
use crate::crypto_key::CryptoKeyType;
use crate::ed25519::ed25519_sign_into;
use crate::key::Algorithm;
use crate::key::CryptoHash;
use crate::key::CryptoNamedCurve;
use crate::mldsa::mldsa_sign;
use crate::shared::ShaHash;
use crate::sign_key_sync;
use crate::subtle_encrypt::extract_name_and_obj;
use crate::subtle_encrypt::v8_str;
use crate::subtle_key::SubtleKey;

/// Normalized per-algorithm sign parameters. Each variant carries
/// exactly the dictionary members the matching `sign_*` helper needs.
/// `Unknown` is produced when the input has a string-coercible `.name`
/// that the sign registry doesn't know about; the impl method turns
/// it into a `NotSupportedError` `DOMException`.
pub enum SubtleSignParams {
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

impl SubtleSignParams {
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

impl<'a> WebIdlConverter<'a> for SubtleSignParams {
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
    let Some(canonical) = canonical_sign_name(&name_str) else {
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

fn canonical_sign_name(name: &str) -> Option<&'static str> {
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

pub(crate) fn read_required_u32<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  field: &'static str,
  prefix: Cow<'static, str>,
  context: &ContextFn<'b>,
) -> Result<u32, WebIdlError> {
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
  val.uint32_value(scope).ok_or_else(|| {
    WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error(format!("'{field}' must be convertible to u32")),
    )
  })
}

pub(crate) fn read_required_hash<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  field: &'static str,
  prefix: Cow<'static, str>,
  context: &ContextFn<'b>,
) -> Result<ShaHash, WebIdlError> {
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
  let name_str = if val.is_string() {
    val.to_rust_string_lossy(scope)
  } else if let Ok(obj) = v8::Local::<v8::Object>::try_from(val) {
    let name_key = v8_str(scope, "name");
    let name_val = obj
      .get(scope, name_key.into())
      .unwrap_or_else(|| v8::undefined(scope).into());
    let s = name_val.to_string(scope).ok_or_else(|| {
      WebIdlError::other(
        prefix.clone(),
        context.borrowed(),
        JsErrorBox::type_error(format!("'{field}.name' is not a DOMString")),
      )
    })?;
    s.to_rust_string_lossy(scope)
  } else {
    return Err(WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error(format!(
        "'{field}' must be a HashAlgorithmIdentifier"
      )),
    ));
  };
  match name_str.as_str() {
    "SHA-1" => Ok(ShaHash::Sha1),
    "SHA-256" => Ok(ShaHash::Sha256),
    "SHA-384" => Ok(ShaHash::Sha384),
    "SHA-512" => Ok(ShaHash::Sha512),
    "SHA3-256" => Ok(ShaHash::Sha3_256),
    "SHA3-384" => Ok(ShaHash::Sha3_384),
    "SHA3-512" => Ok(ShaHash::Sha3_512),
    _ => Err(WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::not_supported(),
    )),
  }
}

pub(crate) fn read_optional_buffer_source<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  field: &str,
) -> Option<Vec<u8>> {
  let key = v8_str(scope, field);
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(val) {
    let byte_length = view.byte_length();
    if byte_length == 0 {
      return Some(Vec::new());
    }
    let byte_offset = view.byte_offset();
    let ab = view.buffer(scope)?;
    // SAFETY: V8 guarantees byte_offset + byte_length stay within the
    // backing store and a non-detached buffer has a non-null data pointer.
    unsafe {
      let base = ab.data()?.as_ptr() as *const u8;
      return Some(
        std::slice::from_raw_parts(base.add(byte_offset), byte_length).to_vec(),
      );
    }
  }
  if let Ok(ab) = v8::Local::<v8::ArrayBuffer>::try_from(val) {
    let byte_length = ab.byte_length();
    if byte_length == 0 {
      return Some(Vec::new());
    }
    // SAFETY: as above.
    unsafe {
      let base = ab.data()?.as_ptr() as *const u8;
      return Some(std::slice::from_raw_parts(base, byte_length).to_vec());
    }
  }
  None
}

const SUPPORTED_NAMED_CURVES: &[&str] = &["P-256", "P-384", "P-521"];

/// Validate the per-algorithm prerequisites (key type, named curve,
/// usages) and dispatch to the existing crate-internal sign helpers.
pub fn run(
  params: SubtleSignParams,
  key: SubtleKey,
  data: Vec<u8>,
) -> Result<Vec<u8>, CryptoError> {
  if params.canonical_name() != key.algorithm_name {
    return Err(invalid_access(format!(
      "Signing algorithm '{}' does not match key algorithm",
      params.canonical_name()
    )));
  }
  if !key.has_usage("sign") {
    return Err(invalid_access(
      "The requested operation is not valid for the provided key".to_string(),
    ));
  }

  match params {
    SubtleSignParams::RsassaPkcs1v15 => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access("Key type not supported".to_string()));
      }
      let hash = key.algorithm_hash.ok_or_else(|| {
        op_error("RSASSA-PKCS1-v1_5 key is missing 'hash'".to_string())
      })?;
      let key_data: KeyData = (&key.raw).into();
      let args = SignArg::new(
        Algorithm::RsassaPkcs1v15,
        None,
        Some(sha_to_crypto_hash(hash)),
        None,
      );
      sign_key_sync(key_data, args, &data)
    }
    SubtleSignParams::RsaPss { salt_length } => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access("Key type not supported".to_string()));
      }
      let hash = key
        .algorithm_hash
        .ok_or_else(|| op_error("RSA-PSS key is missing 'hash'".to_string()))?;
      let key_data: KeyData = (&key.raw).into();
      let args = SignArg::new(
        Algorithm::RsaPss,
        Some(salt_length),
        Some(sha_to_crypto_hash(hash)),
        None,
      );
      sign_key_sync(key_data, args, &data)
    }
    SubtleSignParams::Ecdsa { hash } => {
      if key.key_type != CryptoKeyType::Private {
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
      let args = SignArg::new(
        Algorithm::Ecdsa,
        None,
        Some(sha_to_crypto_hash(hash)),
        Some(named_curve),
      );
      sign_key_sync(key_data, args, &data)
    }
    SubtleSignParams::Hmac => {
      let hash = key
        .algorithm_hash
        .ok_or_else(|| op_error("HMAC key is missing 'hash'".to_string()))?;
      let key_data: KeyData = (&key.raw).into();
      let args = SignArg::new(
        Algorithm::Hmac,
        None,
        Some(sha_to_crypto_hash(hash)),
        None,
      );
      sign_key_sync(key_data, args, &data)
    }
    SubtleSignParams::Ed25519 => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access("Key type not supported".to_string()));
      }
      const SIGNATURE_LEN: usize = 32 * 2;
      let mut signature = vec![0u8; SIGNATURE_LEN];
      if !ed25519_sign_into(key.raw.bytes(), &data, &mut signature) {
        return Err(op_error("Failed to sign".to_string()));
      }
      Ok(signature)
    }
    SubtleSignParams::MlDsa { variant, context } => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access("Key type not supported".to_string()));
      }
      mldsa_sign(
        variant,
        key.raw.expanded_private_key(),
        &data,
        context.as_deref(),
      )
      .map_err(|e| CryptoError::Other(JsErrorBox::from_err(e)))
    }
    SubtleSignParams::Unknown(name) => Err(not_supported(format!(
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
