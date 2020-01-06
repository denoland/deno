// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#![allow(unused)]
#![allow(mutable_transmutes)]
#![allow(clippy::transmute_ptr_to_ptr)]

use crate::bindings;

use rusty_v8 as v8;
use v8::InIsolate;

use libc::c_char;
use libc::c_int;
use libc::c_void;
use libc::size_t;
use std::collections::HashMap;
use std::convert::From;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::ffi::CString;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::option::Option;
use std::ptr::null;
use std::ptr::NonNull;
use std::slice;

pub type OpId = u32;

pub struct ModuleInfo {
  pub main: bool,
  pub name: String,
  pub handle: v8::Global<v8::Module>,
  pub import_specifiers: Vec<String>,
}

pub fn script_origin<'a>(
  s: &mut impl v8::ToLocal<'a>,
  resource_name: v8::Local<'a, v8::String>,
) -> v8::ScriptOrigin<'a> {
  let resource_line_offset = v8::Integer::new(s, 0);
  let resource_column_offset = v8::Integer::new(s, 0);
  let resource_is_shared_cross_origin = v8::Boolean::new(s, false);
  let script_id = v8::Integer::new(s, 123);
  let source_map_url = v8::String::new(s, "source_map_url").unwrap();
  let resource_is_opaque = v8::Boolean::new(s, true);
  let is_wasm = v8::Boolean::new(s, false);
  let is_module = v8::Boolean::new(s, false);
  v8::ScriptOrigin::new(
    resource_name.into(),
    resource_line_offset,
    resource_column_offset,
    resource_is_shared_cross_origin,
    script_id,
    source_map_url.into(),
    resource_is_opaque,
    is_wasm,
    is_module,
  )
}

pub fn module_origin<'a>(
  s: &mut impl v8::ToLocal<'a>,
  resource_name: v8::Local<'a, v8::String>,
) -> v8::ScriptOrigin<'a> {
  let resource_line_offset = v8::Integer::new(s, 0);
  let resource_column_offset = v8::Integer::new(s, 0);
  let resource_is_shared_cross_origin = v8::Boolean::new(s, false);
  let script_id = v8::Integer::new(s, 123);
  let source_map_url = v8::String::new(s, "source_map_url").unwrap();
  let resource_is_opaque = v8::Boolean::new(s, true);
  let is_wasm = v8::Boolean::new(s, false);
  let is_module = v8::Boolean::new(s, true);
  v8::ScriptOrigin::new(
    resource_name.into(),
    resource_line_offset,
    resource_column_offset,
    resource_is_shared_cross_origin,
    script_id,
    source_map_url.into(),
    resource_is_opaque,
    is_wasm,
    is_module,
  )
}

/// This type represents a borrowed slice.
#[repr(C)]
pub struct deno_buf {
  pub data_ptr: *const u8,
  pub data_len: usize,
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

impl<'a> From<&'a mut [u8]> for deno_buf {
  #[inline]
  fn from(x: &'a mut [u8]) -> Self {
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
pub struct PinnedBuf {
  data_ptr: NonNull<u8>,
  data_len: usize,
  backing_store: v8::SharedRef<v8::BackingStore>,
}

unsafe impl Send for PinnedBuf {}

impl PinnedBuf {
  pub fn new(view: v8::Local<v8::ArrayBufferView>) -> Self {
    let mut backing_store = view.buffer().unwrap().get_backing_store();
    let backing_store_ptr = backing_store.data() as *mut _ as *mut u8;
    let view_ptr = unsafe { backing_store_ptr.add(view.byte_offset()) };
    let view_len = view.byte_length();
    Self {
      data_ptr: NonNull::new(view_ptr).unwrap(),
      data_len: view_len,
      backing_store,
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
pub type Snapshot1 = v8::OwnedStartupData;

/// The type created from slice. Used for loading.
pub type Snapshot2<'a> = v8::StartupData<'a>;

#[allow(non_camel_case_types)]
pub type deno_recv_cb = unsafe fn(
  user_data: *mut c_void,
  op_id: OpId,
  control_buf: deno_buf,
  zero_copy_buf: Option<PinnedBuf>,
);

/// Called when dynamic import is called in JS: import('foo')
/// Embedder must call deno_dyn_import_done() with the specified id and
/// the module.
#[allow(non_camel_case_types)]
pub type deno_dyn_import_cb = fn(
  user_data: *mut c_void,
  specifier: &str,
  referrer: &str,
  id: deno_dyn_import_id,
);

#[allow(non_camel_case_types)]
pub type deno_mod = i32;

#[allow(non_camel_case_types)]
pub type deno_dyn_import_id = i32;

#[allow(non_camel_case_types)]
pub type deno_resolve_cb = unsafe extern "C" fn(
  resolve_context: *mut c_void,
  specifier: *const c_char,
  referrer: deno_mod,
) -> deno_mod;

pub enum SnapshotConfig {
  Borrowed(v8::StartupData<'static>),
  Owned(v8::OwnedStartupData),
}

impl From<&'static [u8]> for SnapshotConfig {
  fn from(sd: &'static [u8]) -> Self {
    Self::Borrowed(v8::StartupData::new(sd))
  }
}

impl From<v8::OwnedStartupData> for SnapshotConfig {
  fn from(sd: v8::OwnedStartupData) -> Self {
    Self::Owned(sd)
  }
}

impl Deref for SnapshotConfig {
  type Target = v8::StartupData<'static>;
  fn deref(&self) -> &Self::Target {
    match self {
      Self::Borrowed(sd) => sd,
      Self::Owned(sd) => &*sd,
    }
  }
}

pub unsafe fn deno_init() {
  let platform = v8::platform::new_default_platform();
  v8::V8::initialize_platform(platform);
  v8::V8::initialize();
  // TODO(ry) This makes WASM compile synchronously. Eventually we should
  // remove this to make it work asynchronously too. But that requires getting
  // PumpMessageLoop and RunMicrotasks setup correctly.
  // See https://github.com/denoland/deno/issues/2544
  let argv = vec![
    "".to_string(),
    "--no-wasm-async-compilation".to_string(),
    "--harmony-top-level-await".to_string(),
  ];
  v8::V8::set_flags_from_command_line(argv);
}

lazy_static! {
  pub static ref EXTERNAL_REFERENCES: v8::ExternalReferences =
    v8::ExternalReferences::new(&[
      v8::ExternalReference {
        function: bindings::print
      },
      v8::ExternalReference {
        function: bindings::recv
      },
      v8::ExternalReference {
        function: bindings::send
      },
      v8::ExternalReference {
        function: bindings::eval_context
      },
      v8::ExternalReference {
        function: bindings::error_to_json
      },
      v8::ExternalReference {
        getter: bindings::shared_getter
      },
      v8::ExternalReference {
        message: bindings::message_callback
      },
      v8::ExternalReference {
        function: bindings::queue_microtask
      },
    ]);
}

pub fn initialize_context<'a>(
  scope: &mut impl v8::ToLocal<'a>,
  mut context: v8::Local<v8::Context>,
) {
  context.enter();

  let global = context.global(scope);

  let deno_val = v8::Object::new(scope);

  global.set(
    context,
    v8::String::new(scope, "Deno").unwrap().into(),
    deno_val.into(),
  );

  let mut core_val = v8::Object::new(scope);

  deno_val.set(
    context,
    v8::String::new(scope, "core").unwrap().into(),
    core_val.into(),
  );

  let mut print_tmpl = v8::FunctionTemplate::new(scope, bindings::print);
  let mut print_val = print_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "print").unwrap().into(),
    print_val.into(),
  );

  let mut recv_tmpl = v8::FunctionTemplate::new(scope, bindings::recv);
  let mut recv_val = recv_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "recv").unwrap().into(),
    recv_val.into(),
  );

  let mut send_tmpl = v8::FunctionTemplate::new(scope, bindings::send);
  let mut send_val = send_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "send").unwrap().into(),
    send_val.into(),
  );

  let mut eval_context_tmpl =
    v8::FunctionTemplate::new(scope, bindings::eval_context);
  let mut eval_context_val =
    eval_context_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "evalContext").unwrap().into(),
    eval_context_val.into(),
  );

  let mut error_to_json_tmpl =
    v8::FunctionTemplate::new(scope, bindings::error_to_json);
  let mut error_to_json_val =
    error_to_json_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "errorToJSON").unwrap().into(),
    error_to_json_val.into(),
  );

  core_val.set_accessor(
    context,
    v8::String::new(scope, "shared").unwrap().into(),
    bindings::shared_getter,
  );

  // Direct bindings on `window`.
  let mut queue_microtask_tmpl =
    v8::FunctionTemplate::new(scope, bindings::queue_microtask);
  let mut queue_microtask_val =
    queue_microtask_tmpl.get_function(scope, context).unwrap();
  global.set(
    context,
    v8::String::new(scope, "queueMicrotask").unwrap().into(),
    queue_microtask_val.into(),
  );

  context.exit();
}

pub unsafe fn deno_import_buf<'sc>(
  scope: &mut impl v8::ToLocal<'sc>,
  buf: deno_buf,
) -> v8::Local<'sc, v8::Uint8Array> {
  /*
  if (buf.data_ptr == nullptr) {
    return v8::Local<v8::Uint8Array>();
  }
  */

  if buf.data_ptr.is_null() {
    let mut ab = v8::ArrayBuffer::new(scope, 0);
    return v8::Uint8Array::new(ab, 0, 0).expect("Failed to create UintArray8");
  }

  /*
  // To avoid excessively allocating new ArrayBuffers, we try to reuse a single
  // global ArrayBuffer. The caveat is that users must extract data from it
  // before the next tick. We only do this for ArrayBuffers less than 1024
  // bytes.
  v8::Local<v8::ArrayBuffer> ab;
  void* data;
  if (buf.data_len > GLOBAL_IMPORT_BUF_SIZE) {
    // Simple case. We allocate a new ArrayBuffer for this.
    ab = v8::ArrayBuffer::New(d->isolate_, buf.data_len);
    data = ab->GetBackingStore()->Data();
  } else {
    // Fast case. We reuse the global ArrayBuffer.
    if (d->global_import_buf_.IsEmpty()) {
      // Lazily initialize it.
      DCHECK_NULL(d->global_import_buf_ptr_);
      ab = v8::ArrayBuffer::New(d->isolate_, GLOBAL_IMPORT_BUF_SIZE);
      d->global_import_buf_.Reset(d->isolate_, ab);
      d->global_import_buf_ptr_ = ab->GetBackingStore()->Data();
    } else {
      DCHECK(d->global_import_buf_ptr_);
      ab = d->global_import_buf_.Get(d->isolate_);
    }
    data = d->global_import_buf_ptr_;
  }
  memcpy(data, buf.data_ptr, buf.data_len);
  auto view = v8::Uint8Array::New(ab, 0, buf.data_len);
  return view;
  */

  // TODO(bartlomieju): for now skipping part with `global_import_buf_`
  // and always creating new buffer
  let mut ab = v8::ArrayBuffer::new(scope, buf.data_len);
  let mut backing_store = ab.get_backing_store();
  let data = backing_store.data();
  let data: *mut u8 = unsafe { data as *mut libc::c_void as *mut u8 };
  std::ptr::copy_nonoverlapping(buf.data_ptr, data, buf.data_len);
  v8::Uint8Array::new(ab, 0, buf.data_len).expect("Failed to create UintArray8")
}

/*

#[allow(dead_code)]
pub unsafe fn deno_run_microtasks(i: *mut isolate, core_isolate: *mut c_void) {
  /*
  deno::DenoIsolate* d = reinterpret_cast<deno::DenoIsolate*>(d_);
  deno::UserDataScope user_data_scope(d, user_data);
  v8::Locker locker(d->isolate_);
  v8::Isolate::Scope isolate_scope(d->isolate_);
  d->isolate_->RunMicrotasks();
  */
  let deno_isolate: &mut DenoIsolate = unsafe { &mut *i };
  deno_isolate.core_isolate_ = core_isolate;
  let isolate = deno_isolate.isolate_.as_mut().unwrap();
  let mut locker = v8::Locker::new(isolate);
  isolate.enter();
  isolate.run_microtasks();
  isolate.exit();
  deno_isolate.core_isolate_ = std::ptr::null_mut();
}


#[allow(dead_code)]
pub unsafe fn deno_snapshot_delete(s: &mut deno_snapshot) {
  todo!()
}
*/
