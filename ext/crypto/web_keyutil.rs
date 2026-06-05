// Copyright 2018-2026 the Deno authors. MIT license.

//! Helpers shared by the cppgc `SubtleCrypto` impl methods (`web_subtle.rs`).
//!
//! These read fields back out of the cppgc `CryptoKey` (its `algorithm` dict
//! object and its `{ type, data }` `key_data` value) and out of the normalized
//! algorithm object produced by `web_params::normalize_algorithm`, without any
//! `#[serde]` round-trips.

use deno_core::FromV8;
use deno_core::v8;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;

use crate::shared::V8RawKeyData;
use crate::web_cryptokey::CryptoKey;

/// Read a string property off a v8 object. Returns `None` if absent/undefined.
pub fn get_string(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<v8::Object>,
  key: &str,
) -> Option<String> {
  let k = v8::String::new(scope, key).unwrap();
  match obj.get(scope, k.into()) {
    Some(v) if !v.is_undefined() && !v.is_null() => {
      Some(v.to_rust_string_lossy(scope))
    }
    _ => None,
  }
}

/// Read a numeric property off a v8 object as `usize`.
pub fn get_usize(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<v8::Object>,
  key: &str,
) -> Option<usize> {
  let k = v8::String::new(scope, key).unwrap();
  match obj.get(scope, k.into()) {
    Some(v) if v.is_number() => v.number_value(scope).map(|n| n as usize),
    _ => None,
  }
}

/// Read a nested object property off a v8 object.
pub fn get_object<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<v8::Object>,
  key: &str,
) -> Option<v8::Local<'a, v8::Object>> {
  let k = v8::String::new(scope, key).unwrap();
  match obj.get(scope, k.into()) {
    Some(v) => v.try_cast::<v8::Object>().ok(),
    _ => None,
  }
}

/// Read a value property off a v8 object.
pub fn get_value<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<v8::Object>,
  key: &str,
) -> Option<v8::Local<'a, v8::Value>> {
  let k = v8::String::new(scope, key).unwrap();
  match obj.get(scope, k.into()) {
    Some(v) if !v.is_undefined() => Some(v),
    _ => None,
  }
}

/// Read `algorithm.hash.name` (the hash dictionary member's canonical name).
pub fn get_hash_name(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<v8::Object>,
) -> Option<String> {
  let hash = get_object(scope, obj, "hash")?;
  get_string(scope, hash, "name")
}

/// Copy the bytes out of an `ArrayBuffer` / `ArrayBufferView` value.
pub fn buffer_bytes(value: v8::Local<v8::Value>) -> Option<Vec<u8>> {
  if value.is_array_buffer_view() {
    let view: v8::Local<v8::ArrayBufferView> = value.try_into().ok()?;
    let len = view.byte_length();
    let mut buf = vec![0u8; len];
    view.copy_contents(&mut buf);
    Some(buf)
  } else if value.is_array_buffer() {
    let ab: v8::Local<v8::ArrayBuffer> = value.try_into().ok()?;
    let len = ab.byte_length();
    let mut buf = vec![0u8; len];
    if len > 0
      && let Some(data) = ab.data()
    {
      // SAFETY: `data` points to `len` initialized bytes owned by the buffer.
      unsafe {
        std::ptr::copy_nonoverlapping(
          data.as_ptr() as *const u8,
          buf.as_mut_ptr(),
          len,
        );
      }
    }
    Some(buf)
  } else {
    None
  }
}

/// Read a buffer-valued property off a v8 object (e.g. normalized `iv`).
pub fn get_buffer(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<v8::Object>,
  key: &str,
) -> Option<Vec<u8>> {
  let v = get_value(scope, obj, key)?;
  buffer_bytes(v)
}

/// Owned snapshot of a cppgc `CryptoKey`'s fields, extracted in the synchronous
/// op2 arg-conversion prelude (so it can be moved into an async body that has no
/// `scope`). Mirrors the fields the JS used to pull off the key.
pub struct KeySnapshot {
  pub key_type: String,
  pub usages: Vec<String>,
  // Retained for symmetry with the JS key snapshot; not all consumers read
  // every field.
  #[allow(dead_code)]
  pub extractable: bool,
  pub raw: V8RawKeyData,
  pub algorithm_name: String,
  pub length: Option<usize>,
  pub hash: Option<String>,
  #[allow(dead_code)]
  pub named_curve: Option<String>,
}

impl<'a> FromV8<'a> for KeySnapshot {
  type Error = JsErrorBox;

  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    let key =
      deno_core::cppgc::try_unwrap_cppgc_object::<CryptoKey>(scope, value)
        .ok_or_else(|| {
          JsErrorBox::type_error("Argument is not a CryptoKey".to_string())
        })?;
    let snap = snapshot_key(scope, &key);
    snap.ok_or_else(|| JsErrorBox::type_error("Invalid key data".to_string()))
  }
}

/// Build a [`KeySnapshot`] from a borrowed cppgc `CryptoKey`.
pub fn snapshot_key(
  scope: &mut v8::PinScope<'_, '_>,
  key: &CryptoKey,
) -> Option<KeySnapshot> {
  let alg = v8::Local::new(scope, &key.algorithm);
  let key_data_v = v8::Local::new(scope, &key.key_data);
  let raw = raw_key_data(scope, key_data_v)?;
  Some(KeySnapshot {
    key_type: key.key_type.clone(),
    usages: key.usages.borrow().clone(),
    extractable: *key.extractable.borrow(),
    raw,
    algorithm_name: get_string(scope, alg, "name").unwrap_or_default(),
    length: get_usize(scope, alg, "length"),
    hash: get_hash_name(scope, alg),
    named_curve: get_string(scope, alg, "namedCurve"),
  })
}

/// Owned cipher algorithm parameters, normalized + extracted in the sync op2
/// prelude. The algorithm `name` is canonicalized by `normalize_algorithm`.
pub struct CipherParams {
  pub name: String,
  pub iv: Option<Vec<u8>>,
  pub counter: Option<Vec<u8>>,
  pub length: Option<usize>,
  pub label: Option<Vec<u8>>,
  pub additional_data: Option<Vec<u8>>,
  pub tag_length: Option<usize>,
  pub nonce: Option<Vec<u8>>,
}

fn extract_cipher_params<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  op: &str,
  value: v8::Local<'a, v8::Value>,
) -> Result<CipherParams, JsErrorBox> {
  let normalized = crate::web_params::normalize_algorithm(scope, op, value)
    .map_err(|e| JsErrorBox::new(e.get_class(), e.get_message()))?;
  Ok(CipherParams {
    name: get_string(scope, normalized, "name").unwrap_or_default(),
    iv: get_buffer(scope, normalized, "iv"),
    counter: get_buffer(scope, normalized, "counter"),
    length: get_usize(scope, normalized, "length"),
    label: get_buffer(scope, normalized, "label"),
    additional_data: get_buffer(scope, normalized, "additionalData"),
    tag_length: get_usize(scope, normalized, "tagLength"),
    nonce: get_buffer(scope, normalized, "nonce"),
  })
}

/// `algorithm` arg for `encrypt` (normalized against the "encrypt" registry).
pub struct EncryptAlg(pub CipherParams);
impl<'a> FromV8<'a> for EncryptAlg {
  type Error = JsErrorBox;
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    Ok(EncryptAlg(extract_cipher_params(scope, "encrypt", value)?))
  }
}

/// `algorithm` arg for `decrypt` (normalized against the "decrypt" registry).
pub struct DecryptAlg(pub CipherParams);
impl<'a> FromV8<'a> for DecryptAlg {
  type Error = JsErrorBox;
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    Ok(DecryptAlg(extract_cipher_params(scope, "decrypt", value)?))
  }
}

/// Owned key material + metadata for `sign` / `verify`, extracted in the sync
/// prelude. `key_bytes` is already resolved per-algorithm (RSA/EC/HMAC use the
/// inner `{ type, data }.data`; Ed25519 uses the raw key bytes; ML-DSA uses
/// `privateKey` for signing and the whole `key_data` (raw public bytes) for
/// verifying — captured here as `ml_dsa_private` / `ml_dsa_public`).
pub struct SignKey {
  pub algorithm_name: String,
  /// The cppgc key `type` ("public" | "private" | "secret").
  pub key_type: String,
  /// The inner `{ type, data }.type` for RSA/EC/HMAC keys.
  pub data_type: String,
  /// Resolved key bytes for the algorithm (see struct docs).
  pub key_bytes: Vec<u8>,
  /// `key.algorithm.hash.name` (RSA/HMAC).
  pub hash: Option<String>,
  /// `key.algorithm.namedCurve` (ECDSA).
  pub named_curve: Option<String>,
  pub usages: Vec<String>,
}

impl SignKey {
  /// The key type the signature op expects: the inner `{ type, data }.type`
  /// (RSA/EC/HMAC) when present, otherwise the outer cppgc key type.
  pub fn effective_key_type(&self) -> &str {
    if self.data_type.is_empty() {
      &self.key_type
    } else {
      &self.data_type
    }
  }
}

fn extract_sign_key(
  scope: &mut v8::PinScope<'_, '_>,
  key: &CryptoKey,
) -> Option<SignKey> {
  let alg = v8::Local::new(scope, &key.algorithm);
  let algorithm_name = get_string(scope, alg, "name").unwrap_or_default();
  let key_data_v = v8::Local::new(scope, &key.key_data);
  let (data_type, key_bytes) = match algorithm_name.as_str() {
    "Ed25519" => {
      // key_data is the raw seed/public bytes directly.
      ("".to_string(), buffer_bytes(key_data_v).unwrap_or_default())
    }
    "ML-DSA-44" | "ML-DSA-65" | "ML-DSA-87" => {
      // The private key's `key_data` is `{ seed, privateKey }`; the public
      // key's `key_data` is the raw public-key bytes directly.
      let bytes = if key.key_type == "private" {
        let obj = key_data_v.try_cast::<v8::Object>().ok()?;
        get_buffer(scope, obj, "privateKey").unwrap_or_default()
      } else {
        buffer_bytes(key_data_v).unwrap_or_default()
      };
      ("".to_string(), bytes)
    }
    _ => {
      // RSA / EC / HMAC: inner { type, data }.
      let obj = key_data_v.try_cast::<v8::Object>().ok()?;
      let dt = get_string(scope, obj, "type").unwrap_or_default();
      let bytes = get_buffer(scope, obj, "data").unwrap_or_default();
      (dt, bytes)
    }
  };
  Some(SignKey {
    algorithm_name,
    key_type: key.key_type.clone(),
    data_type,
    key_bytes,
    hash: get_hash_name(scope, alg),
    named_curve: get_string(scope, alg, "namedCurve"),
    usages: key.usages.borrow().clone(),
  })
}

impl<'a> FromV8<'a> for SignKey {
  type Error = JsErrorBox;
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    let key =
      deno_core::cppgc::try_unwrap_cppgc_object::<CryptoKey>(scope, value)
        .ok_or_else(|| {
          JsErrorBox::type_error("Argument is not a CryptoKey".to_string())
        })?;
    extract_sign_key(scope, &key)
      .ok_or_else(|| JsErrorBox::type_error("Invalid key data".to_string()))
  }
}

/// Owned sign/verify algorithm params (normalized against "sign" / "verify").
pub struct SignParams {
  pub name: String,
  pub hash: Option<String>,
  pub salt_length: Option<u32>,
  pub context: Option<Vec<u8>>,
}

fn extract_sign_params<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  op: &str,
  value: v8::Local<'a, v8::Value>,
) -> Result<SignParams, JsErrorBox> {
  let normalized = crate::web_params::normalize_algorithm(scope, op, value)
    .map_err(|e| JsErrorBox::new(e.get_class(), e.get_message()))?;
  Ok(SignParams {
    name: get_string(scope, normalized, "name").unwrap_or_default(),
    hash: get_hash_name(scope, normalized),
    salt_length: get_usize(scope, normalized, "saltLength").map(|n| n as u32),
    context: get_buffer(scope, normalized, "context"),
  })
}

/// `algorithm` arg for `sign`.
pub struct SignAlg(pub SignParams);
impl<'a> FromV8<'a> for SignAlg {
  type Error = JsErrorBox;
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    Ok(SignAlg(extract_sign_params(scope, "sign", value)?))
  }
}

/// `algorithm` arg for `verify`.
pub struct VerifyAlg(pub SignParams);
impl<'a> FromV8<'a> for VerifyAlg {
  type Error = JsErrorBox;
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    Ok(VerifyAlg(extract_sign_params(scope, "verify", value)?))
  }
}

/// Resolve a derive (base/public) key's `(type, data)` per algorithm: X25519 /
/// X448 store raw bytes (use the outer key type); ECDH / PBKDF2 / HKDF store an
/// inner `{ type, data }` object.
fn derive_key_bytes(
  scope: &mut v8::PinScope<'_, '_>,
  key: &CryptoKey,
) -> Option<(String, Vec<u8>)> {
  let alg = v8::Local::new(scope, &key.algorithm);
  let name = get_string(scope, alg, "name").unwrap_or_default();
  let key_data_v = v8::Local::new(scope, &key.key_data);
  if name == "X25519" || name == "X448" {
    Some((key.key_type.clone(), buffer_bytes(key_data_v)?))
  } else {
    let obj = key_data_v.try_cast::<v8::Object>().ok()?;
    let dt = get_string(scope, obj, "type")?;
    let data = get_buffer(scope, obj, "data")?;
    Some((dt, data))
  }
}

/// Owned snapshot of the `deriveBits` base key.
pub struct DeriveBaseSnapshot {
  pub key_type: String,
  pub data: Vec<u8>,
  pub algorithm_name: String,
  pub named_curve: Option<String>,
  pub usages: Vec<String>,
}

impl<'a> FromV8<'a> for DeriveBaseSnapshot {
  type Error = JsErrorBox;
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    let key =
      deno_core::cppgc::try_unwrap_cppgc_object::<CryptoKey>(scope, value)
        .ok_or_else(|| {
          JsErrorBox::type_error("Argument is not a CryptoKey".to_string())
        })?;
    let alg = v8::Local::new(scope, &key.algorithm);
    let algorithm_name = get_string(scope, alg, "name").unwrap_or_default();
    let named_curve = get_string(scope, alg, "namedCurve");
    let usages = key.usages.borrow().clone();
    let (key_type, data) = derive_key_bytes(scope, &key)
      .ok_or_else(|| JsErrorBox::type_error("Invalid key data".to_string()))?;
    Ok(DeriveBaseSnapshot {
      key_type,
      data,
      algorithm_name,
      named_curve,
      usages,
    })
  }
}

/// Owned snapshot of a public key referenced inside a `deriveBits` algorithm
/// (`normalizedAlgorithm.public` for ECDH/X25519/X448).
pub struct DerivePublicSnapshot {
  /// The inner `{ type, data }.type` (ECDH) or outer type (X25519/X448) — used
  /// to shape `DeriveKeyData`.
  pub key_type: String,
  /// The outer cppgc key `type` (`publicKey.type` in the JS) — the value the
  /// op validates against `public`.
  pub outer_key_type: String,
  pub data: Vec<u8>,
  pub algorithm_name: String,
  pub named_curve: Option<String>,
}

fn derive_public_snapshot(
  scope: &mut v8::PinScope<'_, '_>,
  key: &CryptoKey,
) -> Option<DerivePublicSnapshot> {
  let alg = v8::Local::new(scope, &key.algorithm);
  let algorithm_name = get_string(scope, alg, "name").unwrap_or_default();
  let named_curve = get_string(scope, alg, "namedCurve");
  let outer_key_type = key.key_type.clone();
  let (key_type, data) = derive_key_bytes(scope, key)?;
  Some(DerivePublicSnapshot {
    key_type,
    outer_key_type,
    data,
    algorithm_name,
    named_curve,
  })
}

/// Owned `deriveBits` algorithm params (normalized against "deriveBits"),
/// including the embedded public key (if any).
pub struct DeriveParams {
  pub name: String,
  pub hash: Option<String>,
  pub iterations: Option<u32>,
  pub salt: Option<Vec<u8>>,
  pub info: Option<Vec<u8>>,
  pub public: Option<DerivePublicSnapshot>,
}

/// `algorithm` arg for `deriveBits`.
pub struct DeriveAlg(pub DeriveParams);
impl<'a> FromV8<'a> for DeriveAlg {
  type Error = JsErrorBox;
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    let normalized =
      crate::web_params::normalize_algorithm(scope, "deriveBits", value)
        .map_err(|e| JsErrorBox::new(e.get_class(), e.get_message()))?;
    let public = match get_value(scope, normalized, "public") {
      Some(v) if v.is_object() => {
        let pk =
          deno_core::cppgc::try_unwrap_cppgc_object::<CryptoKey>(scope, v)
            .ok_or_else(|| {
              JsErrorBox::type_error("Invalid public key".to_string())
            })?;
        derive_public_snapshot(scope, &pk)
      }
      _ => None,
    };
    Ok(DeriveAlg(DeriveParams {
      name: get_string(scope, normalized, "name").unwrap_or_default(),
      hash: get_hash_name(scope, normalized),
      iterations: get_usize(scope, normalized, "iterations").map(|n| n as u32),
      salt: get_buffer(scope, normalized, "salt"),
      info: get_buffer(scope, normalized, "info"),
      public,
    }))
  }
}

/// `algorithm` arg for `encapsulateBits` (normalized against "encapsulate").
pub struct EncapsulateAlg(pub String);
impl<'a> FromV8<'a> for EncapsulateAlg {
  type Error = JsErrorBox;
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    let n = crate::web_params::normalize_algorithm(scope, "encapsulate", value)
      .map_err(|e| JsErrorBox::new(e.get_class(), e.get_message()))?;
    Ok(EncapsulateAlg(
      get_string(scope, n, "name").unwrap_or_default(),
    ))
  }
}

/// `algorithm` arg for `decapsulateBits` (normalized against "decapsulate").
pub struct DecapsulateAlg(pub String);
impl<'a> FromV8<'a> for DecapsulateAlg {
  type Error = JsErrorBox;
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    let n = crate::web_params::normalize_algorithm(scope, "decapsulate", value)
      .map_err(|e| JsErrorBox::new(e.get_class(), e.get_message()))?;
    Ok(DecapsulateAlg(
      get_string(scope, n, "name").unwrap_or_default(),
    ))
  }
}

/// Owned snapshot of an ML-KEM encapsulation/decapsulation key. `key_data` is
/// the raw key bytes (`key.keyData` is a Uint8Array for ML-KEM).
pub struct KemKeySnapshot {
  pub algorithm_name: String,
  pub key_type: String,
  pub usages: Vec<String>,
  pub key_data: Vec<u8>,
}

impl<'a> FromV8<'a> for KemKeySnapshot {
  type Error = JsErrorBox;
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    let key =
      deno_core::cppgc::try_unwrap_cppgc_object::<CryptoKey>(scope, value)
        .ok_or_else(|| {
          JsErrorBox::type_error("Argument is not a CryptoKey".to_string())
        })?;
    let alg = v8::Local::new(scope, &key.algorithm);
    let key_data_v = v8::Local::new(scope, &key.key_data);
    // ML-KEM private keys store `{ seed, privateKey }`; the encapsulation key
    // (public) and imported expanded keys store the raw bytes directly.
    let key_data = if key.key_type == "private" {
      key_data_v
        .try_cast::<v8::Object>()
        .ok()
        .and_then(|obj| get_buffer(scope, obj, "privateKey"))
        .or_else(|| buffer_bytes(key_data_v))
        .ok_or_else(|| JsErrorBox::type_error("Invalid key data".to_string()))?
    } else {
      buffer_bytes(key_data_v)
        .ok_or_else(|| JsErrorBox::type_error("Invalid key data".to_string()))?
    };
    Ok(KemKeySnapshot {
      algorithm_name: get_string(scope, alg, "name").unwrap_or_default(),
      key_type: key.key_type.clone(),
      usages: key.usages.borrow().clone(),
      key_data,
    })
  }
}

/// `length` arg for `deriveBits`: both `null` and `undefined` map to `None`
/// (matching the JS `if (length !== null) length = converter(length)`), a
/// number is coerced to `u32`.
pub struct DeriveLength(pub Option<u32>);
impl<'a> FromV8<'a> for DeriveLength {
  type Error = JsErrorBox;
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    if value.is_null_or_undefined() {
      return Ok(DeriveLength(None));
    }
    let n = value
      .uint32_value(scope)
      .ok_or_else(|| JsErrorBox::type_error("Invalid length".to_string()))?;
    Ok(DeriveLength(Some(n)))
  }
}

/// Decode a cppgc `CryptoKey`'s `key_data` (`{ type, data }`) into the
/// `V8RawKeyData` the crypto compute functions expect.
pub fn raw_key_data<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  key_data: v8::Local<'a, v8::Value>,
) -> Option<V8RawKeyData> {
  let obj = key_data.try_cast::<v8::Object>().ok()?;
  let ty = get_string(scope, obj, "type")?;
  let data_v = get_value(scope, obj, "data")?;
  let data =
    deno_core::serde_v8::from_v8::<deno_core::JsBuffer>(scope, data_v).ok()?;
  Some(match ty.as_str() {
    "secret" => V8RawKeyData::Secret(data),
    "private" => V8RawKeyData::Private(data),
    "public" => V8RawKeyData::Public(data),
    _ => return None,
  })
}
