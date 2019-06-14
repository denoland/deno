// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

use libc::c_char;
use libc::c_int;
use libc::c_void;
use libc::size_t;
use std::convert::From;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::option::Option;
use std::ptr::null;
use std::ptr::NonNull;
use std::slice;

// TODO(F001): change this definition to `extern { pub type isolate; }`
// After RFC 1861 is stablized. See https://github.com/rust-lang/rust/issues/43467.
#[repr(C)]
pub struct isolate {
  _unused: [u8; 0],
}

/// This type represents a borrowed slice.
#[repr(C)]
pub struct deno_buf {
  data_ptr: *const u8,
  data_len: usize,
}

/// `deno_buf` can not clone, and there is no interior mutability.
/// This type satisfies Send bound.
unsafe impl Send for deno_buf {}

impl deno_buf {
  #[inline]
  pub fn empty() -> Self {
    Self {
      data_ptr: null(),
      data_len: 0,
    }
  }

  #[inline]
  pub unsafe fn from_raw_parts(ptr: *const u8, len: usize) -> Self {
    Self {
      data_ptr: ptr,
      data_len: len,
    }
  }
}

/// Converts Rust &Buf to libdeno `deno_buf`.
impl<'a> From<&'a [u8]> for deno_buf {
  #[inline]
  fn from(x: &'a [u8]) -> Self {
    Self {
      data_ptr: x.as_ref().as_ptr(),
      data_len: x.len(),
    }
  }
}

impl Deref for deno_buf {
  type Target = [u8];
  #[inline]
  fn deref(&self) -> &[u8] {
    unsafe { std::slice::from_raw_parts(self.data_ptr, self.data_len) }
  }
}

impl AsRef<[u8]> for deno_buf {
  #[inline]
  fn as_ref(&self) -> &[u8] {
    &*self
  }
}

/// A PinnedBuf encapsulates a slice that's been borrowed from a JavaScript
/// ArrayBuffer object. JavaScript objects can normally be garbage collected,
/// but the existence of a PinnedBuf inhibits this until it is dropped. It
/// behaves much like an Arc<[u8]>, although a PinnedBuf currently can't be
/// cloned.
#[repr(C)]
pub struct PinnedBuf {
  data_ptr: NonNull<u8>,
  data_len: usize,
  pin: NonNull<c_void>,
}

#[repr(C)]
pub struct PinnedBufRaw {
  data_ptr: *mut u8,
  data_len: usize,
  pin: *mut c_void,
}

unsafe impl Send for PinnedBuf {}
unsafe impl Send for PinnedBufRaw {}

impl PinnedBuf {
  pub fn new(raw: PinnedBufRaw) -> Option<Self> {
    NonNull::new(raw.data_ptr).map(|data_ptr| PinnedBuf {
      data_ptr,
      data_len: raw.data_len,
      pin: NonNull::new(raw.pin).unwrap(),
    })
  }
}

impl Drop for PinnedBuf {
  fn drop(&mut self) {
    unsafe {
      let raw = &mut *(self as *mut PinnedBuf as *mut PinnedBufRaw);
      deno_pinned_buf_delete(raw);
    }
  }
}

impl Deref for PinnedBuf {
  type Target = [u8];
  fn deref(&self) -> &[u8] {
    unsafe { slice::from_raw_parts(self.data_ptr.as_ptr(), self.data_len) }
  }
}

impl DerefMut for PinnedBuf {
  fn deref_mut(&mut self) -> &mut [u8] {
    unsafe { slice::from_raw_parts_mut(self.data_ptr.as_ptr(), self.data_len) }
  }
}

impl AsRef<[u8]> for PinnedBuf {
  fn as_ref(&self) -> &[u8] {
    &*self
  }
}

impl AsMut<[u8]> for PinnedBuf {
  fn as_mut(&mut self) -> &mut [u8] {
    &mut *self
  }
}

pub use PinnedBufRaw as deno_pinned_buf;

#[repr(C)]
pub struct deno_snapshot<'a> {
  pub data_ptr: *const u8,
  pub data_len: usize,
  _marker: PhantomData<&'a [u8]>,
}

/// `deno_snapshot` can not clone, and there is no interior mutability.
/// This type satisfies Send bound.
unsafe impl Send for deno_snapshot<'_> {}

// TODO(ry) Snapshot1 and Snapshot2 are not very good names and need to be
// reconsidered. The entire snapshotting interface is still under construction.

/// The type returned from deno_snapshot_new. Needs to be dropped.
pub type Snapshot1<'a> = deno_snapshot<'a>;

/// The type created from slice. Used for loading.
pub type Snapshot2<'a> = deno_snapshot<'a>;

/// Converts Rust &Buf to libdeno `deno_buf`.
impl<'a> From<&'a [u8]> for Snapshot2<'a> {
  #[inline]
  fn from(x: &'a [u8]) -> Self {
    Self {
      data_ptr: x.as_ref().as_ptr(),
      data_len: x.len(),
      _marker: PhantomData,
    }
  }
}

impl Snapshot2<'_> {
  #[inline]
  pub fn empty() -> Self {
    Self {
      data_ptr: null(),
      data_len: 0,
      _marker: PhantomData,
    }
  }
}

#[allow(non_camel_case_types)]
type deno_recv_cb = unsafe extern "C" fn(
  user_data: *mut c_void,
  control_buf: deno_buf, // deprecated
  zero_copy_buf: deno_pinned_buf,
);

/// Called when dynamic import is called in JS: import('foo')
/// Embedder must call deno_dyn_import() with the specified id and
/// the module.
#[allow(non_camel_case_types)]
type deno_dyn_import_cb = unsafe extern "C" fn(
  user_data: *mut c_void,
  specifier: *const c_char,
  referrer: *const c_char,
  id: deno_dyn_import_id,
);

#[allow(non_camel_case_types)]
pub type deno_mod = i32;

#[allow(non_camel_case_types)]
pub type deno_dyn_import_id = i32;

#[allow(non_camel_case_types)]
type deno_resolve_cb = unsafe extern "C" fn(
  user_data: *mut c_void,
  specifier: *const c_char,
  referrer: deno_mod,
) -> deno_mod;

#[repr(C)]
pub struct deno_config<'a> {
  pub will_snapshot: c_int,
  pub load_snapshot: Snapshot2<'a>,
  pub shared: deno_buf,
  pub recv_cb: deno_recv_cb,
  pub dyn_import_cb: deno_dyn_import_cb,
}

#[cfg(not(windows))]
#[link(name = "deno")]
extern "C" {}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
#[link(name = "c++")]
extern "C" {}

#[cfg(windows)]
#[link(name = "libdeno")]
extern "C" {}

#[cfg(windows)]
#[link(name = "shlwapi")]
extern "C" {}

#[cfg(windows)]
#[link(name = "winmm")]
extern "C" {}

#[cfg(windows)]
#[link(name = "ws2_32")]
extern "C" {}

#[cfg(windows)]
#[link(name = "dbghelp")]
extern "C" {}

extern "C" {
  pub fn deno_init();
  pub fn deno_v8_version() -> *const c_char;
  pub fn deno_set_v8_flags(argc: *mut c_int, argv: *mut *mut c_char);
  pub fn deno_new(config: deno_config) -> *const isolate;
  pub fn deno_delete(i: *const isolate);
  pub fn deno_last_exception(i: *const isolate) -> *const c_char;
  pub fn deno_check_promise_errors(i: *const isolate);
  pub fn deno_lock(i: *const isolate);
  pub fn deno_unlock(i: *const isolate);
  pub fn deno_respond(
    i: *const isolate,
    user_data: *const c_void,
    buf: deno_buf,
  );
  pub fn deno_pinned_buf_delete(buf: &mut deno_pinned_buf);
  pub fn deno_execute(
    i: *const isolate,
    user_data: *const c_void,
    js_filename: *const c_char,
    js_source: *const c_char,
  );
  pub fn deno_terminate_execution(i: *const isolate);

  // Modules

  pub fn deno_mod_new(
    i: *const isolate,
    main: bool,
    name: *const c_char,
    source: *const c_char,
  ) -> deno_mod;

  pub fn deno_mod_imports_len(i: *const isolate, id: deno_mod) -> size_t;

  pub fn deno_mod_imports_get(
    i: *const isolate,
    id: deno_mod,
    index: size_t,
  ) -> *const c_char;

  pub fn deno_mod_instantiate(
    i: *const isolate,
    user_data: *const c_void,
    id: deno_mod,
    resolve_cb: deno_resolve_cb,
  );

  pub fn deno_mod_evaluate(
    i: *const isolate,
    user_data: *const c_void,
    id: deno_mod,
  );

  /// Call exactly once for every deno_dyn_import_cb.
  pub fn deno_dyn_import(
    i: *const isolate,
    user_data: *const c_void,
    id: deno_dyn_import_id,
    mod_id: deno_mod,
  );

  pub fn deno_snapshot_new(i: *const isolate) -> Snapshot1<'static>;

  #[allow(dead_code)]
  pub fn deno_snapshot_delete(s: &mut deno_snapshot);
}
