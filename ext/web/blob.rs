// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::FromV8;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
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
  #[class("DOMExceptionNotReadableError")]
  #[error("The blob could not be read")]
  FileRead(#[from] std::io::Error),
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
  pub async fn read_all(&self) -> Result<Vec<u8>, BlobError> {
    if let [part] = self.parts.as_slice() {
      return Ok(part.read_range(0, part.size()).await?.into_owned());
    }
    let size = self.size();
    let mut bytes = Vec::with_capacity(size);

    for part in &self.parts {
      let chunk = part.read_range(0, part.size()).await?;
      bytes.extend_from_slice(&chunk);
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
  /// Read exactly this range, without reading any preceding data. In-memory
  /// parts can lend their bytes; file-backed parts allocate only the range.
  async fn read_range(
    &self,
    start: usize,
    len: usize,
  ) -> Result<Cow<'_, [u8]>, BlobError>;
  /// Open reader-scoped storage when necessary. Memory parts need no resource.
  async fn open_reader(
    &self,
  ) -> Result<Option<Arc<dyn BlobPart + Send + Sync>>, BlobError> {
    Ok(None)
  }
  fn size(&self) -> usize;
}

fn check_range(start: usize, len: usize, size: usize) -> Result<(), BlobError> {
  if start > size || len > size - start {
    return Err(BlobError::SizeLargerThanBlobPart);
  }
  Ok(())
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
  async fn read_range(
    &self,
    start: usize,
    len: usize,
  ) -> Result<Cow<'_, [u8]>, BlobError> {
    check_range(start, len, self.size())?;
    Ok(Cow::Borrowed(&self.0[start..start + len]))
  }

  fn size(&self) -> usize {
    self.0.len()
  }
}

/// Opens a file through the embedder's filesystem and permission checks.
pub trait BlobFileSource: Debug + Send + Sync {
  fn open(&self) -> std::io::Result<std::fs::File>;
}

/// A file snapshot whose descriptor is opened only while a reader is active.
#[derive(Debug)]
pub struct FileBlobPart(FileBlobStorage);

#[derive(Debug)]
enum FileBlobStorage {
  Native {
    source: Arc<dyn BlobFileSource>,
    metadata: std::fs::Metadata,
  },
  Unreadable(usize),
}

impl FileBlobPart {
  pub fn new(
    source: Arc<dyn BlobFileSource>,
    file: &std::fs::File,
  ) -> std::io::Result<Self> {
    let metadata = file.metadata()?;
    usize::try_from(metadata.len()).map_err(std::io::Error::other)?;
    Ok(Self(FileBlobStorage::Native { source, metadata }))
  }

  /// Match metadata-only blobs (such as directories) that fail when read.
  pub fn unreadable(size: usize) -> Self {
    Self(FileBlobStorage::Unreadable(size))
  }

  async fn open_file(&self) -> Result<OpenFileBlobPart, BlobError> {
    let FileBlobStorage::Native { source, metadata } = &self.0 else {
      return Err(BlobError::FileRead(std::io::ErrorKind::IsADirectory.into()));
    };
    let source = source.clone();
    let metadata = metadata.clone();
    let (file, metadata) = deno_core::unsync::spawn_blocking(move || {
      let file = source.open()?;
      OpenFileBlobPart::check_metadata(&file, &metadata)?;
      Ok::<_, std::io::Error>((file, metadata))
    })
    .await
    .map_err(std::io::Error::other)??;
    Ok(OpenFileBlobPart {
      file: Arc::new(file),
      metadata,
    })
  }
}

#[async_trait]
impl BlobPart for FileBlobPart {
  async fn read_range(
    &self,
    start: usize,
    len: usize,
  ) -> Result<Cow<'_, [u8]>, BlobError> {
    let reader = self.open_file().await?;
    Ok(Cow::Owned(
      reader.read_range(start, len).await?.into_owned(),
    ))
  }

  async fn open_reader(
    &self,
  ) -> Result<Option<Arc<dyn BlobPart + Send + Sync>>, BlobError> {
    Ok(Some(Arc::new(self.open_file().await?)))
  }

  fn size(&self) -> usize {
    match &self.0 {
      FileBlobStorage::Native { metadata, .. } => metadata.len() as usize,
      FileBlobStorage::Unreadable(size) => *size,
    }
  }
}

#[derive(Debug)]
struct OpenFileBlobPart {
  file: Arc<std::fs::File>,
  metadata: std::fs::Metadata,
}

impl OpenFileBlobPart {
  fn check_metadata(
    file: &std::fs::File,
    original: &std::fs::Metadata,
  ) -> std::io::Result<()> {
    let current = file.metadata()?;
    if current.len() != original.len()
      || current.modified()? != original.modified()?
      || current.created().ok() != original.created().ok()
    {
      return Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "File changed after the blob was created",
      ));
    }
    #[cfg(unix)]
    {
      use std::os::unix::fs::MetadataExt;
      if current.dev() != original.dev() || current.ino() != original.ino() {
        return Err(std::io::Error::new(
          std::io::ErrorKind::InvalidData,
          "File replaced after the blob was created",
        ));
      }
    }
    Ok(())
  }
}

#[async_trait]
impl BlobPart for OpenFileBlobPart {
  async fn read_range(
    &self,
    start: usize,
    len: usize,
  ) -> Result<Cow<'_, [u8]>, BlobError> {
    check_range(start, len, self.size())?;
    let file = self.file.clone();
    let metadata = self.metadata.clone();
    let bytes = deno_core::unsync::spawn_blocking(move || {
      Self::check_metadata(&file, &metadata)?;
      let mut bytes = vec![0; len];
      let mut read = 0;
      while read < len {
        let position = (start + read) as u64;
        #[cfg(unix)]
        let result = {
          use std::os::unix::fs::FileExt;
          file.read_at(&mut bytes[read..], position)
        };
        #[cfg(windows)]
        let result = {
          use std::os::windows::fs::FileExt;
          file.seek_read(&mut bytes[read..], position)
        };
        #[cfg(not(any(unix, windows)))]
        let result: std::io::Result<usize> = {
          let _ = position;
          Err(std::io::ErrorKind::Unsupported.into())
        };
        match result {
          Ok(0) => return Err(std::io::ErrorKind::UnexpectedEof.into()),
          Ok(n) => read += n,
          Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
          Err(err) => return Err(err),
        }
      }
      Self::check_metadata(&file, &metadata)?;
      Ok(bytes)
    })
    .await
    .map_err(std::io::Error::other)??;
    Ok(Cow::Owned(bytes))
  }

  fn size(&self) -> usize {
    self.metadata.len() as usize
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
  async fn read_range(
    &self,
    start: usize,
    len: usize,
  ) -> Result<Cow<'_, [u8]>, BlobError> {
    check_range(start, len, self.len)?;
    self.part.read_range(self.start + start, len).await
  }

  async fn open_reader(
    &self,
  ) -> Result<Option<Arc<dyn BlobPart + Send + Sync>>, BlobError> {
    Ok(self.part.open_reader().await?.map(|part| {
      Arc::new(Self {
        part,
        start: self.start,
        len: self.len,
      }) as Arc<dyn BlobPart + Send + Sync>
    }))
  }

  fn size(&self) -> usize {
    self.len
  }
}

#[op2]
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
pub fn op_blob_slice_part(
  state: &mut OpState,
  #[scoped] id: Uuid,
  #[scoped] options: SliceOptions,
) -> Result<Uuid, BlobError> {
  let blob_store = state.borrow::<Arc<dyn BlobStoreTrait>>();
  let part = blob_store
    .get_part(&id)
    .ok_or(BlobError::BlobPartNotFound)?;

  let SliceOptions { start, len } = options;

  let size = part.size();
  check_range(start, len, size)?;

  let sliced_part = SlicedBlobPart { part, start, len };
  let id = blob_store.insert_part(Arc::new(sliced_part));

  Ok(id)
}

#[op2]
pub async fn op_blob_read_part(
  state: Rc<RefCell<OpState>>,
  #[scoped] id: Uuid,
  #[scoped] options: SliceOptions,
) -> Result<Uint8Array, BlobError> {
  let part = {
    let state = state.borrow();
    let blob_store = state.borrow::<Arc<dyn BlobStoreTrait>>();
    blob_store.get_part(&id)
  }
  .ok_or(BlobError::BlobPartNotFound)?;
  let buf = part.read_range(options.start, options.len).await?;
  Ok(Uint8Array::from(buf.into_owned()))
}

struct BlobReader(Arc<dyn BlobPart + Send + Sync>);

impl Resource for BlobReader {
  fn name(&self) -> Cow<'_, str> {
    "blobReader".into()
  }
}

#[op2]
#[smi]
pub async fn op_blob_open_reader(
  state: Rc<RefCell<OpState>>,
  #[scoped] id: Uuid,
) -> Result<Option<ResourceId>, BlobError> {
  let part = state
    .borrow()
    .borrow::<Arc<dyn BlobStoreTrait>>()
    .get_part(&id)
    .ok_or(BlobError::BlobPartNotFound)?;
  let Some(reader) = part.open_reader().await? else {
    return Ok(None);
  };
  Ok(Some(
    state.borrow_mut().resource_table.add(BlobReader(reader)),
  ))
}

#[op2]
pub async fn op_blob_read_reader(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[scoped] options: SliceOptions,
) -> Result<Uint8Array, BlobError> {
  let part = state
    .borrow()
    .resource_table
    .get::<BlobReader>(rid)
    .map_err(|_| BlobError::BlobPartNotFound)?
    .0
    .clone();
  let bytes = part.read_range(options.start, options.len).await?;
  Ok(Uint8Array::from(bytes.into_owned()))
}

#[op2]
pub fn op_blob_remove_part(state: &mut OpState, #[scoped] id: Uuid) {
  let blob_store = state.borrow::<Arc<dyn BlobStoreTrait>>();
  blob_store.remove_part(&id);
}

#[op2]
pub fn op_blob_clone_part(
  state: &mut OpState,
  #[scoped] id: Uuid,
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
  #[scoped] part_ids: Vec<Uuid>,
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
      async fn read_range(
        &self,
        start: usize,
        len: usize,
      ) -> Result<Cow<'_, [u8]>, BlobError> {
        check_range(start, len, self.size())?;
        Ok(Cow::Borrowed(&self.0[start..start + len]))
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

  #[derive(Debug)]
  struct TestFileSource {
    file: tempfile::NamedTempFile,
    opens: AtomicUsize,
  }

  impl BlobFileSource for TestFileSource {
    fn open(&self) -> std::io::Result<std::fs::File> {
      self.opens.fetch_add(1, Ordering::Relaxed);
      std::fs::File::open(self.file.path())
    }
  }

  #[tokio::test]
  async fn file_blob_reads_are_lazy_and_ranged() {
    use std::io::Seek;
    use std::io::Write;

    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.as_file().set_len(1024 * 1024).unwrap();
    file.seek(std::io::SeekFrom::End(-10)).unwrap();
    file.write_all(b"0123456789").unwrap();
    let source = Arc::new(TestFileSource {
      file,
      opens: AtomicUsize::new(0),
    });
    let part = Arc::new(
      FileBlobPart::new(source.clone(), source.file.as_file()).unwrap(),
    );
    assert_eq!(source.opens.load(Ordering::Relaxed), 0);
    let slice = SlicedBlobPart {
      part,
      start: 1024 * 1024 - 100,
      len: 100,
    };
    let reader = slice.open_reader().await.unwrap().unwrap();
    assert_eq!(source.opens.load(Ordering::Relaxed), 1);
    assert_eq!(&*reader.read_range(90, 10).await.unwrap(), b"0123456789");
    assert_eq!(&*reader.read_range(0, 2).await.unwrap(), &[0, 0]);
    assert_eq!(source.opens.load(Ordering::Relaxed), 1);
    assert!(reader.read_range(usize::MAX, 2).await.is_err());
    assert!(reader.read_range(99, 2).await.is_err());
  }

  #[tokio::test]
  async fn memory_blob_range_stays_borrowed() {
    let part = InMemoryBlobPart::from(b"abcdef".to_vec());
    assert!(matches!(
      part.read_range(1, 3).await.unwrap(),
      Cow::Borrowed(b"bcd")
    ));
    assert!(part.open_reader().await.unwrap().is_none());
    assert!(part.read_range(usize::MAX, 2).await.is_err());
  }
}
