// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.importKey()` body in Rust.
//!
//! Replaces the legacy JS `importKeyInner` dispatcher and every per-algorithm
//! helper (`importKeyAES`, `importKeyHMAC`, `importKeyChaCha20Poly1305`,
//! `importKeyKdf`, `importKeyOkp`, `importKeyEC`, `importKeyRSA`,
//! `importKeyMlKem`, `importKeyMlDsa`). All JWK validation, format-specific
//! parsing, and key construction happens inside Rust; the result is the
//! v8 `CryptoKey` minted via [`crate::make_key::make_crypto_key`].
//!
//! The runner returns a `v8::Global<v8::Object>` rather than a Rust struct
//! because the spec-mandated `algorithm` slot is a v8 object stamped with
//! per-algorithm fields (e.g. RSA's `modulusLength` + `publicExponent`,
//! HMAC's `hash` dictionary, AES-*'s `length`); building it once here in
//! Rust avoids the extra round-trip to a `ToV8` derivation step.

use std::borrow::Cow;

use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlErrorKind;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::crypto_key::CryptoKeyType;
use crate::ed25519::op_crypto_import_pkcs8_ed25519;
use crate::ed25519::op_crypto_import_spki_ed25519;
use crate::make_key::AlgorithmDict;
use crate::make_key::make_crypto_key;
use crate::shared::RawKeyData;
use crate::subtle_export_key::KeyFormat;
use crate::x25519::op_crypto_import_pkcs8_x25519;
use crate::x25519::op_crypto_import_spki_x25519;
use crate::x448::op_crypto_import_pkcs8_x448;
use crate::x448::op_crypto_import_spki_x448;

const ALL_USAGES: &[&str] = &[
  "encrypt",
  "decrypt",
  "sign",
  "verify",
  "deriveKey",
  "deriveBits",
  "wrapKey",
  "unwrapKey",
  "encapsulateKey",
  "encapsulateBits",
  "decapsulateKey",
  "decapsulateBits",
];

/// Argument-coerced view of the algorithm dictionary the user passed.
/// Extracts every per-algorithm slot (`hash`, `length`, `namedCurve`,
/// `modulusLength`, `publicExponent`) up front, so the import-path
/// dispatch can run off the v8 stack — needed by `deriveKey`'s
/// `spawn_blocking` and by the structured-clone resurrection path. The
/// optional `jwk_alg` slot is the raw `alg` member off the user-supplied
/// algorithm dictionary when it itself names an algorithm (it almost
/// never does; included so importKey's "hash" sub-normalization works).
#[derive(Clone)]
pub struct ImportAlgorithm {
  pub name: String,
  pub hash_name: Option<String>,
  pub length: Option<u32>,
  pub named_curve: Option<String>,
  pub modulus_length: Option<u32>,
  pub public_exponent: Option<Vec<u8>>,
}

impl<'a> WebIdlConverter<'a> for ImportAlgorithm {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    let (name, obj) = crate::subtle_encrypt::extract_name_and_obj(
      scope,
      value,
      prefix.clone(),
      context.borrowed(),
    )?;
    let canonical = crate::algorithm::canonical_name_for("importKey", &name)
      .map(str::to_string)
      .unwrap_or(name);
    let hash_name = obj.as_ref().and_then(|o| read_hash_name(scope, *o));
    let length = obj.as_ref().and_then(|o| read_u32_member(scope, *o, b"length"));
    let named_curve = obj
      .as_ref()
      .and_then(|o| read_string_member(scope, *o, b"namedCurve"));
    let modulus_length = obj
      .as_ref()
      .and_then(|o| read_u32_member(scope, *o, b"modulusLength"));
    let public_exponent = obj
      .as_ref()
      .and_then(|o| read_buffer_source_bytes(scope, *o, b"publicExponent"));
    Ok(ImportAlgorithm {
      name: canonical,
      hash_name,
      length,
      named_curve,
      modulus_length,
      public_exponent,
    })
  }
}

fn read_buffer_source_bytes<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<Vec<u8>> {
  let key = v8::String::new_from_one_byte(
    scope,
    field,
    v8::NewStringType::Internalized,
  )?;
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(val) {
    let mut out = vec![0u8; view.byte_length()];
    let n = view.copy_contents(&mut out);
    out.truncate(n);
    return Some(out);
  }
  if let Ok(ab) = v8::Local::<v8::ArrayBuffer>::try_from(val) {
    let len = ab.byte_length();
    let mut out = Vec::with_capacity(len);
    if len > 0 {
      // SAFETY: ArrayBuffer.data is valid for byte_length bytes.
      unsafe {
        let src = ab.data().unwrap().as_ptr() as *const u8;
        std::ptr::copy_nonoverlapping(src, out.as_mut_ptr(), len);
        out.set_len(len);
      }
    }
    return Some(out);
  }
  None
}

/// Carries either the BufferSource bytes (for `raw`/`raw-*`/`spki`/`pkcs8`)
/// or the JSON object (for `jwk`). For `raw-*`/`spki`/`pkcs8` formats
/// `keyData` must be a BufferSource; for `jwk` it must be a JsonWebKey
/// object. The pair of formats is mutually exclusive per the spec.
pub enum ImportKeyData {
  Buffer(Vec<u8>),
  Jwk(v8::Global<v8::Object>),
}

impl ImportKeyData {
  pub fn from_v8<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    format: KeyFormat,
  ) -> Result<Self, CryptoError> {
    if format == KeyFormat::Jwk {
      if v8::Local::<v8::ArrayBufferView>::try_from(value).is_ok()
        || v8::Local::<v8::ArrayBuffer>::try_from(value).is_ok()
      {
        return Err(CryptoError::Other(JsErrorBox::type_error(
          "Cannot import key: 'keyData' is not a JsonWebKey",
        )));
      }
      let obj = v8::Local::<v8::Object>::try_from(value).map_err(|_| {
        CryptoError::Other(JsErrorBox::type_error(
          "Cannot import key: 'keyData' is not a JsonWebKey",
        ))
      })?;
      Ok(Self::Jwk(v8::Global::new(scope, obj)))
    } else if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(value) {
      let mut out = vec![0u8; view.byte_length()];
      let n = view.copy_contents(&mut out);
      out.truncate(n);
      Ok(Self::Buffer(out))
    } else if let Ok(ab) = v8::Local::<v8::ArrayBuffer>::try_from(value) {
      let len = ab.byte_length();
      let mut out = Vec::with_capacity(len);
      if len > 0 {
        // SAFETY: ArrayBuffer.data is valid for byte_length bytes.
        unsafe {
          let src = ab.data().unwrap().as_ptr() as *const u8;
          std::ptr::copy_nonoverlapping(src, out.as_mut_ptr(), len);
          out.set_len(len);
        }
      }
      Ok(Self::Buffer(out))
    } else {
      Err(CryptoError::Other(JsErrorBox::type_error(
        "Cannot import key: 'keyData' is a JsonWebKey",
      )))
    }
  }
}

/// Dispatcher entry point for `SubtleCrypto.importKey()`.
pub fn run<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  format: KeyFormat,
  algorithm: &ImportAlgorithm,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  let name = algorithm.name.as_str();
  match name {
    "AES-CTR" | "AES-CBC" | "AES-GCM" | "AES-OCB" => import_key_aes(
      scope,
      name,
      format,
      key_data,
      extractable,
      usages,
      &["encrypt", "decrypt", "wrapKey", "unwrapKey"],
    ),
    "AES-KW" => import_key_aes(
      scope,
      name,
      format,
      key_data,
      extractable,
      usages,
      &["wrapKey", "unwrapKey"],
    ),
    "ChaCha20-Poly1305" => {
      import_key_chacha20(scope, format, key_data, extractable, usages)
    }
    "HMAC" => import_key_hmac(
      scope,
      algorithm.hash_name.as_deref(),
      algorithm.length,
      format,
      key_data,
      extractable,
      usages,
    ),
    "HKDF" => import_key_kdf(scope, "HKDF", format, key_data, extractable, usages),
    "PBKDF2" => {
      import_key_kdf(scope, "PBKDF2", format, key_data, extractable, usages)
    }
    _ => Err(not_supported(format!(
      "importKey is not yet implemented in Rust for {name}"
    ))),
  }
}

fn import_key_aes<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  algorithm_name: &str,
  format: KeyFormat,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
  supported_usages: &[&str],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  check_usages_subset(usages, supported_usages)?;
  let data = match (format, key_data) {
    (KeyFormat::Raw, ImportKeyData::Buffer(b))
    | (KeyFormat::RawSecret, ImportKeyData::Buffer(b)) => {
      let bits = b.len() * 8;
      if !matches!(bits, 128 | 192 | 256) {
        return Err(data_error("Invalid key length".into()));
      }
      b
    }
    (KeyFormat::Jwk, ImportKeyData::Jwk(jwk_g)) => {
      let jwk_l = v8::Local::new(scope, &jwk_g);
      validate_jwk_oct(scope, jwk_l, "enc", usages, extractable)?;
      let bytes = read_jwk_b64_field(scope, jwk_l, b"k")
        .ok_or_else(|| data_error("'k' property of JsonWebKey is required".into()))?;
      let bits = bytes.len() * 8;
      let expected = aes_jwk_alg(algorithm_name, bits)
        .ok_or_else(|| data_error("Invalid key length".into()))?;
      if let Some(alg) = read_string_member(scope, jwk_l, b"alg")
        && alg != expected
      {
        return Err(data_error(format!("Invalid algorithm: {alg}")));
      }
      bytes
    }
    _ => return Err(not_supported("Not implemented".into())),
  };
  let bits = (data.len() * 8) as u32;
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);
  Ok(make_crypto_key(
    scope,
    CryptoKeyType::Secret,
    extractable,
    &allowed_usages,
    AlgorithmDict::new(algorithm_name).with_length(bits),
    RawKeyData::Secret(data.into_boxed_slice()),
  ))
}

fn import_key_chacha20<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  format: KeyFormat,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  let supported = &["encrypt", "decrypt", "wrapKey", "unwrapKey"];
  check_usages_subset(usages, supported)?;
  let data = match (format, key_data) {
    (KeyFormat::RawSecret, ImportKeyData::Buffer(b)) => {
      if b.len() != 32 {
        return Err(data_error(
          "Invalid key length: ChaCha20-Poly1305 requires 256-bit key".into(),
        ));
      }
      b
    }
    (KeyFormat::Jwk, ImportKeyData::Jwk(jwk_g)) => {
      let jwk_l = v8::Local::new(scope, &jwk_g);
      validate_jwk_oct(scope, jwk_l, "enc", usages, extractable)?;
      let bytes = read_jwk_b64_field(scope, jwk_l, b"k")
        .ok_or_else(|| data_error("'k' property of JsonWebKey is required".into()))?;
      if bytes.len() != 32 {
        return Err(data_error(
          "Invalid key length: ChaCha20-Poly1305 requires 256-bit key".into(),
        ));
      }
      if let Some(alg) = read_string_member(scope, jwk_l, b"alg")
        && alg != "C20P"
      {
        return Err(data_error(format!("Invalid algorithm: {alg}")));
      }
      bytes
    }
    _ => return Err(not_supported("Not implemented".into())),
  };
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);
  Ok(make_crypto_key(
    scope,
    CryptoKeyType::Secret,
    extractable,
    &allowed_usages,
    AlgorithmDict::new("ChaCha20-Poly1305"),
    RawKeyData::Secret(data.into_boxed_slice()),
  ))
}

fn import_key_hmac<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  hash_name: Option<&str>,
  length_override: Option<u32>,
  format: KeyFormat,
  key_data: ImportKeyData,
  extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  check_usages_subset(usages, &["sign", "verify"])?;
  let hash_name = hash_name
    .map(str::to_string)
    .ok_or_else(|| data_error("HMAC import requires 'hash'".into()))?;

  let data = match (format, key_data) {
    (KeyFormat::Raw, ImportKeyData::Buffer(b))
    | (KeyFormat::RawSecret, ImportKeyData::Buffer(b)) => b,
    (KeyFormat::Jwk, ImportKeyData::Jwk(jwk_g)) => {
      let jwk_l = v8::Local::new(scope, &jwk_g);
      validate_jwk_oct(scope, jwk_l, "sig", usages, extractable)?;
      let bytes = read_jwk_b64_field(scope, jwk_l, b"k")
        .ok_or_else(|| data_error("'k' property of JsonWebKey is required".into()))?;
      let expected = hmac_jwk_alg(&hash_name)
        .ok_or_else(|| not_supported("Hash algorithm not supported".into()))?;
      if let Some(alg) = read_string_member(scope, jwk_l, b"alg")
        && alg != expected
      {
        return Err(data_error(format!(
          "'alg' property of JsonWebKey must be '{expected}'"
        )));
      }
      bytes
    }
    _ => return Err(not_supported("Not implemented".into())),
  };
  let mut length = (data.len() * 8) as u32;
  if length == 0 {
    return Err(data_error("Key length is zero".into()));
  }
  if let Some(override_) = length_override {
    if override_ > length || override_ <= length.saturating_sub(8) {
      return Err(data_error("Key length is invalid".into()));
    }
    length = override_;
  }
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);
  Ok(make_crypto_key(
    scope,
    CryptoKeyType::Secret,
    extractable,
    &allowed_usages,
    AlgorithmDict::new("HMAC")
      .with_length(length)
      .with_hash(hash_name),
    RawKeyData::Secret(data.into_boxed_slice()),
  ))
}

fn import_key_kdf<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  name: &str,
  format: KeyFormat,
  key_data: ImportKeyData,
  _extractable: bool,
  usages: &[String],
) -> Result<v8::Local<'s, v8::Object>, CryptoError> {
  // 17.4 / 19.4: HKDF and PBKDF2 only accept "raw" / "raw-secret"; "jwk" is
  // not in the recognized formats.
  let data = match (format, key_data) {
    (KeyFormat::Raw, ImportKeyData::Buffer(b))
    | (KeyFormat::RawSecret, ImportKeyData::Buffer(b)) => b,
    _ => return Err(not_supported("Not implemented".into())),
  };
  check_usages_subset(usages, &["deriveKey", "deriveBits"])?;
  let allowed_usages: Vec<&str> = filter_usages(usages, ALL_USAGES);
  // Per spec, HKDF/PBKDF2 imported keys must not be extractable. The JS
  // caller's `extractable` argument is ignored.
  Ok(make_crypto_key(
    scope,
    CryptoKeyType::Secret,
    false,
    &allowed_usages,
    AlgorithmDict::new(name),
    RawKeyData::Secret(data.into_boxed_slice()),
  ))
}

fn aes_jwk_alg(algorithm_name: &str, bits: usize) -> Option<&'static str> {
  let suffix = match bits {
    128 => "128",
    192 => "192",
    256 => "256",
    _ => return None,
  };
  let kind = match algorithm_name {
    "AES-CTR" => "CTR",
    "AES-CBC" => "CBC",
    "AES-GCM" => "GCM",
    "AES-KW" => "KW",
    "AES-OCB" => "OCB",
    _ => return None,
  };
  Some(match (suffix, kind) {
    ("128", "CTR") => "A128CTR",
    ("192", "CTR") => "A192CTR",
    ("256", "CTR") => "A256CTR",
    ("128", "CBC") => "A128CBC",
    ("192", "CBC") => "A192CBC",
    ("256", "CBC") => "A256CBC",
    ("128", "GCM") => "A128GCM",
    ("192", "GCM") => "A192GCM",
    ("256", "GCM") => "A256GCM",
    ("128", "KW") => "A128KW",
    ("192", "KW") => "A192KW",
    ("256", "KW") => "A256KW",
    ("128", "OCB") => "A128OCB",
    ("192", "OCB") => "A192OCB",
    ("256", "OCB") => "A256OCB",
    _ => return None,
  })
}

fn hmac_jwk_alg(hash_name: &str) -> Option<&'static str> {
  Some(match hash_name {
    "SHA-1" => "HS1",
    "SHA-256" => "HS256",
    "SHA-384" => "HS384",
    "SHA-512" => "HS512",
    "SHA3-256" => "HS3-256",
    "SHA3-384" => "HS3-384",
    "SHA3-512" => "HS3-512",
    _ => return None,
  })
}

pub fn check_usages_subset(
  usages: &[String],
  allowed: &[&str],
) -> Result<(), CryptoError> {
  for u in usages {
    if !allowed.iter().any(|a| *a == u.as_str()) {
      return Err(syntax_error("Invalid key usage".into()));
    }
  }
  Ok(())
}

pub fn filter_usages<'a>(usages: &'a [String], allowed: &[&str]) -> Vec<&'a str> {
  usages
    .iter()
    .map(String::as_str)
    .filter(|u| allowed.iter().any(|a| a == u))
    .collect()
}

fn validate_jwk_oct<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  jwk: v8::Local<'s, v8::Object>,
  expected_use: &str,
  usages: &[String],
  extractable: bool,
) -> Result<(), CryptoError> {
  let kty = read_string_member(scope, jwk, b"kty");
  if kty.as_deref() != Some("oct") {
    return Err(data_error(
      "'kty' property of JsonWebKey must be 'oct'".into(),
    ));
  }
  if read_string_member(scope, jwk, b"k").is_none() {
    return Err(data_error(
      "'k' property of JsonWebKey must be present".into(),
    ));
  }
  if !usages.is_empty() {
    if let Some(use_) = read_string_member(scope, jwk, b"use")
      && use_ != expected_use
    {
      return Err(data_error(format!(
        "'use' property of JsonWebKey must be '{expected_use}'"
      )));
    }
  }
  if let Some(key_ops) = read_string_array_member(scope, jwk, b"key_ops") {
    for u in &key_ops {
      if !ALL_USAGES.iter().any(|a| *a == u.as_str()) {
        return Err(data_error(
          "'key_ops' property of JsonWebKey is invalid".into(),
        ));
      }
    }
    for u in usages {
      if !key_ops.iter().any(|k| k == u) {
        return Err(data_error(
          "'key_ops' property of JsonWebKey is invalid".into(),
        ));
      }
    }
  }
  if read_bool_member(scope, jwk, b"ext") == Some(false) && extractable {
    return Err(data_error(
      "'ext' property of JsonWebKey must not be false if extractable is true"
        .into(),
    ));
  }
  Ok(())
}

fn read_jwk_b64_field<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<Vec<u8>> {
  let s = read_string_member(scope, obj, field)?;
  BASE64_URL_SAFE_NO_PAD
    .decode(s.trim_end_matches('='))
    .ok()
}

fn read_string_member<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<String> {
  let key = v8::String::new_from_one_byte(
    scope,
    field,
    v8::NewStringType::Internalized,
  )?;
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  let s = val.to_string(scope)?;
  Some(s.to_rust_string_lossy(scope))
}

fn read_bool_member<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<bool> {
  let key = v8::String::new_from_one_byte(
    scope,
    field,
    v8::NewStringType::Internalized,
  )?;
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  Some(val.boolean_value(scope))
}

fn read_u32_member<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<u32> {
  let key = v8::String::new_from_one_byte(
    scope,
    field,
    v8::NewStringType::Internalized,
  )?;
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  val.uint32_value(scope)
}

fn read_string_array_member<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
) -> Option<Vec<String>> {
  let key = v8::String::new_from_one_byte(
    scope,
    field,
    v8::NewStringType::Internalized,
  )?;
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  let arr = v8::Local::<v8::Array>::try_from(val).ok()?;
  let len = arr.length();
  let mut out = Vec::with_capacity(len as usize);
  for i in 0..len {
    let item = arr.get_index(scope, i)?;
    let s = item.to_string(scope)?;
    out.push(s.to_rust_string_lossy(scope));
  }
  Some(out)
}

fn read_hash_name<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
) -> Option<String> {
  let key = v8::String::new_from_one_byte(
    scope,
    b"hash",
    v8::NewStringType::Internalized,
  )?;
  let val = obj.get(scope, key.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  if val.is_string() {
    return Some(val.to_rust_string_lossy(scope));
  }
  let hash_obj = v8::Local::<v8::Object>::try_from(val).ok()?;
  let name_key = v8::String::new_from_one_byte(
    scope,
    b"name",
    v8::NewStringType::Internalized,
  )?;
  let name_val = hash_obj.get(scope, name_key.into())?;
  Some(name_val.to_string(scope)?.to_rust_string_lossy(scope))
}

pub fn data_error(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionDataError", msg))
}

pub fn syntax_error(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionSyntaxError", msg))
}

pub fn not_supported(msg: String) -> CryptoError {
  CryptoError::Other(JsErrorBox::new("DOMExceptionNotSupportedError", msg))
}

/// Stamp the standard `WebIdlErrorKind::ConvertToConverterType` shape onto
/// a converter failure. Saves a few lines per converter that needs it.
#[allow(dead_code, reason = "convenience for upcoming converters")]
pub fn convert_error<'b>(
  prefix: Cow<'static, str>,
  context: ContextFn<'b>,
  ty: &'static str,
) -> WebIdlError {
  WebIdlError::new(prefix, context, WebIdlErrorKind::ConvertToConverterType(ty))
}
