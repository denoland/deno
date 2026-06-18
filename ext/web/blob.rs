// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::FromV8;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::ToV8;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use uuid::Uuid;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum BlobError {
  #[class(type)]
  #[error("Blob part not found")]
  BlobPartNotFound,
  #[class(type)]
  #[error("start + len can not be larger than blob part size")]
  SizeLargerThanBlobPart,
  #[class(type)]
  #[error("Blob URLs are not supported in this context")]
  BlobURLsNotSupported,
  #[class(generic)]
  #[error(transparent)]
  Url(#[from] deno_core::url::ParseError),
}

use crate::Location;

pub type PartMap = HashMap<Uuid, Arc<dyn BlobPart + Send + Sync>>;

/// Trait abstracting the blob store, allowing custom implementations
/// (e.g. with memory limits or persistence).
///
/// See [`BlobStore`] for a reference implementation.
pub trait BlobStoreTrait: Debug + Send + Sync {
  fn insert_part(&self, part: Arc<dyn BlobPart + Send + Sync>) -> Uuid;
  fn get_part(&self, id: &Uuid) -> Option<Arc<dyn BlobPart + Send + Sync>>;
  fn remove_part(&self, id: &Uuid) -> Option<Arc<dyn BlobPart + Send + Sync>>;
  fn get_object_url(&self, url: Url) -> Option<Arc<Blob>>;
  fn insert_object_url(&self, blob: Blob, maybe_location: Option<Url>) -> Url;
  fn remove_object_url(&self, url: &Url);
  fn clear(&self);
}

#[derive(Default, Debug)]
pub struct BlobStore {
  parts: Mutex<PartMap>,
  object_urls: Mutex<HashMap<Url, Arc<Blob>>>,
}

impl BlobStore {
  pub fn default_arc() -> Arc<dyn BlobStoreTrait> {
    Arc::new(Self::default())
  }

  pub fn insert_part(&self, part: Arc<dyn BlobPart + Send + Sync>) -> Uuid {
    <Self as BlobStoreTrait>::insert_part(self, part)
  }

  pub fn get_part(&self, id: &Uuid) -> Option<Arc<dyn BlobPart + Send + Sync>> {
    <Self as BlobStoreTrait>::get_part(self, id)
  }

  pub fn remove_part(
    &self,
    id: &Uuid,
  ) -> Option<Arc<dyn BlobPart + Send + Sync>> {
    <Self as BlobStoreTrait>::remove_part(self, id)
  }

  pub fn get_object_url(&self, url: Url) -> Option<Arc<Blob>> {
    <Self as BlobStoreTrait>::get_object_url(self, url)
  }

  pub fn insert_object_url(
    &self,
    blob: Blob,
    maybe_location: Option<Url>,
  ) -> Url {
    <Self as BlobStoreTrait>::insert_object_url(self, blob, maybe_location)
  }

  pub fn remove_object_url(&self, url: &Url) {
    <Self as BlobStoreTrait>::remove_object_url(self, url)
  }

  pub fn clear(&self) {
    <Self as BlobStoreTrait>::clear(self)
  }
}

impl BlobStoreTrait for BlobStore {
  fn insert_part(&self, part: Arc<dyn BlobPart + Send + Sync>) -> Uuid {
    let id = Uuid::new_v4();
    let mut parts = self.parts.lock();
    parts.insert(id, part);
    id
  }

  fn get_part(&self, id: &Uuid) -> Option<Arc<dyn BlobPart + Send + Sync>> {
    let parts = self.parts.lock();
    let part = parts.get(id);
    part.cloned()
  }

  fn remove_part(&self, id: &Uuid) -> Option<Arc<dyn BlobPart + Send + Sync>> {
    let mut parts = self.parts.lock();
    parts.remove(id)
  }

  fn get_object_url(&self, mut url: Url) -> Option<Arc<Blob>> {
    let blob_store = self.object_urls.lock();
    url.set_fragment(None);
    blob_store.get(&url).cloned()
  }

  fn insert_object_url(&self, blob: Blob, maybe_location: Option<Url>) -> Url {
    let origin = if let Some(location) = maybe_location {
      location.origin().ascii_serialization()
    } else {
      "null".to_string()
    };
    let id = Uuid::new_v4();
    let url = Url::parse(&format!("blob:{origin}/{id}")).unwrap();

    let mut blob_store = self.object_urls.lock();
    blob_store.insert(url.clone(), Arc::new(blob));

    url
  }

  fn remove_object_url(&self, url: &Url) {
    let mut blob_store = self.object_urls.lock();
    blob_store.remove(url);
  }

  fn clear(&self) {
    self.parts.lock().clear();
    self.object_urls.lock().clear();
  }
}

#[derive(Debug)]
pub struct Blob {
  pub media_type: String,

  pub parts: Vec<Arc<dyn BlobPart + Send + Sync>>,
}

impl Blob {
  // TODO(lucacsonato): this should be a stream!
  pub async fn read_all(&self) -> Vec<u8> {
    let size = self.size();
    let mut bytes = Vec::with_capacity(size);

    for part in &self.parts {
      let chunk = part.read().await;
      bytes.extend_from_slice(chunk);
    }

    assert_eq!(bytes.len(), size);

    bytes
  }

  fn size(&self) -> usize {
    let mut total = 0;
    for part in &self.parts {
      total += part.size()
    }
    total
  }
}

#[async_trait]
pub trait BlobPart: Debug {
  // TODO(lucacsonato): this should be a stream!
  async fn read<'a>(&'a self) -> &'a [u8];
  fn size(&self) -> usize;
}

#[derive(Debug)]
pub struct InMemoryBlobPart(Vec<u8>);

impl From<Vec<u8>> for InMemoryBlobPart {
  fn from(vec: Vec<u8>) -> Self {
    Self(vec)
  }
}

#[async_trait]
impl BlobPart for InMemoryBlobPart {
  async fn read<'a>(&'a self) -> &'a [u8] {
    &self.0
  }

  fn size(&self) -> usize {
    self.0.len()
  }
}

#[derive(Debug)]
pub struct SlicedBlobPart {
  part: Arc<dyn BlobPart + Send + Sync>,
  start: usize,
  len: usize,
}

#[async_trait]
impl BlobPart for SlicedBlobPart {
  async fn read<'a>(&'a self) -> &'a [u8] {
    let original = self.part.read().await;
    &original[self.start..self.start + self.len]
  }

  fn size(&self) -> usize {
    self.len
  }
}

#[op2]
#[serde]
pub fn op_blob_create_part(
  state: &mut OpState,
  #[buffer] data: JsBuffer,
) -> Uuid {
  let blob_store = state.borrow::<Arc<dyn BlobStoreTrait>>();
  let part = InMemoryBlobPart(data.to_vec());
  blob_store.insert_part(Arc::new(part))
}

#[derive(FromV8)]
pub struct SliceOptions {
  start: usize,
  len: usize,
}

#[op2]
#[serde]
pub fn op_blob_slice_part(
  state: &mut OpState,
  #[serde] id: Uuid,
  #[scoped] options: SliceOptions,
) -> Result<Uuid, BlobError> {
  let blob_store = state.borrow::<Arc<dyn BlobStoreTrait>>();
  let part = blob_store
    .get_part(&id)
    .ok_or(BlobError::BlobPartNotFound)?;

  let SliceOptions { start, len } = options;

  let size = part.size();
  if start + len > size {
    return Err(BlobError::SizeLargerThanBlobPart);
  }

  let sliced_part = SlicedBlobPart { part, start, len };
  let id = blob_store.insert_part(Arc::new(sliced_part));

  Ok(id)
}

#[op2]
pub async fn op_blob_read_part(
  state: Rc<RefCell<OpState>>,
  #[serde] id: Uuid,
) -> Result<Uint8Array, BlobError> {
  let part = {
    let state = state.borrow();
    let blob_store = state.borrow::<Arc<dyn BlobStoreTrait>>();
    blob_store.get_part(&id)
  }
  .ok_or(BlobError::BlobPartNotFound)?;
  let buf = part.read().await;
  Ok(Uint8Array::from(buf.to_vec()))
}

#[op2]
pub fn op_blob_remove_part(state: &mut OpState, #[serde] id: Uuid) {
  let blob_store = state.borrow::<Arc<dyn BlobStoreTrait>>();
  blob_store.remove_part(&id);
}

#[op2]
pub fn op_blob_clone_part(
  state: &mut OpState,
  #[serde] id: Uuid,
) -> Result<ReturnBlobPart, BlobError> {
  let blob_store = state.borrow::<Arc<dyn BlobStoreTrait>>();
  let part = blob_store
    .get_part(&id)
    .ok_or(BlobError::BlobPartNotFound)?;
  let size = part.size();
  let new_id = blob_store.insert_part(part);
  Ok(ReturnBlobPart { uuid: new_id, size })
}

#[op2]
#[string]
pub fn op_blob_create_object_url(
  state: &mut OpState,
  #[string] media_type: String,
  #[serde] part_ids: Vec<Uuid>,
) -> Result<String, BlobError> {
  let mut parts = Vec::with_capacity(part_ids.len());
  let blob_store = state.borrow::<Arc<dyn BlobStoreTrait>>();
  for part_id in part_ids {
    let part = blob_store
      .get_part(&part_id)
      .ok_or(BlobError::BlobPartNotFound)?;
    parts.push(part);
  }

  let blob = Blob { media_type, parts };

  let maybe_location = state.try_borrow::<Location>();
  let blob_store = state.borrow::<Arc<dyn BlobStoreTrait>>();

  let url = blob_store
    .insert_object_url(blob, maybe_location.map(|location| location.0.clone()));

  Ok(url.into())
}

#[op2(fast)]
pub fn op_blob_revoke_object_url(
  state: &mut OpState,
  #[string] url: &str,
) -> Result<(), BlobError> {
  let url = Url::parse(url)?;
  let blob_store = state.borrow::<Arc<dyn BlobStoreTrait>>();
  blob_store.remove_object_url(&url);
  Ok(())
}

#[derive(ToV8)]
pub struct ReturnBlob {
  pub media_type: String,
  pub parts: Vec<ReturnBlobPart>,
}

#[derive(ToV8)]
pub struct ReturnBlobPart {
  #[to_v8(serde)]
  pub uuid: Uuid,
  pub size: usize,
}

#[op2]
pub fn op_blob_from_object_url(
  state: &mut OpState,
  #[string] url: String,
) -> Result<Option<ReturnBlob>, BlobError> {
  let url = Url::parse(&url)?;
  if url.scheme() != "blob" {
    return Ok(None);
  }

  let blob_store = state
    .try_borrow::<Arc<dyn BlobStoreTrait>>()
    .ok_or(BlobError::BlobURLsNotSupported)?;
  match blob_store.get_object_url(url) {
    Some(blob) => {
      let parts = blob
        .parts
        .iter()
        .map(|part| ReturnBlobPart {
          uuid: blob_store.insert_part(part.clone()),
          size: part.size(),
        })
        .collect();
      Ok(Some(ReturnBlob {
        media_type: blob.media_type.clone(),
        parts,
      }))
    }
    _ => Ok(None),
  }
}

#[cfg(test)]
mod tests {
  use std::sync::atomic::AtomicUsize;
  use std::sync::atomic::Ordering;

  use deno_core::OpState;

  use super::*;

  /// A counting wrapper around `BlobStore` that increments a counter
  /// on every `insert_part` call. Used to prove that a custom impl
  /// plumbed through `deno_web::init()` is actually invoked by ops.
  #[derive(Debug)]
  struct CountingBlobStore {
    inner: BlobStore,
    insert_count: AtomicUsize,
  }

  impl Default for CountingBlobStore {
    fn default() -> Self {
      Self {
        inner: BlobStore::default(),
        insert_count: AtomicUsize::new(0),
      }
    }
  }

  impl BlobStoreTrait for CountingBlobStore {
    fn insert_part(&self, part: Arc<dyn BlobPart + Send + Sync>) -> Uuid {
      self.insert_count.fetch_add(1, Ordering::SeqCst);
      self.inner.insert_part(part)
    }
    fn get_part(&self, id: &Uuid) -> Option<Arc<dyn BlobPart + Send + Sync>> {
      self.inner.get_part(id)
    }
    fn remove_part(
      &self,
      id: &Uuid,
    ) -> Option<Arc<dyn BlobPart + Send + Sync>> {
      self.inner.remove_part(id)
    }
    fn get_object_url(&self, url: Url) -> Option<Arc<Blob>> {
      self.inner.get_object_url(url)
    }
    fn insert_object_url(
      &self,
      blob: Blob,
      maybe_location: Option<Url>,
    ) -> Url {
      self.inner.insert_object_url(blob, maybe_location)
    }
    fn remove_object_url(&self, url: &Url) {
      self.inner.remove_object_url(url)
    }
    fn clear(&self) {
      self.inner.clear()
    }
  }

  /// Verify that a custom `BlobStoreTrait` impl passed through
  /// `deno_web::init()` ends up in `OpState` and is the same
  /// instance the ops would use.
  #[test]
  fn custom_blob_store_through_init() {
    let counting: Arc<CountingBlobStore> =
      Arc::new(CountingBlobStore::default());
    let blob_store: Arc<dyn BlobStoreTrait> = counting.clone();

    // Build the extension the same way the runtime does.
    let ext = crate::deno_web::init(
      blob_store,
      None,
      Default::default(),
      Default::default(),
    );

    // Run the extension's state initializer on a fresh OpState.
    let mut op_state = OpState::new(None);
    let state_fn = ext
      .op_state_fn
      .expect("deno_web extension should have an op_state_fn");
    state_fn(&mut op_state);

    // Extract the store from OpState — same code path as the ops.
    let store = op_state.borrow::<Arc<dyn BlobStoreTrait>>();

    // A trivial in-memory BlobPart for testing.
    #[derive(Debug)]
    struct MemPart(Vec<u8>);

    #[async_trait]
    impl BlobPart for MemPart {
      async fn read<'a>(&'a self) -> &'a [u8] {
        &self.0
      }
      fn size(&self) -> usize {
        self.0.len()
      }
    }

    assert_eq!(counting.insert_count.load(Ordering::SeqCst), 0);

    let part: Arc<dyn BlobPart + Send + Sync> =
      Arc::new(MemPart(b"hello".to_vec()));
    let id = store.insert_part(part);

    // The counter proves our custom impl was called, not the default.
    assert_eq!(counting.insert_count.load(Ordering::SeqCst), 1);

    // Basic round-trip still works.
    let retrieved = store.get_part(&id).expect("part should exist");
    assert_eq!(retrieved.size(), 5);

    assert!(store.remove_part(&id).is_some());
    assert!(store.get_part(&id).is_none());
  }
}
