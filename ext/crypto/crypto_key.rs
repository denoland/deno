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
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;

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
  Ok(CryptoKey {
    key_type: CryptoKeyType::parse(key_type)?,
    extractable,
    usages: v8::Global::new(scope, usages),
    algorithm: v8::Global::new(scope, algorithm),
    handle: v8::Global::new(scope, handle),
  })
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
