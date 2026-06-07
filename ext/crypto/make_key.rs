// Copyright 2018-2026 the Deno authors. MIT license.

//! Rust-side `constructKey()` analogue.
//!
//! Builds a `CryptoKey` cppgc instance plus the `{ cppgc: CryptoKeyHandle }`
//! handle wrapper object, the algorithm dictionary object, and the frozen
//! `usages` array, and stamps the `webidl.brand` / `kKeyObject` /
//! `hostObjectBrand` symbols the same way the legacy JS `constructKey()`
//! helper did. Used by every `SubtleCrypto` method (import/generate/derive
//! /wrap/unwrap/encapsulate/decapsulate/getPublicKey) now that those bodies
//! live in Rust.

use std::cell::Cell;

use deno_core::cppgc::make_cppgc_object;
use deno_core::v8;

use crate::crypto_key::CryptoKey;
use crate::crypto_key::CryptoKeyType;
use crate::key_store::CryptoKeyHandle;
use crate::shared::RawKeyData;

/// Process-wide cache of the per-isolate symbols the JS `constructKey()`
/// helper used to stamp onto every `CryptoKey`. Populated on the first call
/// to [`stamp_symbols`]; reused on every subsequent call. These are
/// `Symbol.for(...)`-style well-known symbols (or, for `webidl.brand`,
/// fetched once from the loaded webidl ESM), so caching as `Global<Symbol>`
/// is safe across isolates because they always resolve to the same symbol
/// identity per isolate.
pub struct CryptoSymbols {
  pub webidl_brand: v8::Global<v8::Symbol>,
  pub k_key_object: v8::Global<v8::Symbol>,
  pub host_object_brand: v8::Global<v8::Symbol>,
}

thread_local! {
  static SYMBOLS: Cell<Option<&'static CryptoSymbols>> = const { Cell::new(None) };
}

/// Register the three brand symbols that the JS `constructKey()` used to
/// stamp onto every `CryptoKey`. Called once per isolate from JS at module
/// init time (`op_crypto_install_symbols`). The symbols are stored in a
/// thread-local because they are isolate-specific but a `OpState` borrow is
/// not always reachable from inside the cppgc methods that need them (the
/// async dispatcher already holds it mutable).
pub fn set_symbols(symbols: CryptoSymbols) {
  let leaked: &'static CryptoSymbols = Box::leak(Box::new(symbols));
  SYMBOLS.with(|cell| cell.set(Some(leaked)));
}

pub fn symbols<'s>(_scope: &mut v8::PinScope<'s, '_>) -> Option<&'static CryptoSymbols> {
  SYMBOLS.with(|cell| cell.get())
}

/// Convenience: drain the symbols cache. Only intended for the runtime
/// shutdown path used by deno_core's snapshot machinery, where we want to
/// avoid keeping V8 globals alive past isolate teardown.
#[allow(dead_code, reason = "kept for future runtime shutdown wiring")]
pub fn clear_symbols() {
  SYMBOLS.with(|cell| cell.set(None));
}

/// Per-algorithm dictionary slots that get baked into the `CryptoKey`'s
/// `algorithm` v8 object. The set of slots is intentionally minimal: the
/// WebCrypto spec mandates that only `name` is universal; the rest
/// (`length`, `hash`, `namedCurve`, `modulusLength`, `publicExponent`) are
/// per-algorithm.
#[derive(Default)]
pub struct AlgorithmDict {
  pub name: String,
  pub length: Option<u32>,
  pub hash_name: Option<String>,
  pub named_curve: Option<String>,
  pub modulus_length: Option<u32>,
  pub public_exponent: Option<Vec<u8>>,
}

impl AlgorithmDict {
  pub fn new(name: impl Into<String>) -> Self {
    AlgorithmDict {
      name: name.into(),
      ..Default::default()
    }
  }

  pub fn with_length(mut self, length: u32) -> Self {
    self.length = Some(length);
    self
  }

  pub fn with_hash(mut self, hash_name: impl Into<String>) -> Self {
    self.hash_name = Some(hash_name.into());
    self
  }

  pub fn with_named_curve(mut self, curve: impl Into<String>) -> Self {
    self.named_curve = Some(curve.into());
    self
  }

  pub fn with_modulus_length(mut self, modulus_length: u32) -> Self {
    self.modulus_length = Some(modulus_length);
    self
  }

  pub fn with_public_exponent(mut self, e: Vec<u8>) -> Self {
    self.public_exponent = Some(e);
    self
  }
}

pub fn build_algorithm_object<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  dict: &AlgorithmDict,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  set_string(scope, obj, b"name", &dict.name);
  if let Some(length) = dict.length {
    set_u32(scope, obj, b"length", length);
  }
  if let Some(modulus_length) = dict.modulus_length {
    set_u32(scope, obj, b"modulusLength", modulus_length);
  }
  if let Some(ref hash_name) = dict.hash_name {
    let hash_obj = v8::Object::new(scope);
    set_string(scope, hash_obj, b"name", hash_name);
    let key = one_byte_internalized(scope, b"hash");
    obj.set(scope, key.into(), hash_obj.into());
  }
  if let Some(ref curve) = dict.named_curve {
    set_string(scope, obj, b"namedCurve", curve);
  }
  if let Some(ref pe) = dict.public_exponent {
    let backing = if pe.is_empty() {
      v8::ArrayBuffer::new(scope, 0)
    } else {
      let bs = v8::ArrayBuffer::new_backing_store_from_bytes(
        pe.clone().into_boxed_slice(),
      )
      .make_shared();
      v8::ArrayBuffer::with_backing_store(scope, &bs)
    };
    let u8 = v8::Uint8Array::new(scope, backing, 0, pe.len()).unwrap();
    let key = one_byte_internalized(scope, b"publicExponent");
    obj.set(scope, key.into(), u8.into());
  }
  obj
}

pub fn build_usages_array<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  usages: &[&str],
) -> v8::Local<'s, v8::Array> {
  let len = usages.len();
  let arr = v8::Array::new(scope, len as i32);
  for (i, u) in usages.iter().enumerate() {
    let s = v8::String::new(scope, u).unwrap();
    arr.set_index(scope, i as u32, s.into());
  }
  arr
}

pub fn build_handle_object<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  data: RawKeyData,
) -> v8::Local<'s, v8::Object> {
  let handle = make_cppgc_object(scope, CryptoKeyHandle::from_raw(data));
  let wrapper = v8::Object::new(scope);
  let key = one_byte_internalized(scope, b"cppgc");
  wrapper.set(scope, key.into(), handle.into());
  wrapper
}

/// Construct a fully-stamped CryptoKey cppgc instance reachable from JS.
/// Mirrors the legacy JS `constructKey` helper.
#[allow(clippy::too_many_arguments, reason = "dictionary-style key construction")]
pub fn make_crypto_key<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key_type: CryptoKeyType,
  extractable: bool,
  usages: &[&str],
  alg: AlgorithmDict,
  data: RawKeyData,
) -> v8::Local<'s, v8::Object> {
  let handle = build_handle_object(scope, data);
  let algorithm_obj = build_algorithm_object(scope, &alg);
  let usages_arr = build_usages_array(scope, usages);

  let crypto_key = CryptoKey::from_parts(
    scope,
    key_type,
    extractable,
    usages_arr.into(),
    algorithm_obj.into(),
    handle.into(),
  );
  let obj = make_cppgc_object(scope, crypto_key);
  stamp_symbols(scope, obj);
  obj
}

fn stamp_symbols<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: v8::Local<'s, v8::Object>,
) {
  let Some(syms) = symbols(scope) else {
    return;
  };
  let brand = v8::Local::new(scope, &syms.webidl_brand);
  let _ = key.set(scope, brand.into(), brand.into());

  // The other two are read by node:crypto polyfills + structured-clone via
  // the prototype getters installed alongside `op_crypto_install_symbols`,
  // so no per-instance stamping is needed for them.
}

fn set_string<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
  value: &str,
) {
  let k = one_byte_internalized(scope, field);
  let v = v8::String::new(scope, value).unwrap();
  obj.set(scope, k.into(), v.into());
}

fn set_u32<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  field: &[u8],
  value: u32,
) {
  let k = one_byte_internalized(scope, field);
  let n = v8::Number::new(scope, value as f64);
  obj.set(scope, k.into(), n.into());
}

fn one_byte_internalized<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  bytes: &[u8],
) -> v8::Local<'s, v8::String> {
  v8::String::new_from_one_byte(scope, bytes, v8::NewStringType::Internalized)
    .unwrap()
}

/// Body of the `Crypto.registerSymbols(webidlBrand, kKeyObject)` static
/// method. Called once during module load to hand the WebIDL brand symbol
/// (private to the webidl ESM) and the node:crypto `kKeyObject` symbol
/// (private to ext/node) over to the crypto cppgc methods, which need them
/// to brand every freshly-constructed `CryptoKey`. Lives here as a Rust
/// function so it can be reused from a `#[static_method]` on either
/// `Crypto` or `SubtleCrypto` without introducing a new standalone op.
pub fn register_symbols<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  webidl_brand: v8::Local<'s, v8::Value>,
  k_key_object: v8::Local<'s, v8::Value>,
) -> bool {
  let Ok(webidl_brand) = v8::Local::<v8::Symbol>::try_from(webidl_brand) else {
    return false;
  };
  let Ok(k_key_object) = v8::Local::<v8::Symbol>::try_from(k_key_object) else {
    return false;
  };
  let host_obj = {
    let name = v8::String::new(scope, "Deno.core.hostObject").unwrap();
    v8::Symbol::for_key(scope, name)
  };
  set_symbols(CryptoSymbols {
    webidl_brand: v8::Global::new(scope, webidl_brand),
    k_key_object: v8::Global::new(scope, k_key_object),
    host_object_brand: v8::Global::new(scope, host_obj),
  });
  true
}
