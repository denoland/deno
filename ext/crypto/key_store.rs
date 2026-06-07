// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::CStr;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8;

use crate::shared::InsertKeyData;
use crate::shared::RawKeyData;
use crate::shared::StoredKeyData;

/// V8 garbage-collected wrapper around WebCrypto key material.
///
/// Historically the key material for every `CryptoKey` lived in a JavaScript
/// `WeakMap` (`KEY_STORE` in `00_crypto.js`) and was serialized and passed to
/// every crypto op. Instead, the key material now lives in Rust inside this
/// cppgc object, which JavaScript stores as the `CryptoKey`'s handle and passes
/// to ops - so the serialized key no longer has to cross the JS/Rust boundary on
/// every operation.
///
/// Because the handle is a cppgc object, the key material is freed
/// automatically by V8's garbage collector once the handle is collected (i.e.
/// once no `CryptoKey` references it). No `FinalizationRegistry` or manual
/// bookkeeping is required.
pub struct CryptoKeyHandle {
  data: RawKeyData,
}

impl CryptoKeyHandle {
  pub fn data(&self) -> &RawKeyData {
    &self.data
  }

  /// Construct directly from Rust-side raw key data. Used by the
  /// Rust-native [`crate::make_key::make_crypto_key`] helper that
  /// replaces the legacy JS `constructKey` shim.
  pub fn from_raw(data: RawKeyData) -> Self {
    Self { data }
  }
}

// SAFETY: `CryptoKeyHandle` only owns plain key bytes and holds no references
// into the V8 heap, so it is safe to garbage collect.
unsafe impl GarbageCollected for CryptoKeyHandle {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static CStr {
    c"CryptoKeyHandle"
  }
}

/// Wrap key material in a cppgc handle and return it to JavaScript.
#[op2]
#[cppgc]
pub fn op_crypto_key_store_insert(
  #[serde] data: InsertKeyData,
) -> CryptoKeyHandle {
  CryptoKeyHandle { data: data.into() }
}

/// Read key material back out of a handle, for key export, structured clone,
/// node:crypto interop, and the ops that still take key bytes directly.
#[op2]
pub fn op_crypto_key_store_get(
  #[cppgc] handle: &CryptoKeyHandle,
) -> StoredKeyData {
  handle.data.to_stored_key_data()
}
