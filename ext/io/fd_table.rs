// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::rc::Rc;

use deno_core::CancelHandle;

use crate::fs::File;

/// An entry in the FdTable: the File plus a CancelHandle that is
/// triggered when the fd is closed, cancelling any in-flight async ops.
pub struct FdEntry {
  pub file: Rc<dyn File>,
  pub cancel_handle: Rc<CancelHandle>,
}

/// Central table that owns file descriptor -> File mappings.
///
/// Both Deno's resource table (for `Deno.stdin`/`stdout`/`stderr`) and
/// Node's fd-based ops (`op_node_fs_read_sync`, `op_node_fs_write_sync`,
/// etc.) reference the same `Rc<dyn File>` entries. This ensures that
/// closing an fd from either side is visible to the other.
///
/// Each entry also has a `CancelHandle` that is cancelled when the fd
/// is closed, allowing in-flight async reads/writes to be cancelled.
pub struct FdTable {
  entries: HashMap<i32, FdEntry>,
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
    self.entries.insert(
      fd,
      FdEntry {
        file,
        cancel_handle: Rc::new(CancelHandle::new()),
      },
    );
    true
  }

  /// Get the File for an fd.
  pub fn get(&self, fd: i32) -> Option<&Rc<dyn File>> {
    self.entries.get(&fd).map(|e| &e.file)
  }

  /// Get the CancelHandle for an fd.
  pub fn get_cancel_handle(&self, fd: i32) -> Option<Rc<CancelHandle>> {
    self.entries.get(&fd).map(|e| e.cancel_handle.clone())
  }

  /// Remove an fd entry. Cancels the handle (aborting in-flight ops)
  /// and returns the File.
  pub fn remove(&mut self, fd: i32) -> Option<Rc<dyn File>> {
    let entry = self.entries.remove(&fd)?;
    entry.cancel_handle.cancel();
    Some(entry.file)
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
