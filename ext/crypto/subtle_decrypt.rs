// Copyright 2018-2026 the Deno authors. MIT license.

//! `SubtleCrypto.decrypt()` body in Rust.
//!
//! Mirrors [`crate::subtle_encrypt`] for decryption: a `SubtleDecryptParams`
//! `WebIdlConverter` parses the per-algorithm dictionary, [`run`] validates
//! the spec preconditions (key type, iv length, tag length, counter
//! length, etc.) and dispatches into the existing per-algorithm
//! `decrypt_*` helpers in [`crate::decrypt`].

use std::borrow::Cow;

use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_error::JsErrorBox;

use crate::CryptoError;
use crate::crypto_key::CryptoKeyType;
use crate::decrypt;
use crate::subtle_encrypt::extract_name_and_obj;
use crate::subtle_encrypt::v8_str;
use crate::subtle_key::SubtleKey;

/// Normalized per-algorithm decrypt parameters. Each variant carries the
/// exact dictionary members the matching `decrypt_*` helper needs.
///
/// `Unknown` defers `NotSupportedError` to [`run`] so it raises a
/// `DOMException` instead of the `TypeError` a converter-level error
/// would emit.
pub enum SubtleDecryptParams {
  RsaOaep {
    label: Option<Vec<u8>>,
  },
  AesCbc {
    iv: Vec<u8>,
  },
  AesCtr {
    counter: Vec<u8>,
    length: u32,
  },
  AesGcm {
    iv: Vec<u8>,
    additional_data: Option<Vec<u8>>,
    tag_length: Option<u32>,
  },
  AesOcb {
    iv: Vec<u8>,
    additional_data: Option<Vec<u8>>,
    tag_length: Option<u32>,
  },
  ChaCha20Poly1305 {
    iv: Option<Vec<u8>>,
    additional_data: Option<Vec<u8>>,
    tag_length: Option<u32>,
  },
  Unknown(String),
}

impl SubtleDecryptParams {
  pub fn canonical_name(&self) -> &str {
    match self {
      Self::RsaOaep { .. } => "RSA-OAEP",
      Self::AesCbc { .. } => "AES-CBC",
      Self::AesCtr { .. } => "AES-CTR",
      Self::AesGcm { .. } => "AES-GCM",
      Self::AesOcb { .. } => "AES-OCB",
      Self::ChaCha20Poly1305 { .. } => "ChaCha20-Poly1305",
      Self::Unknown(n) => n,
    }
  }
}

impl<'a> WebIdlConverter<'a> for SubtleDecryptParams {
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
    let Some(canonical) = canonical_decrypt_name(&name_str) else {
      return Ok(Self::Unknown(name_str));
    };
    match canonical {
      "RSA-OAEP" => {
        let label = match maybe_obj {
          Some(o) => read_optional_buffer_source(
            scope,
            o,
            "label",
            prefix.clone(),
            &context,
          )?,
          None => None,
        };
        Ok(Self::RsaOaep { label })
      }
      "AES-CBC" => {
        let obj =
          maybe_obj.ok_or_else(|| missing_dict(prefix.clone(), &context))?;
        let iv = read_required_buffer_source(
          scope,
          obj,
          "iv",
          prefix.clone(),
          &context,
        )?;
        Ok(Self::AesCbc { iv })
      }
      "AES-CTR" => {
        let obj =
          maybe_obj.ok_or_else(|| missing_dict(prefix.clone(), &context))?;
        let counter = read_required_buffer_source(
          scope,
          obj,
          "counter",
          prefix.clone(),
          &context,
        )?;
        let length =
          read_required_u32(scope, obj, "length", prefix.clone(), &context)?;
        Ok(Self::AesCtr { counter, length })
      }
      "AES-GCM" => {
        let obj =
          maybe_obj.ok_or_else(|| missing_dict(prefix.clone(), &context))?;
        let iv = read_required_buffer_source(
          scope,
          obj,
          "iv",
          prefix.clone(),
          &context,
        )?;
        let additional_data = read_optional_buffer_source(
          scope,
          obj,
          "additionalData",
          prefix.clone(),
          &context,
        )?;
        let tag_length =
          read_optional_u32(scope, obj, "tagLength", prefix.clone(), &context)?;
        Ok(Self::AesGcm {
          iv,
          additional_data,
          tag_length,
        })
      }
      "AES-OCB" => {
        let obj =
          maybe_obj.ok_or_else(|| missing_dict(prefix.clone(), &context))?;
        let iv = read_required_buffer_source(
          scope,
          obj,
          "iv",
          prefix.clone(),
          &context,
        )?;
        let additional_data = read_optional_buffer_source(
          scope,
          obj,
          "additionalData",
          prefix.clone(),
          &context,
        )?;
        let tag_length =
          read_optional_u32(scope, obj, "tagLength", prefix.clone(), &context)?;
        Ok(Self::AesOcb {
          iv,
          additional_data,
          tag_length,
        })
      }
      "ChaCha20-Poly1305" => {
        let obj =
          maybe_obj.ok_or_else(|| missing_dict(prefix.clone(), &context))?;
        let iv = read_optional_buffer_source(
          scope,
          obj,
          "iv",
          prefix.clone(),
          &context,
        )?;
        let additional_data = read_optional_buffer_source(
          scope,
          obj,
          "additionalData",
          prefix.clone(),
          &context,
        )?;
        let tag_length =
          read_optional_u32(scope, obj, "tagLength", prefix.clone(), &context)?;
        Ok(Self::ChaCha20Poly1305 {
          iv,
          additional_data,
          tag_length,
        })
      }
      _ => unreachable!(),
    }
  }
}

fn canonical_decrypt_name(name: &str) -> Option<&'static str> {
  const NAMES: &[&str] = &[
    "RSA-OAEP",
    "AES-CBC",
    "AES-CTR",
    "AES-GCM",
    "AES-OCB",
    "ChaCha20-Poly1305",
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

fn read_required_buffer_source<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  field: &'static str,
  prefix: Cow<'static, str>,
  context: &ContextFn<'b>,
) -> Result<Vec<u8>, WebIdlError> {
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
  value_to_buffer_source(scope, val, field, prefix, context)
}

fn read_optional_buffer_source<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  field: &'static str,
  prefix: Cow<'static, str>,
  context: &ContextFn<'b>,
) -> Result<Option<Vec<u8>>, WebIdlError> {
  let key = v8_str(scope, field);
  let val = obj
    .get(scope, key.into())
    .unwrap_or_else(|| v8::undefined(scope).into());
  if val.is_undefined() || val.is_null() {
    return Ok(None);
  }
  // Route through the strict `BufferSource` guard so `SharedArrayBuffer`
  // (or a view backed by one) and any non-BufferSource value rejects with
  // `TypeError`, matching the JS `webidl.converters.BufferSource` contract
  // for optional dictionary members (e.g. AES-GCM `additionalData`,
  // RSA-OAEP `label`, ChaCha20-Poly1305 `iv`).
  value_to_buffer_source(scope, val, field, prefix, context).map(Some)
}

fn value_to_buffer_source<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  value: v8::Local<'a, v8::Value>,
  field: &'static str,
  prefix: Cow<'static, str>,
  context: &ContextFn<'b>,
) -> Result<Vec<u8>, WebIdlError> {
  if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(value) {
    if let Some(ab) = view.buffer(scope) {
      let ab_val: v8::Local<v8::Value> = ab.into();
      if ab_val.is_shared_array_buffer() {
        return Err(WebIdlError::other(
          prefix,
          context.borrowed(),
          JsErrorBox::type_error(format!(
            "'{field}' is a view on a SharedArrayBuffer, which is not allowed"
          )),
        ));
      }
    }
    return Ok(view_to_bytes(scope, view));
  }
  if value.is_shared_array_buffer() {
    return Err(WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error(format!(
        "'{field}' is a SharedArrayBuffer, which is not allowed"
      )),
    ));
  }
  if let Ok(ab) = v8::Local::<v8::ArrayBuffer>::try_from(value) {
    return Ok(arraybuffer_to_bytes(ab));
  }
  Err(WebIdlError::other(
    prefix,
    context.borrowed(),
    JsErrorBox::type_error(format!("'{field}' is not a BufferSource")),
  ))
}

fn view_to_bytes<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  view: v8::Local<'a, v8::ArrayBufferView>,
) -> Vec<u8> {
  let byte_length = view.byte_length();
  if byte_length == 0 {
    return Vec::new();
  }
  let byte_offset = view.byte_offset();
  let ab = view.buffer(scope).unwrap();
  // SAFETY: V8 guarantees byte_offset + byte_length stay within the
  // backing store and a non-detached buffer has a non-null data pointer.
  unsafe {
    let base = ab.data().unwrap().as_ptr() as *const u8;
    std::slice::from_raw_parts(base.add(byte_offset), byte_length).to_vec()
  }
}

fn arraybuffer_to_bytes(ab: v8::Local<v8::ArrayBuffer>) -> Vec<u8> {
  let byte_length = ab.byte_length();
  if byte_length == 0 {
    return Vec::new();
  }
  // SAFETY: as above.
  unsafe {
    let base = ab.data().unwrap().as_ptr() as *const u8;
    std::slice::from_raw_parts(base, byte_length).to_vec()
  }
}

/// `[EnforceRange] unsigned long`: NaN/Infinity, negative, and
/// `> 2**32-1` all reject; otherwise truncate toward zero. See
/// [`crate::subtle_encrypt::to_enforce_range_u32`] for the matching
/// encrypt-side helper.
fn to_enforce_range_u32<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  val: v8::Local<'a, v8::Value>,
) -> Option<u32> {
  let n = val.number_value(scope)?;
  if !n.is_finite() {
    return None;
  }
  let trunc = n.trunc();
  if trunc < 0.0 || trunc > u32::MAX as f64 {
    return None;
  }
  Some(trunc as u32)
}

fn read_required_u32<'a, 'b>(
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
  to_enforce_range_u32(scope, val).ok_or_else(|| {
    WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error(format!(
        "'{field}' is outside the [0, 2**32-1] range"
      )),
    )
  })
}

fn read_optional_u32<'a, 'b>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<'a, v8::Object>,
  field: &'static str,
  prefix: Cow<'static, str>,
  context: &ContextFn<'b>,
) -> Result<Option<u32>, WebIdlError> {
  let key = v8_str(scope, field);
  let val = obj
    .get(scope, key.into())
    .unwrap_or_else(|| v8::undefined(scope).into());
  if val.is_undefined() || val.is_null() {
    return Ok(None);
  }
  match to_enforce_range_u32(scope, val) {
    Some(v) => Ok(Some(v)),
    None => Err(WebIdlError::other(
      prefix,
      context.borrowed(),
      JsErrorBox::type_error(format!(
        "'{field}' is outside the [0, 2**32-1] range"
      )),
    )),
  }
}

/// Validate the per-algorithm prerequisites and dispatch to the existing
/// [`crate::decrypt`] backend helpers.
pub fn run(
  params: SubtleDecryptParams,
  key: SubtleKey,
  data: Vec<u8>,
) -> Result<Vec<u8>, CryptoError> {
  if let SubtleDecryptParams::Unknown(name) = &params {
    return Err(not_supported(format!(
      "Algorithm '{name}' is not supported"
    )));
  }
  // Decrypt step 8: algorithm-name match is an `OperationError`
  // (`encrypt` uses `InvalidAccessError`; the JS shim modelled both).
  if params.canonical_name() != key.algorithm_name {
    return Err(op_error(format!(
      "Decryption algorithm \"{}\" does not match key algorithm",
      params.canonical_name()
    )));
  }
  if !key.has_usage("decrypt") {
    return Err(invalid_access(
      "The requested operation is not valid for the provided key".to_string(),
    ));
  }

  match params {
    SubtleDecryptParams::RsaOaep { label } => {
      if key.key_type != CryptoKeyType::Private {
        return Err(invalid_access("Key type not supported".to_string()));
      }
      let hash = key.algorithm_hash.ok_or_else(|| {
        op_error("RSA-OAEP key is missing 'hash'".to_string())
      })?;
      decrypt::decrypt_rsa_oaep(
        &key.raw,
        hash,
        label.unwrap_or_default(),
        &data,
      )
      .map_err(decrypt_error_to_crypto)
    }
    SubtleDecryptParams::AesCbc { iv } => {
      if iv.len() != 16 {
        return Err(op_error("Counter must be 16 bytes".to_string()));
      }
      let length = key.algorithm_length.ok_or_else(|| {
        op_error("AES-CBC key is missing 'length'".to_string())
      })?;
      decrypt::decrypt_aes_cbc(&key.raw, length as usize, iv, &data)
        .map_err(decrypt_error_to_crypto)
    }
    SubtleDecryptParams::AesCtr { counter, length } => {
      if counter.len() != 16 {
        return Err(op_error("Counter vector must be 16 bytes".to_string()));
      }
      if length == 0 || length > 128 {
        return Err(op_error(format!(
          "Counter length must not be 0 or greater than 128: received {length}"
        )));
      }
      let key_length = key.algorithm_length.ok_or_else(|| {
        op_error("AES-CTR key is missing 'length'".to_string())
      })?;
      decrypt::decrypt_aes_ctr(
        &key.raw,
        key_length as usize,
        &counter,
        length as usize,
        &data,
      )
      .map_err(decrypt_error_to_crypto)
    }
    SubtleDecryptParams::AesGcm {
      iv,
      additional_data,
      tag_length,
    } => {
      let tag_length = match tag_length {
        None => 128u32,
        Some(t) if [32, 64, 96, 104, 112, 120, 128].contains(&t) => t,
        Some(t) => {
          return Err(op_error(format!("Invalid tag length: {t}")));
        }
      };
      if data.len() < (tag_length / 8) as usize {
        return Err(op_error("The provided data is too small".to_string()));
      }
      let iv_len = iv.len();
      if iv_len != 12 && iv_len != 16 {
        return Err(not_supported(
          "Initialization vector length not supported".to_string(),
        ));
      }
      let key_length = key.algorithm_length.ok_or_else(|| {
        op_error("AES-GCM key is missing 'length'".to_string())
      })?;
      decrypt::decrypt_aes_gcm(
        &key.raw,
        key_length as usize,
        tag_length as usize,
        iv,
        additional_data,
        &data,
      )
      .map_err(decrypt_error_to_crypto)
    }
    SubtleDecryptParams::AesOcb {
      iv,
      additional_data,
      tag_length,
    } => {
      let tag_length = match tag_length {
        None => 128u32,
        Some(t) if [64, 96, 128].contains(&t) => t,
        Some(t) => {
          return Err(op_error(format!("Invalid tag length: {t}")));
        }
      };
      if data.len() < (tag_length / 8) as usize {
        return Err(op_error("The provided data is too small".to_string()));
      }
      let iv_len = iv.len();
      if !(6..=15).contains(&iv_len) {
        return Err(op_error(
          "Invalid nonce length for AES-OCB (must be 6-15 bytes)".to_string(),
        ));
      }
      let key_length = key.algorithm_length.ok_or_else(|| {
        op_error("AES-OCB key is missing 'length'".to_string())
      })?;
      decrypt::decrypt_aes_ocb(
        &key.raw,
        key_length as usize,
        tag_length as usize,
        iv,
        additional_data,
        &data,
      )
      .map_err(decrypt_error_to_crypto)
    }
    SubtleDecryptParams::ChaCha20Poly1305 {
      iv,
      additional_data,
      tag_length,
    } => {
      let Some(iv) = iv else {
        return Err(CryptoError::Other(JsErrorBox::type_error(
          "iv is required",
        )));
      };
      if iv.len() != 12 {
        return Err(op_error(
          "ChaCha20-Poly1305 iv must be 12 bytes".to_string(),
        ));
      }
      if let Some(t) = tag_length
        && t != 128
      {
        return Err(op_error(
          "ChaCha20-Poly1305 tagLength must be 128".to_string(),
        ));
      }
      if data.len() < 16 {
        return Err(op_error("The provided data is too small".to_string()));
      }
      decrypt::decrypt_chacha20_poly1305(&key.raw, &iv, additional_data, &data)
        .map_err(decrypt_error_to_crypto)
    }
    SubtleDecryptParams::Unknown(name) => {
      Err(CryptoError::Other(JsErrorBox::new(
        "DOMExceptionNotSupportedError",
        format!("Algorithm '{name}' is not supported"),
      )))
    }
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

fn decrypt_error_to_crypto(e: decrypt::DecryptError) -> CryptoError {
  CryptoError::Other(JsErrorBox::from_err(e))
}
