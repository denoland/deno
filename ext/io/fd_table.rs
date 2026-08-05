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
  /// An inherited extra stdio fd (fd >= 3 installed by the Node
  /// `child_process` spawn path) that can be used by node:fs, but may still be
  /// claimed later by libuv APIs such as net.Socket({ fd }).
  ///
  /// The `File` here is a `dup()` of the inherited descriptor, not the original
  /// numeric fd. Dropping this entry closes only the dup, so the original fd
  /// stays open and remains claimable by libuv (this is what lets node:fs and a
  /// later `net.Socket({ fd })` both work). The trade-off is that the original
  /// fd is retained for the process lifetime unless libuv reclaims it, which
  /// differs from Node, where node:fs `autoClose` closes the real fd.
  InheritedExtraStdio(Rc<dyn File>),
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

  /// Register an inherited extra stdio fd. Returns false if already registered.
  pub fn register_inherited_extra_stdio(
    &mut self,
    fd: i32,
    file: Rc<dyn File>,
  ) -> bool {
    if self.entries.contains_key(&fd) {
      return false;
    }
    self
      .entries
      .insert(fd, FdOwnership::InheritedExtraStdio(file));
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
      Some(FdOwnership::InheritedExtraStdio(file)) => Some(file),
      _ => None,
    }
  }

  /// Remove an fd entry. For TableOwned, returns the File (caller drops
  /// to close). For UvOwned, returns None (uv handle closes the fd).
  pub fn remove(&mut self, fd: i32) -> Option<Rc<dyn File>> {
    match self.entries.remove(&fd) {
      Some(FdOwnership::TableOwned(file)) => Some(file),
      Some(FdOwnership::InheritedExtraStdio(file)) => Some(file),
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

  /// Check if an fd is an inherited extra stdio descriptor.
  pub fn is_inherited_extra_stdio(&self, fd: i32) -> bool {
    matches!(
      self.entries.get(&fd),
      Some(FdOwnership::InheritedExtraStdio(_))
    )
  }

  /// Check whether a libuv stream wrap (`PipeWrap::open`, `TCPWrap::open`)
  /// may adopt `fd`. Stdio fds (0-2) are pre-registered and may be re-opened;
  /// inherited extra stdio fds may be adopted (e.g. via `net.Socket({ fd })`)
  /// and are consumed by `finish_uv_adopt` only once the uv open actually
  /// claims the fd, so a failed open leaves the fd usable by node:fs. Any
  /// other tracked fd is a duplicate.
  ///
  /// Returns `Some(was_inherited)` if adoption may proceed (pass the flag to
  /// `finish_uv_adopt` on success), or `None` if the fd is already tracked
  /// and the caller should reject with `EEXIST`.
  pub fn begin_uv_adopt(&self, fd: i32) -> Option<bool> {
    if self.is_inherited_extra_stdio(fd) {
      Some(true)
    } else if self.contains(fd) && !(0..=2).contains(&fd) {
      None
    } else {
      Some(false)
    }
  }

  /// Record a successful uv adoption started with `begin_uv_adopt`. Drops the
  /// inherited entry if there was one (libuv now owns the original fd; only
  /// the node:fs dup is released, and its close is deferred until any
  /// `Rc<dyn File>` clone held by an in-flight node:fs stream drops), then
  /// tracks the fd as UvOwned so it can't be re-adopted by another wrap.
  pub fn finish_uv_adopt(&mut self, fd: i32, was_inherited: bool) {
    if was_inherited {
      self.remove(fd);
    }
    self.register_uv_owned(fd);
  }
}

impl Default for FdTable {
  fn default() -> Self {
    Self::new()
  }
}
