// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use sys_traits::BaseFsCanonicalize;
use sys_traits::BaseFsRead;
use sys_traits::BaseFsReadDir;
use sys_traits::FileType;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;
use sys_traits::FsMetadataValue;
use sys_traits::FsRead;
use sys_traits::FsReadDir;

pub trait NodeResolutionCache:
  std::fmt::Debug + crate::sync::MaybeSend + crate::sync::MaybeSync
{
  fn get_canonicalized(
    &self,
    path: &Path,
  ) -> Option<Result<PathBuf, std::io::Error>>;
  fn set_canonicalized(&self, from: PathBuf, to: &std::io::Result<PathBuf>);
  fn get_file_type(&self, path: &Path) -> Option<Option<FileType>>;
  fn set_file_type(&self, path: PathBuf, value: Option<FileType>);
}

thread_local! {
  static CANONICALIZED_CACHE: RefCell<HashMap<PathBuf, Option<PathBuf>>> = RefCell::new(HashMap::new());
  static FILE_TYPE_CACHE: RefCell<HashMap<PathBuf, Option<FileType>>> = RefCell::new(HashMap::new());
}

// We use thread local caches here because it's just more convenient
// and easily allows workers to have separate caches.
#[derive(Debug)]
pub struct NodeResolutionThreadLocalCache;

impl NodeResolutionThreadLocalCache {
  pub fn clear() {
    CANONICALIZED_CACHE.with_borrow_mut(|cache| cache.clear());
    FILE_TYPE_CACHE.with_borrow_mut(|cache| cache.clear());
  }
}

impl NodeResolutionCache for NodeResolutionThreadLocalCache {
  fn get_canonicalized(
    &self,
    path: &Path,
  ) -> Option<Result<PathBuf, std::io::Error>> {
    CANONICALIZED_CACHE.with_borrow(|cache| {
      let item = cache.get(path)?;
      Some(match item {
        Some(value) => Ok(value.clone()),
        None => Err(std::io::Error::new(
          std::io::ErrorKind::NotFound,
          "Not found.",
        )),
      })
    })
  }

  fn set_canonicalized(&self, from: PathBuf, to: &std::io::Result<PathBuf>) {
    CANONICALIZED_CACHE.with_borrow_mut(|cache| match to {
      Ok(to) => {
        cache.insert(from, Some(to.clone()));
      }
      Err(err) => {
        if err.kind() == std::io::ErrorKind::NotFound {
          cache.insert(from, None);
        }
      }
    });
  }

  fn get_file_type(&self, path: &Path) -> Option<Option<FileType>> {
    FILE_TYPE_CACHE.with_borrow(|cache| cache.get(path).cloned())
  }

  fn set_file_type(&self, path: PathBuf, value: Option<FileType>) {
    FILE_TYPE_CACHE.with_borrow_mut(|cache| {
      cache.insert(path, value);
    })
  }
}

#[allow(clippy::disallowed_types)]
pub type NodeResolutionCacheRc = crate::sync::MaybeArc<dyn NodeResolutionCache>;

#[derive(Debug, Default)]
pub struct NodeResolutionSys<TSys> {
  sys: TSys,
  cache: Option<NodeResolutionCacheRc>,
}

impl<TSys: Clone> Clone for NodeResolutionSys<TSys> {
  fn clone(&self) -> Self {
    Self {
      sys: self.sys.clone(),
      cache: self.cache.clone(),
    }
  }
}

impl<TSys: FsMetadata> NodeResolutionSys<TSys> {
  pub fn new(sys: TSys, store: Option<NodeResolutionCacheRc>) -> Self {
    Self { sys, cache: store }
  }

  pub fn is_file(&self, path: &Path) -> bool {
    match self.get_file_type(path) {
      Ok(file_type) => file_type.is_file(),
      Err(_) => false,
    }
  }

  pub fn is_dir(&self, path: &Path) -> bool {
    match self.get_file_type(path) {
      Ok(file_type) => file_type.is_dir(),
      Err(_) => false,
    }
  }

  pub fn exists_(&self, path: &Path) -> bool {
    self.get_file_type(path).is_ok()
  }

  pub fn get_file_type(&self, path: &Path) -> std::io::Result<FileType> {
    {
      if let Some(maybe_value) =
        self.cache.as_ref().and_then(|c| c.get_file_type(path))
      {
        return match maybe_value {
          Some(value) => Ok(value),
          None => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Not found.",
          )),
        };
      }
    }
    match self.sys.fs_metadata(path) {
      Ok(metadata) => {
        if let Some(cache) = &self.cache {
          cache.set_file_type(path.to_path_buf(), Some(metadata.file_type()));
        }
        Ok(metadata.file_type())
      }
      Err(err) => {
        if let Some(cache) = &self.cache {
          cache.set_file_type(path.to_path_buf(), None);
        }
        Err(err)
      }
    }
  }
}

impl<TSys: FsCanonicalize> BaseFsCanonicalize for NodeResolutionSys<TSys> {
  fn base_fs_canonicalize(&self, from: &Path) -> std::io::Result<PathBuf> {
    if let Some(cache) = &self.cache {
      if let Some(result) = cache.get_canonicalized(from) {
        return result;
      }
    }
    let result = self.sys.base_fs_canonicalize(from);
    if let Some(cache) = &self.cache {
      cache.set_canonicalized(from.to_path_buf(), &result);
    }
    result
  }
}

impl<TSys: FsReadDir> BaseFsReadDir for NodeResolutionSys<TSys> {
  type ReadDirEntry = TSys::ReadDirEntry;

  #[inline(always)]
  fn base_fs_read_dir(
    &self,
    path: &Path,
  ) -> std::io::Result<
    Box<dyn Iterator<Item = std::io::Result<Self::ReadDirEntry>> + '_>,
  > {
    self.sys.base_fs_read_dir(path)
  }
}

impl<TSys: FsRead> BaseFsRead for NodeResolutionSys<TSys> {
  #[inline(always)]
  fn base_fs_read(
    &self,
    path: &Path,
  ) -> std::io::Result<std::borrow::Cow<'static, [u8]>> {
    self.sys.base_fs_read(path)
  }
}
