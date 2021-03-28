// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use rusty_v8 as v8;

use crate::bindings;
use crate::error::attach_handle_to_error;
use crate::error::generic_error;
use crate::error::AnyError;
use crate::error::ErrWithV8Handle;
use crate::error::JsError;
use crate::futures::FutureExt;
use crate::module_specifier::ModuleSpecifier;
use crate::modules::LoadState;
use crate::modules::ModuleId;
use crate::modules::ModuleLoadId;
use crate::modules::ModuleLoader;
use crate::modules::ModuleMap;
use crate::modules::ModuleSource;
use crate::modules::NoopModuleLoader;
use crate::modules::PrepareLoadFuture;
use crate::modules::RecursiveModuleLoad;
use crate::ops::*;
use crate::OpPayload;
use crate::OpResponse;
use crate::OpState;
use crate::PromiseId;
use futures::channel::mpsc;
use futures::future::poll_fn;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use futures::stream::StreamFuture;
use futures::task::AtomicWaker;
use futures::Future;
use log::debug;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::ffi::c_void;
use std::mem::forget;
use std::option::Option;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Once;
use std::task::Context;
use std::task::Poll;

type PendingOpFuture = Pin<Box<dyn Future<Output = (PromiseId, OpResponse)>>>;

pub enum Snapshot {
  Static(&'static [u8]),
  JustCreated(v8::StartupData),
  Boxed(Box<[u8]>),
}

pub type JsErrorCreateFn = dyn Fn(JsError) -> AnyError;

pub type GetErrorClassFn =
  &'static dyn for<'e> Fn(&'e AnyError) -> &'static str;

/// Objects that need to live as long as the isolate
#[derive(Default)]
struct IsolateAllocations {
  near_heap_limit_callback_data:
    Option<(Box<RefCell<dyn Any>>, v8::NearHeapLimitCallback)>,
}

/// A single execution context of JavaScript. Corresponds roughly to the "Web
/// Worker" concept in the DOM. A JsRuntime is a Future that can be used with
/// an event loop (Tokio, async_std).
////
/// The JsRuntime future completes when there is an error or when all
/// pending ops have completed.
///
/// Ops are created in JavaScript by calling Deno.core.dispatch(), and in Rust
/// by implementing dispatcher function that takes control buffer and optional zero copy buffer
/// as arguments. An async Op corresponds exactly to a Promise in JavaScript.
pub struct JsRuntime {
  // This is an Option<OwnedIsolate> instead of just OwnedIsolate to workaround
  // an safety issue with SnapshotCreator. See JsRuntime::drop.
  v8_isolate: Option<v8::OwnedIsolate>,
  snapshot_creator: Option<v8::SnapshotCreator>,
  has_snapshotted: bool,
  allocations: IsolateAllocations,
}

struct DynImportModEvaluate {
  module_id: ModuleId,
  promise: v8::Global<v8::Promise>,
  module: v8::Global<v8::Module>,
}

struct ModEvaluate {
  promise: v8::Global<v8::Promise>,
  sender: mpsc::Sender<Result<(), AnyError>>,
}

/// Internal state for JsRuntime which is stored in one of v8::Isolate's
/// embedder slots.
pub(crate) struct JsRuntimeState {
  pub global_context: Option<v8::Global<v8::Context>>,
  pub(crate) js_recv_cb: Option<v8::Global<v8::Function>>,
  pub(crate) js_macrotask_cb: Option<v8::Global<v8::Function>>,
  pub(crate) pending_promise_exceptions:
    HashMap<v8::Global<v8::Promise>, v8::Global<v8::Value>>,
  pending_dyn_mod_evaluate: HashMap<ModuleLoadId, DynImportModEvaluate>,
  pending_mod_evaluate: Option<ModEvaluate>,
  pub(crate) js_error_create_fn: Rc<JsErrorCreateFn>,
  pub(crate) pending_ops: FuturesUnordered<PendingOpFuture>,
  pub(crate) pending_unref_ops: FuturesUnordered<PendingOpFuture>,
  pub(crate) have_unpolled_ops: bool,
  pub(crate) op_state: Rc<RefCell<OpState>>,
  pub loader: Rc<dyn ModuleLoader>,
  pub module_map: ModuleMap,
  pub(crate) dyn_import_map:
    HashMap<ModuleLoadId, v8::Global<v8::PromiseResolver>>,
  preparing_dyn_imports: FuturesUnordered<Pin<Box<PrepareLoadFuture>>>,
  pending_dyn_imports: FuturesUnordered<StreamFuture<RecursiveModuleLoad>>,
  waker: AtomicWaker,
}

impl Drop for JsRuntime {
  fn drop(&mut self) {
    if let Some(creator) = self.snapshot_creator.take() {
      // TODO(ry): in rusty_v8, `SnapShotCreator::get_owned_isolate()` returns
      // a `struct OwnedIsolate` which is not actually owned, hence the need
      // here to leak the `OwnedIsolate` in order to avoid a double free and
      // the segfault that it causes.
      let v8_isolate = self.v8_isolate.take().unwrap();
      forget(v8_isolate);

      // TODO(ry) V8 has a strange assert which prevents a SnapshotCreator from
      // being deallocated if it hasn't created a snapshot yet.
      // https://github.com/v8/v8/blob/73212783fbd534fac76cc4b66aac899c13f71fc8/src/api.cc#L603
      // If that assert is removed, this if guard could be removed.
      // WARNING: There may be false positive LSAN errors here.
      if self.has_snapshotted {
        drop(creator);
      }
    }
  }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn v8_init() {
  let platform = v8::new_default_platform().unwrap();
  v8::V8::initialize_platform(platform);
  v8::V8::initialize();
  let argv = vec![
    "".to_string(),
    "--wasm-test-streaming".to_string(),
    // TODO(ry) This makes WASM compile synchronously. Eventually we should
    // remove this to make it work asynchronously too. But that requires getting
    // PumpMessageLoop and RunMicrotasks setup correctly.
    // See https://github.com/denoland/deno/issues/2544
    "--no-wasm-async-compilation".to_string(),
    "--harmony-top-level-await".to_string(),
    "--harmony-import-assertions".to_string(),
    "--no-validate-asm".to_string(),
  ];
  v8::V8::set_flags_from_command_line(argv);
}

#[derive(Default)]
pub struct RuntimeOptions {
  /// Allows a callback to be set whenever a V8 exception is made. This allows
  /// the caller to wrap the JsError into an error. By default this callback
  /// is set to `JsError::create()`.
  pub js_error_create_fn: Option<Rc<JsErrorCreateFn>>,

  /// Allows to map error type to a string "class" used to represent
  /// error in JavaScript.
  pub get_error_class_fn: Option<GetErrorClassFn>,

  /// Implementation of `ModuleLoader` which will be
  /// called when V8 requests to load ES modules.
  ///
  /// If not provided runtime will error if code being
  /// executed tries to load modules.
  pub module_loader: Option<Rc<dyn ModuleLoader>>,

  /// V8 snapshot that should be loaded on startup.
  ///
  /// Currently can't be used with `will_snapshot`.
  pub startup_snapshot: Option<Snapshot>,

  /// Prepare runtime to take snapshot of loaded code.
  ///
  /// Currently can't be used with `startup_snapshot`.
  pub will_snapshot: bool,

  /// Isolate creation parameters.
  pub create_params: Option<v8::CreateParams>,
}

impl JsRuntime {
  /// Only constructor, configuration is done through `options`.
  pub fn new(mut options: RuntimeOptions) -> Self {
    static DENO_INIT: Once = Once::new();
    DENO_INIT.call_once(|| {
      // Include 10MB ICU data file.
      #[repr(C, align(16))]
      struct IcuData([u8; 10413584]);
      static ICU_DATA: IcuData = IcuData(*include_bytes!("icudtl.dat"));
      v8::icu::set_common_data(&ICU_DATA.0).unwrap();
      unsafe { v8_init() };
    });

    let has_startup_snapshot = options.startup_snapshot.is_some();

    let global_context;
    let (mut isolate, maybe_snapshot_creator) = if options.will_snapshot {
      // TODO(ry) Support loading snapshots before snapshotting.
      assert!(options.startup_snapshot.is_none());
      let mut creator =
        v8::SnapshotCreator::new(Some(&bindings::EXTERNAL_REFERENCES));
      let isolate = unsafe { creator.get_owned_isolate() };
      let mut isolate = JsRuntime::setup_isolate(isolate);
      {
        let scope = &mut v8::HandleScope::new(&mut isolate);
        let context = bindings::initialize_context(scope);
        global_context = v8::Global::new(scope, context);
        creator.set_default_context(context);
      }
      (isolate, Some(creator))
    } else {
      let mut params = options
        .create_params
        .take()
        .unwrap_or_else(v8::Isolate::create_params)
        .external_references(&**bindings::EXTERNAL_REFERENCES);
      let snapshot_loaded = if let Some(snapshot) = options.startup_snapshot {
        params = match snapshot {
          Snapshot::Static(data) => params.snapshot_blob(data),
          Snapshot::JustCreated(data) => params.snapshot_blob(data),
          Snapshot::Boxed(data) => params.snapshot_blob(data),
        };
        true
      } else {
        false
      };

      let isolate = v8::Isolate::new(params);
      let mut isolate = JsRuntime::setup_isolate(isolate);
      {
        let scope = &mut v8::HandleScope::new(&mut isolate);
        let context = if snapshot_loaded {
          v8::Context::new(scope)
        } else {
          // If no snapshot is provided, we initialize the context with empty
          // main source code and source maps.
          bindings::initialize_context(scope)
        };
        global_context = v8::Global::new(scope, context);
      }
      (isolate, None)
    };

    let loader = options
      .module_loader
      .unwrap_or_else(|| Rc::new(NoopModuleLoader));

    let js_error_create_fn = options
      .js_error_create_fn
      .unwrap_or_else(|| Rc::new(JsError::create));
    let mut op_state = OpState::new();

    if let Some(get_error_class_fn) = options.get_error_class_fn {
      op_state.get_error_class_fn = get_error_class_fn;
    }

    isolate.set_slot(Rc::new(RefCell::new(JsRuntimeState {
      global_context: Some(global_context),
      pending_promise_exceptions: HashMap::new(),
      pending_dyn_mod_evaluate: HashMap::new(),
      pending_mod_evaluate: None,
      js_recv_cb: None,
      js_macrotask_cb: None,
      js_error_create_fn,
      pending_ops: FuturesUnordered::new(),
      pending_unref_ops: FuturesUnordered::new(),
      op_state: Rc::new(RefCell::new(op_state)),
      have_unpolled_ops: false,
      module_map: ModuleMap::new(),
      loader,
      dyn_import_map: HashMap::new(),
      preparing_dyn_imports: FuturesUnordered::new(),
      pending_dyn_imports: FuturesUnordered::new(),
      waker: AtomicWaker::new(),
    })));

    let mut js_runtime = Self {
      v8_isolate: Some(isolate),
      snapshot_creator: maybe_snapshot_creator,
      has_snapshotted: false,
      allocations: IsolateAllocations::default(),
    };

    if !has_startup_snapshot {
      js_runtime.js_init();
    }

    if !options.will_snapshot {
      js_runtime.core_js_init();
    }

    js_runtime
  }

  pub fn global_context(&mut self) -> v8::Global<v8::Context> {
    let state = Self::state(self.v8_isolate());
    let state = state.borrow();
    state.global_context.clone().unwrap()
  }

  pub fn v8_isolate(&mut self) -> &mut v8::OwnedIsolate {
    self.v8_isolate.as_mut().unwrap()
  }

  fn setup_isolate(mut isolate: v8::OwnedIsolate) -> v8::OwnedIsolate {
    isolate.set_capture_stack_trace_for_uncaught_exceptions(true, 10);
    isolate.set_promise_reject_callback(bindings::promise_reject_callback);
    isolate.set_host_initialize_import_meta_object_callback(
      bindings::host_initialize_import_meta_object_callback,
    );
    isolate.set_host_import_module_dynamically_callback(
      bindings::host_import_module_dynamically_callback,
    );
    isolate
  }

  pub(crate) fn state(isolate: &v8::Isolate) -> Rc<RefCell<JsRuntimeState>> {
    let s = isolate.get_slot::<Rc<RefCell<JsRuntimeState>>>().unwrap();
    s.clone()
  }

  /// Executes a JavaScript code to provide Deno.core and error reporting.
  ///
  /// This function can be called during snapshotting.
  fn js_init(&mut self) {
    self
      .execute("deno:core/core.js", include_str!("core.js"))
      .unwrap();
    self
      .execute("deno:core/error.js", include_str!("error.js"))
      .unwrap();
  }

  /// Executes JavaScript code to initialize core.js,
  /// specifically the js_recv_cb setter
  ///
  /// This function mustn't be called during snapshotting.
  fn core_js_init(&mut self) {
    self
      .execute("deno:core/init.js", "Deno.core.init()")
      .unwrap();
  }

  /// Returns the runtime's op state, which can be used to maintain ops
  /// and access resources between op calls.
  pub fn op_state(&mut self) -> Rc<RefCell<OpState>> {
    let state_rc = Self::state(self.v8_isolate());
    let state = state_rc.borrow();
    state.op_state.clone()
  }

  /// Executes traditional JavaScript code (traditional = not ES modules)
  ///
  /// The execution takes place on the current global context, so it is possible
  /// to maintain local JS state and invoke this method multiple times.
  ///
  /// `AnyError` can be downcast to a type that exposes additional information
  /// about the V8 exception. By default this type is `JsError`, however it may
  /// be a different type if `RuntimeOptions::js_error_create_fn` has been set.
  pub fn execute(
    &mut self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), AnyError> {
    let context = self.global_context();

    let scope = &mut v8::HandleScope::with_context(self.v8_isolate(), context);

    let source = v8::String::new(scope, js_source).unwrap();
    let name = v8::String::new(scope, js_filename).unwrap();
    let origin = bindings::script_origin(scope, name);

    let tc_scope = &mut v8::TryCatch::new(scope);

    let script = match v8::Script::compile(tc_scope, source, Some(&origin)) {
      Some(script) => script,
      None => {
        let exception = tc_scope.exception().unwrap();
        return exception_to_err_result(tc_scope, exception, false);
      }
    };

    match script.run(tc_scope) {
      Some(_) => Ok(()),
      None => {
        assert!(tc_scope.has_caught());
        let exception = tc_scope.exception().unwrap();
        exception_to_err_result(tc_scope, exception, false)
      }
    }
  }

  /// Takes a snapshot. The isolate should have been created with will_snapshot
  /// set to true.
  ///
  /// `AnyError` can be downcast to a type that exposes additional information
  /// about the V8 exception. By default this type is `JsError`, however it may
  /// be a different type if `RuntimeOptions::js_error_create_fn` has been set.
  pub fn snapshot(&mut self) -> v8::StartupData {
    assert!(self.snapshot_creator.is_some());
    let state = Self::state(self.v8_isolate());

    // Note: create_blob() method must not be called from within a HandleScope.
    // TODO(piscisaureus): The rusty_v8 type system should enforce this.
    state.borrow_mut().global_context.take();

    std::mem::take(&mut state.borrow_mut().module_map);

    let snapshot_creator = self.snapshot_creator.as_mut().unwrap();
    let snapshot = snapshot_creator
      .create_blob(v8::FunctionCodeHandling::Keep)
      .unwrap();
    self.has_snapshotted = true;

    snapshot
  }

  /// Registers an op that can be called from JavaScript.
  ///
  /// The _op_ mechanism allows to expose Rust functions to the JS runtime,
  /// which can be called using the provided `name`.
  ///
  /// This function provides byte-level bindings. To pass data via JSON, the
  /// following functions can be passed as an argument for `op_fn`:
  /// * [json_op_sync()](fn.json_op_sync.html)
  /// * [json_op_async()](fn.json_op_async.html)
  pub fn register_op<F>(&mut self, name: &str, op_fn: F) -> OpId
  where
    F: Fn(RcOpState, OpPayload, OpBuf) -> Op + 'static,
  {
    Self::state(self.v8_isolate())
      .borrow_mut()
      .op_state
      .borrow_mut()
      .op_table
      .register_op(name, op_fn)
  }

  /// Registers a callback on the isolate when the memory limits are approached.
  /// Use this to prevent V8 from crashing the process when reaching the limit.
  ///
  /// Calls the closure with the current heap limit and the initial heap limit.
  /// The return value of the closure is set as the new limit.
  pub fn add_near_heap_limit_callback<C>(&mut self, cb: C)
  where
    C: FnMut(usize, usize) -> usize + 'static,
  {
    let boxed_cb = Box::new(RefCell::new(cb));
    let data = boxed_cb.as_ptr() as *mut c_void;

    let prev = self
      .allocations
      .near_heap_limit_callback_data
      .replace((boxed_cb, near_heap_limit_callback::<C>));
    if let Some((_, prev_cb)) = prev {
      self
        .v8_isolate()
        .remove_near_heap_limit_callback(prev_cb, 0);
    }

    self
      .v8_isolate()
      .add_near_heap_limit_callback(near_heap_limit_callback::<C>, data);
  }

  pub fn remove_near_heap_limit_callback(&mut self, heap_limit: usize) {
    if let Some((_, cb)) = self.allocations.near_heap_limit_callback_data.take()
    {
      self
        .v8_isolate()
        .remove_near_heap_limit_callback(cb, heap_limit);
    }
  }

  /// Runs event loop to completion
  ///
  /// This future resolves when:
  ///  - there are no more pending dynamic imports
  ///  - there are no more pending ops
  pub async fn run_event_loop(&mut self) -> Result<(), AnyError> {
    poll_fn(|cx| self.poll_event_loop(cx)).await
  }

  /// Runs a single tick of event loop
  pub fn poll_event_loop(
    &mut self,
    cx: &mut Context,
  ) -> Poll<Result<(), AnyError>> {
    let state_rc = Self::state(self.v8_isolate());
    {
      let state = state_rc.borrow();
      state.waker.register(cx.waker());
    }

    // Ops
    {
      let overflow_response = self.poll_pending_ops(cx);
      self.async_op_response(overflow_response)?;
      self.drain_macrotasks()?;
      self.check_promise_exceptions()?;
    }

    // Dynamic module loading - ie. modules loaded using "import()"
    {
      let poll_imports = self.prepare_dyn_imports(cx)?;
      assert!(poll_imports.is_ready());

      let poll_imports = self.poll_dyn_imports(cx)?;
      assert!(poll_imports.is_ready());

      self.evaluate_dyn_imports();

      self.check_promise_exceptions()?;
    }

    // Top level module
    self.evaluate_pending_module();

    let state = state_rc.borrow();
    let has_pending_ops = !state.pending_ops.is_empty();

    let has_pending_dyn_imports = !{
      state.preparing_dyn_imports.is_empty()
        && state.pending_dyn_imports.is_empty()
    };
    let has_pending_dyn_module_evaluation =
      !state.pending_dyn_mod_evaluate.is_empty();
    let has_pending_module_evaluation = state.pending_mod_evaluate.is_some();

    if !has_pending_ops
      && !has_pending_dyn_imports
      && !has_pending_dyn_module_evaluation
      && !has_pending_module_evaluation
    {
      return Poll::Ready(Ok(()));
    }

    // Check if more async ops have been dispatched
    // during this turn of event loop.
    if state.have_unpolled_ops {
      state.waker.wake();
    }

    if has_pending_module_evaluation {
      if has_pending_ops
        || has_pending_dyn_imports
        || has_pending_dyn_module_evaluation
      {
        // pass, will be polled again
      } else {
        let msg = "Module evaluation is still pending but there are no pending ops or dynamic imports. This situation is often caused by unresolved promise.";
        return Poll::Ready(Err(generic_error(msg)));
      }
    }

    if has_pending_dyn_module_evaluation {
      if has_pending_ops || has_pending_dyn_imports {
        // pass, will be polled again
      } else {
        let msg = "Dynamically imported module evaluation is still pending but there are no pending ops. This situation is often caused by unresolved promise.";
        return Poll::Ready(Err(generic_error(msg)));
      }
    }

    Poll::Pending
  }
}

extern "C" fn near_heap_limit_callback<F>(
  data: *mut c_void,
  current_heap_limit: usize,
  initial_heap_limit: usize,
) -> usize
where
  F: FnMut(usize, usize) -> usize,
{
  let callback = unsafe { &mut *(data as *mut F) };
  callback(current_heap_limit, initial_heap_limit)
}

impl JsRuntimeState {
  // Called by V8 during `Isolate::mod_instantiate`.
  pub fn dyn_import_cb(
    &mut self,
    resolver_handle: v8::Global<v8::PromiseResolver>,
    specifier: &str,
    referrer: &str,
  ) {
    debug!("dyn_import specifier {} referrer {} ", specifier, referrer);

    let load = RecursiveModuleLoad::dynamic_import(
      self.op_state.clone(),
      specifier,
      referrer,
      self.loader.clone(),
    );
    self.dyn_import_map.insert(load.id, resolver_handle);
    self.waker.wake();
    let fut = load.prepare().boxed_local();
    self.preparing_dyn_imports.push(fut);
  }
}

pub(crate) fn exception_to_err_result<'s, T>(
  scope: &mut v8::HandleScope<'s>,
  exception: v8::Local<v8::Value>,
  in_promise: bool,
) -> Result<T, AnyError> {
  let is_terminating_exception = scope.is_execution_terminating();
  let mut exception = exception;

  if is_terminating_exception {
    // TerminateExecution was called. Cancel exception termination so that the
    // exception can be created..
    scope.cancel_terminate_execution();

    // Maybe make a new exception object.
    if exception.is_null_or_undefined() {
      let message = v8::String::new(scope, "execution terminated").unwrap();
      exception = v8::Exception::error(scope, message);
    }
  }

  let mut js_error = JsError::from_v8_exception(scope, exception);
  if in_promise {
    js_error.message = format!(
      "Uncaught (in promise) {}",
      js_error.message.trim_start_matches("Uncaught ")
    );
  }

  let state_rc = JsRuntime::state(scope);
  let state = state_rc.borrow();
  let js_error = (state.js_error_create_fn)(js_error);

  if is_terminating_exception {
    // Re-enable exception termination.
    scope.terminate_execution();
  }

  Err(js_error)
}

// Related to module loading
impl JsRuntime {
  /// Low-level module creation.
  ///
  /// Called during module loading or dynamic import loading.
  fn mod_new(
    &mut self,
    main: bool,
    name: &str,
    source: &str,
  ) -> Result<ModuleId, AnyError> {
    let state_rc = Self::state(self.v8_isolate());
    let context = self.global_context();
    let scope = &mut v8::HandleScope::with_context(self.v8_isolate(), context);

    let name_str = v8::String::new(scope, name).unwrap();
    let source_str = v8::String::new(scope, source).unwrap();

    let origin = bindings::module_origin(scope, name_str);
    let source = v8::script_compiler::Source::new(source_str, Some(&origin));

    let tc_scope = &mut v8::TryCatch::new(scope);

    let maybe_module = v8::script_compiler::compile_module(tc_scope, source);

    if tc_scope.has_caught() {
      assert!(maybe_module.is_none());
      let e = tc_scope.exception().unwrap();
      return exception_to_err_result(tc_scope, e, false);
    }

    let module = maybe_module.unwrap();

    let mut import_specifiers: Vec<ModuleSpecifier> = vec![];
    let module_requests = module.get_module_requests();
    for i in 0..module_requests.length() {
      let module_request = v8::Local::<v8::ModuleRequest>::try_from(
        module_requests.get(tc_scope, i).unwrap(),
      )
      .unwrap();
      let import_specifier = module_request
        .get_specifier()
        .to_rust_string_lossy(tc_scope);
      let state = state_rc.borrow();
      let module_specifier = state.loader.resolve(
        state.op_state.clone(),
        &import_specifier,
        name,
        false,
      )?;
      import_specifiers.push(module_specifier);
    }

    let id = state_rc.borrow_mut().module_map.register(
      name,
      main,
      v8::Global::<v8::Module>::new(tc_scope, module),
      import_specifiers,
    );

    Ok(id)
  }

  /// Instantiates a ES module
  ///
  /// `AnyError` can be downcast to a type that exposes additional information
  /// about the V8 exception. By default this type is `JsError`, however it may
  /// be a different type if `RuntimeOptions::js_error_create_fn` has been set.
  fn mod_instantiate(&mut self, id: ModuleId) -> Result<(), AnyError> {
    let state_rc = Self::state(self.v8_isolate());
    let context = self.global_context();

    let scope = &mut v8::HandleScope::with_context(self.v8_isolate(), context);
    let tc_scope = &mut v8::TryCatch::new(scope);

    let module = state_rc
      .borrow()
      .module_map
      .get_handle(id)
      .map(|handle| v8::Local::new(tc_scope, handle))
      .expect("ModuleInfo not found");

    if module.get_status() == v8::ModuleStatus::Errored {
      let exception = module.get_exception();
      exception_to_err_result(tc_scope, exception, false)
        .map_err(|err| attach_handle_to_error(tc_scope, err, exception))
    } else {
      let instantiate_result =
        module.instantiate_module(tc_scope, bindings::module_resolve_callback);
      match instantiate_result {
        Some(_) => Ok(()),
        None => {
          let exception = tc_scope.exception().unwrap();
          exception_to_err_result(tc_scope, exception, false)
            .map_err(|err| attach_handle_to_error(tc_scope, err, exception))
        }
      }
    }
  }

  /// Evaluates an already instantiated ES module.
  ///
  /// `AnyError` can be downcast to a type that exposes additional information
  /// about the V8 exception. By default this type is `JsError`, however it may
  /// be a different type if `RuntimeOptions::js_error_create_fn` has been set.
  pub fn dyn_mod_evaluate(
    &mut self,
    load_id: ModuleLoadId,
    id: ModuleId,
  ) -> Result<(), AnyError> {
    let state_rc = Self::state(self.v8_isolate());
    let context = self.global_context();
    let context1 = self.global_context();

    let module_handle = state_rc
      .borrow()
      .module_map
      .get_handle(id)
      .expect("ModuleInfo not found");

    let status = {
      let scope =
        &mut v8::HandleScope::with_context(self.v8_isolate(), context);
      let module = module_handle.get(scope);
      module.get_status()
    };

    // Since the same module might be dynamically imported more than once,
    // we short-circuit is it is already evaluated.
    if status == v8::ModuleStatus::Evaluated {
      self.dyn_import_done(load_id, id);
      return Ok(());
    }

    if status != v8::ModuleStatus::Instantiated {
      return Ok(());
    }

    // IMPORTANT: Top-level-await is enabled, which means that return value
    // of module evaluation is a promise.
    //
    // This promise is internal, and not the same one that gets returned to
    // the user. We add an empty `.catch()` handler so that it does not result
    // in an exception if it rejects. That will instead happen for the other
    // promise if not handled by the user.
    //
    // For more details see:
    // https://github.com/denoland/deno/issues/4908
    // https://v8.dev/features/top-level-await#module-execution-order
    let scope = &mut v8::HandleScope::with_context(self.v8_isolate(), context1);
    let module = v8::Local::new(scope, &module_handle);
    let maybe_value = module.evaluate(scope);

    // Update status after evaluating.
    let status = module.get_status();

    if let Some(value) = maybe_value {
      assert!(
        status == v8::ModuleStatus::Evaluated
          || status == v8::ModuleStatus::Errored
      );
      let promise = v8::Local::<v8::Promise>::try_from(value)
        .expect("Expected to get promise as module evaluation result");
      let empty_fn = |_scope: &mut v8::HandleScope,
                      _args: v8::FunctionCallbackArguments,
                      _rv: v8::ReturnValue| {};
      let empty_fn = v8::FunctionTemplate::new(scope, empty_fn);
      let empty_fn = empty_fn.get_function(scope).unwrap();
      promise.catch(scope, empty_fn);
      let mut state = state_rc.borrow_mut();
      let promise_global = v8::Global::new(scope, promise);
      let module_global = v8::Global::new(scope, module);

      let dyn_import_mod_evaluate = DynImportModEvaluate {
        module_id: id,
        promise: promise_global,
        module: module_global,
      };

      state
        .pending_dyn_mod_evaluate
        .insert(load_id, dyn_import_mod_evaluate);
    } else {
      assert!(status == v8::ModuleStatus::Errored);
    }

    Ok(())
  }

  // TODO(bartlomieju): make it return `ModuleEvaluationFuture`?
  /// Evaluates an already instantiated ES module.
  ///
  /// Returns a receiver handle that resolves when module promise resolves.
  /// Implementors must manually call `run_event_loop()` to drive module
  /// evaluation future.
  ///
  /// `AnyError` can be downcast to a type that exposes additional information
  /// about the V8 exception. By default this type is `JsError`, however it may
  /// be a different type if `RuntimeOptions::js_error_create_fn` has been set.
  ///
  /// This function panics if module has not been instantiated.
  pub fn mod_evaluate(
    &mut self,
    id: ModuleId,
  ) -> mpsc::Receiver<Result<(), AnyError>> {
    let state_rc = Self::state(self.v8_isolate());
    let context = self.global_context();

    let scope = &mut v8::HandleScope::with_context(self.v8_isolate(), context);

    let module = state_rc
      .borrow()
      .module_map
      .get_handle(id)
      .map(|handle| v8::Local::new(scope, handle))
      .expect("ModuleInfo not found");
    let mut status = module.get_status();
    assert_eq!(status, v8::ModuleStatus::Instantiated);

    let (sender, receiver) = mpsc::channel(1);

    // IMPORTANT: Top-level-await is enabled, which means that return value
    // of module evaluation is a promise.
    //
    // Because that promise is created internally by V8, when error occurs during
    // module evaluation the promise is rejected, and since the promise has no rejection
    // handler it will result in call to `bindings::promise_reject_callback` adding
    // the promise to pending promise rejection table - meaning JsRuntime will return
    // error on next poll().
    //
    // This situation is not desirable as we want to manually return error at the
    // end of this function to handle it further. It means we need to manually
    // remove this promise from pending promise rejection table.
    //
    // For more details see:
    // https://github.com/denoland/deno/issues/4908
    // https://v8.dev/features/top-level-await#module-execution-order
    let maybe_value = module.evaluate(scope);

    // Update status after evaluating.
    status = module.get_status();

    if let Some(value) = maybe_value {
      assert!(
        status == v8::ModuleStatus::Evaluated
          || status == v8::ModuleStatus::Errored
      );
      let promise = v8::Local::<v8::Promise>::try_from(value)
        .expect("Expected to get promise as module evaluation result");
      let promise_global = v8::Global::new(scope, promise);
      let mut state = state_rc.borrow_mut();
      state.pending_promise_exceptions.remove(&promise_global);
      let promise_global = v8::Global::new(scope, promise);
      assert!(
        state.pending_mod_evaluate.is_none(),
        "There is already pending top level module evaluation"
      );

      state.pending_mod_evaluate = Some(ModEvaluate {
        promise: promise_global,
        sender,
      });
      scope.perform_microtask_checkpoint();
    } else {
      assert!(status == v8::ModuleStatus::Errored);
    }

    receiver
  }

  fn dyn_import_error(&mut self, id: ModuleLoadId, err: AnyError) {
    let state_rc = Self::state(self.v8_isolate());
    let context = self.global_context();

    let scope = &mut v8::HandleScope::with_context(self.v8_isolate(), context);

    let resolver_handle = state_rc
      .borrow_mut()
      .dyn_import_map
      .remove(&id)
      .expect("Invalid dyn import id");
    let resolver = resolver_handle.get(scope);

    let exception = err
      .downcast_ref::<ErrWithV8Handle>()
      .map(|err| err.get_handle(scope))
      .unwrap_or_else(|| {
        let message = err.to_string();
        let message = v8::String::new(scope, &message).unwrap();
        v8::Exception::type_error(scope, message)
      });

    resolver.reject(scope, exception).unwrap();
    scope.perform_microtask_checkpoint();
  }

  fn dyn_import_done(&mut self, id: ModuleLoadId, mod_id: ModuleId) {
    let state_rc = Self::state(self.v8_isolate());
    let context = self.global_context();

    debug!("dyn_import_done {} {:?}", id, mod_id);
    let scope = &mut v8::HandleScope::with_context(self.v8_isolate(), context);

    let resolver_handle = state_rc
      .borrow_mut()
      .dyn_import_map
      .remove(&id)
      .expect("Invalid dyn import id");
    let resolver = resolver_handle.get(scope);

    let module = {
      let state = state_rc.borrow();
      state
        .module_map
        .get_handle(mod_id)
        .map(|handle| v8::Local::new(scope, handle))
        .expect("Dyn import module info not found")
    };
    // Resolution success
    assert_eq!(module.get_status(), v8::ModuleStatus::Evaluated);

    let module_namespace = module.get_module_namespace();
    resolver.resolve(scope, module_namespace).unwrap();
    scope.perform_microtask_checkpoint();
  }

  fn prepare_dyn_imports(
    &mut self,
    cx: &mut Context,
  ) -> Poll<Result<(), AnyError>> {
    let state_rc = Self::state(self.v8_isolate());

    if state_rc.borrow().preparing_dyn_imports.is_empty() {
      return Poll::Ready(Ok(()));
    }

    loop {
      let r = {
        let mut state = state_rc.borrow_mut();
        state.preparing_dyn_imports.poll_next_unpin(cx)
      };
      match r {
        Poll::Pending | Poll::Ready(None) => {
          // There are no active dynamic import loaders, or none are ready.
          return Poll::Ready(Ok(()));
        }
        Poll::Ready(Some(prepare_poll)) => {
          let dyn_import_id = prepare_poll.0;
          let prepare_result = prepare_poll.1;

          match prepare_result {
            Ok(load) => {
              let state = state_rc.borrow_mut();
              state.pending_dyn_imports.push(load.into_future());
            }
            Err(err) => {
              self.dyn_import_error(dyn_import_id, err);
            }
          }
        }
      }
    }
  }

  fn poll_dyn_imports(
    &mut self,
    cx: &mut Context,
  ) -> Poll<Result<(), AnyError>> {
    let state_rc = Self::state(self.v8_isolate());

    if state_rc.borrow().pending_dyn_imports.is_empty() {
      return Poll::Ready(Ok(()));
    }

    loop {
      let poll_result = {
        let mut state = state_rc.borrow_mut();
        state.pending_dyn_imports.poll_next_unpin(cx)
      };

      match poll_result {
        Poll::Pending | Poll::Ready(None) => {
          // There are no active dynamic import loaders, or none are ready.
          return Poll::Ready(Ok(()));
        }
        Poll::Ready(Some(load_stream_poll)) => {
          let maybe_result = load_stream_poll.0;
          let mut load = load_stream_poll.1;
          let dyn_import_id = load.id;

          if let Some(load_stream_result) = maybe_result {
            match load_stream_result {
              Ok(info) => {
                // A module (not necessarily the one dynamically imported) has been
                // fetched. Create and register it, and if successful, poll for the
                // next recursive-load event related to this dynamic import.
                match self.register_during_load(info, &mut load) {
                  Ok(()) => {
                    // Keep importing until it's fully drained
                    let state = state_rc.borrow_mut();
                    state.pending_dyn_imports.push(load.into_future());
                  }
                  Err(err) => self.dyn_import_error(dyn_import_id, err),
                }
              }
              Err(err) => {
                // A non-javascript error occurred; this could be due to a an invalid
                // module specifier, or a problem with the source map, or a failure
                // to fetch the module source code.
                self.dyn_import_error(dyn_import_id, err)
              }
            }
          } else {
            // The top-level module from a dynamic import has been instantiated.
            // Load is done.
            let module_id = load.root_module_id.unwrap();
            let result = self.mod_instantiate(module_id);
            if let Err(err) = result {
              self.dyn_import_error(dyn_import_id, err);
            }
            self.dyn_mod_evaluate(dyn_import_id, module_id)?;
          }
        }
      }
    }
  }

  /// "deno_core" runs V8 with "--harmony-top-level-await"
  /// flag on - it means that each module evaluation returns a promise
  /// from V8.
  ///
  /// This promise resolves after all dependent modules have also
  /// resolved. Each dependent module may perform calls to "import()" and APIs
  /// using async ops will add futures to the runtime's event loop.
  /// It means that the promise returned from module evaluation will
  /// resolve only after all futures in the event loop are done.
  ///
  /// Thus during turn of event loop we need to check if V8 has
  /// resolved or rejected the promise. If the promise is still pending
  /// then another turn of event loop must be performed.
  fn evaluate_pending_module(&mut self) {
    let state_rc = Self::state(self.v8_isolate());

    let context = self.global_context();
    {
      let scope =
        &mut v8::HandleScope::with_context(self.v8_isolate(), context);

      let mut state = state_rc.borrow_mut();

      if let Some(module_evaluation) = state.pending_mod_evaluate.as_ref() {
        let promise = module_evaluation.promise.get(scope);
        let mut sender = module_evaluation.sender.clone();
        let promise_state = promise.state();

        match promise_state {
          v8::PromiseState::Pending => {
            // pass, poll_event_loop will decide if
            // runtime would be woken soon
          }
          v8::PromiseState::Fulfilled => {
            state.pending_mod_evaluate.take();
            scope.perform_microtask_checkpoint();
            // Receiver end might have been already dropped, ignore the result
            let _ = sender.try_send(Ok(()));
          }
          v8::PromiseState::Rejected => {
            let exception = promise.result(scope);
            state.pending_mod_evaluate.take();
            drop(state);
            scope.perform_microtask_checkpoint();
            let err1 = exception_to_err_result::<()>(scope, exception, false)
              .map_err(|err| attach_handle_to_error(scope, err, exception))
              .unwrap_err();
            // Receiver end might have been already dropped, ignore the result
            let _ = sender.try_send(Err(err1));
          }
        }
      }
    };
  }

  fn evaluate_dyn_imports(&mut self) {
    let state_rc = Self::state(self.v8_isolate());

    loop {
      let context = self.global_context();
      let maybe_result = {
        let scope =
          &mut v8::HandleScope::with_context(self.v8_isolate(), context);

        let mut state = state_rc.borrow_mut();
        if let Some(&dyn_import_id) =
          state.pending_dyn_mod_evaluate.keys().next()
        {
          let handle = state
            .pending_dyn_mod_evaluate
            .remove(&dyn_import_id)
            .unwrap();
          drop(state);

          let module_id = handle.module_id;
          let promise = handle.promise.get(scope);
          let _module = handle.module.get(scope);

          let promise_state = promise.state();

          match promise_state {
            v8::PromiseState::Pending => {
              state_rc
                .borrow_mut()
                .pending_dyn_mod_evaluate
                .insert(dyn_import_id, handle);
              None
            }
            v8::PromiseState::Fulfilled => Some(Ok((dyn_import_id, module_id))),
            v8::PromiseState::Rejected => {
              let exception = promise.result(scope);
              let err1 = exception_to_err_result::<()>(scope, exception, false)
                .map_err(|err| attach_handle_to_error(scope, err, exception))
                .unwrap_err();
              Some(Err((dyn_import_id, err1)))
            }
          }
        } else {
          None
        }
      };

      if let Some(result) = maybe_result {
        match result {
          Ok((dyn_import_id, module_id)) => {
            self.dyn_import_done(dyn_import_id, module_id);
          }
          Err((dyn_import_id, err1)) => {
            self.dyn_import_error(dyn_import_id, err1);
          }
        }
      } else {
        break;
      }
    }
  }

  fn register_during_load(
    &mut self,
    info: ModuleSource,
    load: &mut RecursiveModuleLoad,
  ) -> Result<(), AnyError> {
    let ModuleSource {
      code,
      module_url_specified,
      module_url_found,
    } = info;

    let is_main =
      load.state == LoadState::LoadingRoot && !load.is_dynamic_import();
    let referrer_specifier = crate::resolve_url(&module_url_found).unwrap();

    let state_rc = Self::state(self.v8_isolate());
    // #A There are 3 cases to handle at this moment:
    // 1. Source code resolved result have the same module name as requested
    //    and is not yet registered
    //     -> register
    // 2. Source code resolved result have a different name as requested:
    //   2a. The module with resolved module name has been registered
    //     -> alias
    //   2b. The module with resolved module name has not yet been registered
    //     -> register & alias

    // If necessary, register an alias.
    if module_url_specified != module_url_found {
      let mut state = state_rc.borrow_mut();
      state
        .module_map
        .alias(&module_url_specified, &module_url_found);
    }

    let maybe_mod_id = {
      let state = state_rc.borrow();
      state.module_map.get_id(&module_url_found)
    };

    let module_id = match maybe_mod_id {
      Some(id) => {
        // Module has already been registered.
        debug!(
          "Already-registered module fetched again: {}",
          module_url_found
        );
        id
      }
      // Module not registered yet, do it now.
      None => self.mod_new(is_main, &module_url_found, &code)?,
    };

    // Now we must iterate over all imports of the module and load them.
    let imports = {
      let state_rc = Self::state(self.v8_isolate());
      let state = state_rc.borrow();
      state.module_map.get_children(module_id).unwrap().clone()
    };

    for module_specifier in imports {
      let is_registered = {
        let state_rc = Self::state(self.v8_isolate());
        let state = state_rc.borrow();
        state.module_map.is_registered(&module_specifier)
      };
      if !is_registered {
        load
          .add_import(module_specifier.to_owned(), referrer_specifier.clone());
      }
    }

    // If we just finished loading the root module, store the root module id.
    if load.state == LoadState::LoadingRoot {
      load.root_module_id = Some(module_id);
      load.state = LoadState::LoadingImports;
    }

    if load.pending.is_empty() {
      load.state = LoadState::Done;
    }

    Ok(())
  }

  /// Asynchronously load specified module and all of its dependencies
  ///
  /// User must call `JsRuntime::mod_evaluate` with returned `ModuleId`
  /// manually after load is finished.
  pub async fn load_module(
    &mut self,
    specifier: &ModuleSpecifier,
    code: Option<String>,
  ) -> Result<ModuleId, AnyError> {
    let loader = {
      let state_rc = Self::state(self.v8_isolate());
      let state = state_rc.borrow();
      state.loader.clone()
    };

    let load = RecursiveModuleLoad::main(
      self.op_state(),
      &specifier.to_string(),
      code,
      loader,
    );
    let (_load_id, prepare_result) = load.prepare().await;

    let mut load = prepare_result?;

    while let Some(info_result) = load.next().await {
      let info = info_result?;
      self.register_during_load(info, &mut load)?;
    }

    let root_id = load.root_module_id.expect("Root module id empty");
    self.mod_instantiate(root_id).map(|_| root_id)
  }

  fn poll_pending_ops(
    &mut self,
    cx: &mut Context,
  ) -> Vec<(PromiseId, OpResponse)> {
    let state_rc = Self::state(self.v8_isolate());
    let mut overflow_response: Vec<(PromiseId, OpResponse)> = Vec::new();

    let mut state = state_rc.borrow_mut();

    // Now handle actual ops.
    state.have_unpolled_ops = false;

    loop {
      let pending_r = state.pending_ops.poll_next_unpin(cx);
      match pending_r {
        Poll::Ready(None) => break,
        Poll::Pending => break,
        Poll::Ready(Some((p_id, resp))) => {
          overflow_response.push((p_id, resp));
        }
      };
    }

    loop {
      let unref_r = state.pending_unref_ops.poll_next_unpin(cx);
      match unref_r {
        Poll::Ready(None) => break,
        Poll::Pending => break,
        Poll::Ready(Some((p_id, resp))) => {
          overflow_response.push((p_id, resp));
        }
      };
    }

    overflow_response
  }

  fn check_promise_exceptions(&mut self) -> Result<(), AnyError> {
    let state_rc = Self::state(self.v8_isolate());
    let mut state = state_rc.borrow_mut();

    if state.pending_promise_exceptions.is_empty() {
      return Ok(());
    }

    let key = {
      state
        .pending_promise_exceptions
        .keys()
        .next()
        .unwrap()
        .clone()
    };
    let handle = state.pending_promise_exceptions.remove(&key).unwrap();
    drop(state);

    let context = self.global_context();
    let scope = &mut v8::HandleScope::with_context(self.v8_isolate(), context);

    let exception = v8::Local::new(scope, handle);
    exception_to_err_result(scope, exception, true)
  }

  // Send finished responses to JS
  fn async_op_response(
    &mut self,
    async_responses: Vec<(PromiseId, OpResponse)>,
  ) -> Result<(), AnyError> {
    let state_rc = Self::state(self.v8_isolate());

    let async_responses_size = async_responses.len();
    if async_responses_size == 0 {
      return Ok(());
    }

    // FIXME(bartlomieju): without check above this call would panic
    // because of lazy initialization in core.js. It seems this lazy initialization
    // hides unnecessary complexity.
    let js_recv_cb_handle = state_rc
      .borrow()
      .js_recv_cb
      .clone()
      .expect("Deno.core.recv has not been called.");

    let context = self.global_context();
    let scope = &mut v8::HandleScope::with_context(self.v8_isolate(), context);
    let context = scope.get_current_context();
    let global: v8::Local<v8::Value> = context.global(scope).into();
    let js_recv_cb = js_recv_cb_handle.get(scope);

    let tc_scope = &mut v8::TryCatch::new(scope);

    let mut args: Vec<v8::Local<v8::Value>> =
      Vec::with_capacity(2 * async_responses_size);
    for overflown_response in async_responses {
      let (p_id, resp) = overflown_response;
      args.push(v8::Integer::new(tc_scope, p_id as i32).into());
      args.push(match resp {
        OpResponse::Value(value) => serde_v8::to_v8(tc_scope, value).unwrap(),
        OpResponse::Buffer(buf) => {
          bindings::boxed_slice_to_uint8array(tc_scope, buf).into()
        }
      });
    }

    if async_responses_size > 0 {
      js_recv_cb.call(tc_scope, global, args.as_slice());
    }

    match tc_scope.exception() {
      None => Ok(()),
      Some(exception) => exception_to_err_result(tc_scope, exception, false),
    }
  }

  fn drain_macrotasks(&mut self) -> Result<(), AnyError> {
    let js_macrotask_cb_handle =
      match &Self::state(self.v8_isolate()).borrow().js_macrotask_cb {
        Some(handle) => handle.clone(),
        None => return Ok(()),
      };

    let context = self.global_context();
    let scope = &mut v8::HandleScope::with_context(self.v8_isolate(), context);
    let context = scope.get_current_context();
    let global: v8::Local<v8::Value> = context.global(scope).into();
    let js_macrotask_cb = js_macrotask_cb_handle.get(scope);

    // Repeatedly invoke macrotask callback until it returns true (done),
    // such that ready microtasks would be automatically run before
    // next macrotask is processed.
    let tc_scope = &mut v8::TryCatch::new(scope);

    loop {
      let is_done = js_macrotask_cb.call(tc_scope, global, &[]);

      if let Some(exception) = tc_scope.exception() {
        return exception_to_err_result(tc_scope, exception, false);
      }

      let is_done = is_done.unwrap();
      if is_done.is_true() {
        break;
      }
    }

    Ok(())
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use crate::modules::ModuleSourceFuture;
  use crate::BufVec;
  use futures::future::lazy;
  use futures::FutureExt;
  use std::io;
  use std::ops::FnOnce;
  use std::rc::Rc;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;

  pub fn run_in_task<F>(f: F)
  where
    F: FnOnce(&mut Context) + Send + 'static,
  {
    futures::executor::block_on(lazy(move |cx| f(cx)));
  }

  fn poll_until_ready(
    runtime: &mut JsRuntime,
    max_poll_count: usize,
  ) -> Result<(), AnyError> {
    let mut cx = Context::from_waker(futures::task::noop_waker_ref());
    for _ in 0..max_poll_count {
      match runtime.poll_event_loop(&mut cx) {
        Poll::Pending => continue,
        Poll::Ready(val) => return val,
      }
    }
    panic!(
      "JsRuntime still not ready after polling {} times.",
      max_poll_count
    )
  }

  enum Mode {
    Async,
    AsyncUnref,
    AsyncZeroCopy(u8),
    OverflowReqSync,
    OverflowResSync,
    OverflowReqAsync,
    OverflowResAsync,
  }

  struct TestState {
    mode: Mode,
    dispatch_count: Arc<AtomicUsize>,
  }

  fn dispatch(op_state: Rc<RefCell<OpState>>, bufs: BufVec) -> Op {
    let op_state_ = op_state.borrow();
    let test_state = op_state_.borrow::<TestState>();
    test_state.dispatch_count.fetch_add(1, Ordering::Relaxed);
    match test_state.mode {
      Mode::Async => {
        assert_eq!(bufs.len(), 1);
        assert_eq!(bufs[0].len(), 1);
        assert_eq!(bufs[0][0], 42);
        let buf = vec![43u8].into_boxed_slice();
        Op::Async(futures::future::ready(buf).boxed())
      }
      Mode::AsyncUnref => {
        assert_eq!(bufs.len(), 1);
        assert_eq!(bufs[0].len(), 1);
        assert_eq!(bufs[0][0], 42);
        let fut = async {
          // This future never finish.
          futures::future::pending::<()>().await;
          vec![43u8].into_boxed_slice()
        };
        Op::AsyncUnref(fut.boxed())
      }
      Mode::AsyncZeroCopy(count) => {
        assert_eq!(bufs.len(), count as usize);
        bufs.iter().enumerate().for_each(|(idx, buf)| {
          assert_eq!(buf.len(), 1);
          assert_eq!(idx, buf[0] as usize);
        });

        let buf = vec![43u8].into_boxed_slice();
        Op::Async(futures::future::ready(buf).boxed())
      }
      Mode::OverflowReqSync => {
        assert_eq!(bufs.len(), 1);
        assert_eq!(bufs[0].len(), 100 * 1024 * 1024);
        let buf = vec![43u8].into_boxed_slice();
        Op::Sync(buf)
      }
      Mode::OverflowResSync => {
        assert_eq!(bufs.len(), 1);
        assert_eq!(bufs[0].len(), 1);
        assert_eq!(bufs[0][0], 42);
        let mut vec = vec![0u8; 100 * 1024 * 1024];
        vec[0] = 99;
        let buf = vec.into_boxed_slice();
        Op::Sync(buf)
      }
      Mode::OverflowReqAsync => {
        assert_eq!(bufs.len(), 1);
        assert_eq!(bufs[0].len(), 100 * 1024 * 1024);
        let buf = vec![43u8].into_boxed_slice();
        Op::Async(futures::future::ready(buf).boxed())
      }
      Mode::OverflowResAsync => {
        assert_eq!(bufs.len(), 1);
        assert_eq!(bufs[0].len(), 1);
        assert_eq!(bufs[0][0], 42);
        let mut vec = vec![0u8; 100 * 1024 * 1024];
        vec[0] = 4;
        let buf = vec.into_boxed_slice();
        Op::Async(futures::future::ready(buf).boxed())
      }
    }
  }

  fn setup(mode: Mode) -> (JsRuntime, Arc<AtomicUsize>) {
    let dispatch_count = Arc::new(AtomicUsize::new(0));
    let mut runtime = JsRuntime::new(Default::default());
    let op_state = runtime.op_state();
    op_state.borrow_mut().put(TestState {
      mode,
      dispatch_count: dispatch_count.clone(),
    });

    runtime.register_op("test", dispatch);

    runtime
      .execute(
        "setup.js",
        r#"
        function assert(cond) {
          if (!cond) {
            throw Error("assert");
          }
        }
        "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    (runtime, dispatch_count)
  }

  #[test]
  fn test_dispatch() {
    let (mut runtime, dispatch_count) = setup(Mode::Async);
    runtime
      .execute(
        "filename.js",
        r#"
        let control = new Uint8Array([42]);
        Deno.core.send(1, control);
        async function main() {
          Deno.core.send(1, control);
        }
        main();
        "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
  }

  #[test]
  fn test_dispatch_no_zero_copy_buf() {
    let (mut runtime, dispatch_count) = setup(Mode::AsyncZeroCopy(0));
    runtime
      .execute(
        "filename.js",
        r#"
        Deno.core.send(1);
        "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn test_dispatch_stack_zero_copy_bufs() {
    let (mut runtime, dispatch_count) = setup(Mode::AsyncZeroCopy(2));
    runtime
      .execute(
        "filename.js",
        r#"
        let zero_copy_a = new Uint8Array([0]);
        let zero_copy_b = new Uint8Array([1]);
        Deno.core.send(1, zero_copy_a, zero_copy_b);
        "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn test_dispatch_heap_zero_copy_bufs() {
    let (mut runtime, dispatch_count) = setup(Mode::AsyncZeroCopy(5));
    runtime.execute(
      "filename.js",
      r#"
        let zero_copy_a = new Uint8Array([0]);
        let zero_copy_b = new Uint8Array([1]);
        let zero_copy_c = new Uint8Array([2]);
        let zero_copy_d = new Uint8Array([3]);
        let zero_copy_e = new Uint8Array([4]);
        Deno.core.send(1, zero_copy_a, zero_copy_b, zero_copy_c, zero_copy_d, zero_copy_e);
        "#,
    ).unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn test_poll_async_delayed_ops() {
    run_in_task(|cx| {
      let (mut runtime, dispatch_count) = setup(Mode::Async);

      runtime
        .execute(
          "setup2.js",
          r#"
         let nrecv = 0;
         Deno.core.setAsyncHandler(1, (buf) => {
           nrecv++;
         });
         "#,
        )
        .unwrap();
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
      runtime
        .execute(
          "check1.js",
          r#"
         assert(nrecv == 0);
         let control = new Uint8Array([42]);
         Deno.core.send(1, control);
         assert(nrecv == 0);
         "#,
        )
        .unwrap();
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      assert!(matches!(runtime.poll_event_loop(cx), Poll::Ready(Ok(_))));
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      runtime
        .execute(
          "check2.js",
          r#"
         assert(nrecv == 1);
         Deno.core.send(1, control);
         assert(nrecv == 1);
         "#,
        )
        .unwrap();
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      assert!(matches!(runtime.poll_event_loop(cx), Poll::Ready(Ok(_))));
      runtime.execute("check3.js", "assert(nrecv == 2)").unwrap();
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      // We are idle, so the next poll should be the last.
      assert!(matches!(runtime.poll_event_loop(cx), Poll::Ready(Ok(_))));
    });
  }

  #[test]
  fn test_poll_async_optional_ops() {
    run_in_task(|cx| {
      let (mut runtime, dispatch_count) = setup(Mode::AsyncUnref);
      runtime
        .execute(
          "check1.js",
          r#"
          Deno.core.setAsyncHandler(1, (buf) => {
            // This handler will never be called
            assert(false);
          });
          let control = new Uint8Array([42]);
          Deno.core.send(1, control);
        "#,
        )
        .unwrap();
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      // The above op never finish, but runtime can finish
      // because the op is an unreffed async op.
      assert!(matches!(runtime.poll_event_loop(cx), Poll::Ready(Ok(_))));
    })
  }

  #[test]
  fn terminate_execution() {
    let (mut isolate, _dispatch_count) = setup(Mode::Async);
    // TODO(piscisaureus): in rusty_v8, the `thread_safe_handle()` method
    // should not require a mutable reference to `struct rusty_v8::Isolate`.
    let v8_isolate_handle = isolate.v8_isolate().thread_safe_handle();

    let terminator_thread = std::thread::spawn(move || {
      // allow deno to boot and run
      std::thread::sleep(std::time::Duration::from_millis(100));

      // terminate execution
      let ok = v8_isolate_handle.terminate_execution();
      assert!(ok);
    });

    // Rn an infinite loop, which should be terminated.
    match isolate.execute("infinite_loop.js", "for(;;) {}") {
      Ok(_) => panic!("execution should be terminated"),
      Err(e) => {
        assert_eq!(e.to_string(), "Uncaught Error: execution terminated")
      }
    };

    // Cancel the execution-terminating exception in order to allow script
    // execution again.
    let ok = isolate.v8_isolate().cancel_terminate_execution();
    assert!(ok);

    // Verify that the isolate usable again.
    isolate
      .execute("simple.js", "1 + 1")
      .expect("execution should be possible again");

    terminator_thread.join().unwrap();
  }

  #[test]
  fn dangling_shared_isolate() {
    let v8_isolate_handle = {
      // isolate is dropped at the end of this block
      let (mut runtime, _dispatch_count) = setup(Mode::Async);
      // TODO(piscisaureus): in rusty_v8, the `thread_safe_handle()` method
      // should not require a mutable reference to `struct rusty_v8::Isolate`.
      runtime.v8_isolate().thread_safe_handle()
    };

    // this should not SEGFAULT
    v8_isolate_handle.terminate_execution();
  }

  #[test]
  fn overflow_req_sync() {
    let (mut runtime, dispatch_count) = setup(Mode::OverflowReqSync);
    runtime
      .execute(
        "overflow_req_sync.js",
        r#"
        let asyncRecv = 0;
        Deno.core.setAsyncHandler(1, (buf) => { asyncRecv++ });
        // Large message that will overflow the shared space.
        let control = new Uint8Array(100 * 1024 * 1024);
        let response = Deno.core.dispatch(1, control);
        assert(response instanceof Uint8Array);
        assert(response.length == 1);
        assert(response[0] == 43);
        assert(asyncRecv == 0);
        "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn overflow_res_sync() {
    // TODO(ry) This test is quite slow due to memcpy-ing 100MB into JS. We
    // should optimize this.
    let (mut runtime, dispatch_count) = setup(Mode::OverflowResSync);
    runtime
      .execute(
        "overflow_res_sync.js",
        r#"
        let asyncRecv = 0;
        Deno.core.setAsyncHandler(1, (buf) => { asyncRecv++ });
        // Large message that will overflow the shared space.
        let control = new Uint8Array([42]);
        let response = Deno.core.dispatch(1, control);
        assert(response instanceof Uint8Array);
        assert(response.length == 100 * 1024 * 1024);
        assert(response[0] == 99);
        assert(asyncRecv == 0);
        "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn overflow_req_async() {
    run_in_task(|cx| {
      let (mut runtime, dispatch_count) = setup(Mode::OverflowReqAsync);
      runtime
        .execute(
          "overflow_req_async.js",
          r#"
         let asyncRecv = 0;
         Deno.core.setAsyncHandler(1, (buf) => {
           assert(buf.byteLength === 1);
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
        )
        .unwrap();
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      assert!(matches!(runtime.poll_event_loop(cx), Poll::Ready(Ok(_))));
      runtime
        .execute("check.js", "assert(asyncRecv == 1);")
        .unwrap();
    });
  }

  #[test]
  fn overflow_res_async_combined_with_unref() {
    run_in_task(|cx| {
      let mut runtime = JsRuntime::new(Default::default());

      runtime.register_op(
        "test1",
        |_op_state: Rc<RefCell<OpState>>, _bufs: BufVec| -> Op {
          let mut vec = vec![0u8; 100 * 1024 * 1024];
          vec[0] = 4;
          let buf = vec.into_boxed_slice();
          Op::Async(futures::future::ready(buf).boxed())
        },
      );

      runtime.register_op(
        "test2",
        |_op_state: Rc<RefCell<OpState>>, _bufs: BufVec| -> Op {
          let mut vec = vec![0u8; 100 * 1024 * 1024];
          vec[0] = 4;
          let buf = vec.into_boxed_slice();
          Op::AsyncUnref(futures::future::ready(buf).boxed())
        },
      );

      runtime
        .execute(
          "overflow_res_async_combined_with_unref.js",
          r#"
        function assert(cond) {
          if (!cond) {
            throw Error("assert");
          }
        }

        let asyncRecv = 0;
        Deno.core.setAsyncHandler(1, (buf) => {
          assert(buf.byteLength === 100 * 1024 * 1024);
          assert(buf[0] === 4);
          asyncRecv++;
        });
        Deno.core.setAsyncHandler(2, (buf) => {
          assert(buf.byteLength === 100 * 1024 * 1024);
          assert(buf[0] === 4);
          asyncRecv++;
        });
        let control = new Uint8Array(1);
        let response1 = Deno.core.dispatch(1, control);
        // Async messages always have null response.
        assert(response1 == null);
        assert(asyncRecv == 0);
        let response2 = Deno.core.dispatch(2, control);
        // Async messages always have null response.
        assert(response2 == null);
        assert(asyncRecv == 0);
        "#,
        )
        .unwrap();
      assert!(matches!(runtime.poll_event_loop(cx), Poll::Ready(Ok(_))));
      runtime
        .execute("check.js", "assert(asyncRecv == 2);")
        .unwrap();
    });
  }

  #[test]
  fn overflow_res_async() {
    run_in_task(|_cx| {
      // TODO(ry) This test is quite slow due to memcpy-ing 100MB into JS. We
      // should optimize this.
      let (mut runtime, dispatch_count) = setup(Mode::OverflowResAsync);
      runtime
        .execute(
          "overflow_res_async.js",
          r#"
         let asyncRecv = 0;
         Deno.core.setAsyncHandler(1, (buf) => {
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
        )
        .unwrap();
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
      poll_until_ready(&mut runtime, 3).unwrap();
      runtime
        .execute("check.js", "assert(asyncRecv == 1);")
        .unwrap();
    });
  }

  #[test]
  fn overflow_res_multiple_dispatch_async() {
    // TODO(ry) This test is quite slow due to memcpy-ing 100MB into JS. We
    // should optimize this.
    run_in_task(|_cx| {
      let (mut runtime, dispatch_count) = setup(Mode::OverflowResAsync);
      runtime
        .execute(
          "overflow_res_multiple_dispatch_async.js",
          r#"
         let asyncRecv = 0;
         Deno.core.setAsyncHandler(1, (buf) => {
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
        )
        .unwrap();
      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      poll_until_ready(&mut runtime, 3).unwrap();
      runtime
        .execute("check.js", "assert(asyncRecv == 2);")
        .unwrap();
    });
  }

  #[test]
  fn shared_queue_not_empty_when_js_error() {
    run_in_task(|_cx| {
      let dispatch_count = Arc::new(AtomicUsize::new(0));
      let mut runtime = JsRuntime::new(Default::default());
      let op_state = runtime.op_state();
      op_state.borrow_mut().put(TestState {
        mode: Mode::Async,
        dispatch_count: dispatch_count.clone(),
      });

      runtime.register_op("test", dispatch);
      runtime
        .execute(
          "shared_queue_not_empty_when_js_error.js",
          r#"
          const assert = (cond) => {if (!cond) throw Error("assert")};
          let asyncRecv = 0;
          Deno.core.setAsyncHandler(1, (buf) => {
            asyncRecv++;
            throw Error('x');
          });

          Deno.core.dispatch(1, new Uint8Array([42]));
          Deno.core.dispatch(1, new Uint8Array([42]));
          "#,
        )
        .unwrap();

      assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
      if poll_until_ready(&mut runtime, 3).is_ok() {
        panic!("Thrown error was not detected!")
      }
      runtime
        .execute("check.js", "assert(asyncRecv == 1);")
        .unwrap();

      let state_rc = JsRuntime::state(runtime.v8_isolate());
      let shared_queue_size = state_rc.borrow().shared.size();
      assert_eq!(shared_queue_size, 1);
    });
  }

  #[test]
  fn test_pre_dispatch() {
    run_in_task(|mut cx| {
      let (mut runtime, _dispatch_count) = setup(Mode::OverflowResAsync);
      runtime
        .execute(
          "bad_op_id.js",
          r#"
          let thrown;
          try {
            Deno.core.dispatch(100);
          } catch (e) {
            thrown = e;
          }
          assert(String(thrown) === "TypeError: Unknown op id: 100");
         "#,
        )
        .unwrap();
      if let Poll::Ready(Err(_)) = runtime.poll_event_loop(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn syntax_error() {
    let mut runtime = JsRuntime::new(Default::default());
    let src = "hocuspocus(";
    let r = runtime.execute("i.js", src);
    let e = r.unwrap_err();
    let js_error = e.downcast::<JsError>().unwrap();
    assert_eq!(js_error.end_column, Some(11));
  }

  #[test]
  fn test_encode_decode() {
    run_in_task(|mut cx| {
      let (mut runtime, _dispatch_count) = setup(Mode::Async);
      runtime
        .execute(
          "encode_decode_test.js",
          include_str!("encode_decode_test.js"),
        )
        .unwrap();
      if let Poll::Ready(Err(_)) = runtime.poll_event_loop(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn test_serialize_deserialize() {
    run_in_task(|mut cx| {
      let (mut runtime, _dispatch_count) = setup(Mode::Async);
      runtime
        .execute(
          "serialize_deserialize_test.js",
          include_str!("serialize_deserialize_test.js"),
        )
        .unwrap();
      if let Poll::Ready(Err(_)) = runtime.poll_event_loop(&mut cx) {
        unreachable!();
      }
    });
  }

  #[test]
  fn will_snapshot() {
    let snapshot = {
      let mut runtime = JsRuntime::new(RuntimeOptions {
        will_snapshot: true,
        ..Default::default()
      });
      runtime.execute("a.js", "a = 1 + 2").unwrap();
      runtime.snapshot()
    };

    let snapshot = Snapshot::JustCreated(snapshot);
    let mut runtime2 = JsRuntime::new(RuntimeOptions {
      startup_snapshot: Some(snapshot),
      ..Default::default()
    });
    runtime2
      .execute("check.js", "if (a != 3) throw Error('x')")
      .unwrap();
  }

  #[test]
  fn test_from_boxed_snapshot() {
    let snapshot = {
      let mut runtime = JsRuntime::new(RuntimeOptions {
        will_snapshot: true,
        ..Default::default()
      });
      runtime.execute("a.js", "a = 1 + 2").unwrap();
      let snap: &[u8] = &*runtime.snapshot();
      Vec::from(snap).into_boxed_slice()
    };

    let snapshot = Snapshot::Boxed(snapshot);
    let mut runtime2 = JsRuntime::new(RuntimeOptions {
      startup_snapshot: Some(snapshot),
      ..Default::default()
    });
    runtime2
      .execute("check.js", "if (a != 3) throw Error('x')")
      .unwrap();
  }

  #[test]
  fn test_heap_limits() {
    let create_params = v8::Isolate::create_params().heap_limits(0, 20 * 1024);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      create_params: Some(create_params),
      ..Default::default()
    });
    let cb_handle = runtime.v8_isolate().thread_safe_handle();

    let callback_invoke_count = Rc::new(AtomicUsize::default());
    let inner_invoke_count = Rc::clone(&callback_invoke_count);

    runtime.add_near_heap_limit_callback(
      move |current_limit, _initial_limit| {
        inner_invoke_count.fetch_add(1, Ordering::SeqCst);
        cb_handle.terminate_execution();
        current_limit * 2
      },
    );
    let err = runtime
      .execute(
        "script name",
        r#"let s = ""; while(true) { s += "Hello"; }"#,
      )
      .expect_err("script should fail");
    assert_eq!(
      "Uncaught Error: execution terminated",
      err.downcast::<JsError>().unwrap().message
    );
    assert!(callback_invoke_count.load(Ordering::SeqCst) > 0)
  }

  #[test]
  fn test_heap_limit_cb_remove() {
    let mut runtime = JsRuntime::new(Default::default());

    runtime.add_near_heap_limit_callback(|current_limit, _initial_limit| {
      current_limit * 2
    });
    runtime.remove_near_heap_limit_callback(20 * 1024);
    assert!(runtime.allocations.near_heap_limit_callback_data.is_none());
  }

  #[test]
  fn test_heap_limit_cb_multiple() {
    let create_params = v8::Isolate::create_params().heap_limits(0, 20 * 1024);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      create_params: Some(create_params),
      ..Default::default()
    });
    let cb_handle = runtime.v8_isolate().thread_safe_handle();

    let callback_invoke_count_first = Rc::new(AtomicUsize::default());
    let inner_invoke_count_first = Rc::clone(&callback_invoke_count_first);
    runtime.add_near_heap_limit_callback(
      move |current_limit, _initial_limit| {
        inner_invoke_count_first.fetch_add(1, Ordering::SeqCst);
        current_limit * 2
      },
    );

    let callback_invoke_count_second = Rc::new(AtomicUsize::default());
    let inner_invoke_count_second = Rc::clone(&callback_invoke_count_second);
    runtime.add_near_heap_limit_callback(
      move |current_limit, _initial_limit| {
        inner_invoke_count_second.fetch_add(1, Ordering::SeqCst);
        cb_handle.terminate_execution();
        current_limit * 2
      },
    );

    let err = runtime
      .execute(
        "script name",
        r#"let s = ""; while(true) { s += "Hello"; }"#,
      )
      .expect_err("script should fail");
    assert_eq!(
      "Uncaught Error: execution terminated",
      err.downcast::<JsError>().unwrap().message
    );
    assert_eq!(0, callback_invoke_count_first.load(Ordering::SeqCst));
    assert!(callback_invoke_count_second.load(Ordering::SeqCst) > 0);
  }

  #[test]
  fn test_mods() {
    #[derive(Default)]
    struct ModsLoader {
      pub count: Arc<AtomicUsize>,
    }

    impl ModuleLoader for ModsLoader {
      fn resolve(
        &self,
        _op_state: Rc<RefCell<OpState>>,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
      ) -> Result<ModuleSpecifier, AnyError> {
        self.count.fetch_add(1, Ordering::Relaxed);
        assert_eq!(specifier, "./b.js");
        assert_eq!(referrer, "file:///a.js");
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _op_state: Rc<RefCell<OpState>>,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        unreachable!()
      }
    }

    let loader = Rc::new(ModsLoader::default());

    let resolve_count = loader.count.clone();
    let dispatch_count = Arc::new(AtomicUsize::new(0));
    let dispatch_count_ = dispatch_count.clone();

    let dispatcher = move |_state: Rc<RefCell<OpState>>, bufs: BufVec| -> Op {
      dispatch_count_.fetch_add(1, Ordering::Relaxed);
      assert_eq!(bufs.len(), 1);
      assert_eq!(bufs[0].len(), 1);
      assert_eq!(bufs[0][0], 42);
      let buf = [43u8, 0, 0, 0][..].into();
      Op::Async(futures::future::ready(buf).boxed())
    };

    let mut runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });
    runtime.register_op("test", dispatcher);

    runtime
      .execute(
        "setup.js",
        r#"
        function assert(cond) {
          if (!cond) {
            throw Error("assert");
          }
        }
        "#,
      )
      .unwrap();

    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);

    let specifier_a = "file:///a.js".to_string();
    let mod_a = runtime
      .mod_new(
        true,
        &specifier_a,
        r#"
        import { b } from './b.js'
        if (b() != 'b') throw Error();
        let control = new Uint8Array([42]);
        Deno.core.send(1, control);
      "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);

    let state_rc = JsRuntime::state(runtime.v8_isolate());
    {
      let state = state_rc.borrow();
      let imports = state.module_map.get_children(mod_a);
      assert_eq!(
        imports,
        Some(&vec![crate::resolve_url("file:///b.js").unwrap()])
      );
    }
    let mod_b = runtime
      .mod_new(false, "file:///b.js", "export function b() { return 'b' }")
      .unwrap();
    {
      let state = state_rc.borrow();
      let imports = state.module_map.get_children(mod_b).unwrap();
      assert_eq!(imports.len(), 0);
    }

    runtime.mod_instantiate(mod_b).unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);

    runtime.mod_instantiate(mod_a).unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);

    runtime.mod_evaluate(mod_a);
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn dyn_import_err() {
    #[derive(Clone, Default)]
    struct DynImportErrLoader {
      pub count: Arc<AtomicUsize>,
    }

    impl ModuleLoader for DynImportErrLoader {
      fn resolve(
        &self,
        _op_state: Rc<RefCell<OpState>>,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
      ) -> Result<ModuleSpecifier, AnyError> {
        self.count.fetch_add(1, Ordering::Relaxed);
        assert_eq!(specifier, "/foo.js");
        assert_eq!(referrer, "file:///dyn_import2.js");
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _op_state: Rc<RefCell<OpState>>,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        async { Err(io::Error::from(io::ErrorKind::NotFound).into()) }.boxed()
      }
    }

    // Test an erroneous dynamic import where the specified module isn't found.
    run_in_task(|cx| {
      let loader = Rc::new(DynImportErrLoader::default());
      let count = loader.count.clone();
      let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(loader),
        ..Default::default()
      });

      runtime
        .execute(
          "file:///dyn_import2.js",
          r#"
        (async () => {
          await import("/foo.js");
        })();
        "#,
        )
        .unwrap();

      assert_eq!(count.load(Ordering::Relaxed), 0);
      // We should get an error here.
      let result = runtime.poll_event_loop(cx);
      if let Poll::Ready(Ok(_)) = result {
        unreachable!();
      }
      assert_eq!(count.load(Ordering::Relaxed), 2);
    })
  }

  #[derive(Clone, Default)]
  struct DynImportOkLoader {
    pub prepare_load_count: Arc<AtomicUsize>,
    pub resolve_count: Arc<AtomicUsize>,
    pub load_count: Arc<AtomicUsize>,
  }

  impl ModuleLoader for DynImportOkLoader {
    fn resolve(
      &self,
      _op_state: Rc<RefCell<OpState>>,
      specifier: &str,
      referrer: &str,
      _is_main: bool,
    ) -> Result<ModuleSpecifier, AnyError> {
      let c = self.resolve_count.fetch_add(1, Ordering::Relaxed);
      assert!(c < 4);
      assert_eq!(specifier, "./b.js");
      assert_eq!(referrer, "file:///dyn_import3.js");
      let s = crate::resolve_import(specifier, referrer).unwrap();
      Ok(s)
    }

    fn load(
      &self,
      _op_state: Rc<RefCell<OpState>>,
      specifier: &ModuleSpecifier,
      _maybe_referrer: Option<ModuleSpecifier>,
      _is_dyn_import: bool,
    ) -> Pin<Box<ModuleSourceFuture>> {
      self.load_count.fetch_add(1, Ordering::Relaxed);
      let info = ModuleSource {
        module_url_specified: specifier.to_string(),
        module_url_found: specifier.to_string(),
        code: "export function b() { return 'b' }".to_owned(),
      };
      async move { Ok(info) }.boxed()
    }

    fn prepare_load(
      &self,
      _op_state: Rc<RefCell<OpState>>,
      _load_id: ModuleLoadId,
      _module_specifier: &ModuleSpecifier,
      _maybe_referrer: Option<String>,
      _is_dyn_import: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), AnyError>>>> {
      self.prepare_load_count.fetch_add(1, Ordering::Relaxed);
      async { Ok(()) }.boxed_local()
    }
  }

  #[test]
  fn dyn_import_ok() {
    run_in_task(|cx| {
      let loader = Rc::new(DynImportOkLoader::default());
      let prepare_load_count = loader.prepare_load_count.clone();
      let resolve_count = loader.resolve_count.clone();
      let load_count = loader.load_count.clone();
      let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(loader),
        ..Default::default()
      });

      // Dynamically import mod_b
      runtime
        .execute(
          "file:///dyn_import3.js",
          r#"
          (async () => {
            let mod = await import("./b.js");
            if (mod.b() !== 'b') {
              throw Error("bad1");
            }
            // And again!
            mod = await import("./b.js");
            if (mod.b() !== 'b') {
              throw Error("bad2");
            }
          })();
          "#,
        )
        .unwrap();

      // First poll runs `prepare_load` hook.
      assert!(matches!(runtime.poll_event_loop(cx), Poll::Pending));
      assert_eq!(prepare_load_count.load(Ordering::Relaxed), 1);

      // Second poll actually loads modules into the isolate.
      assert!(matches!(runtime.poll_event_loop(cx), Poll::Ready(Ok(_))));
      assert_eq!(resolve_count.load(Ordering::Relaxed), 4);
      assert_eq!(load_count.load(Ordering::Relaxed), 2);
      assert!(matches!(runtime.poll_event_loop(cx), Poll::Ready(Ok(_))));
      assert_eq!(resolve_count.load(Ordering::Relaxed), 4);
      assert_eq!(load_count.load(Ordering::Relaxed), 2);
    })
  }

  #[test]
  fn dyn_import_borrow_mut_error() {
    // https://github.com/denoland/deno/issues/6054
    run_in_task(|cx| {
      let loader = Rc::new(DynImportOkLoader::default());
      let prepare_load_count = loader.prepare_load_count.clone();
      let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(loader),
        ..Default::default()
      });
      runtime
        .execute(
          "file:///dyn_import3.js",
          r#"
          (async () => {
            let mod = await import("./b.js");
            if (mod.b() !== 'b') {
              throw Error("bad");
            }
            // Now do any op
            Deno.core.ops();
          })();
          "#,
        )
        .unwrap();
      // First poll runs `prepare_load` hook.
      let _ = runtime.poll_event_loop(cx);
      assert_eq!(prepare_load_count.load(Ordering::Relaxed), 1);
      // Second poll triggers error
      let _ = runtime.poll_event_loop(cx);
    })
  }

  #[test]
  fn es_snapshot() {
    #[derive(Default)]
    struct ModsLoader;

    impl ModuleLoader for ModsLoader {
      fn resolve(
        &self,
        _op_state: Rc<RefCell<OpState>>,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
      ) -> Result<ModuleSpecifier, AnyError> {
        assert_eq!(specifier, "file:///main.js");
        assert_eq!(referrer, ".");
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _op_state: Rc<RefCell<OpState>>,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        unreachable!()
      }
    }

    let loader = std::rc::Rc::new(ModsLoader::default());
    let mut runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      will_snapshot: true,
      ..Default::default()
    });

    let specifier = crate::resolve_url("file:///main.js").unwrap();
    let source_code = "Deno.core.print('hello\\n')".to_string();

    let module_id = futures::executor::block_on(
      runtime.load_module(&specifier, Some(source_code)),
    )
    .unwrap();

    runtime.mod_evaluate(module_id);
    futures::executor::block_on(runtime.run_event_loop()).unwrap();

    let _snapshot = runtime.snapshot();
  }

  #[test]
  fn test_error_without_stack() {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    // SyntaxError
    let result = runtime.execute(
      "error_without_stack.js",
      r#"
function main() {
  console.log("asdf);
}

main();
"#,
    );
    let expected_error = r#"Uncaught SyntaxError: Invalid or unexpected token
    at error_without_stack.js:3:14"#;
    assert_eq!(result.unwrap_err().to_string(), expected_error);
  }

  #[test]
  fn test_error_stack() {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    let result = runtime.execute(
      "error_stack.js",
      r#"
function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

function main() {
  assert(false);
}

main();
        "#,
    );
    let expected_error = r#"Error: assert
    at assert (error_stack.js:4:11)
    at main (error_stack.js:9:3)
    at error_stack.js:12:1"#;
    assert_eq!(result.unwrap_err().to_string(), expected_error);
  }

  #[test]
  fn test_error_async_stack() {
    run_in_task(|cx| {
      let mut runtime = JsRuntime::new(RuntimeOptions::default());
      runtime
        .execute(
          "error_async_stack.js",
          r#"
(async () => {
  const p = (async () => {
    await Promise.resolve().then(() => {
      throw new Error("async");
    });
  })();

  try {
    await p;
  } catch (error) {
    console.log(error.stack);
    throw error;
  }
})();"#,
        )
        .unwrap();
      let expected_error = r#"Error: async
    at error_async_stack.js:5:13
    at async error_async_stack.js:4:5
    at async error_async_stack.js:10:5"#;

      match runtime.poll_event_loop(cx) {
        Poll::Ready(Err(e)) => {
          assert_eq!(e.to_string(), expected_error);
        }
        _ => panic!(),
      };
    })
  }

  #[test]
  fn test_core_js_stack_frame() {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    // Call non-existent op so we get error from `core.js`
    let error = runtime
      .execute(
        "core_js_stack_frame.js",
        "Deno.core.dispatchByName('non_existent');",
      )
      .unwrap_err();
    let error_string = error.to_string();
    // Test that the script specifier is a URL: `deno:<repo-relative path>`.
    assert!(error_string.contains("deno:core/core.js"));
  }
}
