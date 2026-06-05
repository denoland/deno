// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::sync::Arc;

use deno_core::OpState;
use deno_core::op2;

use crate::shared::InsertKeyData;
use crate::shared::RawKeyData;
use crate::shared::SharedError;
use crate::shared::StoredKeyData;

/// Rust-side store for WebCrypto key material.
///
/// Historically the key material for every `CryptoKey` lived in a JavaScript
/// `WeakMap` (`KEY_STORE` in `00_crypto.js`) and was serialized and passed to
/// every crypto op. This store holds the key material in Rust instead, keyed by
/// an integer handle. JavaScript keeps only the handle and passes it to ops,
/// which look the key up here, so the serialized key no longer has to cross the
/// JS/Rust boundary on every operation.
///
/// Entries are removed when the corresponding JS handle object is
/// garbage-collected, via a `FinalizationRegistry` that calls
/// `op_crypto_key_store_remove`. This mirrors the GC semantics of the previous
/// `WeakMap`, where the entry lived exactly as long as the handle object.
#[derive(Default)]
pub struct KeyStore {
  keys: HashMap<u32, Arc<RawKeyData>>,
  next_id: u32,
}

impl KeyStore {
  fn insert(&mut self, data: RawKeyData) -> u32 {
    // Find an unused id. `next_id` is monotonic; the wrapping check guards the
    // (practically impossible) case of more than `u32::MAX` live keys.
    let id = loop {
      let id = self.next_id;
      self.next_id = self.next_id.wrapping_add(1);
      if !self.keys.contains_key(&id) {
        break id;
      }
    };
    self.keys.insert(id, Arc::new(data));
    id
  }

  pub fn get(&self, id: u32) -> Option<Arc<RawKeyData>> {
    self.keys.get(&id).cloned()
  }

  fn remove(&mut self, id: u32) {
    self.keys.remove(&id);
  }
}

/// Look up the key material for `handle`, returning a clone of the reference
/// counted entry. Ops that run on a blocking thread can move the returned
/// `Arc` across the thread boundary.
pub fn get_key(
  state: &OpState,
  handle: u32,
) -> Result<Arc<RawKeyData>, SharedError> {
  state
    .borrow::<KeyStore>()
    .get(handle)
    .ok_or(SharedError::InvalidKeyHandle)
}

#[op2]
#[smi]
pub fn op_crypto_key_store_insert(
  state: &mut OpState,
  #[serde] data: InsertKeyData,
) -> u32 {
  state.borrow_mut::<KeyStore>().insert(data.into())
}

#[op2]
pub fn op_crypto_key_store_get(
  state: &OpState,
  #[smi] handle: u32,
) -> Result<StoredKeyData, SharedError> {
  let key = get_key(state, handle)?;
  Ok(key.to_stored_key_data())
}

#[op2(fast)]
pub fn op_crypto_key_store_remove(state: &mut OpState, #[smi] handle: u32) {
  state.borrow_mut::<KeyStore>().remove(handle);
}
