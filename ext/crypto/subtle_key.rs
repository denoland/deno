// Copyright 2018-2026 the Deno authors. MIT license.

//! `WebIdlConverter` that extracts every field a `SubtleCrypto` method
//! needs from a `CryptoKey` argument, eagerly and synchronously, so the
//! async impl body can be moved into `spawn_blocking` with no v8 deps.
//!
//! Used by every method that takes a `CryptoKey`: `encrypt`, `decrypt`,
//! `sign`, `verify`, `wrapKey`, `unwrapKey`, `deriveBits`, `deriveKey`,
//! `exportKey`, `encapsulateKey`, `encapsulateBits`, `decapsulateKey`,
//! `decapsulateBits`, and `getPublicKey`.

use std::borrow::Cow;

use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlErrorKind;
use deno_error::JsErrorBox;

use crate::crypto_key::CryptoKey;
use crate::crypto_key::CryptoKeyType;
use crate::shared::RawKeyData;
use crate::shared::ShaHash;

/// A snapshot of the `CryptoKey` slots a `SubtleCrypto` method body cares
/// about. Built once during argument coercion so the spec-mandated
/// `InvalidAccessError` / `OperationError` checks in the impl can run
/// without re-entering JS, and so the per-algorithm dispatch can hand
/// the captured `RawKeyData` to `spawn_blocking`.
#[allow(
  dead_code,
  reason = "fields are read by SubtleCrypto methods that are still being ported"
)]
pub struct SubtleKey {
  pub algorithm_name: String,
  /// AES-`*` key length in bits (`128` / `192` / `256`). `None` for any
  /// algorithm whose `algorithm` dictionary doesn't carry one.
  pub algorithm_length: Option<u32>,
  /// Hash for RSA-* and HMAC keys. `None` for any algorithm whose
  /// `algorithm` dictionary doesn't carry one.
  pub algorithm_hash: Option<ShaHash>,
  /// ECDSA / ECDH `namedCurve`. `None` for non-EC keys.
  pub algorithm_named_curve: Option<String>,
  pub usages: Vec<String>,
  pub key_type: CryptoKeyType,
  pub extractable: bool,
  pub raw: RawKeyData,
}

impl SubtleKey {
  pub fn has_usage(&self, usage: &str) -> bool {
    self.usages.iter().any(|u| u == usage)
  }
}

impl<'a> WebIdlConverter<'a> for SubtleKey {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    _options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    let Some(key_ptr) =
      deno_core::cppgc::try_unwrap_cppgc_object::<CryptoKey>(scope, value)
    else {
      return Err(WebIdlError::new(
        prefix,
        context,
        WebIdlErrorKind::ConvertToConverterType("CryptoKey"),
      ));
    };
    let key: &CryptoKey = &key_ptr;

    let algorithm_name = key.algorithm_name(scope).ok_or_else(|| {
      WebIdlError::other(
        prefix.clone(),
        context.borrowed(),
        JsErrorBox::type_error("CryptoKey.algorithm.name is not a string"),
      )
    })?;

    let algorithm_length = read_algorithm_field_u32(scope, key, "length");
    let algorithm_hash = read_algorithm_hash(scope, key);
    let algorithm_named_curve =
      read_algorithm_field_string(scope, key, "namedCurve");

    let usages = key.usages_as_vec(scope).unwrap_or_default();

    let Some(handle_ptr) = key.key_handle(scope) else {
      return Err(WebIdlError::other(
        prefix,
        context,
        JsErrorBox::type_error("CryptoKey handle has been tampered with"),
      ));
    };
    let raw = handle_ptr.data().clone();

    Ok(SubtleKey {
      algorithm_name,
      algorithm_length,
      algorithm_hash,
      algorithm_named_curve,
      usages,
      key_type: key.key_type(),
      extractable: key.extractable_(),
      raw,
    })
  }
}

fn read_algorithm_field_u32<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: &CryptoKey,
  field: &str,
) -> Option<u32> {
  let alg = key.algorithm_local(scope)?;
  let key_v8 = v8::String::new_from_one_byte(
    scope,
    field.as_bytes(),
    v8::NewStringType::Internalized,
  )?;
  let val = alg.get(scope, key_v8.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  val.uint32_value(scope)
}

fn read_algorithm_field_string<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: &CryptoKey,
  field: &str,
) -> Option<String> {
  let alg = key.algorithm_local(scope)?;
  let key_v8 = v8::String::new_from_one_byte(
    scope,
    field.as_bytes(),
    v8::NewStringType::Internalized,
  )?;
  let val = alg.get(scope, key_v8.into())?;
  if val.is_undefined() || val.is_null() {
    return None;
  }
  let s = val.to_string(scope)?;
  Some(s.to_rust_string_lossy(scope))
}

/// RSA-* keys (`RSASSA-PKCS1-v1_5`, `RSA-PSS`, `RSA-OAEP`) and HMAC keys
/// carry their hash on the `algorithm.hash` slot, which is either a
/// `HashAlgorithmIdentifier` dictionary `{ name: <DOMString> }` or the
/// bare name string. Both shapes are normalized into the canonical name
/// here.
fn read_algorithm_hash<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: &CryptoKey,
) -> Option<ShaHash> {
  let alg = key.algorithm_local(scope)?;
  let hash_key = v8::String::new_from_one_byte(
    scope,
    b"hash",
    v8::NewStringType::Internalized,
  )?;
  let hash_val = alg.get(scope, hash_key.into())?;
  if hash_val.is_undefined() || hash_val.is_null() {
    return None;
  }
  let name_str = if hash_val.is_string() {
    hash_val.to_rust_string_lossy(scope)
  } else {
    let obj = v8::Local::<v8::Object>::try_from(hash_val).ok()?;
    let name_key = v8::String::new_from_one_byte(
      scope,
      b"name",
      v8::NewStringType::Internalized,
    )?;
    let name_val = obj.get(scope, name_key.into())?;
    let s = name_val.to_string(scope)?;
    s.to_rust_string_lossy(scope)
  };
  match name_str.as_str() {
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
