use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use sys_traits::FileType;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;
use sys_traits::FsMetadataValue;

// todo(THIS PR): better name lol
#[derive(Debug, Default)]
pub struct SysCache<TSys> {
  sys: TSys,
  // todo: combine Rcs
  cache: Rc<RefCell<HashMap<PathBuf, Option<FileType>>>>,
  canonicalize_cache: Rc<RefCell<HashMap<PathBuf, PathBuf>>>,
}

impl<TSys: Clone> Clone for SysCache<TSys> {
  fn clone(&self) -> Self {
    Self {
      sys: self.sys.clone(),
      cache: self.cache.clone(),
      canonicalize_cache: self.canonicalize_cache.clone(),
    }
  }
}

impl<TSys: FsMetadata> SysCache<TSys> {
  pub fn new(sys: TSys) -> Self {
    Self {
      sys,
      cache: Default::default(),
      canonicalize_cache: Default::default(),
    }
  }

  pub fn sys(&self) -> &TSys {
    &self.sys
  }

  pub fn is_file(&self, path: &Path) -> bool {
    match self.get(path) {
      Ok(file_type) => file_type.is_file(),
      Err(_) => false,
    }
  }

  pub fn is_dir(&self, path: &Path) -> bool {
    match self.get(path) {
      Ok(file_type) => file_type.is_dir(),
      Err(_) => false,
    }
  }

  pub fn exists_(&self, path: &Path) -> bool {
    match self.get(path) {
      Ok(_) => true,
      Err(_) => false,
    }
  }

  // todo(THIS PR): better name
  pub fn get(&self, path: &Path) -> std::io::Result<FileType> {
    {
      if let Some(file_type) = self.cache.borrow().get(path) {
        return match *file_type {
          Some(file_type) => Ok(file_type),
          None => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Not found.",
          )),
        };
      }
    }
    match self.sys.fs_metadata(path) {
      Ok(metadata) => {
        self
          .cache
          .borrow_mut()
          .insert(path.to_path_buf(), Some(metadata.file_type()));
        Ok(metadata.file_type())
      }
      Err(err) => {
        self.cache.borrow_mut().insert(path.to_path_buf(), None);
        Err(err)
      }
    }
  }
}

impl<TSys: FsCanonicalize> SysCache<TSys> {
  pub fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
    {
      if let Some(path) = self.canonicalize_cache.borrow().get(path) {
        return Ok(path.clone());
      }
    }
    let canon = self.sys.fs_canonicalize(path)?;
    self
      .canonicalize_cache
      .borrow_mut()
      .insert(path.to_path_buf(), canon.clone());
    Ok(canon)
  }
}
