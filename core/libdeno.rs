// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#![allow(unused)]

use rusty_v8 as v8;

use libc::c_char;
use libc::c_int;
use libc::c_void;
use libc::size_t;
use std::convert::From;
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

pub struct DenoIsolate {
  isolate_: Option<v8::OwnedIsolate>,
  last_exception_: Option<CString>,
  context_: v8::Global<v8::Context>,
  /*
  v8::Isolate* isolate_;
  v8::Locker* locker_;
  deno_buf shared_;
  const v8::FunctionCallbackInfo<v8::Value>* current_args_;
  v8::SnapshotCreator* snapshot_creator_;
  void* global_import_buf_ptr_;
  deno_recv_cb recv_cb_;
  void* user_data_;

  std::map<deno_mod, ModuleInfo> mods_;
  std::map<std::string, deno_mod> mods_by_name_;
  deno_resolve_cb resolve_cb_;

  deno_dyn_import_id next_dyn_import_id_;
  deno_dyn_import_cb dyn_import_cb_;
  std::map<deno_dyn_import_id, v8::Persistent<v8::Promise::Resolver>>
      dyn_import_map_;

  std::map<int, v8::Persistent<v8::Value>> pending_promise_map_;
  v8::Persistent<v8::Value> last_exception_handle_;
  v8::Persistent<v8::Function> recv_;
  v8::StartupData snapshot_;
  v8::Persistent<v8::ArrayBuffer> global_import_buf_;
  v8::Persistent<v8::SharedArrayBuffer> shared_ab_;
  bool has_snapshotted_;
  */
}

impl Drop for DenoIsolate {
  fn drop(&mut self) {
    // println!("DenoIsolate drop");
  }
}

impl DenoIsolate {
  pub fn new(config: deno_config) -> Self {
    Self {
      isolate_: None,
      last_exception_: None,
      context_: v8::Global::<v8::Context>::new(),
    }
    /*
      : isolate_(nullptr),
        locker_(nullptr),
        shared_(config.shared),
        current_args_(nullptr),
        snapshot_creator_(nullptr),
        global_import_buf_ptr_(nullptr),
        recv_cb_(config.recv_cb),
        user_data_(nullptr),
        resolve_cb_(nullptr),
        next_dyn_import_id_(0),
        dyn_import_cb_(config.dyn_import_cb),
        has_snapshotted_(false) {
    if !config.load_snapshot.data_ptr.is_null() {
      snapshot_.data =
          reinterpret_cast<const char*>(config.load_snapshot.data_ptr);
      snapshot_.raw_size = static_cast<int>(config.load_snapshot.data_len);
    }
    */
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
    // TODO isolate_->SetData(0, this);
    self.isolate_ = Some(isolate);
  }

  pub fn register_module(
    &self,
    main: bool,
    name: &str,
    source: &str,
  ) -> deno_mod {
    let isolate = self.isolate_.as_ref().unwrap();
    let mut locker = v8::Locker::new(&isolate);

    {
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
        // TODO HandleException(context, try_catch.Exception());
        return 0;
      }
      let module = maybe_module.unwrap();
      let id = module.get_identity_hash();

      // assert_eq!(v8::ModuleStatus::Uninstantiated, module.get_status());
      context.exit();
    }

    todo!()
    /*
    v8::Isolate::Scope isolate_scope(isolate_);
    v8::Locker locker(isolate_);
    v8::HandleScope handle_scope(isolate_);
    auto context = context_.Get(isolate_);
    v8::Context::Scope context_scope(context);

    v8::Local<v8::String> name_str = v8_str(name);
    v8::Local<v8::String> source_str = v8_str(source);

    auto origin = ModuleOrigin(isolate_, name_str);
    v8::ScriptCompiler::Source source_(source_str, origin);

    v8::TryCatch try_catch(isolate_);

    auto maybe_module = v8::ScriptCompiler::CompileModule(isolate_, &source_);

    if (try_catch.HasCaught()) {
      CHECK(maybe_module.IsEmpty());
      HandleException(context, try_catch.Exception());
      return 0;
    }

    auto module = maybe_module.ToLocalChecked();

    int id = module->GetIdentityHash();

    std::vector<std::string> import_specifiers;

    for (int i = 0; i < module->GetModuleRequestsLength(); ++i) {
      v8::Local<v8::String> specifier = module->GetModuleRequest(i);
      v8::String::Utf8Value specifier_utf8(isolate_, specifier);
      import_specifiers.push_back(*specifier_utf8);
    }

    mods_.emplace(
        std::piecewise_construct, std::make_tuple(id),
        std::make_tuple(isolate_, module, main, name, import_specifiers));
    mods_by_name_[name] = id;

    return id;
    */
  }
}

fn module_origin<'a>(
  s: &mut impl v8::ToLocal<'a>,
  resource_name: v8::Local<'a, v8::String>,
) -> v8::ScriptOrigin<'a> {
  let resource_line_offset = v8::Integer::new(s, 0);
  let resource_column_offset = v8::Integer::new(s, 0);
  let resource_is_shared_cross_origin = v8::new_false(s);
  let script_id = v8::Integer::new(s, 123);
  let source_map_url = v8::String::new(s, "source_map_url").unwrap();
  let resource_is_opaque = v8::new_true(s);
  let is_wasm = v8::new_false(s);
  let is_module = v8::new_true(s);
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
  _context: v8::Local<v8::Context>,
  _referrer: v8::Local<v8::ScriptOrModule>,
  _specifier: v8::Local<v8::String>,
) -> *mut v8::Promise {
  todo!()
  /*
  auto* isolate = context->GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  v8::Isolate::Scope isolate_scope(isolate);
  v8::Context::Scope context_scope(context);
  v8::EscapableHandleScope handle_scope(isolate);

  v8::String::Utf8Value specifier_str(isolate, specifier);

  auto referrer_name = referrer->GetResourceName();
  v8::String::Utf8Value referrer_name_str(isolate, referrer_name);

  // TODO(ry) I'm not sure what HostDefinedOptions is for or if we're ever going
  // to use it. For now we check that it is not used. This check may need to be
  // changed in the future.
  auto host_defined_options = referrer->GetHostDefinedOptions();
  CHECK_EQ(host_defined_options->Length(), 0);

  v8::Local<v8::Promise::Resolver> resolver =
      v8::Promise::Resolver::New(context).ToLocalChecked();

  deno_dyn_import_id import_id = d->next_dyn_import_id_++;

  d->dyn_import_map_.emplace(std::piecewise_construct,
                             std::make_tuple(import_id),
                             std::make_tuple(d->isolate_, resolver));

  d->dyn_import_cb_(d->user_data_, *specifier_str, *referrer_name_str,
                    import_id);

  auto promise = resolver->GetPromise();
  return handle_scope.Escape(promise);
  */
}

extern "C" fn host_initialize_import_meta_object_callback(
  _context: v8::Local<v8::Context>,
  _module: v8::Local<v8::Module>,
  _meta: v8::Local<v8::Object>,
) {
  todo!()
  /*
  auto* isolate = context->GetIsolate();
  DenoIsolate* d = DenoIsolate::FromIsolate(isolate);
  v8::Isolate::Scope isolate_scope(isolate);

  CHECK(!module.IsEmpty());

  deno_mod id = module->GetIdentityHash();
  CHECK_NE(id, 0);

  auto* info = d->GetModuleInfo(id);

  const char* url = info->name.c_str();
  const bool main = info->main;

  meta->CreateDataProperty(context, v8_str("url"), v8_str(url)).ToChecked();
  meta->CreateDataProperty(context, v8_str("main"), v8_bool(main)).ToChecked();
  */
}

extern "C" fn message_callback(
  _message: v8::Local<v8::Message>,
  _exception: v8::Local<v8::Value>,
) {
  todo!()
  /*
  auto* isolate = message->GetIsolate();
  DenoIsolate* d = static_cast<DenoIsolate*>(isolate->GetData(0));
  v8::HandleScope handle_scope(isolate);
  auto context = d->context_.Get(isolate);
  HandleExceptionMessage(context, message);
  */
}

extern "C" fn promise_reject_callback(
  _promise_reject_message: v8::PromiseRejectMessage,
) {
  todo!()
  /*
  auto* isolate = v8::Isolate::GetCurrent();
  DenoIsolate* d = static_cast<DenoIsolate*>(isolate->GetData(0));
  DCHECK_EQ(d->isolate_, isolate);
  v8::HandleScope handle_scope(d->isolate_);
  auto error = promise_reject_message.GetValue();
  auto context = d->context_.Get(d->isolate_);
  auto promise = promise_reject_message.GetPromise();

  v8::Context::Scope context_scope(context);

  int promise_id = promise->GetIdentityHash();
  switch (promise_reject_message.GetEvent()) {
    case v8::kPromiseRejectWithNoHandler:
      // Insert the error into the pending_promise_map_ using the promise's id
      // as the key.
      d->pending_promise_map_.emplace(std::piecewise_construct,
                                      std::make_tuple(promise_id),
                                      std::make_tuple(d->isolate_, error));
      break;

    case v8::kPromiseHandlerAddedAfterReject:
      d->pending_promise_map_.erase(promise_id);
      break;

    case v8::kPromiseRejectAfterResolved:
      break;

    case v8::kPromiseResolveAfterResolved:
      // Should not warn. See #1272
      break;

    default:
      CHECK(false && "unreachable");
  }
  */
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
  op_id: OpId,
  control_buf: deno_buf,
  zero_copy_buf: deno_pinned_buf,
);

/// Called when dynamic import is called in JS: import('foo')
/// Embedder must call deno_dyn_import_done() with the specified id and
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

pub unsafe fn deno_set_v8_flags(argc: *mut c_int, argv: *mut *mut c_char) {
  todo!()
}

lazy_static! {
  static ref EXTERNAL_REFERENCES: v8::ExternalReferences =
    v8::ExternalReferences::new(&[]);
}

pub unsafe fn deno_new_snapshotter(config: deno_config) -> *const isolate {
  assert_eq!(config.will_snapshot, 0);
  // TODO(ry) Support loading snapshots before snapshotting.
  assert!(config.load_snapshot.data_ptr.is_null());
  let mut snapshot_creator =
    v8::SnapshotCreator::new(Some(&EXTERNAL_REFERENCES));
  let isolate = snapshot_creator.get_isolate();
  let mut locker = v8::Locker::new(&isolate);
  {
    let mut hs = v8::HandleScope::new(&mut locker);
    let scope = hs.enter();
    let mut context = v8::Context::new(scope);
    context.enter();
  }

  /*
    auto* creator = new v8::SnapshotCreator(deno::external_references);
    auto* isolate = creator->GetIsolate();
    auto* d = new deno::DenoIsolate(config);
    d->snapshot_creator_ = creator;
    d->AddIsolate(isolate);
    {
      v8::Locker locker(isolate);
      v8::Isolate::Scope isolate_scope(isolate);
      v8::HandleScope handle_scope(isolate);
      auto context = v8::Context::New(isolate);
      d->context_.Reset(isolate, context);

      creator->SetDefaultContext(context,
                                 v8::SerializeInternalFieldsCallback(
                                     deno::SerializeInternalFields, nullptr));
      deno::InitializeContext(isolate, context);
    }
    return reinterpret_cast<Deno*>(d);
  */
  todo!()
}

extern "C" fn print(info: &v8::FunctionCallbackInfo) {
  todo!()
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

  let core_val = v8::Object::new(scope);

  global.set(
    context,
    v8::String::new(scope, "core").unwrap().into(),
    core_val.into(),
  );

  let mut print_tmpl = v8::FunctionTemplate::new(scope, print);
  let mut print_val = print_tmpl.get_function(scope, context).unwrap();
  /*
  core_val.set(
    context,
    v8::String::new(scope, "print").unwrap().into(),
    print_val.into(),
  );
  */

  // todo!()
  /*


  auto print_tmpl = v8::FunctionTemplate::New(isolate, Print);
  auto print_val = print_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(core_val->Set(context, deno::v8_str("print"), print_val).FromJust());

  auto recv_tmpl = v8::FunctionTemplate::New(isolate, Recv);
  auto recv_val = recv_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(core_val->Set(context, deno::v8_str("recv"), recv_val).FromJust());

  auto send_tmpl = v8::FunctionTemplate::New(isolate, Send);
  auto send_val = send_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(core_val->Set(context, deno::v8_str("send"), send_val).FromJust());

  auto eval_context_tmpl = v8::FunctionTemplate::New(isolate, EvalContext);
  auto eval_context_val =
      eval_context_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(core_val->Set(context, deno::v8_str("evalContext"), eval_context_val)
            .FromJust());

  auto error_to_json_tmpl = v8::FunctionTemplate::New(isolate, ErrorToJSON);
  auto error_to_json_val =
      error_to_json_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(core_val->Set(context, deno::v8_str("errorToJSON"), error_to_json_val)
            .FromJust());

  CHECK(core_val->SetAccessor(context, deno::v8_str("shared"), Shared)
            .FromJust());

  // Direct bindings on `window`.
  auto queue_microtask_tmpl =
      v8::FunctionTemplate::New(isolate, QueueMicrotask);
  auto queue_microtask_val =
      queue_microtask_tmpl->GetFunction(context).ToLocalChecked();
  CHECK(
      global->Set(context, deno::v8_str("queueMicrotask"), queue_microtask_val)
          .FromJust());
  */
  context.exit();
}

pub unsafe fn deno_new(config: deno_config) -> *const isolate {
  if config.will_snapshot != 0 {
    return deno_new_snapshotter(config);
  }

  let load_snapshot_is_null = config.load_snapshot.data_ptr.is_null();

  let mut d = Box::new(DenoIsolate::new(config));
  let mut params = v8::Isolate::create_params();
  params.set_array_buffer_allocator(v8::new_default_allocator());
  params.set_external_references(&EXTERNAL_REFERENCES);
  /*
  if !config.load_snapshot.data_ptr.is_null() {
    params.set_snapshot_blob(d->snapshot_);
  }
  */

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
  /*

  v8::Locker locker(isolate);
  v8::Isolate::Scope isolate_scope(isolate);
  {
    v8::HandleScope handle_scope(isolate);
    auto context =
        v8::Context::New(isolate, nullptr, v8::MaybeLocal<v8::ObjectTemplate>(),
                         v8::MaybeLocal<v8::Value>(),
                         v8::DeserializeInternalFieldsCallback(
                             deno::DeserializeInternalFields, nullptr));
    if (!config.load_snapshot.data_ptr) {
    }
    d->context_.Reset(isolate, context);
  }

  return reinterpret_cast<Deno*>(d);
     */
  //let ptr: *const DenoIsolate = &d;
  //println!("deno_new -> DenoIsolate ptr {:?}", ptr);
  //std::mem::forget(d);
  return Box::into_raw(d);
}

pub unsafe fn deno_delete(i: *const DenoIsolate) {
  let deno_isolate = unsafe { Box::from_raw(i as *mut DenoIsolate) };
  drop(deno_isolate);
}

pub unsafe fn deno_last_exception(i: *const DenoIsolate) -> *const c_char {
  match (*i).last_exception_.as_ref() {
    None => std::ptr::null(),
    Some(e) => e.as_ptr(),
  }
}

pub unsafe fn deno_clear_last_exception(i: *const isolate) {
  todo!()
}
pub unsafe fn deno_check_promise_errors(i: *const isolate) {
  todo!()
}
pub unsafe fn deno_lock(i: *const isolate) {
  todo!()
}
pub unsafe fn deno_unlock(i: *const isolate) {
  todo!()
}
pub unsafe fn deno_throw_exception(i: *const isolate, text: *const c_char) {
  todo!()
}
pub unsafe fn deno_respond(
  i: *const isolate,
  user_data: *const c_void,
  op_id: OpId,
  buf: deno_buf,
) {
  todo!()
}
pub unsafe fn deno_pinned_buf_delete(buf: &mut deno_pinned_buf) {
  todo!()
}
pub unsafe fn deno_execute(
  i: *const DenoIsolate,
  user_data: *const c_void,
  js_filename: *const c_char,
  js_source: *const c_char,
) {
  let isolate = (*i).isolate_.as_ref().unwrap();
  // println!("deno_execute -> Isolate ptr {:?}", isolate);
  let mut locker = v8::Locker::new(isolate);
  // todo!()
  /*
  auto* d = deno::unwrap(d_);
  deno::UserDataScope user_data_scope(d, user_data);
  auto* isolate = d->isolate_;
  v8::Locker locker(isolate);
  v8::Isolate::Scope isolate_scope(isolate);
  v8::HandleScope handle_scope(isolate);
  auto context = d->context_.Get(d->isolate_);
  CHECK(!context.IsEmpty());
  deno::Execute(context, js_filename, js_source);
  */
}

// deno::Execute
fn execute(
  context: v8::Local<v8::Context>,
  js_filename: &str,
  js_source: &str,
) {
  todo!()
  /*
  auto* isolate = context->GetIsolate();
  v8::Isolate::Scope isolate_scope(isolate);
  v8::HandleScope handle_scope(isolate);
  v8::Context::Scope context_scope(context);

  auto source = v8_str(js_source);
  auto name = v8_str(js_filename);

  v8::TryCatch try_catch(isolate);

  v8::ScriptOrigin origin(name);

  auto script = v8::Script::Compile(context, source, &origin);

  if (script.IsEmpty()) {
    DCHECK(try_catch.HasCaught());
    HandleException(context, try_catch.Exception());
    return false;
  }

  auto result = script.ToLocalChecked()->Run(context);

  if (result.IsEmpty()) {
    DCHECK(try_catch.HasCaught());
    HandleException(context, try_catch.Exception());
    return false;
  }

  return true;
  */
}

pub unsafe fn deno_terminate_execution(i: *const isolate) {
  todo!()
}

#[allow(dead_code)]
pub unsafe fn deno_run_microtasks(i: *const isolate, user_data: *const c_void) {
  todo!()
}

// Modules

pub unsafe fn deno_mod_new(
  i: *const DenoIsolate,
  main: bool,
  name: &str,
  source: &str,
) -> deno_mod {
  (*i).register_module(main, name, source)
}

pub unsafe fn deno_mod_imports_len(i: *const isolate, id: deno_mod) -> size_t {
  todo!()
}

pub unsafe fn deno_mod_imports_get(
  i: *const isolate,
  id: deno_mod,
  index: size_t,
) -> *const c_char {
  todo!()
}

pub unsafe fn deno_mod_instantiate(
  i: *const isolate,
  user_data: *const c_void,
  id: deno_mod,
  resolve_cb: deno_resolve_cb,
) {
  todo!()
}

pub unsafe fn deno_mod_evaluate(
  i: *const isolate,
  user_data: *const c_void,
  id: deno_mod,
) {
  todo!()
}

/// Call exactly once for every deno_dyn_import_cb.
pub unsafe fn deno_dyn_import_done(
  i: *const isolate,
  user_data: *const c_void,
  id: deno_dyn_import_id,
  mod_id: deno_mod,
  error_str: *const c_char,
) {
  todo!()
}

pub unsafe fn deno_snapshot_new(i: *const isolate) -> Snapshot1<'static> {
  todo!()
}

#[allow(dead_code)]
pub unsafe fn deno_snapshot_delete(s: &mut deno_snapshot) {
  todo!()
}
