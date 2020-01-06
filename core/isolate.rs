// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Do not add any dependency to modules.rs!
// modules.rs is complex and should remain decoupled from isolate.rs to keep the
// Isolate struct from becoming too bloating for users who do not need
// asynchronous module loading.

use rusty_v8 as v8;

use crate::any_error::ErrBox;
use crate::bindings;
use crate::js_errors::CoreJSError;
use crate::js_errors::V8Exception;
use crate::ops::*;
use crate::shared_queue::SharedQueue;
use crate::shared_queue::RECOMMENDED_SIZE;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use futures::stream::FuturesUnordered;
use futures::stream::IntoStream;
use futures::stream::Stream;
use futures::stream::StreamExt;
use futures::stream::StreamFuture;
use futures::stream::TryStream;
use futures::stream::TryStreamExt;
use futures::task::AtomicWaker;
use libc::c_void;
use std::collections::HashMap;
use std::convert::From;
use std::fmt;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::option::Option;
use std::pin::Pin;
use std::ptr::null;
use std::ptr::NonNull;
use std::slice;
use std::sync::{Arc, Mutex, Once};
use std::task::Context;
use std::task::Poll;

// TODO(bartlomieju): get rid of DenoBuf
/// This type represents a borrowed slice.
#[repr(C)]
pub struct DenoBuf {
  pub data_ptr: *const u8,
  pub data_len: usize,
}

/// `DenoBuf` can not clone, and there is no interior mutability.
/// This type satisfies Send bound.
unsafe impl Send for DenoBuf {}

impl DenoBuf {
  #[inline]
  pub fn empty() -> Self {
    Self {
      data_ptr: null(),
      data_len: 0,
    }
  }

  #[allow(clippy::missing_safety_doc)]
  #[inline]
  pub unsafe fn from_raw_parts(ptr: *const u8, len: usize) -> Self {
    Self {
      data_ptr: ptr,
      data_len: len,
    }
  }
}

/// Converts Rust &Buf to libdeno `DenoBuf`.
impl<'a> From<&'a [u8]> for DenoBuf {
  #[inline]
  fn from(x: &'a [u8]) -> Self {
    Self {
      data_ptr: x.as_ref().as_ptr(),
      data_len: x.len(),
    }
  }
}

impl<'a> From<&'a mut [u8]> for DenoBuf {
  #[inline]
  fn from(x: &'a mut [u8]) -> Self {
    Self {
      data_ptr: x.as_ref().as_ptr(),
      data_len: x.len(),
    }
  }
}

impl Deref for DenoBuf {
  type Target = [u8];
  #[inline]
  fn deref(&self) -> &[u8] {
    unsafe { std::slice::from_raw_parts(self.data_ptr, self.data_len) }
  }
}

impl AsRef<[u8]> for DenoBuf {
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
  #[allow(unused)]
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

// TODO(bartlomieju): move to core/modules.rs
pub type ModuleId = i32;
pub type DynImportId = i32;

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

/// Stores a script used to initalize a Isolate
pub struct Script<'a> {
  pub source: &'a str,
  pub filename: &'a str,
}

/// Represent result of fetching the source code of a module. Found module URL
/// might be different from specified URL used for loading due to redirections
/// (like HTTP 303). E.G. Both https://example.com/a.ts and
/// https://example.com/b.ts may point to https://example.com/c.ts
/// By keeping track of specified and found URL we can alias modules and avoid
/// recompiling the same code 3 times.
#[derive(Debug, Eq, PartialEq)]
pub struct SourceCodeInfo {
  pub code: String,
  pub module_url_specified: String,
  pub module_url_found: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RecursiveLoadEvent {
  Fetch(SourceCodeInfo),
  Instantiate(ModuleId),
}

pub trait ImportStream: TryStream {
  fn register(
    &mut self,
    source_code_info: SourceCodeInfo,
    isolate: &mut Isolate,
  ) -> Result<(), ErrBox>;
}

type DynImportStream = Box<
  dyn ImportStream<
      Ok = RecursiveLoadEvent,
      Error = ErrBox,
      Item = Result<RecursiveLoadEvent, ErrBox>,
    > + Send
    + Unpin,
>;

type DynImportFn = dyn Fn(DynImportId, &str, &str) -> DynImportStream;

/// Wraps DynImportStream to include the DynImportId, so that it doesn't
/// need to be exposed.
#[derive(Debug)]
struct DynImport {
  pub id: DynImportId,
  pub inner: DynImportStream,
}

impl fmt::Debug for DynImportStream {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "DynImportStream(..)")
  }
}

impl Stream for DynImport {
  type Item = Result<(DynImportId, RecursiveLoadEvent), (DynImportId, ErrBox)>;

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Option<Self::Item>> {
    let self_inner = self.get_mut();
    match self_inner.inner.try_poll_next_unpin(cx) {
      Poll::Ready(Some(Ok(event))) => {
        Poll::Ready(Some(Ok((self_inner.id, event))))
      }
      Poll::Ready(None) => unreachable!(),
      Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err((self_inner.id, e)))),
      Poll::Pending => Poll::Pending,
    }
  }
}

impl ImportStream for DynImport {
  fn register(
    &mut self,
    source_code_info: SourceCodeInfo,
    isolate: &mut Isolate,
  ) -> Result<(), ErrBox> {
    self.inner.register(source_code_info, isolate)
  }
}

// TODO(ry) It's ugly that we have both Script and OwnedScript. Ideally we
// wouldn't expose such twiddly complexity.
struct OwnedScript {
  pub source: String,
  pub filename: String,
}

impl From<Script<'_>> for OwnedScript {
  fn from(s: Script) -> OwnedScript {
    OwnedScript {
      source: s.source.to_string(),
      filename: s.filename.to_string(),
    }
  }
}

/// Represents data used to initialize isolate at startup
/// either a binary snapshot or a javascript source file
/// in the form of the StartupScript struct.
pub enum StartupData<'a> {
  Script(Script<'a>),
  Snapshot(&'static [u8]),
  OwnedSnapshot(v8::OwnedStartupData),
  None,
}

type JSErrorCreateFn = dyn Fn(V8Exception) -> ErrBox;
type IsolateErrorHandleFn = dyn FnMut(ErrBox) -> Result<(), ErrBox>;

pub struct ModuleInfo {
  pub main: bool,
  pub name: String,
  pub handle: v8::Global<v8::Module>,
  pub import_specifiers: Vec<String>,
}

/// A single execution context of JavaScript. Corresponds roughly to the "Web
/// Worker" concept in the DOM. An Isolate is a Future that can be used with
/// Tokio.  The Isolate future complete when there is an error or when all
/// pending ops have completed.
///
/// Ops are created in JavaScript by calling Deno.core.dispatch(), and in Rust
/// by implementing dispatcher function that takes control buffer and optional zero copy buffer
/// as arguments. An async Op corresponds exactly to a Promise in JavaScript.
#[allow(unused)]
pub struct Isolate {
  v8_isolate: Option<v8::OwnedIsolate>,
  snapshot_creator: Option<v8::SnapshotCreator>,
  has_snapshotted: bool,
  snapshot: Option<SnapshotConfig>,
  pub(crate) last_exception: Option<String>,
  pub(crate) last_exception_handle: v8::Global<v8::Value>,
  pub(crate) global_context: v8::Global<v8::Context>,
  pub(crate) shared_buf: DenoBuf,
  pub(crate) shared_ab: v8::Global<v8::SharedArrayBuffer>,
  pub(crate) js_recv_cb: v8::Global<v8::Function>,
  pub(crate) current_send_cb_info: *const v8::FunctionCallbackInfo,
  pub(crate) pending_promise_map: HashMap<i32, v8::Global<v8::Value>>,

  // TODO(bartlomieju): move into `core/modules.rs`
  mods_: HashMap<ModuleId, ModuleInfo>,
  pub(crate) next_dyn_import_id: DynImportId,
  pub(crate) dyn_import_map:
    HashMap<DynImportId, v8::Global<v8::PromiseResolver>>,
  pub(crate) resolve_context: *mut c_void,
  // TODO: end

  // TODO: These two fields were not yet ported from libdeno
  // void* global_import_buf_ptr_;
  // v8::Persistent<v8::ArrayBuffer> global_import_buf_;
  shared_isolate_handle: Arc<Mutex<Option<*mut v8::Isolate>>>,
  dyn_import: Option<Arc<DynImportFn>>,
  js_error_create: Arc<JSErrorCreateFn>,
  needs_init: bool,
  shared: SharedQueue,
  pending_ops: FuturesUnordered<PendingOpFuture>,
  pending_dyn_imports: FuturesUnordered<StreamFuture<IntoStream<DynImport>>>,
  have_unpolled_ops: bool,
  startup_script: Option<OwnedScript>,
  pub op_registry: Arc<OpRegistry>,
  waker: AtomicWaker,
  error_handler: Option<Box<IsolateErrorHandleFn>>,
}

unsafe impl Send for Isolate {}

impl Drop for Isolate {
  fn drop(&mut self) {
    // remove shared_libdeno_isolate reference
    *self.shared_isolate_handle.lock().unwrap() = None;

    // TODO Too much boiler plate.
    // <Boilerplate>
    let isolate = self.v8_isolate.take().unwrap();
    {
      let mut locker = v8::Locker::new(&isolate);
      let mut hs = v8::HandleScope::new(&mut locker);
      let scope = hs.enter();
      // </Boilerplate>
      self.global_context.reset(scope);
      self.shared_ab.reset(scope);
      self.last_exception_handle.reset(scope);
      self.js_recv_cb.reset(scope);
      for (_key, module) in self.mods_.iter_mut() {
        module.handle.reset(scope);
      }
      for (_key, handle) in self.dyn_import_map.iter_mut() {
        handle.reset(scope);
      }
      for (_key, handle) in self.pending_promise_map.iter_mut() {
        handle.reset(scope);
      }
    }
    if let Some(creator) = self.snapshot_creator.take() {
      // TODO(ry) V8 has a strange assert which prevents a SnapshotCreator from
      // being deallocated if it hasn't created a snapshot yet.
      // https://github.com/v8/v8/blob/73212783fbd534fac76cc4b66aac899c13f71fc8/src/api.cc#L603
      // If that assert is removed, this if guard could be removed.
      // WARNING: There may be false positive LSAN errors here.
      std::mem::forget(isolate);
      if self.has_snapshotted {
        drop(creator);
      }
    } else {
      drop(isolate);
    }
  }
}

static DENO_INIT: Once = Once::new();

#[allow(clippy::missing_safety_doc)]
pub unsafe fn v8_init() {
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

impl Isolate {
  /// startup_data defines the snapshot or script used at startup to initialize
  /// the isolate.
  pub fn new(startup_data: StartupData, will_snapshot: bool) -> Box<Self> {
    DENO_INIT.call_once(|| {
      unsafe { v8_init() };
    });

    let mut load_snapshot: Option<SnapshotConfig> = None;
    let mut startup_script: Option<OwnedScript> = None;

    // Separate into Option values for each startup type
    match startup_data {
      StartupData::Script(d) => {
        startup_script = Some(d.into());
      }
      StartupData::Snapshot(d) => {
        load_snapshot = Some(d.into());
      }
      StartupData::OwnedSnapshot(d) => {
        load_snapshot = Some(d.into());
      }
      StartupData::None => {}
    };

    let mut global_context = v8::Global::<v8::Context>::new();
    let (mut isolate, maybe_snapshot_creator) = if will_snapshot {
      // TODO(ry) Support loading snapshots before snapshotting.
      assert!(load_snapshot.is_none());
      let mut creator =
        v8::SnapshotCreator::new(Some(&bindings::EXTERNAL_REFERENCES));
      let isolate = unsafe { creator.get_owned_isolate() };
      let isolate = Isolate::setup_isolate(isolate);

      let mut locker = v8::Locker::new(&isolate);
      {
        let mut hs = v8::HandleScope::new(&mut locker);
        let scope = hs.enter();
        let context = v8::Context::new(scope);
        // context.enter();
        global_context.set(scope, context);
        creator.set_default_context(context);
        bindings::initialize_context(scope, context);
        // context.exit();
      }

      (isolate, Some(creator))
    } else {
      let mut params = v8::Isolate::create_params();
      params.set_array_buffer_allocator(v8::new_default_allocator());
      params.set_external_references(&bindings::EXTERNAL_REFERENCES);
      if let Some(ref mut snapshot) = load_snapshot {
        params.set_snapshot_blob(snapshot);
      }

      let load_snapshot_is_null = load_snapshot.is_none();
      let isolate = v8::Isolate::new(params);
      let isolate = Isolate::setup_isolate(isolate);

      {
        let mut locker = v8::Locker::new(&isolate);
        let mut hs = v8::HandleScope::new(&mut locker);
        let scope = hs.enter();
        let context = v8::Context::new(scope);

        if load_snapshot_is_null {
          // If no snapshot is provided, we initialize the context with empty
          // main source code and source maps.
          bindings::initialize_context(scope, context);
        }
        global_context.set(scope, context);
      }
      (isolate, None)
    };

    let shared = SharedQueue::new(RECOMMENDED_SIZE);
    let needs_init = true;

    let core_isolate = Self {
      v8_isolate: None,
      last_exception: None,
      last_exception_handle: v8::Global::<v8::Value>::new(),
      global_context,
      mods_: HashMap::new(),
      pending_promise_map: HashMap::new(),
      shared_buf: shared.as_deno_buf(),
      shared_ab: v8::Global::<v8::SharedArrayBuffer>::new(),
      js_recv_cb: v8::Global::<v8::Function>::new(),
      current_send_cb_info: std::ptr::null(),
      snapshot_creator: maybe_snapshot_creator,
      snapshot: load_snapshot,
      has_snapshotted: false,
      next_dyn_import_id: 0,
      dyn_import_map: HashMap::new(),
      resolve_context: std::ptr::null_mut(),
      shared_isolate_handle: Arc::new(Mutex::new(None)),
      dyn_import: None,
      js_error_create: Arc::new(CoreJSError::from_v8_exception),
      shared,
      needs_init,
      pending_ops: FuturesUnordered::new(),
      have_unpolled_ops: false,
      pending_dyn_imports: FuturesUnordered::new(),
      startup_script,
      op_registry: Arc::new(OpRegistry::new()),
      waker: AtomicWaker::new(),
      error_handler: None,
    };

    let mut boxed_isolate = Box::new(core_isolate);
    {
      let core_isolate_ptr: *mut Self = Box::into_raw(boxed_isolate);
      unsafe { isolate.set_data(0, core_isolate_ptr as *mut c_void) };
      boxed_isolate = unsafe { Box::from_raw(core_isolate_ptr) };
      let shared_handle_ptr = &mut *isolate;
      *boxed_isolate.shared_isolate_handle.lock().unwrap() =
        Some(shared_handle_ptr);
      boxed_isolate.v8_isolate = Some(isolate);
    }

    boxed_isolate
  }

  // Methods ported from libdeno, to be refactored
  pub fn setup_isolate(mut isolate: v8::OwnedIsolate) -> v8::OwnedIsolate {
    isolate.set_capture_stack_trace_for_uncaught_exceptions(true, 10);
    isolate.set_promise_reject_callback(bindings::promise_reject_callback);
    isolate.add_message_listener(bindings::message_callback);
    isolate.set_host_initialize_import_meta_object_callback(
      bindings::host_initialize_import_meta_object_callback,
    );
    isolate.set_host_import_module_dynamically_callback(
      bindings::host_import_module_dynamically_callback,
    );
    isolate
  }

  pub fn get_module_info(&self, id: ModuleId) -> Option<&ModuleInfo> {
    if id == 0 {
      return None;
    }
    self.mods_.get(&id)
  }

  pub fn handle_exception<'a>(
    &mut self,
    s: &mut impl v8::ToLocal<'a>,
    mut context: v8::Local<'a, v8::Context>,
    exception: v8::Local<'a, v8::Value>,
  ) {
    let isolate = context.get_isolate();
    // TerminateExecution was called
    if isolate.is_execution_terminating() {
      // cancel exception termination so that the exception can be created
      isolate.cancel_terminate_execution();

      // maybe make a new exception object
      let exception = if exception.is_null_or_undefined() {
        let exception_str = v8::String::new(s, "execution terminated").unwrap();
        isolate.enter();
        let e = v8::error(s, exception_str);
        isolate.exit();
        e
      } else {
        exception
      };

      // handle the exception as if it is a regular exception
      self.handle_exception(s, context, exception);

      // re-enable exception termination
      context.get_isolate().terminate_execution();
      return;
    }

    let json_str = self.encode_exception_as_json(s, context, exception);
    self.last_exception = Some(json_str);
    self.last_exception_handle.set(s, exception);
  }

  pub fn encode_exception_as_json<'a>(
    &mut self,
    s: &mut impl v8::ToLocal<'a>,
    context: v8::Local<'a, v8::Context>,
    exception: v8::Local<'a, v8::Value>,
  ) -> String {
    let message = v8::create_message(s, exception);
    self.encode_message_as_json(s, context, message)
  }

  pub fn encode_message_as_json<'a>(
    &mut self,
    s: &mut impl v8::ToLocal<'a>,
    context: v8::Local<v8::Context>,
    message: v8::Local<v8::Message>,
  ) -> String {
    let json_obj = self.encode_message_as_object(s, context, message);
    let json_string = v8::json::stringify(context, json_obj.into()).unwrap();
    json_string.to_rust_string_lossy(s)
  }

  fn encode_message_as_object<'a>(
    &mut self,
    s: &mut impl v8::ToLocal<'a>,
    context: v8::Local<v8::Context>,
    message: v8::Local<v8::Message>,
  ) -> v8::Local<'a, v8::Object> {
    let json_obj = v8::Object::new(s);

    let exception_str = message.get(s);
    json_obj.set(
      context,
      v8::String::new(s, "message").unwrap().into(),
      exception_str.into(),
    );

    let script_resource_name = message
      .get_script_resource_name(s)
      .expect("Missing ScriptResourceName");
    json_obj.set(
      context,
      v8::String::new(s, "scriptResourceName").unwrap().into(),
      script_resource_name,
    );

    let source_line = message
      .get_source_line(s, context)
      .expect("Missing SourceLine");
    json_obj.set(
      context,
      v8::String::new(s, "sourceLine").unwrap().into(),
      source_line.into(),
    );

    let line_number = message
      .get_line_number(context)
      .expect("Missing LineNumber");
    json_obj.set(
      context,
      v8::String::new(s, "lineNumber").unwrap().into(),
      v8::Integer::new(s, line_number as i32).into(),
    );

    json_obj.set(
      context,
      v8::String::new(s, "startPosition").unwrap().into(),
      v8::Integer::new(s, message.get_start_position() as i32).into(),
    );

    json_obj.set(
      context,
      v8::String::new(s, "endPosition").unwrap().into(),
      v8::Integer::new(s, message.get_end_position() as i32).into(),
    );

    json_obj.set(
      context,
      v8::String::new(s, "errorLevel").unwrap().into(),
      v8::Integer::new(s, message.error_level() as i32).into(),
    );

    json_obj.set(
      context,
      v8::String::new(s, "startColumn").unwrap().into(),
      v8::Integer::new(s, message.get_start_column() as i32).into(),
    );

    json_obj.set(
      context,
      v8::String::new(s, "endColumn").unwrap().into(),
      v8::Integer::new(s, message.get_end_column() as i32).into(),
    );

    let is_shared_cross_origin =
      v8::Boolean::new(s, message.is_shared_cross_origin());

    json_obj.set(
      context,
      v8::String::new(s, "isSharedCrossOrigin").unwrap().into(),
      is_shared_cross_origin.into(),
    );

    let is_opaque = v8::Boolean::new(s, message.is_opaque());

    json_obj.set(
      context,
      v8::String::new(s, "isOpaque").unwrap().into(),
      is_opaque.into(),
    );

    let frames = if let Some(stack_trace) = message.get_stack_trace(s) {
      let count = stack_trace.get_frame_count() as i32;
      let frames = v8::Array::new(s, count);

      for i in 0..count {
        let frame = stack_trace
          .get_frame(s, i as usize)
          .expect("No frame found");
        let frame_obj = v8::Object::new(s);
        frames.set(context, v8::Integer::new(s, i).into(), frame_obj.into());
        frame_obj.set(
          context,
          v8::String::new(s, "line").unwrap().into(),
          v8::Integer::new(s, frame.get_line_number() as i32).into(),
        );
        frame_obj.set(
          context,
          v8::String::new(s, "column").unwrap().into(),
          v8::Integer::new(s, frame.get_column() as i32).into(),
        );

        if let Some(function_name) = frame.get_function_name(s) {
          frame_obj.set(
            context,
            v8::String::new(s, "functionName").unwrap().into(),
            function_name.into(),
          );
        }

        let script_name = match frame.get_script_name_or_source_url(s) {
          Some(name) => name,
          None => v8::String::new(s, "<unknown>").unwrap(),
        };
        frame_obj.set(
          context,
          v8::String::new(s, "scriptName").unwrap().into(),
          script_name.into(),
        );

        frame_obj.set(
          context,
          v8::String::new(s, "isEval").unwrap().into(),
          v8::Boolean::new(s, frame.is_eval()).into(),
        );

        frame_obj.set(
          context,
          v8::String::new(s, "isConstructor").unwrap().into(),
          v8::Boolean::new(s, frame.is_constructor()).into(),
        );

        frame_obj.set(
          context,
          v8::String::new(s, "isWasm").unwrap().into(),
          v8::Boolean::new(s, frame.is_wasm()).into(),
        );
      }

      frames
    } else {
      // No stack trace. We only have one stack frame of info..
      let frames = v8::Array::new(s, 1);
      let frame_obj = v8::Object::new(s);
      frames.set(context, v8::Integer::new(s, 0).into(), frame_obj.into());

      frame_obj.set(
        context,
        v8::String::new(s, "scriptResourceName").unwrap().into(),
        script_resource_name,
      );
      frame_obj.set(
        context,
        v8::String::new(s, "line").unwrap().into(),
        v8::Integer::new(s, line_number as i32).into(),
      );
      frame_obj.set(
        context,
        v8::String::new(s, "column").unwrap().into(),
        v8::Integer::new(s, message.get_start_column() as i32).into(),
      );

      frames
    };

    json_obj.set(
      context,
      v8::String::new(s, "frames").unwrap().into(),
      frames.into(),
    );

    json_obj
  }

  #[allow(dead_code)]
  pub fn run_microtasks(&mut self) {
    let isolate = self.v8_isolate.as_mut().unwrap();
    let _locker = v8::Locker::new(isolate);
    isolate.enter();
    isolate.run_microtasks();
    isolate.exit();
  }
  // End of methods from libdeno

  pub fn set_error_handler(&mut self, handler: Box<IsolateErrorHandleFn>) {
    self.error_handler = Some(handler);
  }

  /// Defines the how Deno.core.dispatch() acts.
  /// Called whenever Deno.core.dispatch() is called in JavaScript. zero_copy_buf
  /// corresponds to the second argument of Deno.core.dispatch().
  ///
  /// Requires runtime to explicitly ask for op ids before using any of the ops.
  pub fn register_op<F>(&self, name: &str, op: F) -> OpId
  where
    F: Fn(&[u8], Option<PinnedBuf>) -> CoreOp + Send + Sync + 'static,
  {
    self.op_registry.register(name, op)
  }

  pub fn set_dyn_import<F>(&mut self, f: F)
  where
    F: Fn(DynImportId, &str, &str) -> DynImportStream + Send + Sync + 'static,
  {
    self.dyn_import = Some(Arc::new(f));
  }

  /// Allows a callback to be set whenever a V8 exception is made. This allows
  /// the caller to wrap the V8Exception into an error. By default this callback
  /// is set to CoreJSError::from_v8_exception.
  pub fn set_js_error_create<F>(&mut self, f: F)
  where
    F: Fn(V8Exception) -> ErrBox + 'static,
  {
    self.js_error_create = Arc::new(f);
  }

  /// Get a thread safe handle on the isolate.
  pub fn shared_isolate_handle(&mut self) -> IsolateHandle {
    IsolateHandle {
      shared_isolate: self.shared_isolate_handle.clone(),
    }
  }

  /// Executes a bit of built-in JavaScript to provide Deno.sharedQueue.
  fn shared_init(&mut self) {
    if self.needs_init {
      self.needs_init = false;
      js_check(
        self.execute("shared_queue.js", include_str!("shared_queue.js")),
      );
      // Maybe execute the startup script.
      if let Some(s) = self.startup_script.take() {
        self.execute(&s.filename, &s.source).unwrap()
      }
    }
  }

  pub fn dyn_import_cb(
    &mut self,
    specifier: &str,
    referrer: &str,
    id: DynImportId,
  ) {
    debug!("dyn_import specifier {} referrer {} ", specifier, referrer);

    if let Some(ref f) = self.dyn_import {
      let inner = f(id, specifier, referrer);
      let stream = DynImport { inner, id };
      self.waker.wake();
      self
        .pending_dyn_imports
        .push(stream.into_stream().into_future());
    } else {
      panic!("dyn_import callback not set")
    }
  }

  pub fn pre_dispatch(
    &mut self,
    op_id: OpId,
    control_buf: DenoBuf,
    zero_copy_buf: Option<PinnedBuf>,
  ) {
    let maybe_op =
      self
        .op_registry
        .call(op_id, control_buf.as_ref(), zero_copy_buf);

    let op = match maybe_op {
      Some(op) => op,
      None => {
        return self.throw_exception(&format!("Unknown op id: {}", op_id))
      }
    };

    debug_assert_eq!(self.shared.size(), 0);
    match op {
      Op::Sync(buf) => {
        // For sync messages, we always return the response via Deno.core.send's
        // return value. Sync messages ignore the op_id.
        let op_id = 0;
        self
          .respond(Some((op_id, &buf)))
          // Because this is a sync op, deno_respond() does not actually call
          // into JavaScript. We should not get an error here.
          .expect("unexpected error");
      }
      Op::Async(fut) => {
        let fut2 = fut.map_ok(move |buf| (op_id, buf));
        self.pending_ops.push(fut2.boxed());
        self.have_unpolled_ops = true;
      }
    }
  }

  fn libdeno_execute<'a>(
    &mut self,
    s: &mut impl v8::ToLocal<'a>,
    context: v8::Local<'a, v8::Context>,
    js_filename: &str,
    js_source: &str,
  ) -> bool {
    let mut hs = v8::HandleScope::new(s);
    let s = hs.enter();
    let source = v8::String::new(s, js_source).unwrap();
    let name = v8::String::new(s, js_filename).unwrap();
    let mut try_catch = v8::TryCatch::new(s);
    let tc = try_catch.enter();
    let origin = bindings::script_origin(s, name);
    let mut script =
      v8::Script::compile(s, context, source, Some(&origin)).unwrap();
    let result = script.run(s, context);
    if result.is_none() {
      assert!(tc.has_caught());
      let exception = tc.exception().unwrap();
      self.handle_exception(s, context, exception);
      false
    } else {
      true
    }
  }

  /// Executes traditional JavaScript code (traditional = not ES modules)
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is CoreJSError, however it may be a
  /// different type if Isolate::set_js_error_create() has been used.
  pub fn execute(
    &mut self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), ErrBox> {
    self.shared_init();
    let isolate = self.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(isolate);
    assert!(!self.global_context.is_empty());
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    let mut context = self.global_context.get(scope).unwrap();
    context.enter();
    self.libdeno_execute(scope, context, js_filename, js_source);
    context.exit();
    self.check_last_exception()
  }

  fn check_last_exception(&mut self) -> Result<(), ErrBox> {
    let maybe_err = self.last_exception.clone();
    match maybe_err {
      None => Ok(()),
      Some(json_str) => {
        let js_error_create = &*self.js_error_create;
        if self.error_handler.is_some() {
          // We need to clear last exception to avoid double handling.
          self.last_exception = None;
          let v8_exception = V8Exception::from_json(&json_str).unwrap();
          let js_error = js_error_create(v8_exception);
          let handler = self.error_handler.as_mut().unwrap();
          handler(js_error)
        } else {
          let v8_exception = V8Exception::from_json(&json_str).unwrap();
          let js_error = js_error_create(v8_exception);
          Err(js_error)
        }
      }
    }
  }

  fn check_promise_errors(&mut self) {
    let isolate = self.v8_isolate.as_ref().unwrap();

    if self.pending_promise_map.is_empty() {
      return;
    }

    let mut locker = v8::Locker::new(isolate);
    assert!(!self.global_context.is_empty());
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    let mut context = self.global_context.get(scope).unwrap();
    context.enter();

    let pending_promises: Vec<(i32, v8::Global<v8::Value>)> =
      self.pending_promise_map.drain().collect();
    for (_promise_id, mut handle) in pending_promises {
      let error = handle.get(scope).expect("Empty error handle");
      self.handle_exception(scope, context, error);
      handle.reset(scope);
    }

    context.exit();
  }

  fn throw_exception(&mut self, text: &str) {
    let isolate = self.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(isolate);
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    let msg = v8::String::new(scope, text).unwrap();
    isolate.throw_exception(msg.into());
  }

  fn libdeno_respond(&mut self, op_id: OpId, buf: DenoBuf) {
    if !self.current_send_cb_info.is_null() {
      // Synchronous response.
      // Note op_id is not passed back in the case of synchronous response.
      let isolate = self.v8_isolate.as_ref().unwrap();
      let mut locker = v8::Locker::new(isolate);
      assert!(!self.global_context.is_empty());
      let mut hs = v8::HandleScope::new(&mut locker);
      let scope = hs.enter();

      if !buf.data_ptr.is_null() && buf.data_len > 0 {
        let ab = unsafe { bindings::buf_to_uint8array(scope, buf) };
        let info: &v8::FunctionCallbackInfo =
          unsafe { &*self.current_send_cb_info };
        let rv = &mut info.get_return_value();
        rv.set(ab.into())
      }

      self.current_send_cb_info = std::ptr::null();
      return;
    }

    let isolate = self.v8_isolate.as_ref().unwrap();
    // println!("deno_execute -> Isolate ptr {:?}", isolate);
    let mut locker = v8::Locker::new(isolate);
    assert!(!self.global_context.is_empty());
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    let mut context = self.global_context.get(scope).unwrap();
    context.enter();

    let mut try_catch = v8::TryCatch::new(scope);
    let tc = try_catch.enter();

    let js_recv_cb = self.js_recv_cb.get(scope);

    if js_recv_cb.is_none() {
      let msg = "Deno.core.recv has not been called.".to_string();
      self.last_exception = Some(msg);
      return;
    }

    let mut argc = 0;
    let mut args: Vec<v8::Local<v8::Value>> = vec![];

    if !buf.data_ptr.is_null() {
      argc = 2;
      let op_id = v8::Integer::new(scope, op_id as i32);
      args.push(op_id.into());
      let buf = unsafe { bindings::buf_to_uint8array(scope, buf) };
      args.push(buf.into());
    }

    let global = context.global(scope);
    let maybe_value =
      js_recv_cb
        .unwrap()
        .call(scope, context, global.into(), argc, args);

    if tc.has_caught() {
      assert!(maybe_value.is_none());
      self.handle_exception(scope, context, tc.exception().unwrap());
    }
    context.exit();
  }

  fn respond(
    &mut self,
    maybe_buf: Option<(OpId, &[u8])>,
  ) -> Result<(), ErrBox> {
    let (op_id, buf) = match maybe_buf {
      None => (0, DenoBuf::empty()),
      Some((op_id, r)) => (op_id, DenoBuf::from(r)),
    };
    self.libdeno_respond(op_id, buf);
    self.check_last_exception()
  }

  fn mod_new2(&mut self, main: bool, name: &str, source: &str) -> ModuleId {
    let isolate = self.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(&isolate);

    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    assert!(!self.global_context.is_empty());
    let mut context = self.global_context.get(scope).unwrap();
    context.enter();

    let name_str = v8::String::new(scope, name).unwrap();
    let source_str = v8::String::new(scope, source).unwrap();

    let origin = bindings::module_origin(scope, name_str);
    let source = v8::script_compiler::Source::new(source_str, &origin);

    let mut try_catch = v8::TryCatch::new(scope);
    let tc = try_catch.enter();

    let maybe_module = v8::script_compiler::compile_module(&isolate, source);

    if tc.has_caught() {
      assert!(maybe_module.is_none());
      self.handle_exception(scope, context, tc.exception().unwrap());
      context.exit();
      return 0;
    }
    let module = maybe_module.unwrap();
    let id = module.get_identity_hash();

    let mut import_specifiers: Vec<String> = vec![];
    for i in 0..module.get_module_requests_length() {
      let specifier = module.get_module_request(i);
      import_specifiers.push(specifier.to_rust_string_lossy(scope));
    }

    let mut handle = v8::Global::<v8::Module>::new();
    handle.set(scope, module);
    self.mods_.insert(
      id,
      ModuleInfo {
        main,
        name: name.to_string(),
        import_specifiers,
        handle,
      },
    );
    context.exit();
    id
  }

  /// Low-level module creation.
  pub fn mod_new(
    &mut self,
    main: bool,
    name: &str,
    source: &str,
  ) -> Result<ModuleId, ErrBox> {
    let id = self.mod_new2(main, name, source);
    self.check_last_exception().map(|_| id)
  }

  pub fn mod_get_imports(&self, id: ModuleId) -> Vec<String> {
    let info = self.get_module_info(id).unwrap();
    let len = info.import_specifiers.len();
    let mut out = Vec::new();
    for i in 0..len {
      let info = self.get_module_info(id).unwrap();
      let specifier = info.import_specifiers.get(i).unwrap().to_string();
      out.push(specifier);
    }
    out
  }

  /// Takes a snapshot. The isolate should have been created with will_snapshot
  /// set to true.
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is CoreJSError, however it may be a
  /// different type if Isolate::set_js_error_create() has been used.
  pub fn snapshot(&mut self) -> Result<v8::OwnedStartupData, ErrBox> {
    assert!(self.snapshot_creator.is_some());

    let isolate = self.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(isolate);
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();

    for (_key, module) in self.mods_.iter_mut() {
      module.handle.reset(scope);
    }
    self.mods_.clear();
    self.global_context.reset(scope);

    let snapshot_creator = self.snapshot_creator.as_mut().unwrap();
    let snapshot = snapshot_creator
      .create_blob(v8::FunctionCodeHandling::Keep)
      .unwrap();
    self.has_snapshotted = true;
    match self.check_last_exception() {
      Ok(..) => Ok(snapshot),
      Err(err) => Err(err),
    }
  }

  fn dyn_import_done(
    &mut self,
    id: DynImportId,
    result: Result<ModuleId, Option<String>>,
  ) -> Result<(), ErrBox> {
    debug!("dyn_import_done {} {:?}", id, result);
    let (mod_id, maybe_err_str) = match result {
      Ok(mod_id) => (mod_id, None),
      Err(None) => (0, None),
      Err(Some(err_str)) => (0, Some(err_str)),
    };

    assert!(
      (mod_id == 0 && maybe_err_str.is_some())
        || (mod_id != 0 && maybe_err_str.is_none())
        || (mod_id == 0 && !self.last_exception_handle.is_empty())
    );

    let isolate = self.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(isolate);
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    assert!(!self.global_context.is_empty());
    let mut context = self.global_context.get(scope).unwrap();
    context.enter();

    // TODO(ry) error on bad import_id.
    let mut resolver_handle = self.dyn_import_map.remove(&id).unwrap();
    // Resolve.
    let mut resolver = resolver_handle.get(scope).unwrap();
    resolver_handle.reset(scope);

    let maybe_info = self.get_module_info(mod_id);

    if let Some(info) = maybe_info {
      // Resolution success
      let mut module = info.handle.get(scope).unwrap();
      assert_eq!(module.get_status(), v8::ModuleStatus::Evaluated);
      let module_namespace = module.get_module_namespace();
      resolver.resolve(context, module_namespace).unwrap();
    } else {
      // Resolution error.
      if let Some(error_str) = maybe_err_str {
        let msg = v8::String::new(scope, &error_str).unwrap();
        let isolate = context.get_isolate();
        isolate.enter();
        let e = v8::type_error(scope, msg);
        isolate.exit();
        resolver.reject(context, e).unwrap();
      } else {
        let e = self.last_exception_handle.get(scope).unwrap();
        self.last_exception_handle.reset(scope);
        self.last_exception.take();
        resolver.reject(context, e).unwrap();
      }
    }

    isolate.run_microtasks();

    context.exit();
    self.check_last_exception()
  }

  fn poll_dyn_imports(&mut self, cx: &mut Context) -> Poll<Result<(), ErrBox>> {
    use RecursiveLoadEvent::*;
    loop {
      match self.pending_dyn_imports.poll_next_unpin(cx) {
        Poll::Pending | Poll::Ready(None) => {
          // There are no active dynamic import loaders, or none are ready.
          return Poll::Ready(Ok(()));
        }
        Poll::Ready(Some((
          Some(Ok((dyn_import_id, Fetch(source_code_info)))),
          mut stream,
        ))) => {
          // A module (not necessarily the one dynamically imported) has been
          // fetched. Create and register it, and if successful, poll for the
          // next recursive-load event related to this dynamic import.
          match stream.get_mut().register(source_code_info, self) {
            Ok(()) => self.pending_dyn_imports.push(stream.into_future()),
            Err(err) => {
              self.dyn_import_done(dyn_import_id, Err(Some(err.to_string())))?
            }
          }
        }
        Poll::Ready(Some((
          Some(Ok((dyn_import_id, Instantiate(module_id)))),
          _,
        ))) => {
          // The top-level module from a dynamic import has been instantiated.
          match self.mod_evaluate(module_id) {
            Ok(()) => self.dyn_import_done(dyn_import_id, Ok(module_id))?,
            Err(..) => self.dyn_import_done(dyn_import_id, Err(None))?,
          }
        }
        Poll::Ready(Some((Some(Err((dyn_import_id, err))), _))) => {
          // A non-javascript error occurred; this could be due to a an invalid
          // module specifier, or a problem with the source map, or a failure
          // to fetch the module source code.
          self.dyn_import_done(dyn_import_id, Err(Some(err.to_string())))?
        }
        Poll::Ready(Some((None, _))) => unreachable!(),
      }
    }
  }
}

/// Called during mod_instantiate() to resolve imports.
type ResolveFn<'a> = dyn FnMut(&str, ModuleId) -> ModuleId + 'a;

/// Used internally by Isolate::mod_instantiate to wrap ResolveFn and
/// encapsulate pointer casts.
pub struct ResolveContext<'a> {
  pub resolve_fn: &'a mut ResolveFn<'a>,
}

impl<'a> ResolveContext<'a> {
  #[inline]
  fn as_raw_ptr(&mut self) -> *mut c_void {
    self as *mut _ as *mut c_void
  }

  #[allow(clippy::missing_safety_doc)]
  #[inline]
  pub(crate) unsafe fn from_raw_ptr(ptr: *mut c_void) -> &'a mut Self {
    &mut *(ptr as *mut _)
  }
}

impl Isolate {
  fn libdeno_mod_instantiate(
    &mut self,
    mut ctx: ResolveContext<'_>,
    id: ModuleId,
  ) {
    self.resolve_context = ctx.as_raw_ptr();
    let isolate = self.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(isolate);
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    assert!(!self.global_context.is_empty());
    let mut context = self.global_context.get(scope).unwrap();
    context.enter();
    let mut try_catch = v8::TryCatch::new(scope);
    let tc = try_catch.enter();

    let maybe_info = self.get_module_info(id);

    if maybe_info.is_none() {
      return;
    }

    let module_handle = &maybe_info.unwrap().handle;
    let mut module = module_handle.get(scope).unwrap();

    if module.get_status() == v8::ModuleStatus::Errored {
      return;
    }

    let maybe_ok =
      module.instantiate_module(context, bindings::module_resolve_callback);
    assert!(maybe_ok.is_some() || tc.has_caught());

    if tc.has_caught() {
      self.handle_exception(scope, context, tc.exception().unwrap());
    }

    context.exit();
    self.resolve_context = std::ptr::null_mut();
  }
  /// Instanciates a ES module
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is CoreJSError, however it may be a
  /// different type if Isolate::set_js_error_create() has been used.
  pub fn mod_instantiate(
    &mut self,
    id: ModuleId,
    resolve_fn: &mut ResolveFn,
  ) -> Result<(), ErrBox> {
    let ctx = ResolveContext { resolve_fn };
    self.libdeno_mod_instantiate(ctx, id);
    self.check_last_exception()
  }

  /// Evaluates an already instantiated ES module.
  ///
  /// ErrBox can be downcast to a type that exposes additional information about
  /// the V8 exception. By default this type is CoreJSError, however it may be a
  /// different type if Isolate::set_js_error_create() has been used.
  pub fn mod_evaluate(&mut self, id: ModuleId) -> Result<(), ErrBox> {
    self.shared_init();
    let isolate = self.v8_isolate.as_ref().unwrap();
    let mut locker = v8::Locker::new(isolate);
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    assert!(!self.global_context.is_empty());
    let mut context = self.global_context.get(scope).unwrap();
    context.enter();

    let info = self.get_module_info(id).expect("ModuleInfo not found");
    let mut module = info.handle.get(scope).expect("Empty module handle");
    let mut status = module.get_status();

    if status == v8::ModuleStatus::Instantiated {
      let ok = module.evaluate(scope, context).is_some();
      // Update status after evaluating.
      status = module.get_status();
      if ok {
        assert!(
          status == v8::ModuleStatus::Evaluated
            || status == v8::ModuleStatus::Errored
        );
      } else {
        assert!(status == v8::ModuleStatus::Errored);
      }
    }

    match status {
      v8::ModuleStatus::Evaluated => {
        self.last_exception_handle.reset(scope);
        self.last_exception.take();
      }
      v8::ModuleStatus::Errored => {
        self.handle_exception(scope, context, module.get_exception());
      }
      other => panic!("Unexpected module status {:?}", other),
    };

    context.exit();

    self.check_last_exception()
  }
}

impl Future for Isolate {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();

    inner.waker.register(cx.waker());

    inner.shared_init();

    let mut overflow_response: Option<(OpId, Buf)> = None;

    loop {
      // If there are any pending dyn_import futures, do those first.
      if !inner.pending_dyn_imports.is_empty() {
        let poll_imports = inner.poll_dyn_imports(cx)?;
        assert!(poll_imports.is_ready());
      }

      // Now handle actual ops.
      inner.have_unpolled_ops = false;
      #[allow(clippy::match_wild_err_arm)]
      match inner.pending_ops.poll_next_unpin(cx) {
        Poll::Ready(Some(Err(_))) => panic!("unexpected op error"),
        Poll::Ready(None) => break,
        Poll::Pending => break,
        Poll::Ready(Some(Ok((op_id, buf)))) => {
          let successful_push = inner.shared.push(op_id, &buf);
          if !successful_push {
            // If we couldn't push the response to the shared queue, because
            // there wasn't enough size, we will return the buffer via the
            // legacy route, using the argument of deno_respond.
            overflow_response = Some((op_id, buf));
            break;
          }
        }
      }
    }

    if inner.shared.size() > 0 {
      inner.respond(None)?;
      // The other side should have shifted off all the messages.
      assert_eq!(inner.shared.size(), 0);
    }

    if overflow_response.is_some() {
      let (op_id, buf) = overflow_response.take().unwrap();
      inner.respond(Some((op_id, &buf)))?;
    }

    inner.check_promise_errors();
    inner.check_last_exception()?;

    // We're idle if pending_ops is empty.
    if inner.pending_ops.is_empty() && inner.pending_dyn_imports.is_empty() {
      Poll::Ready(Ok(()))
    } else {
      if inner.have_unpolled_ops {
        inner.waker.wake();
      }
      Poll::Pending
    }
  }
}

/// IsolateHandle is a thread safe handle on an Isolate. It exposed thread safe V8 functions.
#[derive(Clone)]
pub struct IsolateHandle {
  shared_isolate: Arc<Mutex<Option<*mut v8::Isolate>>>,
}

unsafe impl Send for IsolateHandle {}

impl IsolateHandle {
  /// Terminate the execution of any currently running javascript.
  /// After terminating execution it is probably not wise to continue using
  /// the isolate.
  pub fn terminate_execution(&self) {
    if let Some(isolate) = *self.shared_isolate.lock().unwrap() {
      let isolate = unsafe { &mut *isolate };
      isolate.terminate_execution();
    }
  }
}

pub fn js_check<T>(r: Result<T, ErrBox>) -> T {
  if let Err(e) = r {
    panic!(e.to_string());
  }
  r.unwrap()
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use futures::future::lazy;
  use std::io;
  use std::ops::FnOnce;
  use std::sync::atomic::{AtomicUsize, Ordering};

  pub fn run_in_task<F>(f: F)
  where
    F: FnOnce(&mut Context) + Send + 'static,
  {
    futures::executor::block_on(lazy(move |cx| f(cx)));
  }

  fn poll_until_ready<F>(future: &mut F, max_poll_count: usize) -> F::Output
  where
    F: Future + Unpin,
  {
    let mut cx = Context::from_waker(futures::task::noop_waker_ref());
    for _ in 0..max_poll_count {
      match future.poll_unpin(&mut cx) {
        Poll::Pending => continue,
        Poll::Ready(val) => return val,
      }
    }
    panic!(
      "Isolate still not ready after polling {} times.",
      max_poll_count
    )
  }

  pub enum Mode {
    Async,
    OverflowReqSync,
    OverflowResSync,
    OverflowReqAsync,
    OverflowResAsync,
  }

  pub fn setup(mode: Mode) -> (Box<Isolate>, Arc<AtomicUsize>) {
    let dispatch_count = Arc::new(AtomicUsize::new(0));
    let dispatch_count_ = dispatch_count.clone();

    let mut isolate = Isolate::new(StartupData::None, false);

    let dispatcher =
      move |control: &[u8], _zero_copy: Option<PinnedBuf>| -> CoreOp {
        dispatch_count_.fetch_add(1, Ordering::Relaxed);
        match mode {
          Mode::Async => {
            assert_eq!(control.len(), 1);
            assert_eq!(control[0], 42);
            let buf = vec![43u8, 0, 0, 0].into_boxed_slice();
            Op::Async(futures::future::ok(buf).boxed())
          }
          Mode::OverflowReqSync => {
            assert_eq!(control.len(), 100 * 1024 * 1024);
            let buf = vec![43u8, 0, 0, 0].into_boxed_slice();
            Op::Sync(buf)
          }
          Mode::OverflowResSync => {
            assert_eq!(control.len(), 1);
            assert_eq!(control[0], 42);
            let mut vec = Vec::<u8>::new();
            vec.resize(100 * 1024 * 1024, 0);
            vec[0] = 99;
            let buf = vec.into_boxed_slice();
            Op::Sync(buf)
          }
          Mode::OverflowReqAsync => {
            assert_eq!(control.len(), 100 * 1024 * 1024);
            let buf = vec![43u8, 0, 0, 0].into_boxed_slice();
            Op::Async(futures::future::ok(buf).boxed())
          }
          Mode::OverflowResAsync => {
            assert_eq!(control.len(), 1);
            assert_eq!(control[0], 42);
            let mut vec = Vec::<u8>::new();
            vec.resize(100 * 1024 * 1024, 0);
            vec[0] = 4;
            let buf = vec.into_boxed_slice();
            Op::Async(futures::future::ok(buf).boxed())
          }
        }
      };

    isolate.register_op("test", dispatcher);

    js_check(isolate.execute(
      "setup.js",
      r#"
        function assert(cond) {
          if (!cond) {
            throw Error("assert");
          }
        }
        "#,
    ));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    (isolate, dispatch_count)
  }

  #[test]
  fn test_dispatch() {
    let (mut isolate, dispatch_count) = setup(Mode::Async);
    js_check(isolate.execute(
      "filename.js",
      r#"
        let control = new Uint8Array([42]);
        Deno.core.send(1, control);
        async function main() {
          Deno.core.send(1, control);
        }
        main();
        "#,
    ));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
  }

  #[test]
  fn test_mods() {
    let (mut isolate, dispatch_count) = setup(Mode::Async);
    let mod_a = isolate
      .mod_new(
        true,
        "a.js",
        r#"
        import { b } from 'b.js'
        if (b() != 'b') throw Error();
        let control = new Uint8Array([42]);
        Deno.core.send(1, control);
      "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);

    let imports = isolate.mod_get_imports(mod_a);
    assert_eq!(imports, vec!["b.js".to_string()]);
    let mod_b = isolate
      .mod_new(false, "b.js", "export function b() { return 'b' }")
      .unwrap();
    let imports = isolate.mod_get_imports(mod_b);
    assert_eq!(imports.len(), 0);

    let resolve_count = Arc::new(AtomicUsize::new(0));
    let resolve_count_ = resolve_count.clone();

    let mut resolve = move |specifier: &str, _referrer: ModuleId| -> ModuleId {
      resolve_count_.fetch_add(1, Ordering::SeqCst);
      assert_eq!(specifier, "b.js");
      mod_b
    };

    js_check(isolate.mod_instantiate(mod_b, &mut resolve));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 0);

    js_check(isolate.mod_instantiate(mod_a, &mut resolve));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);

    js_check(isolate.mod_evaluate(mod_a));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);
  }

  #[test]
  fn test_poll_async_delayed_ops() {
    run_in_task(|cx| {
      let (mut isolate, dispatch_count) = setup(Mode::Async);

      js_check(isolate.execute(
        "setup2.js",
        r#"
         let nrecv = 0;
         Deno.core.setAsyncHandler((opId, buf) => {
           nrecv++;
         });
         "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
      js_check(isolate.execute(
        "check1.js",
        r#"
         assert(nrecv == 0);
         let control = new Uint8Array([42]);
         Deno.core.send(1, control);
         assert(nrecv == 0);
         "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      js_check(isolate.execute(
        "check2.js",
        r#"
         assert(nrecv == 1);
         Deno.core.send(1, control);
         assert(nrecv == 1);
         "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      js_check(isolate.execute("check3.js", "assert(nrecv == 2)"));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      // We are idle, so the next poll should be the last.
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
    });
  }

  struct MockImportStream(Vec<Result<RecursiveLoadEvent, ErrBox>>);

  impl Stream for MockImportStream {
    type Item = Result<RecursiveLoadEvent, ErrBox>;

    fn poll_next(
      self: Pin<&mut Self>,
      _cx: &mut Context,
    ) -> Poll<Option<Self::Item>> {
      let inner = self.get_mut();
      let event = if inner.0.is_empty() {
        None
      } else {
        Some(inner.0.remove(0))
      };
      Poll::Ready(event)
    }
  }

  impl ImportStream for MockImportStream {
    fn register(
      &mut self,
      module_data: SourceCodeInfo,
      isolate: &mut Isolate,
    ) -> Result<(), ErrBox> {
      let id = isolate.mod_new(
        false,
        &module_data.module_url_found,
        &module_data.code,
      )?;
      println!(
        "MockImportStream register {} {}",
        id, module_data.module_url_found
      );
      Ok(())
    }
  }

  #[test]
  fn dyn_import_err() {
    // Test an erroneous dynamic import where the specified module isn't found.
    run_in_task(|cx| {
      let count = Arc::new(AtomicUsize::new(0));
      let count_ = count.clone();
      let mut isolate = Isolate::new(StartupData::None, false);
      isolate.set_dyn_import(move |_, specifier, referrer| {
        count_.fetch_add(1, Ordering::Relaxed);
        assert_eq!(specifier, "foo.js");
        assert_eq!(referrer, "dyn_import2.js");
        let err = io::Error::from(io::ErrorKind::NotFound);
        let stream = MockImportStream(vec![Err(err.into())]);
        Box::new(stream)
      });
      js_check(isolate.execute(
        "dyn_import2.js",
        r#"
        (async () => {
          await import("foo.js");
        })();
        "#,
      ));
      assert_eq!(count.load(Ordering::Relaxed), 1);

      // We should get an error here.
      let result = isolate.poll_unpin(cx);
      if let Poll::Ready(Ok(_)) = result {
        unreachable!();
      }
    })
  }

  #[test]
  fn dyn_import_err2() {
    use std::convert::TryInto;
    // Import multiple modules to demonstrate that after failed dynamic import
    // another dynamic import can still be run
    run_in_task(|cx| {
      let count = Arc::new(AtomicUsize::new(0));
      let count_ = count.clone();
      let mut isolate = Isolate::new(StartupData::None, false);
      isolate.set_dyn_import(move |_, specifier, referrer| {
        let c = count_.fetch_add(1, Ordering::Relaxed);
        match c {
          0 => assert_eq!(specifier, "foo1.js"),
          1 => assert_eq!(specifier, "foo2.js"),
          2 => assert_eq!(specifier, "foo3.js"),
          _ => unreachable!(),
        }
        assert_eq!(referrer, "dyn_import_error.js");

        let source_code_info = SourceCodeInfo {
          module_url_specified: specifier.to_owned(),
          module_url_found: specifier.to_owned(),
          code: "# not valid JS".to_owned(),
        };
        let stream = MockImportStream(vec![
          Ok(RecursiveLoadEvent::Fetch(source_code_info)),
          Ok(RecursiveLoadEvent::Instantiate(c.try_into().unwrap())),
        ]);
        Box::new(stream)
      });

      js_check(isolate.execute(
        "dyn_import_error.js",
        r#"
        (async () => {
          await import("foo1.js");
        })();
        (async () => {
          await import("foo2.js");
        })();
        (async () => {
          await import("foo3.js");
        })();
        "#,
      ));

      assert_eq!(count.load(Ordering::Relaxed), 3);
      // Now each poll should return error
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Err(_)) => true,
        _ => false,
      });
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Err(_)) => true,
        _ => false,
      });
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Err(_)) => true,
        _ => false,
      });
    })
  }

  #[test]
  fn dyn_import_ok() {
    run_in_task(|cx| {
      let count = Arc::new(AtomicUsize::new(0));
      let count_ = count.clone();

      // Sometimes Rust is really annoying.
      let mod_b = Arc::new(Mutex::new(0));
      let mod_b2 = mod_b.clone();

      let mut isolate = Isolate::new(StartupData::None, false);
      isolate.set_dyn_import(move |_id, specifier, referrer| {
        let c = count_.fetch_add(1, Ordering::Relaxed);
        match c {
          0 => assert_eq!(specifier, "foo1.js"),
          1 => assert_eq!(specifier, "foo2.js"),
          _ => unreachable!(),
        }
        assert_eq!(referrer, "dyn_import3.js");
        let mod_id = *mod_b2.lock().unwrap();
        let source_code_info = SourceCodeInfo {
          module_url_specified: "foo.js".to_owned(),
          module_url_found: "foo.js".to_owned(),
          code: "".to_owned(),
        };
        let stream = MockImportStream(vec![
          Ok(RecursiveLoadEvent::Fetch(source_code_info)),
          Ok(RecursiveLoadEvent::Instantiate(mod_id)),
        ]);
        Box::new(stream)
      });

      // Instantiate mod_b
      {
        let mut mod_id = mod_b.lock().unwrap();
        *mod_id = isolate
          .mod_new(false, "b.js", "export function b() { return 'b' }")
          .unwrap();
        let mut resolve = move |_specifier: &str,
                                _referrer: ModuleId|
              -> ModuleId { unreachable!() };
        js_check(isolate.mod_instantiate(*mod_id, &mut resolve));
      }
      // Dynamically import mod_b
      js_check(isolate.execute(
        "dyn_import3.js",
        r#"
          (async () => {
            let mod = await import("foo1.js");
            if (mod.b() !== 'b') {
              throw Error("bad1");
            }
            // And again!
            mod = await import("foo2.js");
            if (mod.b() !== 'b') {
              throw Error("bad2");
            }
          })();
          "#,
      ));

      assert_eq!(count.load(Ordering::Relaxed), 1);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      assert_eq!(count.load(Ordering::Relaxed), 2);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      assert_eq!(count.load(Ordering::Relaxed), 2);
    })
  }

  #[test]
  fn terminate_execution() {
    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    let tx_clone = tx.clone();

    let (mut isolate, _dispatch_count) = setup(Mode::Async);
    let shared = isolate.shared_isolate_handle();

    let t1 = std::thread::spawn(move || {
      // allow deno to boot and run
      std::thread::sleep(std::time::Duration::from_millis(100));

      // terminate execution
      shared.terminate_execution();

      // allow shutdown
      std::thread::sleep(std::time::Duration::from_millis(200));

      // unless reported otherwise the test should fail after this point
      tx_clone.send(false).ok();
    });

    let t2 = std::thread::spawn(move || {
      // run an infinite loop
      let res = isolate.execute(
        "infinite_loop.js",
        r#"
          let i = 0;
          while (true) { i++; }
        "#,
      );

      // execute() terminated, which means terminate_execution() was successful.
      tx.send(true).ok();

      if let Err(e) = res {
        assert_eq!(e.to_string(), "Uncaught Error: execution terminated");
      } else {
        panic!("should return an error");
      }

      // make sure the isolate is still unusable
      let res = isolate.execute("simple.js", "1+1;");
      if let Err(e) = res {
        assert_eq!(e.to_string(), "Uncaught Error: execution terminated");
      } else {
        panic!("should return an error");
      }
    });

    if !rx.recv().unwrap() {
      panic!("should have terminated")
    }

    t1.join().unwrap();
    t2.join().unwrap();
  }

  #[test]
  fn dangling_shared_isolate() {
    let shared = {
      // isolate is dropped at the end of this block
      let (mut isolate, _dispatch_count) = setup(Mode::Async);
      isolate.shared_isolate_handle()
    };

    // this should not SEGFAULT
    shared.terminate_execution();
  }

  #[test]
  fn overflow_req_sync() {
    let (mut isolate, dispatch_count) = setup(Mode::OverflowReqSync);
    js_check(isolate.execute(
      "overflow_req_sync.js",
      r#"
        let asyncRecv = 0;
        Deno.core.setAsyncHandler((opId, buf) => { asyncRecv++ });
        // Large message that will overflow the shared space.
        let control = new Uint8Array(100 * 1024 * 1024);
        let response = Deno.core.dispatch(1, control);
        assert(response instanceof Uint8Array);
        assert(response.length == 4);
        assert(response[0] == 43);
        assert(asyncRecv == 0);
        "#,
    ));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn overflow_res_sync() {
    // TODO(ry) This test is quite slow due to memcpy-ing 100MB into JS. We
    // should optimize this.
    let (mut isolate, dispatch_count) = setup(Mode::OverflowResSync);
    js_check(isolate.execute(
      "overflow_res_sync.js",
      r#"
        let asyncRecv = 0;
        Deno.core.setAsyncHandler((opId, buf) => { asyncRecv++ });
        // Large message that will overflow the shared space.
        let control = new Uint8Array([42]);
        let response = Deno.core.dispatch(1, control);
        assert(response instanceof Uint8Array);
        assert(response.length == 100 * 1024 * 1024);
        assert(response[0] == 99);
        assert(asyncRecv == 0);
        "#,
    ));
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn overflow_req_async() {
    run_in_task(|cx| {
      let (mut isolate, dispatch_count) = setup(Mode::OverflowReqAsync);
      js_check(isolate.execute(
        "overflow_req_async.js",
        r#"
         let asyncRecv = 0;
         Deno.core.setAsyncHandler((opId, buf) => {
           assert(opId == 1);
           assert(buf.byteLength === 4);
           assert(buf[0] === 43);
           asyncRecv++;
         });
         // Large message that will overflow the shared space.
         let control = new Uint8Array(100 * 1024 * 1024);
         let response = Deno.core.dispatch(1, control);
         // Async messages always have null response.
         assert(response == null);
         assert(asyncRecv == 0);
         "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      assert!(match isolate.poll_unpin(cx) {
        Poll::Ready(Ok(_)) => true,
        _ => false,
      });
      js_check(isolate.execute("check.js", "assert(asyncRecv == 1);"));
    });
  }

  #[test]
  fn overflow_res_async() {
    run_in_task(|_cx| {
      // TODO(ry) This test is quite slow due to memcpy-ing 100MB into JS. We
      // should optimize this.
      let (mut isolate, dispatch_count) = setup(Mode::OverflowResAsync);
      js_check(isolate.execute(
        "overflow_res_async.js",
        r#"
         let asyncRecv = 0;
         Deno.core.setAsyncHandler((opId, buf) => {
           assert(opId == 1);
           assert(buf.byteLength === 100 * 1024 * 1024);
           assert(buf[0] === 4);
           asyncRecv++;
         });
         // Large message that will overflow the shared space.
         let control = new Uint8Array([42]);
         let response = Deno.core.dispatch(1, control);
         assert(response == null);
         assert(asyncRecv == 0);
         "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      poll_until_ready(&mut isolate, 3).unwrap();
      js_check(isolate.execute("check.js", "assert(asyncRecv == 1);"));
    });
  }

  #[test]
  fn overflow_res_multiple_dispatch_async() {
    // TODO(ry) This test is quite slow due to memcpy-ing 100MB into JS. We
    // should optimize this.
    run_in_task(|_cx| {
      let (mut isolate, dispatch_count) = setup(Mode::OverflowResAsync);
      js_check(isolate.execute(
        "overflow_res_multiple_dispatch_async.js",
        r#"
         let asyncRecv = 0;
         Deno.core.setAsyncHandler((opId, buf) => {
           assert(opId === 1);
           assert(buf.byteLength === 100 * 1024 * 1024);
           assert(buf[0] === 4);
           asyncRecv++;
         });
         // Large message that will overflow the shared space.
         let control = new Uint8Array([42]);
         let response = Deno.core.dispatch(1, control);
         assert(response == null);
         assert(asyncRecv == 0);
         // Dispatch another message to verify that pending ops
         // are done even if shared space overflows
         Deno.core.dispatch(1, control);
         "#,
      ));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      poll_until_ready(&mut isolate, 3).unwrap();
      js_check(isolate.execute("check.js", "assert(asyncRecv == 2);"));
    });
  }

  #[test]
  fn test_pre_dispatch() {
    run_in_task(|mut cx| {
      let (mut isolate, _dispatch_count) = setup(Mode::OverflowResAsync);
      js_check(isolate.execute(
        "bad_op_id.js",
        r#"
          let thrown;
          try {
            Deno.core.dispatch(100, []);
          } catch (e) {
            thrown = e;
          }
          assert(thrown == "Unknown op id: 100");
         "#,
      ));
      if let Poll::Ready(Err(_)) = isolate.poll_unpin(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn test_js() {
    run_in_task(|mut cx| {
      let (mut isolate, _dispatch_count) = setup(Mode::Async);
      js_check(
        isolate.execute(
          "shared_queue_test.js",
          include_str!("shared_queue_test.js"),
        ),
      );
      if let Poll::Ready(Err(_)) = isolate.poll_unpin(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn will_snapshot() {
    let snapshot = {
      let mut isolate = Isolate::new(StartupData::None, true);
      js_check(isolate.execute("a.js", "a = 1 + 2"));
      let s = isolate.snapshot().unwrap();
      drop(isolate);
      s
    };

    let startup_data = StartupData::OwnedSnapshot(snapshot);
    let mut isolate2 = Isolate::new(startup_data, false);
    js_check(isolate2.execute("check.js", "if (a != 3) throw Error('x')"));
  }
}
