// Copyright 2018-2026 the Deno authors. MIT license.

//! The WebCrypto `CryptoKey` interface, implemented as a deno_core cppgc
//! (`GarbageCollected`) object — modelled on the webgpu cppgc objects (see
//! `ext/webgpu/buffer.rs`).
//!
//! This replaces the JS `CryptoKey` class and the `KEY_STORE` WeakMap that used
//! to live in `ext/crypto/00_crypto.js`. The key material (the value that used
//! to be stored in `KEY_STORE`) and the algorithm dictionary object are held
//! directly on the Rust object as `v8::Global`s, so the existing per-algorithm
//! import/export JS helpers (and the Node.js `KeyObject` interop in
//! `ext/node/polyfills/internal/crypto/keys.ts`) keep working: they read the
//! key material and algorithm back out via getter ops.

use std::cell::RefCell;

use deno_core::GarbageCollected;
use deno_core::cppgc::make_cppgc_object;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;

/// `CryptoKey` cppgc object. Holds the key material + metadata in Rust.
///
/// `key_data` is the same value that used to be stored in the JS `KEY_STORE`
/// WeakMap. Its shape varies per algorithm (e.g. `{ type, data }` for
/// AES/HMAC/RSA/EC, raw bytes for Ed25519/X25519/X448, `{ privateKey,
/// publicKey, seed }` for ML-DSA), so it is kept as an opaque `v8::Global`.
pub struct CryptoKey {
  pub key_type: String,
  pub extractable: RefCell<bool>,
  pub usages: RefCell<Vec<String>>,
  /// The JS algorithm dictionary object (`{ name, ... }`).
  pub algorithm: v8::Global<v8::Object>,
  /// The raw key material (formerly the `KEY_STORE` value).
  pub key_data: v8::Global<v8::Value>,
  /// For ML-DSA private keys: the associated public `CryptoKey`, returned by
  /// `getPublicKey()` (replaces the JS `MLDSA_PUBLIC_FROM_PRIVATE` WeakMap).
  pub mldsa_public: RefCell<Option<v8::Global<v8::Object>>>,
}

impl WebIdlInterfaceConverter for CryptoKey {
  const NAME: &'static str = "CryptoKey";
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CryptoKeyError {
  #[class(type)]
  #[error("Illegal constructor")]
  #[property("code" = "ERR_ILLEGAL_CONSTRUCTOR")]
  IllegalConstructor,
}

// SAFETY: holding v8::Globals that are traced via the GC roots; no raw pointers.
unsafe impl GarbageCollected for CryptoKey {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"CryptoKey"
  }
}

#[op2]
impl CryptoKey {
  #[constructor]
  #[cppgc]
  fn constructor() -> Result<CryptoKey, CryptoKeyError> {
    // CryptoKey can only be constructed internally (via `op_crypto_construct_key`).
    Err(CryptoKeyError::IllegalConstructor)
  }

  // `type` is a JS reserved word, so the Rust getter is named `keyType` and the
  // spec-compliant `type` accessor is aliased onto the prototype in JS.
  #[getter]
  #[string]
  fn key_type(&self) -> String {
    self.key_type.clone()
  }

  #[getter]
  fn extractable(&self) -> bool {
    *self.extractable.borrow()
  }

  #[setter]
  fn extractable(&self, value: bool) {
    *self.extractable.borrow_mut() = value;
  }

  #[setter]
  fn usages(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<v8::Value>,
  ) {
    *self.usages.borrow_mut() = usages_from_v8(scope, value);
  }

  #[getter]
  fn algorithm<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    v8::Local::new(scope, &self.algorithm)
  }

  #[getter]
  fn usages<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Array> {
    let usages = self.usages.borrow();
    let elements = usages
      .iter()
      .map(|u| v8::String::new(scope, u).unwrap().into())
      .collect::<Vec<v8::Local<v8::Value>>>();
    v8::Array::new_with_elements(scope, &elements)
  }

  /// Internal accessor used by the JS export helpers and the Node.js interop:
  /// returns the raw key material (formerly the `KEY_STORE` value).
  #[getter]
  fn key_data<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    v8::Local::new(scope, &self.key_data)
  }

  /// `getPublicKey()` support for ML-DSA private keys: returns the associated
  /// public `CryptoKey`, or `undefined` if none (e.g. ML-KEM, handled in JS).
  #[getter]
  fn mldsa_public_key<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    match self.mldsa_public.borrow().as_ref() {
      Some(p) => v8::Local::new(scope, p).into(),
      None => v8::undefined(scope).into(),
    }
  }
}

/// Construct a `CryptoKey` cppgc object internally. Replaces the JS
/// `constructKey(type, extractable, usages, algorithm, keyData)`.
#[op2]
#[cppgc]
pub fn op_crypto_construct_key(
  scope: &mut v8::PinScope<'_, '_>,
  #[string] key_type: String,
  extractable: bool,
  usages: v8::Local<v8::Value>,
  algorithm: v8::Local<v8::Object>,
  key_data: v8::Local<v8::Value>,
) -> CryptoKey {
  let usages = usages_from_v8(scope, usages);
  CryptoKey {
    key_type,
    extractable: RefCell::new(extractable),
    usages: RefCell::new(usages),
    algorithm: v8::Global::new(scope, algorithm),
    key_data: v8::Global::new(scope, key_data),
    mldsa_public: RefCell::new(None),
  }
}

fn usages_from_v8(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> Vec<String> {
  let mut out = Vec::new();
  if let Ok(arr) = v8::Local::<v8::Array>::try_from(value) {
    for i in 0..arr.length() {
      if let Some(v) = arr.get_index(scope, i) {
        out.push(v.to_rust_string_lossy(scope));
      }
    }
  }
  out
}

// ---------------------------------------------------------------------------
// Helpers for constructing CryptoKey cppgc objects from Rust (used by the
// `importKey` / `generateKey` / `deriveKey` / `unwrapKey` / `encapsulateKey` /
// `decapsulateKey` SubtleCrypto methods in `web_subtle.rs`).
// ---------------------------------------------------------------------------

/// Build the JS `algorithm` dictionary object for a CryptoKey.
///
/// `extras` lets callers attach the algorithm-specific members (e.g. `hash`,
/// `length`, `namedCurve`, `modulusLength`, `publicExponent`).
pub fn alg_object<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  name: &str,
) -> v8::Local<'a, v8::Object> {
  let obj = v8::Object::new(scope);
  set_str(scope, obj, "name", name);
  obj
}

/// Set a string property on an object.
pub fn set_str(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<v8::Object>,
  key: &str,
  value: &str,
) {
  let k = v8::String::new(scope, key).unwrap();
  let v = v8::String::new(scope, value).unwrap();
  obj.set(scope, k.into(), v.into());
}

/// Set a number property on an object.
pub fn set_num(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<v8::Object>,
  key: &str,
  value: f64,
) {
  let k = v8::String::new(scope, key).unwrap();
  let v = v8::Number::new(scope, value);
  obj.set(scope, k.into(), v.into());
}

/// Set an arbitrary value property on an object.
pub fn set_val(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<v8::Object>,
  key: &str,
  value: v8::Local<v8::Value>,
) {
  let k = v8::String::new(scope, key).unwrap();
  obj.set(scope, k.into(), value);
}

/// Build a `{ name, hash: { name } }` dict, optionally with `length`.
pub fn alg_object_hash<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  name: &str,
  hash: &str,
  length: Option<usize>,
) -> v8::Local<'a, v8::Object> {
  let obj = alg_object(scope, name);
  let hash_obj = v8::Object::new(scope);
  set_str(scope, hash_obj, "name", hash);
  set_val(scope, obj, "hash", hash_obj.into());
  if let Some(l) = length {
    set_num(scope, obj, "length", l as f64);
  }
  obj
}

/// Build a Uint8Array from bytes.
pub fn u8_array<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  bytes: &[u8],
) -> v8::Local<'a, v8::Value> {
  let store = if bytes.is_empty() {
    v8::ArrayBuffer::new_backing_store_from_vec(Vec::new()).make_shared()
  } else {
    v8::ArrayBuffer::new_backing_store_from_vec(bytes.to_vec()).make_shared()
  };
  let ab = v8::ArrayBuffer::with_backing_store(scope, &store);
  let len = bytes.len();
  v8::Uint8Array::new(scope, ab, 0, len).unwrap().into()
}

/// Build a `{ type, data }` key-material object (the shape used for
/// AES/HMAC/RSA/EC/ChaCha/HKDF/PBKDF2 keys).
pub fn type_data_value<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  data_type: &str,
  data: &[u8],
) -> v8::Local<'a, v8::Value> {
  let obj = v8::Object::new(scope);
  set_str(scope, obj, "type", data_type);
  let arr = u8_array(scope, data);
  set_val(scope, obj, "data", arr);
  obj.into()
}

/// Construct a `CryptoKey` cppgc object as a `v8::Local`, given the JS
/// `algorithm` object and the raw `key_data` value. `mldsa_public` is the
/// associated public key for ML-DSA private keys (else `None`).
pub fn build_crypto_key<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  key_type: &str,
  extractable: bool,
  usages: Vec<String>,
  algorithm: v8::Local<v8::Object>,
  key_data: v8::Local<v8::Value>,
  mldsa_public: Option<v8::Local<v8::Object>>,
) -> v8::Local<'a, v8::Object> {
  let mldsa_public = mldsa_public.map(|p| v8::Global::new(scope, p));
  let key = CryptoKey {
    key_type: key_type.to_string(),
    extractable: RefCell::new(extractable),
    usages: RefCell::new(usages),
    algorithm: v8::Global::new(scope, algorithm),
    key_data: v8::Global::new(scope, key_data),
    mldsa_public: RefCell::new(mldsa_public),
  };
  make_cppgc_object(scope, key)
}
