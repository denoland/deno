// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Mutex;
use std::sync::OnceLock;

use crate::fs::File;

/// How an fd's lifetime is managed.
pub enum FdOwnership {
  /// FdTable owns the File; dropping the entry closes the fd.
  /// Used by fs.openSync, stdio fds 0/1/2, etc.
  TableOwned(Rc<dyn File>),
  /// A uv handle (e.g. uv_pipe_t) owns the fd; FdTable just tracks
  /// that it exists for duplicate detection. The entry is removed
  /// when uv_close fires, but no file is dropped.
  UvOwned,
}

/// Central table tracking all known file descriptors.
///
/// Both Deno's resource table and Node's fd-based ops use this table
/// to look up files and detect duplicate registrations.
pub struct FdTable {
  entries: HashMap<i32, FdOwnership>,
}

static INTERNAL_FDS: OnceLock<Mutex<HashSet<i32>>> = OnceLock::new();

/// Register a process-owned fd that must not be adopted by user fd APIs.
pub fn register_internal_fd(fd: i32) {
  INTERNAL_FDS
    .get_or_init(Default::default)
    .lock()
    .unwrap()
    .insert(fd);
}

impl FdTable {
  pub fn new() -> Self {
    Self {
      entries: HashMap::new(),
    }
  }

  /// Register a TableOwned fd. Returns false if already registered.
  pub fn register(&mut self, fd: i32, file: Rc<dyn File>) -> bool {
    if self.entries.contains_key(&fd) {
      return false;
    }
    self.entries.insert(fd, FdOwnership::TableOwned(file));
    true
  }

  /// Register a UvOwned fd (tracked but not owned). Returns false if
  /// already registered.
  pub fn register_uv_owned(&mut self, fd: i32) -> bool {
    if self.entries.contains_key(&fd) {
      return false;
    }
    self.entries.insert(fd, FdOwnership::UvOwned);
    true
  }

  /// Get the File for a TableOwned fd. Returns None for UvOwned or missing.
  pub fn get(&self, fd: i32) -> Option<&Rc<dyn File>> {
    match self.entries.get(&fd) {
      Some(FdOwnership::TableOwned(file)) => Some(file),
      _ => None,
    }
  }

  /// Remove an fd entry. For TableOwned, returns the File (caller drops
  /// to close). For UvOwned, returns None (uv handle closes the fd).
  pub fn remove(&mut self, fd: i32) -> Option<Rc<dyn File>> {
    match self.entries.remove(&fd) {
      Some(FdOwnership::TableOwned(file)) => Some(file),
      Some(FdOwnership::UvOwned) => None,
      None => None,
    }
  }

  /// Check if an fd is registered (either ownership type).
  pub fn contains(&self, fd: i32) -> bool {
    if self.entries.contains_key(&fd) {
      return true;
    }
    INTERNAL_FDS
      .get()
      .is_some_and(|fds| fds.lock().unwrap().contains(&fd))
  }
}

impl Default for FdTable {
  fn default() -> Self {
    Self::new()
  }
}
