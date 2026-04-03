// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::rc::Rc;

use crate::fs::File;

/// Central table that owns file descriptor -> File mappings.
///
/// Both Deno's resource table (for `Deno.stdin`/`stdout`/`stderr`) and
/// Node's fd-based ops (`op_node_fs_read_sync`, `op_node_fs_write_sync`,
/// etc.) reference the same `Rc<dyn File>` entries. This ensures that
/// closing an fd from either side is visible to the other.
pub struct FdTable {
  entries: HashMap<i32, Rc<dyn File>>,
}

impl FdTable {
  pub fn new() -> Self {
    Self {
      entries: HashMap::new(),
    }
  }

  /// Register an fd with its File. Returns false if the fd was already
  /// registered (caller should handle this as appropriate).
  pub fn register(&mut self, fd: i32, file: Rc<dyn File>) -> bool {
    if self.entries.contains_key(&fd) {
      return false;
    }
    self.entries.insert(fd, file);
    true
  }

  /// Get the File for an fd.
  pub fn get(&self, fd: i32) -> Option<&Rc<dyn File>> {
    self.entries.get(&fd)
  }

  /// Remove and return the File for an fd.
  pub fn remove(&mut self, fd: i32) -> Option<Rc<dyn File>> {
    self.entries.remove(&fd)
  }

  /// Check if an fd is registered.
  pub fn contains(&self, fd: i32) -> bool {
    self.entries.contains_key(&fd)
  }
}

impl Default for FdTable {
  fn default() -> Self {
    Self::new()
  }
}
