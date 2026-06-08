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

pub fn symbols<'s>(
  _scope: &mut v8::PinScope<'s, '_>,
) -> Option<&'static CryptoSymbols> {
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
    hash_obj
      .set_integrity_level(scope, v8::IntegrityLevel::Frozen)
      .unwrap();
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
    .set_integrity_level(scope, v8::IntegrityLevel::Frozen)
    .unwrap();
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
  let obj: v8::Local<v8::Object> = arr.into();
  obj
    .set_integrity_level(scope, v8::IntegrityLevel::Frozen)
    .unwrap();
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
#[allow(
  clippy::too_many_arguments,
  reason = "dictionary-style key construction"
)]
pub fn make_crypto_key<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key_type: CryptoKeyType,
  extractable: bool,
  usages: &[&str],
  alg: AlgorithmDict,
  data: RawKeyData,
) -> v8::Local<'s, v8::Object> {
  let key_data_jsval = key_data_to_jsval(scope, &data);
  let host_object_snapshot = build_host_object_snapshot(
    scope,
    key_type,
    extractable,
    usages,
    &alg,
    &data,
  );
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
  stamp_symbols(scope, obj, key_data_jsval, host_object_snapshot);
  obj
}

fn stamp_symbols<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: v8::Local<'s, v8::Object>,
  k_key_object_val: v8::Local<'s, v8::Value>,
  host_object_snapshot: v8::Local<'s, v8::Value>,
) {
  let Some(syms) = symbols(scope) else {
    return;
  };
  // The brand symbols must be non-enumerable to match the legacy JS
  // `ObjectDefineProperty(key, sym, { value: ... })` shape. Without
  // `DONT_ENUM` the cppgc instance exposes them as plain own properties,
  // which breaks `assert.deepStrictEqual` of two distinct keys with the
  // same material (test/parallel/test-assert-deep.js Crypto subtest) and
  // shows up as `[Symbol(...)]` slots in `Deno.inspect()` output.
  let brand = v8::Local::new(scope, &syms.webidl_brand);
  let _ = key.define_own_property(
    scope,
    brand.into(),
    brand.into(),
    v8::PropertyAttribute::DONT_ENUM,
  );

  let k_key_object = v8::Local::new(scope, &syms.k_key_object);
  let _ = key.define_own_property(
    scope,
    k_key_object.into(),
    k_key_object_val,
    v8::PropertyAttribute::DONT_ENUM,
  );

  // The hostObjectBrand is a function-valued property that the
  // structured-clone serializer calls; replicate the legacy JS shape.
  let host_brand_sym = v8::Local::new(scope, &syms.host_object_brand);
  // The legacy JS used `ObjectDefineProperty(key, hostObjectBrand, { value:
  // () => snapshot })` -- a closure-bound function. From Rust the same
  // shape is built via a FunctionTemplate whose `data` slot carries the
  // snapshot and whose body returns it.
  let ft = v8::FunctionTemplate::builder(host_object_thunk)
    .data(host_object_snapshot)
    .build(scope);
  let host_fn = ft.get_function(scope).unwrap();
  let _ = key.define_own_property(
    scope,
    host_brand_sym.into(),
    host_fn.into(),
    v8::PropertyAttribute::DONT_ENUM,
  );
}

fn host_object_thunk(
  _scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  rv.set(args.data());
}

/// Build the `{ type: "CryptoKey", keyType, extractable, usages, algorithm,
/// keyData }` snapshot the legacy `hostObjectBrand` getter returned.
fn build_host_object_snapshot<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key_type: CryptoKeyType,
  extractable: bool,
  usages: &[&str],
  alg: &AlgorithmDict,
  data: &RawKeyData,
) -> v8::Local<'s, v8::Value> {
  let obj = v8::Object::new(scope);
  let type_key = one_byte_internalized(scope, b"type");
  let type_val = v8::String::new(scope, "CryptoKey").unwrap();
  obj.set(scope, type_key.into(), type_val.into());

  let key_type_key = one_byte_internalized(scope, b"keyType");
  let key_type_val = v8::String::new(scope, key_type_str(key_type)).unwrap();
  obj.set(scope, key_type_key.into(), key_type_val.into());

  let ext_key = one_byte_internalized(scope, b"extractable");
  let ext_val = v8::Boolean::new(scope, extractable);
  obj.set(scope, ext_key.into(), ext_val.into());

  let usages_key = one_byte_internalized(scope, b"usages");
  let usages_val = build_usages_array(scope, usages);
  obj.set(scope, usages_key.into(), usages_val.into());

  let alg_key = one_byte_internalized(scope, b"algorithm");
  let alg_val = build_algorithm_object(scope, alg);
  obj.set(scope, alg_key.into(), alg_val.into());

  let kd_key = one_byte_internalized(scope, b"keyData");
  let kd_val = key_data_to_jsval(scope, data);
  obj.set(scope, kd_key.into(), kd_val);

  obj.into()
}

fn key_type_str(t: CryptoKeyType) -> &'static str {
  match t {
    CryptoKeyType::Public => "public",
    CryptoKeyType::Private => "private",
    CryptoKeyType::Secret => "secret",
  }
}

/// Reconstruct the `getKeyData(handle)` JS shape from raw key data.
/// `RawKeyData::Raw` returns a bare `Uint8Array`; `Secret`/`Private`/
/// `Public` return `{ type, data }`; `SeededPrivate` returns
/// `{ seed, privateKey }`.
fn key_data_to_jsval<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  data: &RawKeyData,
) -> v8::Local<'s, v8::Value> {
  fn u8a<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    bytes: &[u8],
  ) -> v8::Local<'s, v8::Uint8Array> {
    let backing = if bytes.is_empty() {
      v8::ArrayBuffer::new(scope, 0)
    } else {
      let bs = v8::ArrayBuffer::new_backing_store_from_bytes(
        bytes.to_vec().into_boxed_slice(),
      )
      .make_shared();
      v8::ArrayBuffer::with_backing_store(scope, &bs)
    };
    v8::Uint8Array::new(scope, backing, 0, bytes.len()).unwrap()
  }
  match data {
    RawKeyData::Raw(b) => u8a(scope, b).into(),
    RawKeyData::Secret(b) => tagged(scope, "secret", b),
    RawKeyData::Private(b) => tagged(scope, "private", b),
    RawKeyData::Public(b) => tagged(scope, "public", b),
    RawKeyData::SeededPrivate { seed, private_key } => {
      let obj = v8::Object::new(scope);
      let pk_key = one_byte_internalized(scope, b"privateKey");
      let pk_arr = u8a(scope, private_key);
      obj.set(scope, pk_key.into(), pk_arr.into());
      if let Some(seed) = seed {
        let seed_key = one_byte_internalized(scope, b"seed");
        let seed_arr = u8a(scope, seed);
        obj.set(scope, seed_key.into(), seed_arr.into());
      }
      obj.into()
    }
  }
}

fn tagged<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  kind: &str,
  bytes: &[u8],
) -> v8::Local<'s, v8::Value> {
  let obj = v8::Object::new(scope);
  let type_key = one_byte_internalized(scope, b"type");
  let type_val = v8::String::new(scope, kind).unwrap();
  obj.set(scope, type_key.into(), type_val.into());
  let data_key = one_byte_internalized(scope, b"data");
  let backing = if bytes.is_empty() {
    v8::ArrayBuffer::new(scope, 0)
  } else {
    let bs = v8::ArrayBuffer::new_backing_store_from_bytes(
      bytes.to_vec().into_boxed_slice(),
    )
    .make_shared();
    v8::ArrayBuffer::with_backing_store(scope, &bs)
  };
  let data_arr = v8::Uint8Array::new(scope, backing, 0, bytes.len()).unwrap();
  obj.set(scope, data_key.into(), data_arr.into());
  obj.into()
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
