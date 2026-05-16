// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Mutex;
#[cfg(unix)]
use std::sync::Once;
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
#[cfg(unix)]
static REGISTER_CURRENT_PROCESS_INTERNAL_FDS: Once = Once::new();

/// Register a process-owned fd that must not be adopted by user fd APIs.
pub fn register_internal_fd(fd: i32) {
  INTERNAL_FDS
    .get_or_init(Default::default)
    .lock()
    .unwrap()
    .insert(fd);
}

/// Register fds that were already open in the current process before this
/// table was created. These are Deno-owned implementation details, not
/// user-inherited descriptors for Node fd adoption APIs.
#[cfg(target_os = "linux")]
fn register_current_process_internal_fds() {
  REGISTER_CURRENT_PROCESS_INTERNAL_FDS
    .call_once(register_current_process_internal_fds_inner);
}

#[cfg(target_os = "linux")]
fn register_current_process_internal_fds_inner() {
  register_current_process_internal_fds_from_dir("/proc/self/fd");
}

#[cfg(all(unix, not(target_os = "linux")))]
fn register_current_process_internal_fds() {
  REGISTER_CURRENT_PROCESS_INTERNAL_FDS
    .call_once(register_current_process_internal_fds_inner);
}

#[cfg(all(unix, not(target_os = "linux")))]
fn register_current_process_internal_fds_inner() {
  register_current_process_internal_fds_from_dir("/dev/fd");
}

#[cfg(unix)]
fn register_current_process_internal_fds_from_dir(path: &str) {
  let Ok(path) = std::ffi::CString::new(path) else {
    return;
  };
  // SAFETY: path is a valid nul-terminated string.
  let dir = unsafe { libc::opendir(path.as_ptr()) };
  if dir.is_null() {
    return;
  }
  // SAFETY: dir is a valid directory stream until closed below.
  let dir_fd = unsafe { libc::dirfd(dir) };

  loop {
    // SAFETY: dir is a valid directory stream. readdir returns null at
    // end-of-directory or on error.
    let entry = unsafe { libc::readdir(dir) };
    if entry.is_null() {
      break;
    }
    // SAFETY: d_name is nul-terminated by readdir.
    let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) };
    let Ok(name) = name.to_str() else {
      continue;
    };
    let Ok(fd) = name.parse::<i32>() else {
      continue;
    };
    if fd <= 2 || fd == dir_fd {
      continue;
    }
    register_internal_fd(fd);
  }

  // SAFETY: dir was opened by opendir and is closed once here.
  unsafe {
    libc::closedir(dir);
  }
}

#[cfg(not(unix))]
fn register_current_process_internal_fds() {}

impl FdTable {
  pub fn new() -> Self {
    register_current_process_internal_fds();
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
