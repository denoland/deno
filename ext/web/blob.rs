// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::op2;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::ToJsBuffer;
use serde::Deserialize;
use serde::Serialize;
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

#[derive(Default, Debug)]
pub struct BlobStore {
  parts: Mutex<PartMap>,
  object_urls: Mutex<HashMap<Url, Arc<Blob>>>,
}

impl BlobStore {
  pub fn insert_part(&self, part: Arc<dyn BlobPart + Send + Sync>) -> Uuid {
    let id = Uuid::new_v4();
    let mut parts = self.parts.lock();
    parts.insert(id, part);
    id
  }

  pub fn get_part(&self, id: &Uuid) -> Option<Arc<dyn BlobPart + Send + Sync>> {
    let parts = self.parts.lock();
    let part = parts.get(id);
    part.cloned()
  }

  pub fn remove_part(
    &self,
    id: &Uuid,
  ) -> Option<Arc<dyn BlobPart + Send + Sync>> {
    let mut parts = self.parts.lock();
    parts.remove(id)
  }

  pub fn get_object_url(&self, mut url: Url) -> Option<Arc<Blob>> {
    let blob_store = self.object_urls.lock();
    url.set_fragment(None);
    blob_store.get(&url).cloned()
  }

  pub fn insert_object_url(
    &self,
    blob: Blob,
    maybe_location: Option<Url>,
  ) -> Url {
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

  pub fn remove_object_url(&self, url: &Url) {
    let mut blob_store = self.object_urls.lock();
    blob_store.remove(url);
  }

  pub fn clear(&self) {
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
  async fn read(&self) -> &[u8];
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
  async fn read(&self) -> &[u8] {
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
  async fn read(&self) -> &[u8] {
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
  let blob_store = state.borrow::<Arc<BlobStore>>();
  let part = InMemoryBlobPart(data.to_vec());
  blob_store.insert_part(Arc::new(part))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SliceOptions {
  start: usize,
  len: usize,
}

#[op2]
#[serde]
pub fn op_blob_slice_part(
  state: &mut OpState,
  #[serde] id: Uuid,
  #[serde] options: SliceOptions,
) -> Result<Uuid, BlobError> {
  let blob_store = state.borrow::<Arc<BlobStore>>();
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

#[op2(async)]
#[serde]
pub async fn op_blob_read_part(
  state: Rc<RefCell<OpState>>,
  #[serde] id: Uuid,
) -> Result<ToJsBuffer, BlobError> {
  let part = {
    let state = state.borrow();
    let blob_store = state.borrow::<Arc<BlobStore>>();
    blob_store.get_part(&id)
  }
  .ok_or(BlobError::BlobPartNotFound)?;
  let buf = part.read().await;
  Ok(ToJsBuffer::from(buf.to_vec()))
}

#[op2]
pub fn op_blob_remove_part(state: &mut OpState, #[serde] id: Uuid) {
  let blob_store = state.borrow::<Arc<BlobStore>>();
  blob_store.remove_part(&id);
}

#[op2]
#[string]
pub fn op_blob_create_object_url(
  state: &mut OpState,
  #[string] media_type: String,
  #[serde] part_ids: Vec<Uuid>,
) -> Result<String, BlobError> {
  let mut parts = Vec::with_capacity(part_ids.len());
  let blob_store = state.borrow::<Arc<BlobStore>>();
  for part_id in part_ids {
    let part = blob_store
      .get_part(&part_id)
      .ok_or(BlobError::BlobPartNotFound)?;
    parts.push(part);
  }

  let blob = Blob { media_type, parts };

  let maybe_location = state.try_borrow::<Location>();
  let blob_store = state.borrow::<Arc<BlobStore>>();

  let url = blob_store
    .insert_object_url(blob, maybe_location.map(|location| location.0.clone()));

  Ok(url.to_string())
}

#[op2(fast)]
pub fn op_blob_revoke_object_url(
  state: &mut OpState,
  #[string] url: &str,
) -> Result<(), BlobError> {
  let url = Url::parse(url)?;
  let blob_store = state.borrow::<Arc<BlobStore>>();
  blob_store.remove_object_url(&url);
  Ok(())
}

#[derive(Serialize)]
pub struct ReturnBlob {
  pub media_type: String,
  pub parts: Vec<ReturnBlobPart>,
}

#[derive(Serialize)]
pub struct ReturnBlobPart {
  pub uuid: Uuid,
  pub size: usize,
}

#[op2]
#[serde]
pub fn op_blob_from_object_url(
  state: &mut OpState,
  #[string] url: String,
) -> Result<Option<ReturnBlob>, BlobError> {
  let url = Url::parse(&url)?;
  if url.scheme() != "blob" {
    return Ok(None);
  }

  let blob_store = state
    .try_borrow::<Arc<BlobStore>>()
    .ok_or(BlobError::BlobURLsNotSupported)?;
  if let Some(blob) = blob_store.get_object_url(url) {
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
  } else {
    Ok(None)
  }
}
