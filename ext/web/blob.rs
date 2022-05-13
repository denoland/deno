use async_trait::async_trait;
use deno_core::error::type_error;
use deno_core::op;

use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_core::ZeroCopyBuf;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::error::AnyError;
use uuid::Uuid;

use crate::Location;

pub type PartMap = HashMap<Uuid, Arc<dyn BlobPart + Send + Sync>>;

#[derive(Clone, Default, Debug)]
pub struct BlobStore {
  parts: Arc<Mutex<PartMap>>,
  object_urls: Arc<Mutex<HashMap<Url, Arc<Blob>>>>,
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

  pub fn get_object_url(
    &self,
    mut url: Url,
  ) -> Result<Option<Arc<Blob>>, AnyError> {
    let blob_store = self.object_urls.lock();
    url.set_fragment(None);
    Ok(blob_store.get(&url).cloned())
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
    let url = Url::parse(&format!("blob:{}/{}", origin, id)).unwrap();

    let mut blob_store = self.object_urls.lock();
    blob_store.insert(url.clone(), Arc::new(blob));

    url
  }

  pub fn remove_object_url(&self, url: &Url) {
    let mut blob_store = self.object_urls.lock();
    blob_store.remove(url);
  }
}

#[derive(Debug)]
pub struct Blob {
  pub media_type: String,

  pub parts: Vec<Arc<dyn BlobPart + Send + Sync>>,
}

impl Blob {
  // TODO(lucacsonato): this should be a stream!
  pub async fn read_all(&self) -> Result<Vec<u8>, AnyError> {
    let size = self.size();
    let mut bytes = Vec::with_capacity(size);

    for part in &self.parts {
      let chunk = part.read().await?;
      bytes.extend_from_slice(chunk);
    }

    assert_eq!(bytes.len(), size);

    Ok(bytes)
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
  async fn read(&self) -> Result<&[u8], AnyError>;
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
  async fn read(&self) -> Result<&[u8], AnyError> {
    Ok(&self.0)
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
  async fn read(&self) -> Result<&[u8], AnyError> {
    let original = self.part.read().await?;
    Ok(&original[self.start..self.start + self.len])
  }

  fn size(&self) -> usize {
    self.len
  }
}

#[op]
pub fn op_blob_create_part(
  state: &mut deno_core::OpState,
  data: ZeroCopyBuf,
) -> Uuid {
  let blob_store = state.borrow::<BlobStore>();
  let part = InMemoryBlobPart(data.to_vec());
  blob_store.insert_part(Arc::new(part))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SliceOptions {
  start: usize,
  len: usize,
}

#[op]
pub fn op_blob_slice_part(
  state: &mut deno_core::OpState,
  id: Uuid,
  options: SliceOptions,
) -> Result<Uuid, AnyError> {
  let blob_store = state.borrow::<BlobStore>();
  let part = blob_store
    .get_part(&id)
    .ok_or_else(|| type_error("Blob part not found"))?;

  let SliceOptions { start, len } = options;

  let size = part.size();
  if start + len > size {
    return Err(type_error(
      "start + len can not be larger than blob part size",
    ));
  }

  let sliced_part = SlicedBlobPart { part, start, len };
  let id = blob_store.insert_part(Arc::new(sliced_part));

  Ok(id)
}

#[op]
pub async fn op_blob_read_part(
  state: Rc<RefCell<deno_core::OpState>>,
  id: Uuid,
) -> Result<ZeroCopyBuf, AnyError> {
  let part = {
    let state = state.borrow();
    let blob_store = state.borrow::<BlobStore>();
    blob_store.get_part(&id)
  }
  .ok_or_else(|| type_error("Blob part not found"))?;
  let buf = part.read().await?;
  Ok(ZeroCopyBuf::from(buf.to_vec()))
}

#[op]
pub fn op_blob_remove_part(state: &mut deno_core::OpState, id: Uuid) {
  let blob_store = state.borrow::<BlobStore>();
  blob_store.remove_part(&id);
}

#[op]
pub fn op_blob_create_object_url(
  state: &mut deno_core::OpState,
  media_type: String,
  part_ids: Vec<Uuid>,
) -> Result<String, AnyError> {
  let mut parts = Vec::with_capacity(part_ids.len());
  let blob_store = state.borrow::<BlobStore>();
  for part_id in part_ids {
    let part = blob_store
      .get_part(&part_id)
      .ok_or_else(|| type_error("Blob part not found"))?;
    parts.push(part);
  }

  let blob = Blob { media_type, parts };

  let maybe_location = state.try_borrow::<Location>();
  let blob_store = state.borrow::<BlobStore>();

  let url = blob_store
    .insert_object_url(blob, maybe_location.map(|location| location.0.clone()));

  Ok(url.to_string())
}

#[op]
pub fn op_blob_revoke_object_url(
  state: &mut deno_core::OpState,
  url: String,
) -> Result<(), AnyError> {
  let url = Url::parse(&url)?;
  let blob_store = state.borrow::<BlobStore>();
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

#[op]
pub fn op_blob_from_object_url(
  state: &mut deno_core::OpState,
  url: String,
) -> Result<Option<ReturnBlob>, AnyError> {
  let url = Url::parse(&url)?;
  if url.scheme() != "blob" {
    return Ok(None);
  }

  let blob_store = state.try_borrow::<BlobStore>().ok_or_else(|| {
    type_error("Blob URLs are not supported in this context.")
  })?;
  if let Some(blob) = blob_store.get_object_url(url)? {
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
