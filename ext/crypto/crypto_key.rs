// Copyright 2018-2026 the Deno authors. MIT license.

//! WebCrypto `CryptoKey` as a cppgc-wrapped Rust object.
//!
//! The class identity (`CryptoKey`, `CryptoKey.prototype`) and the per-key
//! state -- `type`, `extractable`, `algorithm`, `usages`, and the internal
//! handle pointing at the key material in [`crate::key_store`] -- all live
//! on this Rust struct. The JS shim only attaches the inspector hook and
//! the structured-clone `hostObjectBrand` to the prototype/instance.

use std::ffi::CStr;

use deno_core::GarbageCollected;
use deno_core::cppgc::UnsafePtr;
use deno_core::cppgc::try_unwrap_cppgc_object;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;

use crate::key_store::CryptoKeyHandle;
use crate::shared::SharedError;

/// `CryptoKey.type` — the lowercase string returned by the `type` getter.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CryptoKeyType {
  Public,
  Private,
  Secret,
}

impl CryptoKeyType {
  fn as_str(self) -> &'static str {
    match self {
      Self::Public => "public",
      Self::Private => "private",
      Self::Secret => "secret",
    }
  }

  fn parse(s: &str) -> Result<Self, SharedError> {
    Ok(match s {
      "public" => Self::Public,
      "private" => Self::Private,
      "secret" => Self::Secret,
      _ => return Err(SharedError::InvalidKeyType),
    })
  }
}

pub struct CryptoKey {
  key_type: CryptoKeyType,
  extractable: bool,
  /// The frozen-array of `KeyUsage` strings returned by the `usages` getter.
  /// Stored as `v8::Global` so the getter satisfies `SameObject` per spec.
  usages: v8::Global<v8::Value>,
  /// The algorithm dictionary returned by the `algorithm` getter (also
  /// `SameObject`).
  algorithm: v8::Global<v8::Value>,
  /// Opaque, JS-side wrapper carrying a cppgc-tracked
  /// [`crate::key_store::CryptoKeyHandle`] on its `cppgc` property. Held by
  /// reference so two `CryptoKey`s representing the two halves of a key pair
  /// can share the same underlying key material.
  handle: v8::Global<v8::Value>,
}

impl WebIdlInterfaceConverter for CryptoKey {
  const NAME: &'static str = "CryptoKey";
}

// SAFETY: All v8 values are stored in `v8::Global`, which is its own
// strong root for V8's GC; cppgc has no Rust-side fields to trace.
unsafe impl GarbageCollected for CryptoKey {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static CStr {
    c"CryptoKey"
  }
}

#[op2]
impl CryptoKey {
  /// `new CryptoKey()` is illegal per the WebCrypto spec.
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<CryptoKey, SharedError> {
    Err(SharedError::IllegalConstructor)
  }

  #[getter]
  #[string]
  #[rename("type")]
  fn r#type(&self) -> &'static str {
    self.key_type.as_str()
  }

  #[getter]
  fn extractable(&self) -> bool {
    self.extractable
  }

  #[getter]
  fn usages<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> v8::Local<'s, v8::Value> {
    v8::Local::new(scope, &self.usages)
  }

  #[getter]
  fn algorithm<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> v8::Local<'s, v8::Value> {
    v8::Local::new(scope, &self.algorithm)
  }

  /// Internal `CryptoKey.exportNodeMaterial(key)` — used by the
  /// `ext/node/polyfills/internal/crypto/keys.ts` interop bridge.
  /// Returns `{ type, data: Uint8Array }`.
  #[rename("exportNodeMaterial")]
  #[required(1)]
  #[static_method]
  fn export_node_material<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    key: v8::Local<'s, v8::Value>,
  ) -> Result<v8::Local<'s, v8::Value>, crate::CryptoError> {
    crate::node_interop::export_node_key_material(scope, key)
  }

  /// Internal `CryptoKey.importSync(format, keyData, algorithm,
  /// extractable, usages)` — synchronous version of
  /// `SubtleCrypto.importKey` for `ext/node/polyfills/internal/crypto/keys.ts`.
  #[rename("importSync")]
  #[required(5)]
  #[static_method]
  fn import_sync<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    #[webidl] format: crate::subtle_export_key::KeyFormat,
    key_data: v8::Local<'s, v8::Value>,
    #[webidl] algorithm: crate::subtle_import_key::ImportAlgorithm,
    extractable: bool,
    #[webidl] usages: Vec<String>,
  ) -> Result<v8::Local<'s, v8::Object>, crate::CryptoError> {
    crate::node_interop::import_sync(
      scope,
      format,
      key_data,
      algorithm,
      extractable,
      usages,
    )
  }
}

#[allow(
  dead_code,
  reason = "wired up incrementally as each SubtleCrypto method moves into Rust"
)]
impl CryptoKey {
  pub fn key_type(&self) -> CryptoKeyType {
    self.key_type
  }

  pub fn extractable_(&self) -> bool {
    self.extractable
  }

  /// Snapshot the `algorithm` dictionary's `.name` field as a Rust String.
  /// Returns `None` if the slot doesn't expose a string-coercible `name`,
  /// which on a spec-conformant `CryptoKey` cannot happen (the prototype
  /// `algorithm` getter always yields a frozen object with a string `name`).
  pub fn algorithm_name<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> Option<String> {
    let alg = v8::Local::new(scope, &self.algorithm);
    let obj = v8::Local::<v8::Object>::try_from(alg).ok()?;
    let key = v8::String::new_from_one_byte(
      scope,
      b"name",
      v8::NewStringType::Internalized,
    )?;
    let val = obj.get(scope, key.into())?;
    let s = val.to_string(scope)?;
    Some(s.to_rust_string_lossy(scope))
  }

  /// Membership test for the `usages` frozen array. Spec callers use this
  /// to enforce the "valid usage" InvalidAccessError throw on every method
  /// that takes a key. Returns `false` if the slot isn't an array (which
  /// can't happen on a spec-conformant instance).
  pub fn has_usage<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    usage: &str,
  ) -> bool {
    let Some(usages) = self.usages_as_vec(scope) else {
      return false;
    };
    usages.iter().any(|u| u == usage)
  }

  /// Materialize the `usages` frozen array as a Rust `Vec<String>`. Returns
  /// `None` if the slot has been tampered with from JS, which can't happen
  /// on a spec-conformant instance. Used by the [`SubtleKey`] converter so
  /// every method that takes a key can validate its usage list in plain
  /// Rust, off the v8 stack.
  ///
  /// [`SubtleKey`]: crate::subtle_key::SubtleKey
  pub fn usages_as_vec<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> Option<Vec<String>> {
    let usages = v8::Local::new(scope, &self.usages);
    let arr = v8::Local::<v8::Array>::try_from(usages).ok()?;
    let len = arr.length();
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len {
      let item = arr.get_index(scope, i)?;
      let s = item.to_string(scope)?;
      out.push(s.to_rust_string_lossy(scope));
    }
    Some(out)
  }

  /// Borrow the algorithm dictionary as a v8 `Object`. Returns `None` when
  /// the slot has been replaced from JS with a non-object, which can't
  /// happen on a spec-conformant instance.
  pub fn algorithm_local<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> Option<v8::Local<'s, v8::Object>> {
    let alg = v8::Local::new(scope, &self.algorithm);
    v8::Local::<v8::Object>::try_from(alg).ok()
  }

  /// Unwrap the cppgc-tracked [`CryptoKeyHandle`] hidden behind the JS-side
  /// `{ cppgc: CryptoKeyHandle }` handle object stored on this `CryptoKey`.
  /// Returns `None` only if the handle slot has been tampered with from
  /// JS, in which case the caller should propagate that as an
  /// `InvalidAccessError` via [`SharedError::InvalidKeyHandle`].
  pub fn key_handle<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> Option<UnsafePtr<CryptoKeyHandle>> {
    let handle = v8::Local::new(scope, &self.handle);
    let obj = v8::Local::<v8::Object>::try_from(handle).ok()?;
    let key = v8::String::new_from_one_byte(
      scope,
      b"cppgc",
      v8::NewStringType::Internalized,
    )?;
    let cppgc_val = obj.get(scope, key.into())?;
    try_unwrap_cppgc_object::<CryptoKeyHandle>(scope, cppgc_val)
  }
}

/// Construct a `CryptoKey`. Called from the JS shim as the Rust analogue of
/// the old `constructKey(type, extractable, usages, algorithm, handle)`
/// helper.
#[op2]
#[cppgc]
pub fn op_create_crypto_key(
  scope: &mut v8::PinScope<'_, '_>,
  #[string] key_type: &str,
  extractable: bool,
  usages: v8::Local<v8::Value>,
  algorithm: v8::Local<v8::Value>,
  handle: v8::Local<v8::Value>,
) -> Result<CryptoKey, SharedError> {
  Ok(CryptoKey::from_parts(
    scope,
    CryptoKeyType::parse(key_type)?,
    extractable,
    usages,
    algorithm,
    handle,
  ))
}

impl CryptoKey {
  /// Construct a `CryptoKey` from its slot values. Used by both
  /// `op_create_crypto_key` (the JS-facing op) and the Rust-native
  /// `make_crypto_key` helper that replaces the legacy JS `constructKey`.
  pub fn from_parts(
    scope: &mut v8::PinScope<'_, '_>,
    key_type: CryptoKeyType,
    extractable: bool,
    usages: v8::Local<v8::Value>,
    algorithm: v8::Local<v8::Value>,
    handle: v8::Local<v8::Value>,
  ) -> Self {
    Self {
      key_type,
      extractable,
      usages: v8::Global::new(scope, usages),
      algorithm: v8::Global::new(scope, algorithm),
      handle: v8::Global::new(scope, handle),
    }
  }
}

/// Internal accessor used by JS code that still needs the opaque handle
/// (the `{ cppgc }` wrapper) to pass to ops in `lib.rs`. Will go away as
/// each consumer is lifted onto the Rust impl block of `SubtleCrypto`.
#[op2]
pub fn op_crypto_key_handle<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  #[cppgc] key: &CryptoKey,
) -> v8::Local<'s, v8::Value> {
  v8::Local::new(scope, &key.handle)
}
