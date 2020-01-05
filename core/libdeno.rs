// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#![allow(unused)]
#![allow(mutable_transmutes)]
#![allow(clippy::transmute_ptr_to_ptr)]

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

#[allow(non_camel_case_types)]
pub type isolate = DenoIsolate;

struct ModuleInfo {
  main: bool,
  name: String,
  handle: v8::Global<v8::Module>,
  import_specifiers: Vec<String>,
}

pub struct DenoIsolate {
  isolate_: Option<v8::OwnedIsolate>,
  last_exception_: Option<String>,
  last_exception_handle_: v8::Global<v8::Value>,
  context_: v8::Global<v8::Context>,
  mods_: HashMap<deno_mod, ModuleInfo>,
  mods_by_name_: HashMap<String, deno_mod>,
  locker_: Option<v8::Locker>,
  shared_: deno_buf,
  shared_ab_: v8::Global<v8::SharedArrayBuffer>,
  resolve_cb_: Option<deno_resolve_cb>,
  recv_: v8::Global<v8::Function>,
  current_args_: *const v8::FunctionCallbackInfo,
  recv_cb_: deno_recv_cb,
  snapshot_creator_: Option<v8::SnapshotCreator>,
  has_snapshotted_: bool,
  snapshot_: Option<SnapshotConfig>,
  next_dyn_import_id_: deno_dyn_import_id,
  dyn_import_cb_: deno_dyn_import_cb,
  dyn_import_map_: HashMap<deno_dyn_import_id, v8::Global<v8::PromiseResolver>>,
  pending_promise_map_: HashMap<i32, v8::Global<v8::Value>>,
  // Used in deno_mod_instantiate
  resolve_context_: *mut c_void,
  // Used in deno_mod_evaluate
  core_isolate_: *mut c_void,
  /*
  void* global_import_buf_ptr_;
  v8::Persistent<v8::ArrayBuffer> global_import_buf_;
  */
}

impl Drop for DenoIsolate {
  fn drop(&mut self) {
    // TODO Too much boiler plate.
    // <Boilerplate>
    let mut isolate = self.isolate_.take().unwrap();
    {
      let mut locker = v8::Locker::new(&isolate);
      let mut hs = v8::HandleScope::new(&mut locker);
      let scope = hs.enter();
      // </Boilerplate>
      self.context_.reset(scope);
      self.shared_ab_.reset(scope);
      self.last_exception_handle_.reset(scope);
      self.recv_.reset(scope);
      for (key, module) in self.mods_.iter_mut() {
        module.handle.reset(scope);
      }
      for (key, handle) in self.dyn_import_map_.iter_mut() {
        handle.reset(scope);
      }
      for (key, handle) in self.pending_promise_map_.iter_mut() {
        handle.reset(scope);
      }
    }
    if let Some(locker_) = self.locker_.take() {
      drop(locker_);
    }
    if let Some(creator) = self.snapshot_creator_.take() {
      // TODO(ry) V8 has a strange assert which prevents a SnapshotCreator from
      // being deallocated if it hasn't created a snapshot yet.
      // https://github.com/v8/v8/blob/73212783fbd534fac76cc4b66aac899c13f71fc8/src/api.cc#L603
      // If that assert is removed, this if guard could be removed.
      // WARNING: There may be false positive LSAN errors here.
      std::mem::forget(isolate);
      if self.has_snapshotted_ {
        drop(creator);
      }
    } else {
      drop(isolate);
    }
  }
}

impl DenoIsolate {
  pub fn new(config: deno_config) -> Self {
    Self {
      isolate_: None,
      last_exception_: None,
      last_exception_handle_: v8::Global::<v8::Value>::new(),
      context_: v8::Global::<v8::Context>::new(),
      mods_: HashMap::new(),
      mods_by_name_: HashMap::new(),
      pending_promise_map_: HashMap::new(),
      locker_: None,
      shared_: config.shared,
      shared_ab_: v8::Global::<v8::SharedArrayBuffer>::new(),
      resolve_cb_: None,
      recv_: v8::Global::<v8::Function>::new(),
      current_args_: std::ptr::null(),
      recv_cb_: config.recv_cb,
      snapshot_creator_: None,
      snapshot_: config.load_snapshot,
      has_snapshotted_: false,
      next_dyn_import_id_: 0,
      dyn_import_cb_: config.dyn_import_cb,
      dyn_import_map_: HashMap::new(),
      resolve_context_: std::ptr::null_mut(),
      core_isolate_: std::ptr::null_mut(),
    }
  }

  pub fn add_isolate(&mut self, mut isolate: v8::OwnedIsolate) {
    isolate.set_capture_stack_trace_for_uncaught_exceptions(true, 10);
    isolate.set_promise_reject_callback(promise_reject_callback);
    isolate.add_message_listener(message_callback);
    isolate.set_host_initialize_import_meta_object_callback(
      host_initialize_import_meta_object_callback,
    );
    isolate.set_host_import_module_dynamically_callback(
      host_import_module_dynamically_callback,
    );
    let self_ptr: *mut Self = self;
    unsafe { isolate.set_data(0, self_ptr as *mut c_void) };
    self.isolate_ = Some(isolate);
  }

  pub fn register_module(
    &mut self,
    main: bool,
    name: &str,
    source: &str,
  ) -> deno_mod {
    let isolate = self.isolate_.as_ref().unwrap();
    let mut locker = v8::Locker::new(&isolate);

    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    let mut context = v8::Context::new(scope);
    context.enter();

    let name_str = v8::String::new(scope, name).unwrap();
    let source_str = v8::String::new(scope, source).unwrap();

    let origin = module_origin(scope, name_str);
    let source = v8::script_compiler::Source::new(source_str, &origin);

    let mut try_catch = v8::TryCatch::new(scope);
    let tc = try_catch.enter();

    let mut maybe_module =
      v8::script_compiler::compile_module(&isolate, source);

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
    self.mods_by_name_.insert(name.to_string(), id);
    context.exit();
    id
  }

  fn get_module_info(&self, id: deno_mod) -> Option<&ModuleInfo> {
    if id == 0 {
      return None;
    }
    self.mods_.get(&id)
  }

  fn execute<'a>(
    &mut self,
    s: &mut impl v8::ToLocal<'a>,
    mut context: v8::Local<'a, v8::Context>,
    js_filename: &str,
    js_source: &str,
  ) -> bool {
    let mut hs = v8::HandleScope::new(s);
    let s = hs.enter();
    let source = v8::String::new(s, js_source).unwrap();
    let name = v8::String::new(s, js_filename).unwrap();
    let mut try_catch = v8::TryCatch::new(s);
    let tc = try_catch.enter();
    let origin = script_origin(s, name);
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

  fn handle_exception<'a>(
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
      let exception = if (exception.is_null_or_undefined()) {
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
    self.last_exception_ = Some(json_str);
    self.last_exception_handle_.set(s, exception);
  }

  fn encode_exception_as_json<'a>(
    &mut self,
    s: &mut impl v8::ToLocal<'a>,
    mut context: v8::Local<'a, v8::Context>,
    exception: v8::Local<'a, v8::Value>,
  ) -> String {
    let message = v8::create_message(s, exception);
    self.encode_message_as_json(s, context, message)
  }

  fn encode_message_as_json<'a>(
    &mut self,
    s: &mut impl v8::ToLocal<'a>,
    mut context: v8::Local<v8::Context>,
    message: v8::Local<v8::Message>,
  ) -> String {
    let json_obj = self.encode_message_as_object(s, context, message);
    let json_string = v8::json::stringify(context, json_obj.into()).unwrap();
    json_string.to_rust_string_lossy(s)
  }

  fn encode_message_as_object<'a>(
    &mut self,
    s: &mut impl v8::ToLocal<'a>,
    mut context: v8::Local<v8::Context>,
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
}

fn script_origin<'a>(
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

fn module_origin<'a>(
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

extern "C" fn host_import_module_dynamically_callback(
  context: v8::Local<v8::Context>,
  referrer: v8::Local<v8::ScriptOrModule>,
  specifier: v8::Local<v8::String>,
) -> *mut v8::Promise {
  let mut cbs = v8::CallbackScope::new(context);
  let mut hs = v8::EscapableHandleScope::new(cbs.enter());
  let scope = hs.enter();
  let mut isolate = scope.isolate();
  let deno_isolate: &mut DenoIsolate =
    unsafe { &mut *(isolate.get_data(0) as *mut DenoIsolate) };

  // NOTE(bartlomieju): will crash for non-UTF-8 specifier
  let specifier_str = specifier
    .to_string(scope)
    .unwrap()
    .to_rust_string_lossy(scope);
  let referrer_name = referrer.get_resource_name();
  let referrer_name_str = referrer_name
    .to_string(scope)
    .unwrap()
    .to_rust_string_lossy(scope);

  // TODO(ry) I'm not sure what HostDefinedOptions is for or if we're ever going
  // to use it. For now we check that it is not used. This check may need to be
  // changed in the future.
  let host_defined_options = referrer.get_host_defined_options();
  assert_eq!(host_defined_options.length(), 0);

  let mut resolver = v8::PromiseResolver::new(scope, context).unwrap();
  let promise = resolver.get_promise(scope);

  let mut resolver_handle = v8::Global::new();
  resolver_handle.set(scope, resolver);

  let import_id = deno_isolate.next_dyn_import_id_;
  deno_isolate.next_dyn_import_id_ += 1;
  deno_isolate
    .dyn_import_map_
    .insert(import_id, resolver_handle);

  let cb = deno_isolate.dyn_import_cb_;
  cb(
    deno_isolate.core_isolate_,
    &specifier_str,
    &referrer_name_str,
    import_id,
  );

  &mut *scope.escape(promise)
}

extern "C" fn host_initialize_import_meta_object_callback(
  context: v8::Local<v8::Context>,
  module: v8::Local<v8::Module>,
  meta: v8::Local<v8::Object>,
) {
  let mut cbs = v8::CallbackScope::new(context);
  let mut hs = v8::HandleScope::new(cbs.enter());
  let scope = hs.enter();
  let mut isolate = scope.isolate();
  let deno_isolate: &mut DenoIsolate =
    unsafe { &mut *(isolate.get_data(0) as *mut DenoIsolate) };

  let id = module.get_identity_hash();
  assert_ne!(id, 0);

  let info = deno_isolate.get_module_info(id).expect("Module not found");

  meta.create_data_property(
    context,
    v8::String::new(scope, "url").unwrap().into(),
    v8::String::new(scope, &info.name).unwrap().into(),
  );
  meta.create_data_property(
    context,
    v8::String::new(scope, "main").unwrap().into(),
    v8::Boolean::new(scope, info.main).into(),
  );
}

extern "C" fn message_callback(
  message: v8::Local<v8::Message>,
  exception: v8::Local<v8::Value>,
) {
  let mut message: v8::Local<v8::Message> =
    unsafe { std::mem::transmute(message) };
  let isolate = message.get_isolate();
  let deno_isolate: &mut DenoIsolate =
    unsafe { &mut *(isolate.get_data(0) as *mut DenoIsolate) };
  let mut locker = v8::Locker::new(isolate);
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();
  assert!(!deno_isolate.context_.is_empty());
  let mut context = deno_isolate.context_.get(scope).unwrap();

  // TerminateExecution was called
  if isolate.is_execution_terminating() {
    let u = v8::new_undefined(scope);
    deno_isolate.handle_exception(scope, context, u.into());
    return;
  }

  let json_str = deno_isolate.encode_message_as_json(scope, context, message);
  deno_isolate.last_exception_ = Some(json_str);
}

extern "C" fn promise_reject_callback(msg: v8::PromiseRejectMessage) {
  #[allow(mutable_transmutes)]
  let mut msg: v8::PromiseRejectMessage = unsafe { std::mem::transmute(msg) };
  let mut isolate = msg.isolate();
  let deno_isolate: &mut DenoIsolate =
    unsafe { &mut *(isolate.get_data(0) as *mut DenoIsolate) };
  let mut locker = v8::Locker::new(isolate);
  assert!(!deno_isolate.context_.is_empty());
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();
  let mut context = deno_isolate.context_.get(scope).unwrap();
  context.enter();

  let promise = msg.get_promise();
  let promise_id = promise.get_identity_hash();

  match msg.get_event() {
    v8::PromiseRejectEvent::PromiseRejectWithNoHandler => {
      let error = msg.get_value();
      let mut error_global = v8::Global::<v8::Value>::new();
      error_global.set(scope, error);
      deno_isolate
        .pending_promise_map_
        .insert(promise_id, error_global);
    }
    v8::PromiseRejectEvent::PromiseHandlerAddedAfterReject => {
      if let Some(mut handle) =
        deno_isolate.pending_promise_map_.remove(&promise_id)
      {
        handle.reset(scope);
      }
    }
    v8::PromiseRejectEvent::PromiseRejectAfterResolved => {}
    v8::PromiseRejectEvent::PromiseResolveAfterResolved => {
      // Should not warn. See #1272
    }
  };

  context.exit();
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
type deno_recv_cb = unsafe fn(
  user_data: *mut c_void,
  op_id: OpId,
  control_buf: deno_buf,
  zero_copy_buf: Option<PinnedBuf>,
);

/// Called when dynamic import is called in JS: import('foo')
/// Embedder must call deno_dyn_import_done() with the specified id and
/// the module.
#[allow(non_camel_case_types)]
type deno_dyn_import_cb = fn(
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
type deno_resolve_cb = unsafe extern "C" fn(
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

#[repr(C)]
pub struct deno_config {
  pub will_snapshot: c_int,
  pub load_snapshot: Option<SnapshotConfig>,
  pub shared: deno_buf,
  pub recv_cb: deno_recv_cb,
  pub dyn_import_cb: deno_dyn_import_cb,
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
  static ref EXTERNAL_REFERENCES: v8::ExternalReferences =
    v8::ExternalReferences::new(&[
      v8::ExternalReference { function: print },
      v8::ExternalReference { function: recv },
      v8::ExternalReference { function: send },
      v8::ExternalReference {
        function: eval_context
      },
      v8::ExternalReference {
        function: error_to_json
      },
      v8::ExternalReference {
        getter: shared_getter
      },
      v8::ExternalReference {
        message: message_callback
      },
      v8::ExternalReference {
        function: queue_microtask
      },
    ]);
}

pub unsafe fn deno_new_snapshotter(config: deno_config) -> *mut isolate {
  assert_ne!(config.will_snapshot, 0);
  // TODO(ry) Support loading snapshots before snapshotting.
  assert!(config.load_snapshot.is_none());
  let mut creator = v8::SnapshotCreator::new(Some(&EXTERNAL_REFERENCES));

  let mut d = Box::new(DenoIsolate::new(config));
  let isolate = creator.get_owned_isolate();

  let mut locker = v8::Locker::new(&isolate);
  {
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    let mut context = v8::Context::new(scope);
    // context.enter();
    d.context_.set(scope, context);
    creator.set_default_context(context);
    initialize_context(scope, context);
    // context.exit();
  }
  d.add_isolate(isolate);

  d.snapshot_creator_ = Some(creator);

  Box::into_raw(d)
}

extern "C" fn print(info: &v8::FunctionCallbackInfo) {
  let info: &mut v8::FunctionCallbackInfo =
    unsafe { std::mem::transmute(info) };

  let arg_len = info.length();
  assert!(arg_len >= 0 && arg_len <= 2);

  let obj = info.get_argument(0);
  let is_err_arg = info.get_argument(1);

  let mut hs = v8::HandleScope::new(info);
  let scope = hs.enter();

  let mut is_err = false;
  if arg_len == 2 {
    let int_val = is_err_arg
      .integer_value(scope)
      .expect("Unable to convert to integer");
    is_err = int_val != 0;
  };
  let mut isolate = scope.isolate();
  let mut try_catch = v8::TryCatch::new(scope);
  let tc = try_catch.enter();
  let str_ = match obj.to_string(scope) {
    Some(s) => s,
    None => v8::String::new(scope, "").unwrap(),
  };
  if is_err {
    eprint!("{}", str_.to_rust_string_lossy(scope));
  } else {
    print!("{}", str_.to_rust_string_lossy(scope));
  }
}

extern "C" fn recv(info: &v8::FunctionCallbackInfo) {
  #[allow(mutable_transmutes)]
  #[allow(clippy::transmute_ptr_to_ptr)]
  let info: &mut v8::FunctionCallbackInfo =
    unsafe { std::mem::transmute(info) };
  assert_eq!(info.length(), 1);
  let mut isolate = info.get_isolate();
  let deno_isolate: &mut DenoIsolate =
    unsafe { &mut *(isolate.get_data(0) as *mut DenoIsolate) };
  let mut locker = v8::Locker::new(&isolate);
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();

  if !deno_isolate.recv_.is_empty() {
    let msg = v8::String::new(scope, "Deno.core.recv already called.").unwrap();
    isolate.throw_exception(msg.into());
    return;
  }

  let recv_fn =
    v8::Local::<v8::Function>::try_from(info.get_argument(0)).unwrap();
  deno_isolate.recv_.set(scope, recv_fn);
}

extern "C" fn send(info: &v8::FunctionCallbackInfo) {
  #[allow(mutable_transmutes)]
  #[allow(clippy::transmute_ptr_to_ptr)]
  let info: &mut v8::FunctionCallbackInfo =
    unsafe { std::mem::transmute(info) };

  let mut hs = v8::HandleScope::new(info);
  let scope = hs.enter();
  let mut isolate = scope.isolate();
  let deno_isolate: &mut DenoIsolate =
    unsafe { &mut *(isolate.get_data(0) as *mut DenoIsolate) };
  assert!(!deno_isolate.context_.is_empty());

  let op_id = v8::Local::<v8::Uint32>::try_from(info.get_argument(0))
    .unwrap()
    .value() as u32;

  let control =
    v8::Local::<v8::ArrayBufferView>::try_from(info.get_argument(1))
      .map(|view| {
        let mut backing_store = view.buffer().unwrap().get_backing_store();
        let backing_store_ptr = backing_store.data() as *mut _ as *mut u8;
        let view_ptr = unsafe { backing_store_ptr.add(view.byte_offset()) };
        let view_len = view.byte_length();
        unsafe { deno_buf::from_raw_parts(view_ptr, view_len) }
      })
      .unwrap_or_else(|_| deno_buf::empty());

  let zero_copy: Option<PinnedBuf> =
    v8::Local::<v8::ArrayBufferView>::try_from(info.get_argument(2))
      .map(PinnedBuf::new)
      .ok();

  // TODO: what's the point of this again?
  // DCHECK_NULL(d->current_args_);
  // d->current_args_ = &args;
  assert!(deno_isolate.current_args_.is_null());
  deno_isolate.current_args_ = info;

  unsafe {
    (deno_isolate.recv_cb_)(
      deno_isolate.core_isolate_,
      op_id,
      control,
      zero_copy,
    );
  }

  if deno_isolate.current_args_.is_null() {
    // This indicates that deno_repond() was called already.
  } else {
    // Asynchronous.
    deno_isolate.current_args_ = null();
  }
}

extern "C" fn eval_context(info: &v8::FunctionCallbackInfo) {
  let rv = &mut info.get_return_value();

  #[allow(mutable_transmutes)]
  #[allow(clippy::transmute_ptr_to_ptr)]
  let info: &mut v8::FunctionCallbackInfo =
    unsafe { std::mem::transmute(info) };
  let arg0 = info.get_argument(0);

  let mut hs = v8::HandleScope::new(info);
  let scope = hs.enter();
  let mut isolate = scope.isolate();
  let deno_isolate: &mut DenoIsolate =
    unsafe { &mut *(isolate.get_data(0) as *mut DenoIsolate) };
  assert!(!deno_isolate.context_.is_empty());
  let mut context = deno_isolate.context_.get(scope).unwrap();

  let source = match v8::Local::<v8::String>::try_from(arg0) {
    Ok(s) => s,
    Err(_) => {
      let msg = v8::String::new(scope, "Invalid argument").unwrap();
      let exception = v8::type_error(scope, msg);
      scope.isolate().throw_exception(exception);
      return;
    }
  };

  let output = v8::Array::new(scope, 2);
  /**
   * output[0] = result
   * output[1] = ErrorInfo | null
   *   ErrorInfo = {
   *     thrown: Error | any,
   *     isNativeError: boolean,
   *     isCompileError: boolean,
   *   }
   */
  let mut try_catch = v8::TryCatch::new(scope);
  let tc = try_catch.enter();
  let name = v8::String::new(scope, "<unknown>").unwrap();
  let origin = script_origin(scope, name);
  let maybe_script = v8::Script::compile(scope, context, source, Some(&origin));

  if maybe_script.is_none() {
    assert!(tc.has_caught());
    let exception = tc.exception().unwrap();

    output.set(
      context,
      v8::Integer::new(scope, 0).into(),
      v8::new_null(scope).into(),
    );

    let errinfo_obj = v8::Object::new(scope);
    errinfo_obj.set(
      context,
      v8::String::new(scope, "isCompileError").unwrap().into(),
      v8::Boolean::new(scope, true).into(),
    );

    errinfo_obj.set(
      context,
      v8::String::new(scope, "isNativeError").unwrap().into(),
      v8::Boolean::new(scope, exception.is_native_error()).into(),
    );

    errinfo_obj.set(
      context,
      v8::String::new(scope, "thrown").unwrap().into(),
      exception,
    );

    output.set(
      context,
      v8::Integer::new(scope, 1).into(),
      errinfo_obj.into(),
    );

    rv.set(output.into());
    return;
  }

  let result = maybe_script.unwrap().run(scope, context);

  if result.is_none() {
    assert!(tc.has_caught());
    let exception = tc.exception().unwrap();

    output.set(
      context,
      v8::Integer::new(scope, 0).into(),
      v8::new_null(scope).into(),
    );

    let errinfo_obj = v8::Object::new(scope);
    errinfo_obj.set(
      context,
      v8::String::new(scope, "isCompileError").unwrap().into(),
      v8::Boolean::new(scope, false).into(),
    );

    let is_native_error = if exception.is_native_error() {
      v8::Boolean::new(scope, true)
    } else {
      v8::Boolean::new(scope, false)
    };

    errinfo_obj.set(
      context,
      v8::String::new(scope, "isNativeError").unwrap().into(),
      is_native_error.into(),
    );

    errinfo_obj.set(
      context,
      v8::String::new(scope, "thrown").unwrap().into(),
      exception,
    );

    output.set(
      context,
      v8::Integer::new(scope, 1).into(),
      errinfo_obj.into(),
    );

    rv.set(output.into());
    return;
  }

  output.set(context, v8::Integer::new(scope, 0).into(), result.unwrap());
  output.set(
    context,
    v8::Integer::new(scope, 1).into(),
    v8::new_null(scope).into(),
  );
  rv.set(output.into());
}

extern "C" fn error_to_json(info: &v8::FunctionCallbackInfo) {
  #[allow(mutable_transmutes)]
  #[allow(clippy::transmute_ptr_to_ptr)]
  let info: &mut v8::FunctionCallbackInfo =
    unsafe { std::mem::transmute(info) };
  assert_eq!(info.length(), 1);
  // <Boilerplate>
  let mut isolate = info.get_isolate();
  let deno_isolate: &mut DenoIsolate =
    unsafe { &mut *(isolate.get_data(0) as *mut DenoIsolate) };
  let mut locker = v8::Locker::new(&isolate);
  assert!(!deno_isolate.context_.is_empty());
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();
  let mut context = deno_isolate.context_.get(scope).unwrap();
  // </Boilerplate>
  let exception = info.get_argument(0);
  let json_string =
    deno_isolate.encode_exception_as_json(scope, context, exception);
  let s = v8::String::new(scope, &json_string).unwrap();
  let mut rv = info.get_return_value();
  rv.set(s.into());
}

extern "C" fn queue_microtask(info: &v8::FunctionCallbackInfo) {
  #[allow(mutable_transmutes)]
  #[allow(clippy::transmute_ptr_to_ptr)]
  let info: &mut v8::FunctionCallbackInfo =
    unsafe { std::mem::transmute(info) };
  assert_eq!(info.length(), 1);
  let arg0 = info.get_argument(0);
  let mut isolate = info.get_isolate();
  let deno_isolate: &mut DenoIsolate =
    unsafe { &mut *(isolate.get_data(0) as *mut DenoIsolate) };
  let mut locker = v8::Locker::new(&isolate);
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();

  match v8::Local::<v8::Function>::try_from(arg0) {
    Ok(f) => isolate.enqueue_microtask(f),
    Err(_) => {
      let msg = v8::String::new(scope, "Invalid argument").unwrap();
      let exception = v8::type_error(scope, msg);
      isolate.throw_exception(exception);
    }
  };
}

extern "C" fn shared_getter(
  name: v8::Local<v8::Name>,
  info: &v8::PropertyCallbackInfo,
) {
  let shared_ab = {
    #[allow(mutable_transmutes)]
    #[allow(clippy::transmute_ptr_to_ptr)]
    let info: &mut v8::PropertyCallbackInfo =
      unsafe { std::mem::transmute(info) };

    let mut hs = v8::EscapableHandleScope::new(info);
    let scope = hs.enter();
    let mut isolate = scope.isolate();
    let deno_isolate: &mut DenoIsolate =
      unsafe { &mut *(isolate.get_data(0) as *mut DenoIsolate) };

    if deno_isolate.shared_.data_ptr.is_null() {
      return;
    }

    // Lazily initialize the persistent external ArrayBuffer.
    if deno_isolate.shared_ab_.is_empty() {
      #[allow(mutable_transmutes)]
      #[allow(clippy::transmute_ptr_to_ptr)]
      let data_ptr: *mut u8 =
        unsafe { std::mem::transmute(deno_isolate.shared_.data_ptr) };
      let ab = unsafe {
        v8::SharedArrayBuffer::new_DEPRECATED(
          scope,
          data_ptr as *mut c_void,
          deno_isolate.shared_.data_len,
        )
      };
      deno_isolate.shared_ab_.set(scope, ab);
    }

    let shared_ab = deno_isolate.shared_ab_.get(scope).unwrap();
    scope.escape(shared_ab)
  };

  let rv = &mut info.get_return_value();
  rv.set(shared_ab.into());
}

fn initialize_context<'a>(
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

  let mut print_tmpl = v8::FunctionTemplate::new(scope, print);
  let mut print_val = print_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "print").unwrap().into(),
    print_val.into(),
  );

  let mut recv_tmpl = v8::FunctionTemplate::new(scope, recv);
  let mut recv_val = recv_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "recv").unwrap().into(),
    recv_val.into(),
  );

  let mut send_tmpl = v8::FunctionTemplate::new(scope, send);
  let mut send_val = send_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "send").unwrap().into(),
    send_val.into(),
  );

  let mut eval_context_tmpl = v8::FunctionTemplate::new(scope, eval_context);
  let mut eval_context_val =
    eval_context_tmpl.get_function(scope, context).unwrap();
  core_val.set(
    context,
    v8::String::new(scope, "evalContext").unwrap().into(),
    eval_context_val.into(),
  );

  let mut error_to_json_tmpl = v8::FunctionTemplate::new(scope, error_to_json);
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
    shared_getter,
  );

  // Direct bindings on `window`.
  let mut queue_microtask_tmpl =
    v8::FunctionTemplate::new(scope, queue_microtask);
  let mut queue_microtask_val =
    queue_microtask_tmpl.get_function(scope, context).unwrap();
  global.set(
    context,
    v8::String::new(scope, "queueMicrotask").unwrap().into(),
    queue_microtask_val.into(),
  );

  context.exit();
}

pub unsafe fn deno_new(config: deno_config) -> *mut isolate {
  if config.will_snapshot != 0 {
    return deno_new_snapshotter(config);
  }

  let load_snapshot_is_null = config.load_snapshot.is_none();

  let mut d = Box::new(DenoIsolate::new(config));
  let mut params = v8::Isolate::create_params();
  params.set_array_buffer_allocator(v8::new_default_allocator());
  params.set_external_references(&EXTERNAL_REFERENCES);
  if let Some(ref mut snapshot) = d.snapshot_ {
    params.set_snapshot_blob(snapshot);
  }

  let isolate = v8::Isolate::new(params);
  d.add_isolate(isolate);

  let mut locker = v8::Locker::new(d.isolate_.as_ref().unwrap());
  {
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    let mut context = v8::Context::new(scope);

    if load_snapshot_is_null {
      // If no snapshot is provided, we initialize the context with empty
      // main source code and source maps.
      initialize_context(scope, context);
    }
    d.context_.set(scope, context);
  }
  Box::into_raw(d)
}

pub unsafe fn deno_delete(i: *mut DenoIsolate) {
  let deno_isolate = unsafe { Box::from_raw(i as *mut DenoIsolate) };
  drop(deno_isolate);
}

pub unsafe fn deno_last_exception(i: *mut DenoIsolate) -> Option<String> {
  (*i).last_exception_.clone()
}

pub unsafe fn deno_clear_last_exception(i: *mut DenoIsolate) {
  let i_mut: &mut DenoIsolate = unsafe { &mut *i };
  i_mut.last_exception_ = None;
}

pub unsafe fn deno_check_promise_errors(i: *mut DenoIsolate) {
  /*
  if (d->pending_promise_map_.size() > 0) {
    auto* isolate = d->isolate_;
    v8::Locker locker(isolate);
    v8::Isolate::Scope isolate_scope(isolate);
    v8::HandleScope handle_scope(isolate);
    auto context = d->context_.Get(d->isolate_);
    v8::Context::Scope context_scope(context);

    auto it = d->pending_promise_map_.begin();
    while (it != d->pending_promise_map_.end()) {
      auto error = it->second.Get(isolate);
      deno::HandleException(context, error);
      it = d->pending_promise_map_.erase(it);
    }
  }
  */
  let i_mut: &mut DenoIsolate = unsafe { &mut *i };
  let isolate = i_mut.isolate_.as_ref().unwrap();

  if i_mut.pending_promise_map_.is_empty() {
    return;
  }

  let mut locker = v8::Locker::new(isolate);
  assert!(!i_mut.context_.is_empty());
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();
  let mut context = i_mut.context_.get(scope).unwrap();
  context.enter();

  let pending_promises: Vec<(i32, v8::Global<v8::Value>)> =
    i_mut.pending_promise_map_.drain().collect();
  for (promise_id, mut handle) in pending_promises {
    let error = handle.get(scope).expect("Empty error handle");
    i_mut.handle_exception(scope, context, error);
    handle.reset(scope);
  }

  context.exit();
}

pub unsafe fn deno_lock(i: *mut DenoIsolate) {
  let i_mut: &mut DenoIsolate = unsafe { &mut *i };
  assert!(i_mut.locker_.is_none());
  let mut locker = v8::Locker::new(i_mut.isolate_.as_ref().unwrap());
  i_mut.locker_ = Some(locker);
}

pub unsafe fn deno_unlock(i: *mut DenoIsolate) {
  let i_mut: &mut DenoIsolate = unsafe { &mut *i };
  i_mut.locker_.take().unwrap();
}

pub unsafe fn deno_throw_exception(i: *mut DenoIsolate, text: &str) {
  let i_mut: &mut DenoIsolate = unsafe { &mut *i };
  let isolate = i_mut.isolate_.as_ref().unwrap();
  let mut locker = v8::Locker::new(isolate);
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();
  let msg = v8::String::new(scope, text).unwrap();
  isolate.throw_exception(msg.into());
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

pub unsafe fn deno_respond(
  i: *mut isolate,
  core_isolate: *const c_void,
  op_id: OpId,
  buf: deno_buf,
) {
  /*
  auto* d = deno::unwrap(d_);
  if (d->current_args_ != nullptr) {
    // Synchronous response.
    // Note op_id is not passed back in the case of synchronous response.
    if (buf.data_ptr != nullptr && buf.data_len > 0) {
      auto ab = deno::ImportBuf(d, buf);
      d->current_args_->GetReturnValue().Set(ab);
    }
    d->current_args_ = nullptr;
    return;
  }
  */
  let deno_isolate: &mut DenoIsolate = unsafe { &mut *i };

  if !deno_isolate.current_args_.is_null() {
    // Synchronous response.
    // Note op_id is not passed back in the case of synchronous response.
    if !buf.data_ptr.is_null() && buf.data_len > 0 {
      let isolate = deno_isolate.isolate_.as_ref().unwrap();
      let mut locker = v8::Locker::new(isolate);
      assert!(!deno_isolate.context_.is_empty());
      let mut hs = v8::HandleScope::new(&mut locker);
      let scope = hs.enter();
      let ab = deno_import_buf(scope, buf);
      let info: &v8::FunctionCallbackInfo =
        unsafe { &*deno_isolate.current_args_ };
      let rv = &mut info.get_return_value();
      rv.set(ab.into())
    }
    deno_isolate.current_args_ = std::ptr::null();
    return;
  }

  /*
  // Asynchronous response.
  deno::UserDataScope user_data_scope(d, user_data);
  v8::Isolate::Scope isolate_scope(d->isolate_);
  v8::HandleScope handle_scope(d->isolate_);

  auto context = d->context_.Get(d->isolate_);
  v8::Context::Scope context_scope(context);

  v8::TryCatch try_catch(d->isolate_);

  auto recv_ = d->recv_.Get(d->isolate_);
  if (recv_.IsEmpty()) {
    d->last_exception_ = "Deno.core.recv has not been called.";
    return;
  }

  v8::Local<v8::Value> args[2];
  int argc = 0;

  if (buf.data_ptr != nullptr) {
    args[0] = v8::Integer::New(d->isolate_, op_id);
    args[1] = deno::ImportBuf(d, buf);
    argc = 2;
  }

  auto v = recv_->Call(context, context->Global(), argc, args);

  if (try_catch.HasCaught()) {
    CHECK(v.IsEmpty());
    deno::HandleException(context, try_catch.Exception());
  }
  */

  let core_isolate: *mut c_void = unsafe { std::mem::transmute(core_isolate) };
  deno_isolate.core_isolate_ = core_isolate;

  let isolate = deno_isolate.isolate_.as_ref().unwrap();
  // println!("deno_execute -> Isolate ptr {:?}", isolate);
  let mut locker = v8::Locker::new(isolate);
  assert!(!deno_isolate.context_.is_empty());
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();
  let mut context = deno_isolate.context_.get(scope).unwrap();
  context.enter();

  let mut try_catch = v8::TryCatch::new(scope);
  let tc = try_catch.enter();

  let recv_ = deno_isolate.recv_.get(scope);

  if recv_.is_none() {
    let msg = "Deno.core.recv has not been called.".to_string();
    deno_isolate.last_exception_ = Some(msg);
    return;
  }

  let mut argc = 0;
  let mut args: Vec<v8::Local<v8::Value>> = vec![];

  if !buf.data_ptr.is_null() {
    argc = 2;
    let op_id = v8::Integer::new(scope, op_id as i32);
    args.push(op_id.into());
    let buf = deno_import_buf(scope, buf);
    args.push(buf.into());
  }

  let global = context.global(scope);
  let maybe_value =
    recv_
      .unwrap()
      .call(scope, context, global.into(), argc, args);

  if tc.has_caught() {
    assert!(maybe_value.is_none());
    deno_isolate.handle_exception(scope, context, tc.exception().unwrap());
  }
  context.exit();
  deno_isolate.core_isolate_ = std::ptr::null_mut();
}

pub unsafe fn deno_execute(
  i: *mut DenoIsolate,
  core_isolate: *mut c_void,
  js_filename: &str,
  js_source: &str,
) {
  let i_mut: &mut DenoIsolate = unsafe { &mut *i };
  i_mut.core_isolate_ = core_isolate;
  let isolate = i_mut.isolate_.as_ref().unwrap();
  // println!("deno_execute -> Isolate ptr {:?}", isolate);
  let mut locker = v8::Locker::new(isolate);
  assert!(!i_mut.context_.is_empty());
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();
  let mut context = i_mut.context_.get(scope).unwrap();
  context.enter();

  i_mut.execute(scope, context, js_filename, js_source);

  context.exit();
  i_mut.core_isolate_ = std::ptr::null_mut();
  /*
  auto* d = deno::unwrap(d_);
  deno::UserDataScope user_data_scope(d, user_data);
  auto* isolate = d->isolate_;
  v8::Locker locker(isolate);
  v8::Isolate::Scope isolate_scope(isolate);
  v8::HandleScope handle_scope(isolate);
  auto context = d->context_.Get(d->isolate_);
  CHECK(!context.IsEmpty());
  execute(context, js_filename, js_source);
  */
}

pub unsafe fn deno_terminate_execution(i: *mut DenoIsolate) {
  /*
  deno::DenoIsolate* d = reinterpret_cast<deno::DenoIsolate*>(d_);
  d->isolate_->TerminateExecution();
  */
  let i_mut: &mut DenoIsolate = unsafe { &mut *i };
  let isolate = i_mut.isolate_.as_ref().unwrap();
  isolate.terminate_execution();
}

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

// Modules

pub unsafe fn deno_mod_new(
  i: *mut DenoIsolate,
  main: bool,
  name: &str,
  source: &str,
) -> deno_mod {
  let i_mut: &mut DenoIsolate = unsafe { &mut *i };
  i_mut.register_module(main, name, source)
}

pub unsafe fn deno_mod_imports_len(i: *mut DenoIsolate, id: deno_mod) -> usize {
  let info = (*i).get_module_info(id).unwrap();
  info.import_specifiers.len()
}

pub unsafe fn deno_mod_imports_get(
  i: *mut DenoIsolate,
  id: deno_mod,
  index: size_t,
) -> Option<String> {
  match (*i).get_module_info(id) {
    Some(info) => match info.import_specifiers.get(index) {
      Some(specifier) => Some(specifier.to_string()),
      None => None,
    },
    None => None,
  }
}

fn resolve_callback(
  context: v8::Local<v8::Context>,
  specifier: v8::Local<v8::String>,
  referrer: v8::Local<v8::Module>,
) -> *mut v8::Module {
  let mut cbs = v8::CallbackScope::new(context);
  let cb_scope = cbs.enter();
  let isolate = cb_scope.isolate();
  let deno_isolate: &mut DenoIsolate =
    unsafe { &mut *(isolate.get_data(0) as *mut DenoIsolate) };

  let mut locker = v8::Locker::new(isolate);
  let mut hs = v8::EscapableHandleScope::new(&mut locker);
  let scope = hs.enter();

  let referrer_id = referrer.get_identity_hash();
  let referrer_info = deno_isolate
    .get_module_info(referrer_id)
    .expect("ModuleInfo not found");
  let len_ = referrer.get_module_requests_length();

  let specifier_str = specifier.to_rust_string_lossy(scope);

  for i in 0..len_ {
    let req = referrer.get_module_request(i);
    let req_str = req.to_rust_string_lossy(scope);

    if req_str == specifier_str {
      let resolve_cb = deno_isolate.resolve_cb_.unwrap();
      let c_str = CString::new(req_str.to_string()).unwrap();
      let c_req_str: *const c_char = c_str.as_ptr() as *const c_char;
      let id = unsafe {
        resolve_cb(deno_isolate.resolve_context_, c_req_str, referrer_id)
      };
      let maybe_info = deno_isolate.get_module_info(id);

      if maybe_info.is_none() {
        let msg = format!(
          "Cannot resolve module \"{}\" from \"{}\"",
          req_str, referrer_info.name
        );
        let msg = v8::String::new(scope, &msg).unwrap();
        isolate.throw_exception(msg.into());
        break;
      }

      let child_mod =
        maybe_info.unwrap().handle.get(scope).expect("Empty handle");
      return &mut *scope.escape(child_mod);
    }
  }

  std::ptr::null_mut()
}

pub unsafe fn deno_mod_instantiate(
  i: *mut DenoIsolate,
  resolve_context: *mut c_void,
  id: deno_mod,
  resolve_cb: deno_resolve_cb,
) {
  let i_mut: &mut DenoIsolate = unsafe { &mut *i };
  i_mut.resolve_context_ = resolve_context;
  let isolate = i_mut.isolate_.as_ref().unwrap();
  let mut locker = v8::Locker::new(isolate);
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();
  assert!(!i_mut.context_.is_empty());
  let mut context = i_mut.context_.get(scope).unwrap();
  context.enter();
  let mut try_catch = v8::TryCatch::new(scope);
  let tc = try_catch.enter();

  assert!(i_mut.resolve_cb_.is_none());
  i_mut.resolve_cb_ = Some(resolve_cb);

  let maybe_info = i_mut.get_module_info(id);

  if maybe_info.is_none() {
    return;
  }

  let module_handle = &maybe_info.unwrap().handle;
  let mut module = module_handle.get(scope).unwrap();

  if module.get_status() == v8::ModuleStatus::Errored {
    return;
  }

  let maybe_ok = module.instantiate_module(context, resolve_callback);
  assert!(maybe_ok.is_some() || tc.has_caught());
  i_mut.resolve_cb_.take();

  if tc.has_caught() {
    i_mut.handle_exception(scope, context, tc.exception().unwrap());
  }

  context.exit();
  i_mut.resolve_context_ = std::ptr::null_mut();
}

pub unsafe fn deno_mod_evaluate(
  i: *mut DenoIsolate,
  core_isolate: *const c_void,
  id: deno_mod,
) {
  let deno_isolate: &mut DenoIsolate = unsafe { &mut *i };
  let core_isolate: *mut c_void = unsafe { std::mem::transmute(core_isolate) };
  deno_isolate.core_isolate_ = core_isolate;
  let isolate = deno_isolate.isolate_.as_ref().unwrap();
  let mut locker = v8::Locker::new(isolate);
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();
  assert!(!deno_isolate.context_.is_empty());
  let mut context = deno_isolate.context_.get(scope).unwrap();
  context.enter();

  let info = deno_isolate
    .get_module_info(id)
    .expect("ModuleInfo not found");
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
      deno_isolate.last_exception_handle_.reset(scope);
      deno_isolate.last_exception_.take();
    }
    v8::ModuleStatus::Errored => {
      deno_isolate.handle_exception(scope, context, module.get_exception());
    }
    other => panic!("Unexpected module status {:?}", other),
  };

  context.exit();
  deno_isolate.core_isolate_ = std::ptr::null_mut();
}

/// Call exactly once for every deno_dyn_import_cb.
pub unsafe fn deno_dyn_import_done(
  i: *mut DenoIsolate,
  core_isolate: *const c_void,
  id: deno_dyn_import_id,
  mod_id: deno_mod,
  error_str: Option<String>,
) {
  let deno_isolate: &mut DenoIsolate = unsafe { &mut *i };
  assert!(
    (mod_id == 0 && error_str.is_some())
      || (mod_id != 0 && error_str.is_none())
      || (mod_id == 0 && !deno_isolate.last_exception_handle_.is_empty())
  );

  let core_isolate: *mut c_void = unsafe { std::mem::transmute(core_isolate) };
  deno_isolate.core_isolate_ = core_isolate;

  let isolate = deno_isolate.isolate_.as_ref().unwrap();
  let mut locker = v8::Locker::new(isolate);
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();
  assert!(!deno_isolate.context_.is_empty());
  let mut context = deno_isolate.context_.get(scope).unwrap();
  context.enter();

  // TODO(ry) error on bad import_id.
  let mut resolver_handle = deno_isolate.dyn_import_map_.remove(&id).unwrap();
  /// Resolve.
  let mut resolver = resolver_handle.get(scope).unwrap();
  resolver_handle.reset(scope);

  let maybe_info = deno_isolate.get_module_info(mod_id);

  if let Some(info) = maybe_info {
    // Resolution success
    let mut module = info.handle.get(scope).unwrap();
    assert_eq!(module.get_status(), v8::ModuleStatus::Evaluated);
    let module_namespace = module.get_module_namespace();
    resolver.resolve(context, module_namespace).unwrap();
  } else {
    // Resolution error.
    if let Some(error_str) = error_str {
      let msg = v8::String::new(scope, &error_str).unwrap();
      let isolate = context.get_isolate();
      isolate.enter();
      let e = v8::type_error(scope, msg);
      isolate.exit();
      resolver.reject(context, e).unwrap();
    } else {
      let e = deno_isolate.last_exception_handle_.get(scope).unwrap();
      deno_isolate.last_exception_handle_.reset(scope);
      deno_isolate.last_exception_.take();
      resolver.reject(context, e).unwrap();
    }
  }

  isolate.run_microtasks();

  context.exit();
  deno_isolate.core_isolate_ = std::ptr::null_mut();
}

pub fn deno_snapshot_new(i: *mut DenoIsolate) -> v8::OwnedStartupData {
  let deno_isolate: &mut DenoIsolate = unsafe { &mut *i };
  assert!(deno_isolate.snapshot_creator_.is_some());

  let isolate = deno_isolate.isolate_.as_ref().unwrap();
  let mut locker = v8::Locker::new(isolate);
  let mut hs = v8::HandleScope::new(&mut locker);
  let scope = hs.enter();

  // d.clear_modules();
  deno_isolate.context_.reset(scope);

  let snapshot_creator = deno_isolate.snapshot_creator_.as_mut().unwrap();
  let startup_data = snapshot_creator
    .create_blob(v8::FunctionCodeHandling::Keep)
    .unwrap();
  deno_isolate.has_snapshotted_ = true;
  startup_data
}

#[allow(dead_code)]
pub unsafe fn deno_snapshot_delete(s: &mut deno_snapshot) {
  todo!()
}
