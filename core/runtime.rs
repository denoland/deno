// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::bindings;
use crate::error::generic_error;
use crate::error::to_v8_type_error;
use crate::error::JsError;
use crate::extensions::OpDecl;
use crate::extensions::OpEventLoopFn;
use crate::inspector::JsRuntimeInspector;
use crate::module_specifier::ModuleSpecifier;
use crate::modules::ModuleError;
use crate::modules::ModuleId;
use crate::modules::ModuleLoadId;
use crate::modules::ModuleLoader;
use crate::modules::ModuleMap;
use crate::modules::NoopModuleLoader;
use crate::op_void_async;
use crate::op_void_sync;
use crate::ops::*;
use crate::source_map::SourceMapCache;
use crate::source_map::SourceMapGetter;
use crate::Extension;
use crate::OpMiddlewareFn;
use crate::OpResult;
use crate::OpState;
use crate::PromiseId;
use anyhow::Error;
use futures::channel::oneshot;
use futures::future::poll_fn;
use futures::future::Future;
use futures::future::FutureExt;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use futures::task::AtomicWaker;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::option::Option;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Once;
use std::task::Context;
use std::task::Poll;
use v8::OwnedIsolate;

type PendingOpFuture =
  OpCall<(v8::Global<v8::Context>, PromiseId, OpId, OpResult)>;

pub enum Snapshot {
  Static(&'static [u8]),
  JustCreated(v8::StartupData),
  Boxed(Box<[u8]>),
}

pub type JsErrorCreateFn = dyn Fn(JsError) -> Error;

pub type GetErrorClassFn = &'static dyn for<'e> Fn(&'e Error) -> &'static str;

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
/// Pending ops are created in JavaScript by calling Deno.core.opAsync(), and in Rust
/// by implementing an async function that takes a serde::Deserialize "control argument"
/// and an optional zero copy buffer, each async Op is tied to a Promise in JavaScript.
pub struct JsRuntime {
  // This is an Option<OwnedIsolate> instead of just OwnedIsolate to workaround
  // a safety issue with SnapshotCreator. See JsRuntime::drop.
  v8_isolate: Option<v8::OwnedIsolate>,
  built_from_snapshot: bool,
  allocations: IsolateAllocations,
  extensions: Vec<Extension>,
  event_loop_middlewares: Vec<Box<OpEventLoopFn>>,
}

pub(crate) struct DynImportModEvaluate {
  load_id: ModuleLoadId,
  module_id: ModuleId,
  promise: v8::Global<v8::Promise>,
  module: v8::Global<v8::Module>,
}

pub(crate) struct ModEvaluate {
  pub(crate) promise: Option<v8::Global<v8::Promise>>,
  pub(crate) has_evaluated: bool,
  pub(crate) handled_promise_rejections: Vec<v8::Global<v8::Promise>>,
  sender: oneshot::Sender<Result<(), Error>>,
}

pub struct CrossIsolateStore<T>(Arc<Mutex<CrossIsolateStoreInner<T>>>);

struct CrossIsolateStoreInner<T> {
  map: HashMap<u32, T>,
  last_id: u32,
}

impl<T> CrossIsolateStore<T> {
  pub(crate) fn insert(&self, value: T) -> u32 {
    let mut store = self.0.lock().unwrap();
    let last_id = store.last_id;
    store.map.insert(last_id, value);
    store.last_id += 1;
    last_id
  }

  pub(crate) fn take(&self, id: u32) -> Option<T> {
    let mut store = self.0.lock().unwrap();
    store.map.remove(&id)
  }
}

impl<T> Default for CrossIsolateStore<T> {
  fn default() -> Self {
    CrossIsolateStore(Arc::new(Mutex::new(CrossIsolateStoreInner {
      map: Default::default(),
      last_id: 0,
    })))
  }
}

impl<T> Clone for CrossIsolateStore<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

pub type SharedArrayBufferStore =
  CrossIsolateStore<v8::SharedRef<v8::BackingStore>>;

pub type CompiledWasmModuleStore = CrossIsolateStore<v8::CompiledWasmModule>;

#[derive(Default)]
pub(crate) struct ContextState {
  js_recv_cb: Option<v8::Global<v8::Function>>,
  pub(crate) js_build_custom_error_cb: Option<v8::Global<v8::Function>>,
  pub(crate) js_promise_reject_cb: Option<v8::Global<v8::Function>>,
  pub(crate) js_format_exception_cb: Option<v8::Global<v8::Function>>,
  pub(crate) js_wasm_streaming_cb: Option<v8::Global<v8::Function>>,
  pub(crate) unrefed_ops: HashSet<i32>,
}

/// Internal state for JsRuntime which is stored in one of v8::Isolate's
/// embedder slots.
pub(crate) struct JsRuntimeState {
  global_realm: Option<JsRealm>,
  known_realms: Vec<v8::Weak<v8::Context>>,
  pub(crate) js_macrotask_cbs: Vec<v8::Global<v8::Function>>,
  pub(crate) js_nexttick_cbs: Vec<v8::Global<v8::Function>>,
  pub(crate) has_tick_scheduled: bool,
  pub(crate) pending_promise_exceptions:
    HashMap<v8::Global<v8::Promise>, v8::Global<v8::Value>>,
  pub(crate) pending_dyn_mod_evaluate: Vec<DynImportModEvaluate>,
  pub(crate) pending_mod_evaluate: Option<ModEvaluate>,
  /// A counter used to delay our dynamic import deadlock detection by one spin
  /// of the event loop.
  dyn_module_evaluate_idle_counter: u32,
  pub(crate) source_map_getter: Option<Box<dyn SourceMapGetter>>,
  pub(crate) source_map_cache: SourceMapCache,
  pub(crate) pending_ops: FuturesUnordered<PendingOpFuture>,
  pub(crate) have_unpolled_ops: bool,
  pub(crate) op_state: Rc<RefCell<OpState>>,
  #[allow(dead_code)]
  // We don't explicitly re-read this prop but need the slice to live alongside the isolate
  pub(crate) op_ctxs: Box<[OpCtx]>,
  pub(crate) shared_array_buffer_store: Option<SharedArrayBufferStore>,
  pub(crate) compiled_wasm_module_store: Option<CompiledWasmModuleStore>,
  /// The error that was passed to an `op_dispatch_exception` call.
  /// It will be retrieved by `exception_to_err_result` and used as an error
  /// instead of any other exceptions.
  // TODO(nayeemrmn): This is polled in `exception_to_err_result()` which is
  // flimsy. Try to poll it similarly to `pending_promise_exceptions`.
  pub(crate) dispatched_exceptions: VecDeque<v8::Global<v8::Value>>,
  inspector: Option<Rc<RefCell<JsRuntimeInspector>>>,
  waker: AtomicWaker,
}

fn v8_init(
  v8_platform: Option<v8::SharedRef<v8::Platform>>,
  predictable: bool,
) {
  // Include 10MB ICU data file.
  #[repr(C, align(16))]
  struct IcuData([u8; 10454784]);
  static ICU_DATA: IcuData = IcuData(*include_bytes!("icudtl.dat"));
  v8::icu::set_common_data_71(&ICU_DATA.0).unwrap();

  let flags = concat!(
    " --experimental-wasm-threads",
    " --wasm-test-streaming",
    " --harmony-import-assertions",
    " --no-validate-asm",
    " --turbo_fast_api_calls",
    // This flag prevents "unresolved external reference" panic during
    // build, which started happening in V8 10.6
    " --noexperimental-async-stack-tagging-api",
  );

  if predictable {
    v8::V8::set_flags_from_string(&format!(
      "{}{}",
      flags, " --predictable --random-seed=42"
    ));
  } else {
    v8::V8::set_flags_from_string(flags);
  }

  let v8_platform = v8_platform
    .unwrap_or_else(|| v8::new_default_platform(0, false).make_shared());
  v8::V8::initialize_platform(v8_platform);
  v8::V8::initialize();
}

pub const V8_WRAPPER_TYPE_INDEX: i32 = 0;
pub const V8_WRAPPER_OBJECT_INDEX: i32 = 1;

#[derive(Default)]
pub struct RuntimeOptions {
  /// Source map reference for errors.
  pub source_map_getter: Option<Box<dyn SourceMapGetter>>,

  /// Allows to map error type to a string "class" used to represent
  /// error in JavaScript.
  pub get_error_class_fn: Option<GetErrorClassFn>,

  /// Implementation of `ModuleLoader` which will be
  /// called when V8 requests to load ES modules.
  ///
  /// If not provided runtime will error if code being
  /// executed tries to load modules.
  pub module_loader: Option<Rc<dyn ModuleLoader>>,

  /// JsRuntime extensions, not to be confused with ES modules
  /// these are sets of ops and other JS code to be initialized.
  pub extensions: Vec<Extension>,

  /// V8 snapshot that should be loaded on startup.
  ///
  /// Currently can't be used with `will_snapshot`.
  pub startup_snapshot: Option<Snapshot>,

  /// Prepare runtime to take snapshot of loaded code.
  /// The snapshot is determinstic and uses predictable random numbers.
  ///
  /// Currently can't be used with `startup_snapshot`.
  pub will_snapshot: bool,

  /// Isolate creation parameters.
  pub create_params: Option<v8::CreateParams>,

  /// V8 platform instance to use. Used when Deno initializes V8
  /// (which it only does once), otherwise it's silenty dropped.
  pub v8_platform: Option<v8::SharedRef<v8::Platform>>,

  /// The store to use for transferring SharedArrayBuffers between isolates.
  /// If multiple isolates should have the possibility of sharing
  /// SharedArrayBuffers, they should use the same [SharedArrayBufferStore]. If
  /// no [SharedArrayBufferStore] is specified, SharedArrayBuffer can not be
  /// serialized.
  pub shared_array_buffer_store: Option<SharedArrayBufferStore>,

  /// The store to use for transferring `WebAssembly.Module` objects between
  /// isolates.
  /// If multiple isolates should have the possibility of sharing
  /// `WebAssembly.Module` objects, they should use the same
  /// [CompiledWasmModuleStore]. If no [CompiledWasmModuleStore] is specified,
  /// `WebAssembly.Module` objects cannot be serialized.
  pub compiled_wasm_module_store: Option<CompiledWasmModuleStore>,
}

impl Drop for JsRuntime {
  fn drop(&mut self) {
    if let Some(v8_isolate) = self.v8_isolate.as_mut() {
      Self::drop_state_and_module_map(v8_isolate);
    }
  }
}

impl JsRuntime {
  const STATE_DATA_OFFSET: u32 = 0;
  const MODULE_MAP_DATA_OFFSET: u32 = 1;

  /// Only constructor, configuration is done through `options`.
  pub fn new(mut options: RuntimeOptions) -> Self {
    let v8_platform = options.v8_platform.take();

    static DENO_INIT: Once = Once::new();
    DENO_INIT.call_once(move || v8_init(v8_platform, options.will_snapshot));

    let has_startup_snapshot = options.startup_snapshot.is_some();

    // Add builtins extension
    options
      .extensions
      .insert(0, crate::ops_builtin::init_builtins());

    let ops = Self::collect_ops(&mut options.extensions);
    let mut op_state = OpState::new(ops.len());

    if let Some(get_error_class_fn) = options.get_error_class_fn {
      op_state.get_error_class_fn = get_error_class_fn;
    }
    let op_state = Rc::new(RefCell::new(op_state));
    let op_ctxs = ops
      .into_iter()
      .enumerate()
      .map(|(id, decl)| OpCtx {
        id,
        state: op_state.clone(),
        decl,
      })
      .collect::<Vec<_>>()
      .into_boxed_slice();

    let refs = bindings::external_references(&op_ctxs, !options.will_snapshot);
    // V8 takes ownership of external_references.
    let refs: &'static v8::ExternalReferences = Box::leak(Box::new(refs));
    let global_context;

    let align = std::mem::align_of::<usize>();
    let layout = std::alloc::Layout::from_size_align(
      std::mem::size_of::<*mut v8::OwnedIsolate>(),
      align,
    )
    .unwrap();
    assert!(layout.size() > 0);
    let isolate_ptr: *mut v8::OwnedIsolate =
      // SAFETY: we just asserted that layout has non-0 size.
      unsafe { std::alloc::alloc(layout) as *mut _ };

    let mut isolate = if options.will_snapshot {
      // TODO(ry) Support loading snapshots before snapshotting.
      assert!(options.startup_snapshot.is_none());
      let snapshot_creator = v8::Isolate::snapshot_creator(Some(refs));
      let mut isolate = JsRuntime::setup_isolate(snapshot_creator);
      {
        // SAFETY: this is first use of `isolate_ptr` so we are sure we're
        // not overwriting an existing pointer.
        isolate = unsafe {
          isolate_ptr.write(isolate);
          isolate_ptr.read()
        };
        let scope = &mut v8::HandleScope::new(&mut isolate);
        let context = bindings::initialize_context(scope, &op_ctxs, false);
        global_context = v8::Global::new(scope, context);
        scope.set_default_context(context);
      }
      isolate
    } else {
      let mut params = options
        .create_params
        .take()
        .unwrap_or_else(|| {
          v8::CreateParams::default().embedder_wrapper_type_info_offsets(
            V8_WRAPPER_TYPE_INDEX,
            V8_WRAPPER_OBJECT_INDEX,
          )
        })
        .external_references(&**refs);
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
        // SAFETY: this is first use of `isolate_ptr` so we are sure we're
        // not overwriting an existing pointer.
        isolate = unsafe {
          isolate_ptr.write(isolate);
          isolate_ptr.read()
        };
        let scope = &mut v8::HandleScope::new(&mut isolate);
        let context =
          bindings::initialize_context(scope, &op_ctxs, snapshot_loaded);

        global_context = v8::Global::new(scope, context);
      }

      isolate
    };

    op_state.borrow_mut().put(isolate_ptr);
    let inspector =
      JsRuntimeInspector::new(&mut isolate, global_context.clone());

    let loader = options
      .module_loader
      .unwrap_or_else(|| Rc::new(NoopModuleLoader));

    let known_realms = vec![v8::Weak::new(&mut isolate, &global_context)];
    let state_rc = Rc::new(RefCell::new(JsRuntimeState {
      global_realm: Some(JsRealm(global_context.clone())),
      known_realms,
      pending_promise_exceptions: HashMap::new(),
      pending_dyn_mod_evaluate: vec![],
      pending_mod_evaluate: None,
      dyn_module_evaluate_idle_counter: 0,
      js_macrotask_cbs: vec![],
      js_nexttick_cbs: vec![],
      has_tick_scheduled: false,
      source_map_getter: options.source_map_getter,
      source_map_cache: Default::default(),
      pending_ops: FuturesUnordered::new(),
      shared_array_buffer_store: options.shared_array_buffer_store,
      compiled_wasm_module_store: options.compiled_wasm_module_store,
      op_state: op_state.clone(),
      op_ctxs,
      have_unpolled_ops: false,
      dispatched_exceptions: Default::default(),
      inspector: Some(inspector),
      waker: AtomicWaker::new(),
    }));
    isolate.set_data(
      Self::STATE_DATA_OFFSET,
      Rc::into_raw(state_rc) as *mut c_void,
    );

    global_context
      .open(&mut isolate)
      .set_slot(&mut isolate, Rc::<RefCell<ContextState>>::default());

    let module_map_rc = Rc::new(RefCell::new(ModuleMap::new(loader, op_state)));
    isolate.set_data(
      Self::MODULE_MAP_DATA_OFFSET,
      Rc::into_raw(module_map_rc) as *mut c_void,
    );

    let mut js_runtime = Self {
      v8_isolate: Some(isolate),
      built_from_snapshot: has_startup_snapshot,
      allocations: IsolateAllocations::default(),
      event_loop_middlewares: Vec::with_capacity(options.extensions.len()),
      extensions: options.extensions,
    };

    // Init resources and ops before extensions to make sure they are
    // available during the initialization process.
    js_runtime.init_extension_ops().unwrap();
    // TODO(@AaronO): diff extensions inited in snapshot and those provided
    // for now we assume that snapshot and extensions always match
    if !has_startup_snapshot {
      let realm = js_runtime.global_realm();
      js_runtime.init_extension_js(&realm).unwrap();
    }
    // Init callbacks (opresolve)
    let global_realm = js_runtime.global_realm();
    js_runtime.init_cbs(&global_realm);

    js_runtime
  }

  fn drop_state_and_module_map(v8_isolate: &mut OwnedIsolate) {
    let state_ptr = v8_isolate.get_data(Self::STATE_DATA_OFFSET);
    let state_rc =
    // SAFETY: We are sure that it's a valid pointer for whole lifetime of
    // the runtime.
    unsafe { Rc::from_raw(state_ptr as *const RefCell<JsRuntimeState>) };
    drop(state_rc);

    let module_map_ptr = v8_isolate.get_data(Self::MODULE_MAP_DATA_OFFSET);
    let module_map_rc =
    // SAFETY: We are sure that it's a valid pointer for whole lifetime of
    // the runtime.
    unsafe { Rc::from_raw(module_map_ptr as *const RefCell<ModuleMap>) };
    drop(module_map_rc);
  }

  pub fn global_context(&mut self) -> v8::Global<v8::Context> {
    self.global_realm().0
  }

  pub fn v8_isolate(&mut self) -> &mut v8::OwnedIsolate {
    self.v8_isolate.as_mut().unwrap()
  }

  pub fn inspector(&mut self) -> Rc<RefCell<JsRuntimeInspector>> {
    Self::state(self.v8_isolate()).borrow().inspector()
  }

  pub fn global_realm(&mut self) -> JsRealm {
    let state = Self::state(self.v8_isolate());
    let state = state.borrow();
    state.global_realm.clone().unwrap()
  }

  /// Creates a new realm (V8 context) in this JS execution context,
  /// pre-initialized with all of the extensions that were passed in
  /// [`RuntimeOptions::extensions`] when the [`JsRuntime`] was constructed.
  ///
  /// If the [`JsRuntime`] was not built from a snapshot (see
  /// [`RuntimeOptions::startup_snapshot`]), the JS code for the extensions will
  /// be run in the call to this method. In contrast, if there is a snapshot,
  /// that will be used instead, and the extensions' initialization will come
  /// "for free".
  pub fn create_realm(&mut self) -> Result<JsRealm, Error> {
    let realm = {
      // SAFETY: Having the scope tied to self's lifetime makes it impossible to
      // reference self.ops while the scope is alive. Here we turn it into an
      // unbound lifetime, which is sound because 1. it only lives until the end
      // of this block, and 2. the HandleScope only has access to the isolate,
      // and nothing else we're accessing from self does.
      let scope = &mut v8::HandleScope::new(unsafe {
        &mut *(self.v8_isolate() as *mut v8::OwnedIsolate)
      });
      let context = bindings::initialize_context(
        scope,
        &Self::state(self.v8_isolate()).borrow().op_ctxs,
        self.built_from_snapshot,
      );
      context.set_slot(scope, Rc::<RefCell<ContextState>>::default());

      Self::state(scope)
        .borrow_mut()
        .known_realms
        .push(v8::Weak::new(scope, &context));

      JsRealm::new(v8::Global::new(scope, context))
    };

    if !self.built_from_snapshot {
      self.init_extension_js(&realm)?;
    }

    self.init_cbs(&realm);

    Ok(realm)
  }

  pub fn handle_scope(&mut self) -> v8::HandleScope {
    self.global_realm().handle_scope(self.v8_isolate())
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
    isolate.set_wasm_async_resolve_promise_callback(
      bindings::wasm_async_resolve_promise_callback,
    );
    isolate
  }

  pub(crate) fn state(isolate: &v8::Isolate) -> Rc<RefCell<JsRuntimeState>> {
    let state_ptr = isolate.get_data(Self::STATE_DATA_OFFSET);
    let state_rc =
      // SAFETY: We are sure that it's a valid pointer for whole lifetime of
      // the runtime.
      unsafe { Rc::from_raw(state_ptr as *const RefCell<JsRuntimeState>) };
    let state = state_rc.clone();
    Rc::into_raw(state_rc);
    state
  }

  pub(crate) fn module_map(isolate: &v8::Isolate) -> Rc<RefCell<ModuleMap>> {
    let module_map_ptr = isolate.get_data(Self::MODULE_MAP_DATA_OFFSET);
    let module_map_rc =
      // SAFETY: We are sure that it's a valid pointer for whole lifetime of
      // the runtime.
      unsafe { Rc::from_raw(module_map_ptr as *const RefCell<ModuleMap>) };
    let module_map = module_map_rc.clone();
    Rc::into_raw(module_map_rc);
    module_map
  }

  /// Initializes JS of provided Extensions in the given realm
  fn init_extension_js(&mut self, realm: &JsRealm) -> Result<(), Error> {
    // Take extensions to avoid double-borrow
    let mut extensions: Vec<Extension> = std::mem::take(&mut self.extensions);
    for m in extensions.iter_mut() {
      let js_files = m.init_js();
      for (filename, source) in js_files {
        // TODO(@AaronO): use JsRuntime::execute_static() here to move src off heap
        realm.execute_script(self.v8_isolate(), filename, source)?;
      }
    }
    // Restore extensions
    self.extensions = extensions;

    Ok(())
  }

  /// Collects ops from extensions & applies middleware
  fn collect_ops(extensions: &mut [Extension]) -> Vec<OpDecl> {
    // Middleware
    let middleware: Vec<Box<OpMiddlewareFn>> = extensions
      .iter_mut()
      .filter_map(|e| e.init_middleware())
      .collect();

    // macroware wraps an opfn in all the middleware
    let macroware = move |d| middleware.iter().fold(d, |d, m| m(d));

    // Flatten ops, apply middlware & override disabled ops
    extensions
      .iter_mut()
      .filter_map(|e| e.init_ops())
      .flatten()
      .map(|d| OpDecl {
        name: d.name,
        ..macroware(d)
      })
      .map(|op| match op.enabled {
        true => op,
        false => OpDecl {
          v8_fn_ptr: match op.is_async {
            true => op_void_async::v8_fn_ptr(),
            false => op_void_sync::v8_fn_ptr(),
          },
          ..op
        },
      })
      .collect()
  }

  /// Initializes ops of provided Extensions
  fn init_extension_ops(&mut self) -> Result<(), Error> {
    let op_state = self.op_state();
    // Take extensions to avoid double-borrow
    let mut extensions: Vec<Extension> = std::mem::take(&mut self.extensions);

    // Setup state
    for e in extensions.iter_mut() {
      // ops are already registered during in bindings::initialize_context();
      e.init_state(&mut op_state.borrow_mut())?;

      // Setup event-loop middleware
      if let Some(middleware) = e.init_event_loop_middleware() {
        self.event_loop_middlewares.push(middleware);
      }
    }

    // Restore extensions
    self.extensions = extensions;

    Ok(())
  }

  /// Grab a Global handle to a v8 value returned by the expression
  pub(crate) fn grab<'s, T>(
    scope: &mut v8::HandleScope<'s>,
    root: v8::Local<'s, v8::Value>,
    path: &str,
  ) -> Option<v8::Local<'s, T>>
  where
    v8::Local<'s, T>: TryFrom<v8::Local<'s, v8::Value>, Error = v8::DataError>,
  {
    path
      .split('.')
      .fold(Some(root), |p, k| {
        let p = v8::Local::<v8::Object>::try_from(p?).ok()?;
        let k = v8::String::new(scope, k)?;
        p.get(scope, k.into())
      })?
      .try_into()
      .ok()
  }

  pub fn grab_global<'s, T>(
    scope: &mut v8::HandleScope<'s>,
    path: &str,
  ) -> Option<v8::Local<'s, T>>
  where
    v8::Local<'s, T>: TryFrom<v8::Local<'s, v8::Value>, Error = v8::DataError>,
  {
    let context = scope.get_current_context();
    let global = context.global(scope);
    Self::grab(scope, global.into(), path)
  }

  pub(crate) fn ensure_objs<'s>(
    scope: &mut v8::HandleScope<'s>,
    root: v8::Local<'s, v8::Object>,
    path: &str,
  ) -> Option<v8::Local<'s, v8::Object>> {
    path.split('.').fold(Some(root), |p, k| {
      let k = v8::String::new(scope, k)?.into();
      match p?.get(scope, k) {
        Some(v) if !v.is_null_or_undefined() => v.try_into().ok(),
        _ => {
          let o = v8::Object::new(scope);
          p?.set(scope, k, o.into());
          Some(o)
        }
      }
    })
  }

  /// Grabs a reference to core.js' opresolve & syncOpsCache()
  fn init_cbs(&mut self, realm: &JsRealm) {
    let (recv_cb, build_custom_error_cb) = {
      let scope = &mut realm.handle_scope(self.v8_isolate());
      let recv_cb =
        Self::grab_global::<v8::Function>(scope, "Deno.core.opresolve")
          .expect("Deno.core.opresolve is undefined in the realm");
      let recv_cb = v8::Global::new(scope, recv_cb);
      let build_custom_error_cb =
        Self::grab_global::<v8::Function>(scope, "Deno.core.buildCustomError")
          .expect("Deno.core.buildCustomError is undefined in the realm");
      let build_custom_error_cb = v8::Global::new(scope, build_custom_error_cb);
      (recv_cb, build_custom_error_cb)
    };
    // Put global handle in callback state
    let state = realm.state(self.v8_isolate());
    state.borrow_mut().js_recv_cb.replace(recv_cb);
    state
      .borrow_mut()
      .js_build_custom_error_cb
      .replace(build_custom_error_cb);
  }

  /// Returns the runtime's op state, which can be used to maintain ops
  /// and access resources between op calls.
  pub fn op_state(&mut self) -> Rc<RefCell<OpState>> {
    let state_rc = Self::state(self.v8_isolate());
    let state = state_rc.borrow();
    state.op_state.clone()
  }

  /// Executes traditional JavaScript code (traditional = not ES modules).
  ///
  /// The execution takes place on the current global context, so it is possible
  /// to maintain local JS state and invoke this method multiple times.
  ///
  /// `name` can be a filepath or any other string, eg.
  ///
  ///   - "/some/file/path.js"
  ///   - "<anon>"
  ///   - "[native code]"
  ///
  /// The same `name` value can be used for multiple executions.
  ///
  /// `Error` can usually be downcast to `JsError`.
  pub fn execute_script(
    &mut self,
    name: &str,
    source_code: &str,
  ) -> Result<v8::Global<v8::Value>, Error> {
    self
      .global_realm()
      .execute_script(self.v8_isolate(), name, source_code)
  }

  /// Takes a snapshot. The isolate should have been created with will_snapshot
  /// set to true.
  ///
  /// `Error` can usually be downcast to `JsError`.
  pub fn snapshot(mut self) -> v8::StartupData {
    // Nuke Deno.core.ops.* to avoid ExternalReference snapshotting issues
    // TODO(@AaronO): make ops stable across snapshots
    {
      let scope = &mut self.handle_scope();
      let o = Self::grab_global::<v8::Object>(scope, "Deno.core.ops").unwrap();
      let names = o.get_own_property_names(scope, Default::default()).unwrap();
      for i in 0..names.length() {
        let key = names.get_index(scope, i).unwrap();
        o.delete(scope, key);
      }
    }

    let state = Self::state(self.v8_isolate());

    state.borrow_mut().global_realm.take();

    state.borrow_mut().inspector.take();

    // Drop existing ModuleMap to drop v8::Global handles
    {
      let v8_isolate = self.v8_isolate();
      Self::drop_state_and_module_map(v8_isolate);
    }

    // Drop other v8::Global handles before snapshotting
    {
      for weak_context in &state.borrow().known_realms {
        if let Some(context) = weak_context.to_global(self.v8_isolate()) {
          let realm = JsRealm::new(context.clone());
          let realm_state = realm.state(self.v8_isolate());
          std::mem::take(&mut realm_state.borrow_mut().js_recv_cb);
          std::mem::take(
            &mut realm_state.borrow_mut().js_build_custom_error_cb,
          );
          context
            .open(self.v8_isolate())
            .clear_all_slots(self.v8_isolate());
        }
      }
      let mut state = state.borrow_mut();
      state.known_realms.clear();
      // Free up additional global handles before creating the snapshot
      state.js_macrotask_cbs.clear();
      state.js_nexttick_cbs.clear();
    }

    let snapshot_creator = self.v8_isolate.take().unwrap();
    snapshot_creator
      .create_blob(v8::FunctionCodeHandling::Keep)
      .unwrap()
  }

  /// Returns the namespace object of a module.
  ///
  /// This is only available after module evaluation has completed.
  /// This function panics if module has not been instantiated.
  pub fn get_module_namespace(
    &mut self,
    module_id: ModuleId,
  ) -> Result<v8::Global<v8::Object>, Error> {
    let module_map_rc = Self::module_map(self.v8_isolate());

    let module_handle = module_map_rc
      .borrow()
      .get_handle(module_id)
      .expect("ModuleInfo not found");

    let scope = &mut self.handle_scope();

    let module = module_handle.open(scope);

    if module.get_status() == v8::ModuleStatus::Errored {
      let exception = module.get_exception();
      return exception_to_err_result(scope, exception, false);
    }

    assert!(matches!(
      module.get_status(),
      v8::ModuleStatus::Instantiated | v8::ModuleStatus::Evaluated
    ));

    let module_namespace: v8::Local<v8::Object> =
      v8::Local::try_from(module.get_module_namespace())
        .map_err(|err: v8::DataError| generic_error(err.to_string()))?;

    Ok(v8::Global::new(scope, module_namespace))
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

  fn pump_v8_message_loop(&mut self) -> Result<(), Error> {
    let scope = &mut self.handle_scope();
    while v8::Platform::pump_message_loop(
      &v8::V8::get_current_platform(),
      scope,
      false, // don't block if there are no tasks
    ) {
      // do nothing
    }

    let tc_scope = &mut v8::TryCatch::new(scope);
    tc_scope.perform_microtask_checkpoint();
    match tc_scope.exception() {
      None => Ok(()),
      Some(exception) => exception_to_err_result(tc_scope, exception, false),
    }
  }

  pub fn poll_value(
    &mut self,
    global: &v8::Global<v8::Value>,
    cx: &mut Context,
  ) -> Poll<Result<v8::Global<v8::Value>, Error>> {
    let state = self.poll_event_loop(cx, false);

    let mut scope = self.handle_scope();
    let local = v8::Local::<v8::Value>::new(&mut scope, global);

    if let Ok(promise) = v8::Local::<v8::Promise>::try_from(local) {
      match promise.state() {
        v8::PromiseState::Pending => match state {
          Poll::Ready(Ok(_)) => {
            let msg = "Promise resolution is still pending but the event loop has already resolved.";
            Poll::Ready(Err(generic_error(msg)))
          }
          Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
          Poll::Pending => Poll::Pending,
        },
        v8::PromiseState::Fulfilled => {
          let value = promise.result(&mut scope);
          let value_handle = v8::Global::new(&mut scope, value);
          Poll::Ready(Ok(value_handle))
        }
        v8::PromiseState::Rejected => {
          let exception = promise.result(&mut scope);
          Poll::Ready(exception_to_err_result(&mut scope, exception, false))
        }
      }
    } else {
      let value_handle = v8::Global::new(&mut scope, local);
      Poll::Ready(Ok(value_handle))
    }
  }

  /// Waits for the given value to resolve while polling the event loop.
  ///
  /// This future resolves when either the value is resolved or the event loop runs to
  /// completion.
  pub async fn resolve_value(
    &mut self,
    global: v8::Global<v8::Value>,
  ) -> Result<v8::Global<v8::Value>, Error> {
    poll_fn(|cx| self.poll_value(&global, cx)).await
  }

  /// Runs event loop to completion
  ///
  /// This future resolves when:
  ///  - there are no more pending dynamic imports
  ///  - there are no more pending ops
  ///  - there are no more active inspector sessions (only if `wait_for_inspector` is set to true)
  pub async fn run_event_loop(
    &mut self,
    wait_for_inspector: bool,
  ) -> Result<(), Error> {
    poll_fn(|cx| self.poll_event_loop(cx, wait_for_inspector)).await
  }

  /// Runs a single tick of event loop
  ///
  /// If `wait_for_inspector` is set to true event loop
  /// will return `Poll::Pending` if there are active inspector sessions.
  pub fn poll_event_loop(
    &mut self,
    cx: &mut Context,
    wait_for_inspector: bool,
  ) -> Poll<Result<(), Error>> {
    // We always poll the inspector first
    let _ = self.inspector().borrow_mut().poll_unpin(cx);
    let state_rc = Self::state(self.v8_isolate());
    let module_map_rc = Self::module_map(self.v8_isolate());
    {
      let state = state_rc.borrow();
      state.waker.register(cx.waker());
    }

    self.pump_v8_message_loop()?;

    // Ops
    {
      self.resolve_async_ops(cx)?;
      self.drain_nexttick()?;
      self.drain_macrotasks()?;
      self.check_promise_exceptions()?;
    }
    // Dynamic module loading - ie. modules loaded using "import()"
    {
      // Run in a loop so that dynamic imports that only depend on another
      // dynamic import can be resolved in this event loop iteration.
      //
      // For example, a dynamically imported module like the following can be
      // immediately resolved after `dependency.ts` is fully evaluated, but it
      // wouldn't if not for this loop.
      //
      //    await delay(1000);
      //    await import("./dependency.ts");
      //    console.log("test")
      //
      loop {
        let poll_imports = self.prepare_dyn_imports(cx)?;
        assert!(poll_imports.is_ready());

        let poll_imports = self.poll_dyn_imports(cx)?;
        assert!(poll_imports.is_ready());

        if !self.evaluate_dyn_imports() {
          break;
        }
      }

      self.check_promise_exceptions()?;
    }

    // Event loop middlewares
    let mut maybe_scheduling = false;
    {
      let op_state = state_rc.borrow().op_state.clone();
      for f in &self.event_loop_middlewares {
        if f(op_state.clone(), cx) {
          maybe_scheduling = true;
        }
      }
    }

    // Top level module
    self.evaluate_pending_module();

    let pending_state = Self::event_loop_pending_state(self.v8_isolate());
    let inspector_has_active_sessions =
      self.inspector().borrow_mut().has_active_sessions();

    if !pending_state.is_pending() && !maybe_scheduling {
      if wait_for_inspector && inspector_has_active_sessions {
        return Poll::Pending;
      }

      return Poll::Ready(Ok(()));
    }

    let mut state = state_rc.borrow_mut();
    let module_map = module_map_rc.borrow();

    // Check if more async ops have been dispatched
    // during this turn of event loop.
    // If there are any pending background tasks, we also wake the runtime to
    // make sure we don't miss them.
    // TODO(andreubotella) The event loop will spin as long as there are pending
    // background tasks. We should look into having V8 notify us when a
    // background task is done.
    if state.have_unpolled_ops
      || pending_state.has_pending_background_tasks
      || pending_state.has_tick_scheduled
      || maybe_scheduling
    {
      state.waker.wake();
    }

    if pending_state.has_pending_module_evaluation {
      if pending_state.has_pending_refed_ops
        || pending_state.has_pending_dyn_imports
        || pending_state.has_pending_dyn_module_evaluation
        || pending_state.has_pending_background_tasks
        || pending_state.has_tick_scheduled
        || maybe_scheduling
      {
        // pass, will be polled again
      } else {
        let msg = "Module evaluation is still pending but there are no pending ops or dynamic imports. This situation is often caused by unresolved promises.";
        return Poll::Ready(Err(generic_error(msg)));
      }
    }

    if pending_state.has_pending_dyn_module_evaluation {
      if pending_state.has_pending_refed_ops
        || pending_state.has_pending_dyn_imports
        || pending_state.has_pending_background_tasks
        || pending_state.has_tick_scheduled
      {
        // pass, will be polled again
      } else if state.dyn_module_evaluate_idle_counter >= 1 {
        let mut msg = "Dynamically imported module evaluation is still pending but there are no pending ops. This situation is often caused by unresolved promises.
Pending dynamic modules:\n".to_string();
        for pending_evaluate in &state.pending_dyn_mod_evaluate {
          let module_info = module_map
            .get_info_by_id(&pending_evaluate.module_id)
            .unwrap();
          msg.push_str("- ");
          msg.push_str(module_info.name.as_str());
        }
        return Poll::Ready(Err(generic_error(msg)));
      } else {
        // Delay the above error by one spin of the event loop. A dynamic import
        // evaluation may complete during this, in which case the counter will
        // reset.
        state.dyn_module_evaluate_idle_counter += 1;
        state.waker.wake();
      }
    }

    Poll::Pending
  }

  pub(crate) fn event_loop_pending_state(
    isolate: &mut v8::Isolate,
  ) -> EventLoopPendingState {
    let state_rc = Self::state(isolate);
    let module_map_rc = Self::module_map(isolate);
    let state = state_rc.borrow_mut();
    let module_map = module_map_rc.borrow();

    let mut num_unrefed_ops = 0;
    for weak_context in &state.known_realms {
      if let Some(context) = weak_context.to_global(isolate) {
        let realm = JsRealm::new(context);
        num_unrefed_ops += realm.state(isolate).borrow().unrefed_ops.len();
      }
    }

    EventLoopPendingState {
      has_pending_refed_ops: state.pending_ops.len() > num_unrefed_ops,
      has_pending_dyn_imports: module_map.has_pending_dynamic_imports(),
      has_pending_dyn_module_evaluation: !state
        .pending_dyn_mod_evaluate
        .is_empty(),
      has_pending_module_evaluation: state.pending_mod_evaluate.is_some(),
      has_pending_background_tasks: isolate.has_pending_background_tasks(),
      has_tick_scheduled: state.has_tick_scheduled,
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct EventLoopPendingState {
  has_pending_refed_ops: bool,
  has_pending_dyn_imports: bool,
  has_pending_dyn_module_evaluation: bool,
  has_pending_module_evaluation: bool,
  has_pending_background_tasks: bool,
  has_tick_scheduled: bool,
}
impl EventLoopPendingState {
  pub fn is_pending(&self) -> bool {
    self.has_pending_refed_ops
      || self.has_pending_dyn_imports
      || self.has_pending_dyn_module_evaluation
      || self.has_pending_module_evaluation
      || self.has_pending_background_tasks
      || self.has_tick_scheduled
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
  // SAFETY: The data is a pointer to the Rust callback function. It is stored
  // in `JsRuntime::allocations` and thus is guaranteed to outlive the isolate.
  let callback = unsafe { &mut *(data as *mut F) };
  callback(current_heap_limit, initial_heap_limit)
}

impl JsRuntimeState {
  pub(crate) fn inspector(&self) -> Rc<RefCell<JsRuntimeInspector>> {
    self.inspector.as_ref().unwrap().clone()
  }

  /// Called by `bindings::host_import_module_dynamically_callback`
  /// after initiating new dynamic import load.
  pub fn notify_new_dynamic_import(&mut self) {
    // Notify event loop to poll again soon.
    self.waker.wake();
  }
}

pub(crate) fn exception_to_err_result<'s, T>(
  scope: &mut v8::HandleScope<'s>,
  exception: v8::Local<v8::Value>,
  in_promise: bool,
) -> Result<T, Error> {
  let state_rc = JsRuntime::state(scope);

  let was_terminating_execution = scope.is_execution_terminating();
  // If TerminateExecution was called, cancel isolate termination so that the
  // exception can be created. Note that `scope.is_execution_terminating()` may
  // have returned false if TerminateExecution was indeed called but there was
  // no JS to execute after the call.
  scope.cancel_terminate_execution();
  let mut exception = exception;
  {
    // If termination is the result of a `op_dispatch_exception` call, we want
    // to use the exception that was passed to it rather than the exception that
    // was passed to this function.
    let state = state_rc.borrow();
    exception = state
      .dispatched_exceptions
      .back()
      .map(|exception| v8::Local::new(scope, exception.clone()))
      .unwrap_or_else(|| {
        // Maybe make a new exception object.
        if was_terminating_execution && exception.is_null_or_undefined() {
          let message = v8::String::new(scope, "execution terminated").unwrap();
          v8::Exception::error(scope, message)
        } else {
          exception
        }
      });
  }

  let mut js_error = JsError::from_v8_exception(scope, exception);
  if in_promise {
    js_error.exception_message = format!(
      "Uncaught (in promise) {}",
      js_error.exception_message.trim_start_matches("Uncaught ")
    );
  }

  if was_terminating_execution {
    // Resume exception termination.
    scope.terminate_execution();
  }

  Err(js_error.into())
}

// Related to module loading
impl JsRuntime {
  pub(crate) fn instantiate_module(
    &mut self,
    id: ModuleId,
  ) -> Result<(), v8::Global<v8::Value>> {
    let module_map_rc = Self::module_map(self.v8_isolate());
    let scope = &mut self.handle_scope();
    let tc_scope = &mut v8::TryCatch::new(scope);

    let module = module_map_rc
      .borrow()
      .get_handle(id)
      .map(|handle| v8::Local::new(tc_scope, handle))
      .expect("ModuleInfo not found");

    if module.get_status() == v8::ModuleStatus::Errored {
      return Err(v8::Global::new(tc_scope, module.get_exception()));
    }

    // IMPORTANT: No borrows to `ModuleMap` can be held at this point because
    // `module_resolve_callback` will be calling into `ModuleMap` from within
    // the isolate.
    let instantiate_result =
      module.instantiate_module(tc_scope, bindings::module_resolve_callback);

    if instantiate_result.is_none() {
      let exception = tc_scope.exception().unwrap();
      return Err(v8::Global::new(tc_scope, exception));
    }

    Ok(())
  }

  fn dynamic_import_module_evaluate(
    &mut self,
    load_id: ModuleLoadId,
    id: ModuleId,
  ) -> Result<(), Error> {
    let state_rc = Self::state(self.v8_isolate());
    let module_map_rc = Self::module_map(self.v8_isolate());

    let module_handle = module_map_rc
      .borrow()
      .get_handle(id)
      .expect("ModuleInfo not found");

    let status = {
      let scope = &mut self.handle_scope();
      let module = module_handle.open(scope);
      module.get_status()
    };

    match status {
      v8::ModuleStatus::Instantiated | v8::ModuleStatus::Evaluated => {}
      _ => return Ok(()),
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
    let scope = &mut self.handle_scope();
    let tc_scope = &mut v8::TryCatch::new(scope);
    let module = v8::Local::new(tc_scope, &module_handle);
    let maybe_value = module.evaluate(tc_scope);

    // Update status after evaluating.
    let status = module.get_status();

    if let Some(value) = maybe_value {
      assert!(
        status == v8::ModuleStatus::Evaluated
          || status == v8::ModuleStatus::Errored
      );
      let promise = v8::Local::<v8::Promise>::try_from(value)
        .expect("Expected to get promise as module evaluation result");
      let empty_fn = bindings::create_empty_fn(tc_scope).unwrap();
      promise.catch(tc_scope, empty_fn);
      let mut state = state_rc.borrow_mut();
      let promise_global = v8::Global::new(tc_scope, promise);
      let module_global = v8::Global::new(tc_scope, module);

      let dyn_import_mod_evaluate = DynImportModEvaluate {
        load_id,
        module_id: id,
        promise: promise_global,
        module: module_global,
      };

      state.pending_dyn_mod_evaluate.push(dyn_import_mod_evaluate);
    } else if tc_scope.has_terminated() || tc_scope.is_execution_terminating() {
      return Err(
        generic_error("Cannot evaluate dynamically imported module, because JavaScript execution has been terminated.")
      );
    } else {
      assert!(status == v8::ModuleStatus::Errored);
    }

    Ok(())
  }

  // TODO(bartlomieju): make it return `ModuleEvaluationFuture`?
  /// Evaluates an already instantiated ES module.
  ///
  /// Returns a receiver handle that resolves when module promise resolves.
  /// Implementors must manually call [`JsRuntime::run_event_loop`] to drive
  /// module evaluation future.
  ///
  /// `Error` can usually be downcast to `JsError` and should be awaited and
  /// checked after [`JsRuntime::run_event_loop`] completion.
  ///
  /// This function panics if module has not been instantiated.
  pub fn mod_evaluate(
    &mut self,
    id: ModuleId,
  ) -> oneshot::Receiver<Result<(), Error>> {
    let state_rc = Self::state(self.v8_isolate());
    let module_map_rc = Self::module_map(self.v8_isolate());
    let scope = &mut self.handle_scope();
    let tc_scope = &mut v8::TryCatch::new(scope);

    let module = module_map_rc
      .borrow()
      .get_handle(id)
      .map(|handle| v8::Local::new(tc_scope, handle))
      .expect("ModuleInfo not found");
    let mut status = module.get_status();
    assert_eq!(status, v8::ModuleStatus::Instantiated);

    let (sender, receiver) = oneshot::channel();

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
    {
      let mut state = state_rc.borrow_mut();
      assert!(
        state.pending_mod_evaluate.is_none(),
        "There is already pending top level module evaluation"
      );
      state.pending_mod_evaluate = Some(ModEvaluate {
        promise: None,
        has_evaluated: false,
        handled_promise_rejections: vec![],
        sender,
      });
    }

    let maybe_value = module.evaluate(tc_scope);
    {
      let mut state = state_rc.borrow_mut();
      let pending_mod_evaluate = state.pending_mod_evaluate.as_mut().unwrap();
      pending_mod_evaluate.has_evaluated = true;
    }

    // Update status after evaluating.
    status = module.get_status();

    let has_dispatched_exception =
      !state_rc.borrow_mut().dispatched_exceptions.is_empty();
    if has_dispatched_exception {
      // This will be overrided in `exception_to_err_result()`.
      let exception = v8::undefined(tc_scope).into();
      let pending_mod_evaluate = {
        let mut state = state_rc.borrow_mut();
        state.pending_mod_evaluate.take().unwrap()
      };
      pending_mod_evaluate
        .sender
        .send(exception_to_err_result(tc_scope, exception, false))
        .expect("Failed to send module evaluation error.");
    } else if let Some(value) = maybe_value {
      assert!(
        status == v8::ModuleStatus::Evaluated
          || status == v8::ModuleStatus::Errored
      );
      let promise = v8::Local::<v8::Promise>::try_from(value)
        .expect("Expected to get promise as module evaluation result");
      let promise_global = v8::Global::new(tc_scope, promise);
      let mut state = state_rc.borrow_mut();
      {
        let pending_mod_evaluate = state.pending_mod_evaluate.as_ref().unwrap();
        let pending_rejection_was_already_handled = pending_mod_evaluate
          .handled_promise_rejections
          .contains(&promise_global);
        if !pending_rejection_was_already_handled {
          state.pending_promise_exceptions.remove(&promise_global);
        }
      }
      let promise_global = v8::Global::new(tc_scope, promise);
      state.pending_mod_evaluate.as_mut().unwrap().promise =
        Some(promise_global);
      tc_scope.perform_microtask_checkpoint();
    } else if tc_scope.has_terminated() || tc_scope.is_execution_terminating() {
      let pending_mod_evaluate = {
        let mut state = state_rc.borrow_mut();
        state.pending_mod_evaluate.take().unwrap()
      };
      pending_mod_evaluate.sender.send(Err(
        generic_error("Cannot evaluate module, because JavaScript execution has been terminated.")
      )).expect("Failed to send module evaluation error.");
    } else {
      assert!(status == v8::ModuleStatus::Errored);
    }

    receiver
  }

  fn dynamic_import_reject(
    &mut self,
    id: ModuleLoadId,
    exception: v8::Global<v8::Value>,
  ) {
    let module_map_rc = Self::module_map(self.v8_isolate());
    let scope = &mut self.handle_scope();

    let resolver_handle = module_map_rc
      .borrow_mut()
      .dynamic_import_map
      .remove(&id)
      .expect("Invalid dynamic import id");
    let resolver = resolver_handle.open(scope);

    // IMPORTANT: No borrows to `ModuleMap` can be held at this point because
    // rejecting the promise might initiate another `import()` which will
    // in turn call `bindings::host_import_module_dynamically_callback` which
    // will reach into `ModuleMap` from within the isolate.
    let exception = v8::Local::new(scope, exception);
    resolver.reject(scope, exception).unwrap();
    scope.perform_microtask_checkpoint();
  }

  fn dynamic_import_resolve(&mut self, id: ModuleLoadId, mod_id: ModuleId) {
    let state_rc = Self::state(self.v8_isolate());
    let module_map_rc = Self::module_map(self.v8_isolate());
    let scope = &mut self.handle_scope();

    let resolver_handle = module_map_rc
      .borrow_mut()
      .dynamic_import_map
      .remove(&id)
      .expect("Invalid dynamic import id");
    let resolver = resolver_handle.open(scope);

    let module = {
      module_map_rc
        .borrow()
        .get_handle(mod_id)
        .map(|handle| v8::Local::new(scope, handle))
        .expect("Dyn import module info not found")
    };
    // Resolution success
    assert_eq!(module.get_status(), v8::ModuleStatus::Evaluated);

    // IMPORTANT: No borrows to `ModuleMap` can be held at this point because
    // resolving the promise might initiate another `import()` which will
    // in turn call `bindings::host_import_module_dynamically_callback` which
    // will reach into `ModuleMap` from within the isolate.
    let module_namespace = module.get_module_namespace();
    resolver.resolve(scope, module_namespace).unwrap();
    state_rc.borrow_mut().dyn_module_evaluate_idle_counter = 0;
    scope.perform_microtask_checkpoint();
  }

  fn prepare_dyn_imports(
    &mut self,
    cx: &mut Context,
  ) -> Poll<Result<(), Error>> {
    let module_map_rc = Self::module_map(self.v8_isolate());

    if module_map_rc.borrow().preparing_dynamic_imports.is_empty() {
      return Poll::Ready(Ok(()));
    }

    loop {
      let poll_result = module_map_rc
        .borrow_mut()
        .preparing_dynamic_imports
        .poll_next_unpin(cx);

      if let Poll::Ready(Some(prepare_poll)) = poll_result {
        let dyn_import_id = prepare_poll.0;
        let prepare_result = prepare_poll.1;

        match prepare_result {
          Ok(load) => {
            module_map_rc
              .borrow_mut()
              .pending_dynamic_imports
              .push(load.into_future());
          }
          Err(err) => {
            let exception = to_v8_type_error(&mut self.handle_scope(), err);
            self.dynamic_import_reject(dyn_import_id, exception);
          }
        }
        // Continue polling for more prepared dynamic imports.
        continue;
      }

      // There are no active dynamic import loads, or none are ready.
      return Poll::Ready(Ok(()));
    }
  }

  fn poll_dyn_imports(&mut self, cx: &mut Context) -> Poll<Result<(), Error>> {
    let module_map_rc = Self::module_map(self.v8_isolate());

    if module_map_rc.borrow().pending_dynamic_imports.is_empty() {
      return Poll::Ready(Ok(()));
    }

    loop {
      let poll_result = module_map_rc
        .borrow_mut()
        .pending_dynamic_imports
        .poll_next_unpin(cx);

      if let Poll::Ready(Some(load_stream_poll)) = poll_result {
        let maybe_result = load_stream_poll.0;
        let mut load = load_stream_poll.1;
        let dyn_import_id = load.id;

        if let Some(load_stream_result) = maybe_result {
          match load_stream_result {
            Ok((request, info)) => {
              // A module (not necessarily the one dynamically imported) has been
              // fetched. Create and register it, and if successful, poll for the
              // next recursive-load event related to this dynamic import.
              let register_result = load.register_and_recurse(
                &mut self.handle_scope(),
                &request,
                &info,
              );

              match register_result {
                Ok(()) => {
                  // Keep importing until it's fully drained
                  module_map_rc
                    .borrow_mut()
                    .pending_dynamic_imports
                    .push(load.into_future());
                }
                Err(err) => {
                  let exception = match err {
                    ModuleError::Exception(e) => e,
                    ModuleError::Other(e) => {
                      to_v8_type_error(&mut self.handle_scope(), e)
                    }
                  };
                  self.dynamic_import_reject(dyn_import_id, exception)
                }
              }
            }
            Err(err) => {
              // A non-javascript error occurred; this could be due to a an invalid
              // module specifier, or a problem with the source map, or a failure
              // to fetch the module source code.
              let exception = to_v8_type_error(&mut self.handle_scope(), err);
              self.dynamic_import_reject(dyn_import_id, exception);
            }
          }
        } else {
          // The top-level module from a dynamic import has been instantiated.
          // Load is done.
          let module_id =
            load.root_module_id.expect("Root module should be loaded");
          let result = self.instantiate_module(module_id);
          if let Err(exception) = result {
            self.dynamic_import_reject(dyn_import_id, exception);
          }
          self.dynamic_import_module_evaluate(dyn_import_id, module_id)?;
        }

        // Continue polling for more ready dynamic imports.
        continue;
      }

      // There are no active dynamic import loads, or none are ready.
      return Poll::Ready(Ok(()));
    }
  }

  /// "deno_core" runs V8 with Top Level Await enabled. It means that each
  /// module evaluation returns a promise from V8.
  /// Feature docs: https://v8.dev/features/top-level-await
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

    let maybe_module_evaluation =
      state_rc.borrow_mut().pending_mod_evaluate.take();

    if maybe_module_evaluation.is_none() {
      return;
    }

    let mut module_evaluation = maybe_module_evaluation.unwrap();
    let scope = &mut self.handle_scope();

    let promise_global = module_evaluation.promise.clone().unwrap();
    let promise = promise_global.open(scope);
    let promise_state = promise.state();

    match promise_state {
      v8::PromiseState::Pending => {
        // NOTE: `poll_event_loop` will decide if
        // runtime would be woken soon
        state_rc.borrow_mut().pending_mod_evaluate = Some(module_evaluation);
      }
      v8::PromiseState::Fulfilled => {
        scope.perform_microtask_checkpoint();
        // Receiver end might have been already dropped, ignore the result
        let _ = module_evaluation.sender.send(Ok(()));
        module_evaluation.handled_promise_rejections.clear();
      }
      v8::PromiseState::Rejected => {
        let exception = promise.result(scope);
        scope.perform_microtask_checkpoint();

        // Receiver end might have been already dropped, ignore the result
        if module_evaluation
          .handled_promise_rejections
          .contains(&promise_global)
        {
          let _ = module_evaluation.sender.send(Ok(()));
          module_evaluation.handled_promise_rejections.clear();
        } else {
          let _ = module_evaluation
            .sender
            .send(exception_to_err_result(scope, exception, false));
        }
      }
    }
  }

  // Returns true if some dynamic import was resolved.
  fn evaluate_dyn_imports(&mut self) -> bool {
    let mut resolved_any = false;
    let state_rc = Self::state(self.v8_isolate());
    let mut still_pending = vec![];
    let pending =
      std::mem::take(&mut state_rc.borrow_mut().pending_dyn_mod_evaluate);
    for pending_dyn_evaluate in pending {
      let maybe_result = {
        let scope = &mut self.handle_scope();

        let module_id = pending_dyn_evaluate.module_id;
        let promise = pending_dyn_evaluate.promise.open(scope);
        let _module = pending_dyn_evaluate.module.open(scope);
        let promise_state = promise.state();

        match promise_state {
          v8::PromiseState::Pending => {
            still_pending.push(pending_dyn_evaluate);
            None
          }
          v8::PromiseState::Fulfilled => {
            Some(Ok((pending_dyn_evaluate.load_id, module_id)))
          }
          v8::PromiseState::Rejected => {
            let exception = promise.result(scope);
            let exception = v8::Global::new(scope, exception);
            Some(Err((pending_dyn_evaluate.load_id, exception)))
          }
        }
      };

      if let Some(result) = maybe_result {
        resolved_any = true;
        match result {
          Ok((dyn_import_id, module_id)) => {
            self.dynamic_import_resolve(dyn_import_id, module_id);
          }
          Err((dyn_import_id, exception)) => {
            self.dynamic_import_reject(dyn_import_id, exception);
          }
        }
      }
    }
    state_rc.borrow_mut().pending_dyn_mod_evaluate = still_pending;
    resolved_any
  }

  /// Asynchronously load specified module and all of its dependencies.
  ///
  /// The module will be marked as "main", and because of that
  /// "import.meta.main" will return true when checked inside that module.
  ///
  /// User must call [`JsRuntime::mod_evaluate`] with returned `ModuleId`
  /// manually after load is finished.
  pub async fn load_main_module(
    &mut self,
    specifier: &ModuleSpecifier,
    code: Option<String>,
  ) -> Result<ModuleId, Error> {
    let module_map_rc = Self::module_map(self.v8_isolate());
    if let Some(code) = code {
      let scope = &mut self.handle_scope();
      module_map_rc
        .borrow_mut()
        .new_es_module(
          scope,
          // main module
          true,
          specifier.as_str(),
          code.as_bytes(),
        )
        .map_err(|e| match e {
          ModuleError::Exception(exception) => {
            let exception = v8::Local::new(scope, exception);
            exception_to_err_result::<()>(scope, exception, false).unwrap_err()
          }
          ModuleError::Other(error) => error,
        })?;
    }

    let mut load =
      ModuleMap::load_main(module_map_rc.clone(), specifier.as_str()).await?;

    while let Some(load_result) = load.next().await {
      let (request, info) = load_result?;
      let scope = &mut self.handle_scope();
      load.register_and_recurse(scope, &request, &info).map_err(
        |e| match e {
          ModuleError::Exception(exception) => {
            let exception = v8::Local::new(scope, exception);
            exception_to_err_result::<()>(scope, exception, false).unwrap_err()
          }
          ModuleError::Other(error) => error,
        },
      )?;
    }

    let root_id = load.root_module_id.expect("Root module should be loaded");
    self.instantiate_module(root_id).map_err(|e| {
      let scope = &mut self.handle_scope();
      let exception = v8::Local::new(scope, e);
      exception_to_err_result::<()>(scope, exception, false).unwrap_err()
    })?;
    Ok(root_id)
  }

  /// Asynchronously load specified ES module and all of its dependencies.
  ///
  /// This method is meant to be used when loading some utility code that
  /// might be later imported by the main module (ie. an entry point module).
  ///
  /// User must call [`JsRuntime::mod_evaluate`] with returned `ModuleId`
  /// manually after load is finished.
  pub async fn load_side_module(
    &mut self,
    specifier: &ModuleSpecifier,
    code: Option<String>,
  ) -> Result<ModuleId, Error> {
    let module_map_rc = Self::module_map(self.v8_isolate());
    if let Some(code) = code {
      let scope = &mut self.handle_scope();
      module_map_rc
        .borrow_mut()
        .new_es_module(
          scope,
          // not main module
          false,
          specifier.as_str(),
          code.as_bytes(),
        )
        .map_err(|e| match e {
          ModuleError::Exception(exception) => {
            let exception = v8::Local::new(scope, exception);
            exception_to_err_result::<()>(scope, exception, false).unwrap_err()
          }
          ModuleError::Other(error) => error,
        })?;
    }

    let mut load =
      ModuleMap::load_side(module_map_rc.clone(), specifier.as_str()).await?;

    while let Some(load_result) = load.next().await {
      let (request, info) = load_result?;
      let scope = &mut self.handle_scope();
      load.register_and_recurse(scope, &request, &info).map_err(
        |e| match e {
          ModuleError::Exception(exception) => {
            let exception = v8::Local::new(scope, exception);
            exception_to_err_result::<()>(scope, exception, false).unwrap_err()
          }
          ModuleError::Other(error) => error,
        },
      )?;
    }

    let root_id = load.root_module_id.expect("Root module should be loaded");
    self.instantiate_module(root_id).map_err(|e| {
      let scope = &mut self.handle_scope();
      let exception = v8::Local::new(scope, e);
      exception_to_err_result::<()>(scope, exception, false).unwrap_err()
    })?;
    Ok(root_id)
  }

  fn check_promise_exceptions(&mut self) -> Result<(), Error> {
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

    let scope = &mut self.handle_scope();
    let exception = v8::Local::new(scope, handle);
    exception_to_err_result(scope, exception, true)
  }

  // Send finished responses to JS
  fn resolve_async_ops(&mut self, cx: &mut Context) -> Result<(), Error> {
    let state_rc = Self::state(self.v8_isolate());

    // We keep a list of promise IDs and OpResults per realm. Since v8::Context
    // isn't hashable, `results_per_realm` is a vector of (context, list) tuples
    type ResultList = Vec<(i32, OpResult)>;
    let mut results_per_realm: Vec<(v8::Global<v8::Context>, ResultList)> = {
      let known_realms = &mut state_rc.borrow_mut().known_realms;
      let mut results = Vec::with_capacity(known_realms.len());

      // Avoid calling the method multiple times
      let isolate = self.v8_isolate();

      // Remove GC'd realms from `known_realms` at the same time as we populate
      // `results` with those that are still alive.
      known_realms.retain(|weak| {
        if !weak.is_empty() {
          let context = weak.to_global(isolate).unwrap();
          results.push((context, vec![]));
          true
        } else {
          false
        }
      });

      results
    };

    // Now handle actual ops.
    {
      let mut state = state_rc.borrow_mut();
      state.have_unpolled_ops = false;

      while let Poll::Ready(Some(item)) = state.pending_ops.poll_next_unpin(cx)
      {
        let (context, promise_id, op_id, resp) = item;
        state.op_state.borrow().tracker.track_async_completed(op_id);
        for (context2, results) in results_per_realm.iter_mut() {
          if context == *context2 {
            results.push((promise_id, resp));
            break;
          }
        }
        JsRealm::new(context)
          .state(self.v8_isolate())
          .borrow_mut()
          .unrefed_ops
          .remove(&promise_id);
      }
    }

    for (context, results) in results_per_realm {
      if results.is_empty() {
        continue;
      }

      let realm = JsRealm::new(context);
      let js_recv_cb_handle = realm
        .state(self.v8_isolate())
        .borrow()
        .js_recv_cb
        .clone()
        .unwrap();
      let scope = &mut realm.handle_scope(self.v8_isolate());

      // We return async responses to JS in unbounded batches (may change),
      // each batch is a flat vector of tuples:
      // `[promise_id1, op_result1, promise_id2, op_result2, ...]`
      // promise_id is a simple integer, op_result is an ops::OpResult
      // which contains a value OR an error, encoded as a tuple.
      // This batch is received in JS via the special `arguments` variable
      // and then each tuple is used to resolve or reject promises
      let mut args = vec![];

      for (promise_id, mut resp) in results.into_iter() {
        args.push(v8::Integer::new(scope, promise_id).into());
        args.push(match resp.to_v8(scope) {
          Ok(v) => v,
          Err(e) => OpResult::Err(OpError::new(&|_| "TypeError", e.into()))
            .to_v8(scope)
            .unwrap(),
        });
      }

      let tc_scope = &mut v8::TryCatch::new(scope);
      let js_recv_cb = js_recv_cb_handle.open(tc_scope);
      let this = v8::undefined(tc_scope).into();
      js_recv_cb.call(tc_scope, this, args.as_slice());

      if let Some(exception) = tc_scope.exception() {
        return exception_to_err_result(tc_scope, exception, false);
      }
    }

    Ok(())
  }

  fn drain_macrotasks(&mut self) -> Result<(), Error> {
    let state = Self::state(self.v8_isolate());

    if state.borrow().js_macrotask_cbs.is_empty() {
      return Ok(());
    }

    let js_macrotask_cb_handles = state.borrow().js_macrotask_cbs.clone();
    let scope = &mut self.handle_scope();

    for js_macrotask_cb_handle in js_macrotask_cb_handles {
      let js_macrotask_cb = js_macrotask_cb_handle.open(scope);

      // Repeatedly invoke macrotask callback until it returns true (done),
      // such that ready microtasks would be automatically run before
      // next macrotask is processed.
      let tc_scope = &mut v8::TryCatch::new(scope);
      let this = v8::undefined(tc_scope).into();
      loop {
        let is_done = js_macrotask_cb.call(tc_scope, this, &[]);

        if let Some(exception) = tc_scope.exception() {
          return exception_to_err_result(tc_scope, exception, false);
        }

        if tc_scope.has_terminated() || tc_scope.is_execution_terminating() {
          return Ok(());
        }

        let is_done = is_done.unwrap();
        if is_done.is_true() {
          break;
        }
      }
    }

    Ok(())
  }

  fn drain_nexttick(&mut self) -> Result<(), Error> {
    let state = Self::state(self.v8_isolate());

    if state.borrow().js_nexttick_cbs.is_empty() {
      return Ok(());
    }

    if !state.borrow().has_tick_scheduled {
      let scope = &mut self.handle_scope();
      scope.perform_microtask_checkpoint();
    }

    // TODO(bartlomieju): Node also checks for absence of "rejection_to_warn"
    if !state.borrow().has_tick_scheduled {
      return Ok(());
    }

    let js_nexttick_cb_handles = state.borrow().js_nexttick_cbs.clone();
    let scope = &mut self.handle_scope();

    for js_nexttick_cb_handle in js_nexttick_cb_handles {
      let js_nexttick_cb = js_nexttick_cb_handle.open(scope);

      let tc_scope = &mut v8::TryCatch::new(scope);
      let this = v8::undefined(tc_scope).into();
      js_nexttick_cb.call(tc_scope, this, &[]);

      if let Some(exception) = tc_scope.exception() {
        return exception_to_err_result(tc_scope, exception, false);
      }
    }

    Ok(())
  }
}

/// A representation of a JavaScript realm tied to a [`JsRuntime`], that allows
/// execution in the realm's context.
///
/// A [`JsRealm`] instance is a reference to an already existing realm, which
/// does not hold ownership of it, so instances can be created and dropped as
/// needed. As such, calling [`JsRealm::new`] doesn't create a new realm, and
/// cloning a [`JsRealm`] only creates a new reference. See
/// [`JsRuntime::create_realm`] to create new realms instead.
///
/// Despite [`JsRealm`] instances being references, multiple instances that
/// point to the same realm won't overlap because every operation requires
/// passing a mutable reference to the [`v8::Isolate`]. Therefore, no operation
/// on two [`JsRealm`] instances tied to the same isolate can be run at the same
/// time, regardless of whether they point to the same realm.
///
/// # Panics
///
/// Every method of [`JsRealm`] will panic if you call if with a reference to a
/// [`v8::Isolate`] other than the one that corresponds to the current context.
///
/// # Lifetime of the realm
///
/// As long as the corresponding isolate is alive, a [`JsRealm`] instance will
/// keep the underlying V8 context alive even if it would have otherwise been
/// garbage collected.
#[derive(Clone)]
pub struct JsRealm(v8::Global<v8::Context>);
impl JsRealm {
  pub fn new(context: v8::Global<v8::Context>) -> Self {
    JsRealm(context)
  }

  pub fn context(&self) -> &v8::Global<v8::Context> {
    &self.0
  }

  pub(crate) fn state(
    &self,
    isolate: &mut v8::Isolate,
  ) -> Rc<RefCell<ContextState>> {
    self
      .context()
      .open(isolate)
      .get_slot::<Rc<RefCell<ContextState>>>(isolate)
      .unwrap()
      .clone()
  }

  pub(crate) fn state_from_scope(
    scope: &mut v8::HandleScope,
  ) -> Rc<RefCell<ContextState>> {
    let context = scope.get_current_context();
    context
      .get_slot::<Rc<RefCell<ContextState>>>(scope)
      .unwrap()
      .clone()
  }

  pub fn handle_scope<'s>(
    &self,
    isolate: &'s mut v8::Isolate,
  ) -> v8::HandleScope<'s> {
    v8::HandleScope::with_context(isolate, &self.0)
  }

  pub fn global_object<'s>(
    &self,
    isolate: &'s mut v8::Isolate,
  ) -> v8::Local<'s, v8::Object> {
    let scope = &mut self.handle_scope(isolate);
    self.0.open(scope).global(scope)
  }

  /// Executes traditional JavaScript code (traditional = not ES modules) in the
  /// realm's context.
  ///
  /// `name` can be a filepath or any other string, eg.
  ///
  ///   - "/some/file/path.js"
  ///   - "<anon>"
  ///   - "[native code]"
  ///
  /// The same `name` value can be used for multiple executions.
  ///
  /// `Error` can usually be downcast to `JsError`.
  pub fn execute_script(
    &self,
    isolate: &mut v8::Isolate,
    name: &str,
    source_code: &str,
  ) -> Result<v8::Global<v8::Value>, Error> {
    let scope = &mut self.handle_scope(isolate);

    let source = v8::String::new(scope, source_code).unwrap();
    let name = v8::String::new(scope, name).unwrap();
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
      Some(value) => {
        let value_handle = v8::Global::new(tc_scope, value);
        Ok(value_handle)
      }
      None => {
        assert!(tc_scope.has_caught());
        let exception = tc_scope.exception().unwrap();
        exception_to_err_result(tc_scope, exception, false)
      }
    }
  }

  // TODO(andreubotella): `mod_evaluate`, `load_main_module`, `load_side_module`
}

#[inline]
pub fn queue_async_op(
  state: Rc<RefCell<OpState>>,
  scope: &mut v8::HandleScope,
  deferred: bool,
  op: impl Future<Output = (v8::Global<v8::Context>, PromiseId, OpId, OpResult)>
    + 'static,
) {
  match OpCall::eager(op) {
    // This calls promise.resolve() before the control goes back to userland JS. It works something
    // along the lines of:
    //
    // function opresolve(promiseId, ...) {
    //   getPromise(promiseId).resolve(...);
    // }
    // const p = setPromise();
    // op.op_async(promiseId, ...); // Calls `opresolve`
    // return p;
    EagerPollResult::Ready((context, promise_id, op_id, mut resp))
      if !deferred =>
    {
      let args = &[
        v8::Integer::new(scope, promise_id).into(),
        resp.to_v8(scope).unwrap(),
      ];

      let realm = JsRealm::new(context);
      let js_recv_cb_handle =
        realm.state(scope).borrow().js_recv_cb.clone().unwrap();
      state.borrow().tracker.track_async_completed(op_id);

      let tc_scope = &mut v8::TryCatch::new(scope);
      let js_recv_cb = js_recv_cb_handle.open(tc_scope);
      let this = v8::undefined(tc_scope).into();
      js_recv_cb.call(tc_scope, this, args);
    }
    EagerPollResult::Ready(op) => {
      let ready = OpCall::ready(op);
      let state_rc = JsRuntime::state(scope);
      let mut state = state_rc.borrow_mut();
      state.pending_ops.push(ready);
      state.have_unpolled_ops = true;
    }
    EagerPollResult::Pending(op) => {
      let state_rc = JsRuntime::state(scope);
      let mut state = state_rc.borrow_mut();
      state.pending_ops.push(op);
      state.have_unpolled_ops = true;
    }
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use crate::error::custom_error;
  use crate::error::AnyError;
  use crate::modules::ModuleSource;
  use crate::modules::ModuleSourceFuture;
  use crate::modules::ModuleType;
  use crate::ZeroCopyBuf;
  use deno_ops::op;
  use futures::future::lazy;
  use std::ops::FnOnce;
  use std::pin::Pin;
  use std::rc::Rc;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;
  // deno_ops macros generate code assuming deno_core in scope.
  mod deno_core {
    pub use crate::*;
  }

  pub fn run_in_task<F>(f: F)
  where
    F: FnOnce(&mut Context) + Send + 'static,
  {
    futures::executor::block_on(lazy(move |cx| f(cx)));
  }

  #[derive(Copy, Clone)]
  enum Mode {
    Async,
    AsyncZeroCopy(bool),
  }

  struct TestState {
    mode: Mode,
    dispatch_count: Arc<AtomicUsize>,
  }

  #[op(deferred)]
  async fn op_test(
    rc_op_state: Rc<RefCell<OpState>>,
    control: u8,
    buf: Option<ZeroCopyBuf>,
  ) -> Result<u8, AnyError> {
    let op_state_ = rc_op_state.borrow();
    let test_state = op_state_.borrow::<TestState>();
    test_state.dispatch_count.fetch_add(1, Ordering::Relaxed);
    match test_state.mode {
      Mode::Async => {
        assert_eq!(control, 42);
        Ok(43)
      }
      Mode::AsyncZeroCopy(has_buffer) => {
        assert_eq!(buf.is_some(), has_buffer);
        if let Some(buf) = buf {
          assert_eq!(buf.len(), 1);
        }
        Ok(43)
      }
    }
  }

  fn setup(mode: Mode) -> (JsRuntime, Arc<AtomicUsize>) {
    let dispatch_count = Arc::new(AtomicUsize::new(0));
    let dispatch_count2 = dispatch_count.clone();
    let ext = Extension::builder()
      .ops(vec![op_test::decl()])
      .state(move |state| {
        state.put(TestState {
          mode,
          dispatch_count: dispatch_count2.clone(),
        });
        Ok(())
      })
      .build();
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![ext],
      get_error_class_fn: Some(&|error| {
        crate::error::get_custom_error_class(error).unwrap()
      }),
      ..Default::default()
    });

    runtime
      .execute_script(
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
  fn test_ref_unref_ops() {
    let (mut runtime, _dispatch_count) = setup(Mode::Async);
    runtime
      .execute_script(
        "filename.js",
        r#"
        var promiseIdSymbol = Symbol.for("Deno.core.internalPromiseId");
        var p1 = Deno.core.opAsync("op_test", 42);
        var p2 = Deno.core.opAsync("op_test", 42);
        "#,
      )
      .unwrap();
    {
      let realm = runtime.global_realm();
      let isolate = runtime.v8_isolate();
      let state_rc = JsRuntime::state(isolate);
      assert_eq!(state_rc.borrow().pending_ops.len(), 2);
      assert_eq!(realm.state(isolate).borrow().unrefed_ops.len(), 0);
    }
    runtime
      .execute_script(
        "filename.js",
        r#"
        Deno.core.ops.op_unref_op(p1[promiseIdSymbol]);
        Deno.core.ops.op_unref_op(p2[promiseIdSymbol]);
        "#,
      )
      .unwrap();
    {
      let realm = runtime.global_realm();
      let isolate = runtime.v8_isolate();
      let state_rc = JsRuntime::state(isolate);
      assert_eq!(state_rc.borrow().pending_ops.len(), 2);
      assert_eq!(realm.state(isolate).borrow().unrefed_ops.len(), 2);
    }
    runtime
      .execute_script(
        "filename.js",
        r#"
        Deno.core.ops.op_ref_op(p1[promiseIdSymbol]);
        Deno.core.ops.op_ref_op(p2[promiseIdSymbol]);
        "#,
      )
      .unwrap();
    {
      let realm = runtime.global_realm();
      let isolate = runtime.v8_isolate();
      let state_rc = JsRuntime::state(isolate);
      assert_eq!(state_rc.borrow().pending_ops.len(), 2);
      assert_eq!(realm.state(isolate).borrow().unrefed_ops.len(), 0);
    }
  }

  #[test]
  fn test_dispatch() {
    let (mut runtime, dispatch_count) = setup(Mode::Async);
    runtime
      .execute_script(
        "filename.js",
        r#"
        let control = 42;
        Deno.core.opAsync("op_test", control);
        async function main() {
          Deno.core.opAsync("op_test", control);
        }
        main();
        "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 2);
  }

  #[test]
  fn test_op_async_promise_id() {
    let (mut runtime, _dispatch_count) = setup(Mode::Async);
    runtime
      .execute_script(
        "filename.js",
        r#"
        const p = Deno.core.opAsync("op_test", 42);
        if (p[Symbol.for("Deno.core.internalPromiseId")] == undefined) {
          throw new Error("missing id on returned promise");
        }
        "#,
      )
      .unwrap();
  }

  #[test]
  fn test_dispatch_no_zero_copy_buf() {
    let (mut runtime, dispatch_count) = setup(Mode::AsyncZeroCopy(false));
    runtime
      .execute_script(
        "filename.js",
        r#"
        Deno.core.opAsync("op_test");
        "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn test_dispatch_stack_zero_copy_bufs() {
    let (mut runtime, dispatch_count) = setup(Mode::AsyncZeroCopy(true));
    runtime
      .execute_script(
        "filename.js",
        r#"
        let zero_copy_a = new Uint8Array([0]);
        Deno.core.opAsync("op_test", null, zero_copy_a);
        "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn test_execute_script_return_value() {
    let mut runtime = JsRuntime::new(Default::default());
    let value_global = runtime.execute_script("a.js", "a = 1 + 2").unwrap();
    {
      let scope = &mut runtime.handle_scope();
      let value = value_global.open(scope);
      assert_eq!(value.integer_value(scope).unwrap(), 3);
    }
    let value_global = runtime.execute_script("b.js", "b = 'foobar'").unwrap();
    {
      let scope = &mut runtime.handle_scope();
      let value = value_global.open(scope);
      assert!(value.is_string());
      assert_eq!(
        value.to_string(scope).unwrap().to_rust_string_lossy(scope),
        "foobar"
      );
    }
  }

  #[tokio::test]
  async fn test_poll_value() {
    run_in_task(|cx| {
      let mut runtime = JsRuntime::new(Default::default());
      let value_global = runtime
        .execute_script("a.js", "Promise.resolve(1 + 2)")
        .unwrap();
      let v = runtime.poll_value(&value_global, cx);
      {
        let scope = &mut runtime.handle_scope();
        assert!(
          matches!(v, Poll::Ready(Ok(v)) if v.open(scope).integer_value(scope).unwrap() == 3)
        );
      }

      let value_global = runtime
        .execute_script(
          "a.js",
          "Promise.resolve(new Promise(resolve => resolve(2 + 2)))",
        )
        .unwrap();
      let v = runtime.poll_value(&value_global, cx);
      {
        let scope = &mut runtime.handle_scope();
        assert!(
          matches!(v, Poll::Ready(Ok(v)) if v.open(scope).integer_value(scope).unwrap() == 4)
        );
      }

      let value_global = runtime
        .execute_script("a.js", "Promise.reject(new Error('fail'))")
        .unwrap();
      let v = runtime.poll_value(&value_global, cx);
      assert!(
        matches!(v, Poll::Ready(Err(e)) if e.downcast_ref::<JsError>().unwrap().exception_message == "Uncaught Error: fail")
      );

      let value_global = runtime
        .execute_script("a.js", "new Promise(resolve => {})")
        .unwrap();
      let v = runtime.poll_value(&value_global, cx);
      matches!(v, Poll::Ready(Err(e)) if e.to_string() == "Promise resolution is still pending but the event loop has already resolved.");
    });
  }

  #[tokio::test]
  async fn test_resolve_value() {
    let mut runtime = JsRuntime::new(Default::default());
    let value_global = runtime
      .execute_script("a.js", "Promise.resolve(1 + 2)")
      .unwrap();
    let result_global = runtime.resolve_value(value_global).await.unwrap();
    {
      let scope = &mut runtime.handle_scope();
      let value = result_global.open(scope);
      assert_eq!(value.integer_value(scope).unwrap(), 3);
    }

    let value_global = runtime
      .execute_script(
        "a.js",
        "Promise.resolve(new Promise(resolve => resolve(2 + 2)))",
      )
      .unwrap();
    let result_global = runtime.resolve_value(value_global).await.unwrap();
    {
      let scope = &mut runtime.handle_scope();
      let value = result_global.open(scope);
      assert_eq!(value.integer_value(scope).unwrap(), 4);
    }

    let value_global = runtime
      .execute_script("a.js", "Promise.reject(new Error('fail'))")
      .unwrap();
    let err = runtime.resolve_value(value_global).await.unwrap_err();
    assert_eq!(
      "Uncaught Error: fail",
      err.downcast::<JsError>().unwrap().exception_message
    );

    let value_global = runtime
      .execute_script("a.js", "new Promise(resolve => {})")
      .unwrap();
    let error_string = runtime
      .resolve_value(value_global)
      .await
      .unwrap_err()
      .to_string();
    assert_eq!(
      "Promise resolution is still pending but the event loop has already resolved.",
      error_string,
    );
  }

  #[test]
  fn terminate_execution_webassembly() {
    let (mut runtime, _dispatch_count) = setup(Mode::Async);
    let v8_isolate_handle = runtime.v8_isolate().thread_safe_handle();

    // Run an infinite loop in Webassemby code, which should be terminated.
    let promise = runtime.execute_script("infinite_wasm_loop.js",
                                 r#"
                                 (async () => {
                                  const wasmCode = new Uint8Array([
                                      0,    97,   115,  109,  1,    0,    0,    0,    1,   4,    1,
                                      96,   0,    0,    3,    2,    1,    0,    7,    17,  1,    13,
                                      105,  110,  102,  105,  110,  105,  116,  101,  95,  108,  111,
                                      111,  112,  0,    0,    10,   9,    1,    7,    0,   3,    64,
                                      12,   0,    11,   11,
                                  ]);
                                  const wasmModule = await WebAssembly.compile(wasmCode);
                                  globalThis.wasmInstance = new WebAssembly.Instance(wasmModule);
                                  })()
                                      "#).unwrap();
    futures::executor::block_on(runtime.resolve_value(promise)).unwrap();
    let terminator_thread = std::thread::spawn(move || {
      std::thread::sleep(std::time::Duration::from_millis(1000));

      // terminate execution
      let ok = v8_isolate_handle.terminate_execution();
      assert!(ok);
    });
    let err = runtime
      .execute_script(
        "infinite_wasm_loop2.js",
        "globalThis.wasmInstance.exports.infinite_loop();",
      )
      .unwrap_err();
    assert_eq!(err.to_string(), "Uncaught Error: execution terminated");
    // Cancel the execution-terminating exception in order to allow script
    // execution again.
    let ok = runtime.v8_isolate().cancel_terminate_execution();
    assert!(ok);

    // Verify that the isolate usable again.
    runtime
      .execute_script("simple.js", "1 + 1")
      .expect("execution should be possible again");

    terminator_thread.join().unwrap();
  }

  #[test]
  fn terminate_execution() {
    let (mut isolate, _dispatch_count) = setup(Mode::Async);
    let v8_isolate_handle = isolate.v8_isolate().thread_safe_handle();

    let terminator_thread = std::thread::spawn(move || {
      // allow deno to boot and run
      std::thread::sleep(std::time::Duration::from_millis(100));

      // terminate execution
      let ok = v8_isolate_handle.terminate_execution();
      assert!(ok);
    });

    // Rn an infinite loop, which should be terminated.
    match isolate.execute_script("infinite_loop.js", "for(;;) {}") {
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
      .execute_script("simple.js", "1 + 1")
      .expect("execution should be possible again");

    terminator_thread.join().unwrap();
  }

  #[test]
  fn dangling_shared_isolate() {
    let v8_isolate_handle = {
      // isolate is dropped at the end of this block
      let (mut runtime, _dispatch_count) = setup(Mode::Async);
      runtime.v8_isolate().thread_safe_handle()
    };

    // this should not SEGFAULT
    v8_isolate_handle.terminate_execution();
  }

  #[test]
  fn syntax_error() {
    let mut runtime = JsRuntime::new(Default::default());
    let src = "hocuspocus(";
    let r = runtime.execute_script("i.js", src);
    let e = r.unwrap_err();
    let js_error = e.downcast::<JsError>().unwrap();
    let frame = js_error.frames.first().unwrap();
    assert_eq!(frame.column_number, Some(12));
  }

  #[test]
  fn test_encode_decode() {
    run_in_task(|cx| {
      let (mut runtime, _dispatch_count) = setup(Mode::Async);
      runtime
        .execute_script(
          "encode_decode_test.js",
          include_str!("encode_decode_test.js"),
        )
        .unwrap();
      if let Poll::Ready(Err(_)) = runtime.poll_event_loop(cx, false) {
        unreachable!();
      }
    });
  }

  #[test]
  fn test_serialize_deserialize() {
    run_in_task(|cx| {
      let (mut runtime, _dispatch_count) = setup(Mode::Async);
      runtime
        .execute_script(
          "serialize_deserialize_test.js",
          include_str!("serialize_deserialize_test.js"),
        )
        .unwrap();
      if let Poll::Ready(Err(_)) = runtime.poll_event_loop(cx, false) {
        unreachable!();
      }
    });
  }

  #[test]
  fn test_error_builder() {
    #[op]
    fn op_err() -> Result<(), Error> {
      Err(custom_error("DOMExceptionOperationError", "abc"))
    }

    pub fn get_error_class_name(_: &Error) -> &'static str {
      "DOMExceptionOperationError"
    }

    run_in_task(|cx| {
      let ext = Extension::builder().ops(vec![op_err::decl()]).build();
      let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![ext],
        get_error_class_fn: Some(&get_error_class_name),
        ..Default::default()
      });
      runtime
        .execute_script(
          "error_builder_test.js",
          include_str!("error_builder_test.js"),
        )
        .unwrap();
      if let Poll::Ready(Err(_)) = runtime.poll_event_loop(cx, false) {
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
      runtime.execute_script("a.js", "a = 1 + 2").unwrap();
      runtime.snapshot()
    };

    let snapshot = Snapshot::JustCreated(snapshot);
    let mut runtime2 = JsRuntime::new(RuntimeOptions {
      startup_snapshot: Some(snapshot),
      ..Default::default()
    });
    runtime2
      .execute_script("check.js", "if (a != 3) throw Error('x')")
      .unwrap();
  }
  #[test]
  fn test_snapshot_callbacks() {
    let snapshot = {
      let mut runtime = JsRuntime::new(RuntimeOptions {
        will_snapshot: true,
        ..Default::default()
      });
      runtime
        .execute_script(
          "a.js",
          r#"
          Deno.core.ops.op_set_macrotask_callback(() => {
            return true;
          });
          Deno.core.ops.op_set_format_exception_callback(()=> {
            return null;
          })
          Deno.core.setPromiseRejectCallback(() => {
            return false;
          });
          a = 1 + 2;
      "#,
        )
        .unwrap();
      runtime.snapshot()
    };

    let snapshot = Snapshot::JustCreated(snapshot);
    let mut runtime2 = JsRuntime::new(RuntimeOptions {
      startup_snapshot: Some(snapshot),
      ..Default::default()
    });
    runtime2
      .execute_script("check.js", "if (a != 3) throw Error('x')")
      .unwrap();
  }

  #[test]
  fn test_from_boxed_snapshot() {
    let snapshot = {
      let mut runtime = JsRuntime::new(RuntimeOptions {
        will_snapshot: true,
        ..Default::default()
      });
      runtime.execute_script("a.js", "a = 1 + 2").unwrap();
      let snap: &[u8] = &*runtime.snapshot();
      Vec::from(snap).into_boxed_slice()
    };

    let snapshot = Snapshot::Boxed(snapshot);
    let mut runtime2 = JsRuntime::new(RuntimeOptions {
      startup_snapshot: Some(snapshot),
      ..Default::default()
    });
    runtime2
      .execute_script("check.js", "if (a != 3) throw Error('x')")
      .unwrap();
  }

  #[test]
  fn test_get_module_namespace() {
    #[derive(Default)]
    struct ModsLoader;

    impl ModuleLoader for ModsLoader {
      fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
      ) -> Result<ModuleSpecifier, Error> {
        assert_eq!(specifier, "file:///main.js");
        assert_eq!(referrer, ".");
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        async { Err(generic_error("Module loading is not supported")) }
          .boxed_local()
      }
    }

    let loader = std::rc::Rc::new(ModsLoader::default());
    let mut runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });

    let specifier = crate::resolve_url("file:///main.js").unwrap();
    let source_code = r#"
      export const a = "b";
      export default 1 + 2;
      "#
    .to_string();

    let module_id = futures::executor::block_on(
      runtime.load_main_module(&specifier, Some(source_code)),
    )
    .unwrap();

    let _ = runtime.mod_evaluate(module_id);

    let module_namespace = runtime.get_module_namespace(module_id).unwrap();

    let scope = &mut runtime.handle_scope();

    let module_namespace =
      v8::Local::<v8::Object>::new(scope, module_namespace);

    assert!(module_namespace.is_module_namespace_object());

    let unknown_export_name = v8::String::new(scope, "none").unwrap();
    let binding = module_namespace.get(scope, unknown_export_name.into());

    assert!(binding.is_some());
    assert!(binding.unwrap().is_undefined());

    let empty_export_name = v8::String::new(scope, "").unwrap();
    let binding = module_namespace.get(scope, empty_export_name.into());

    assert!(binding.is_some());
    assert!(binding.unwrap().is_undefined());

    let a_export_name = v8::String::new(scope, "a").unwrap();
    let binding = module_namespace.get(scope, a_export_name.into());

    assert!(binding.unwrap().is_string());
    assert_eq!(binding.unwrap(), v8::String::new(scope, "b").unwrap());

    let default_export_name = v8::String::new(scope, "default").unwrap();
    let binding = module_namespace.get(scope, default_export_name.into());

    assert!(binding.unwrap().is_number());
    assert_eq!(binding.unwrap(), v8::Number::new(scope, 3_f64));
  }

  #[test]
  fn test_heap_limits() {
    let create_params =
      v8::Isolate::create_params().heap_limits(0, 3 * 1024 * 1024);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      create_params: Some(create_params),
      ..Default::default()
    });
    let cb_handle = runtime.v8_isolate().thread_safe_handle();

    let callback_invoke_count = Rc::new(AtomicUsize::new(0));
    let inner_invoke_count = Rc::clone(&callback_invoke_count);

    runtime.add_near_heap_limit_callback(
      move |current_limit, _initial_limit| {
        inner_invoke_count.fetch_add(1, Ordering::SeqCst);
        cb_handle.terminate_execution();
        current_limit * 2
      },
    );
    let err = runtime
      .execute_script(
        "script name",
        r#"let s = ""; while(true) { s += "Hello"; }"#,
      )
      .expect_err("script should fail");
    assert_eq!(
      "Uncaught Error: execution terminated",
      err.downcast::<JsError>().unwrap().exception_message
    );
    assert!(callback_invoke_count.load(Ordering::SeqCst) > 0)
  }

  #[test]
  fn test_heap_limit_cb_remove() {
    let mut runtime = JsRuntime::new(Default::default());

    runtime.add_near_heap_limit_callback(|current_limit, _initial_limit| {
      current_limit * 2
    });
    runtime.remove_near_heap_limit_callback(3 * 1024 * 1024);
    assert!(runtime.allocations.near_heap_limit_callback_data.is_none());
  }

  #[test]
  fn test_heap_limit_cb_multiple() {
    let create_params =
      v8::Isolate::create_params().heap_limits(0, 3 * 1024 * 1024);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      create_params: Some(create_params),
      ..Default::default()
    });
    let cb_handle = runtime.v8_isolate().thread_safe_handle();

    let callback_invoke_count_first = Rc::new(AtomicUsize::new(0));
    let inner_invoke_count_first = Rc::clone(&callback_invoke_count_first);
    runtime.add_near_heap_limit_callback(
      move |current_limit, _initial_limit| {
        inner_invoke_count_first.fetch_add(1, Ordering::SeqCst);
        current_limit * 2
      },
    );

    let callback_invoke_count_second = Rc::new(AtomicUsize::new(0));
    let inner_invoke_count_second = Rc::clone(&callback_invoke_count_second);
    runtime.add_near_heap_limit_callback(
      move |current_limit, _initial_limit| {
        inner_invoke_count_second.fetch_add(1, Ordering::SeqCst);
        cb_handle.terminate_execution();
        current_limit * 2
      },
    );

    let err = runtime
      .execute_script(
        "script name",
        r#"let s = ""; while(true) { s += "Hello"; }"#,
      )
      .expect_err("script should fail");
    assert_eq!(
      "Uncaught Error: execution terminated",
      err.downcast::<JsError>().unwrap().exception_message
    );
    assert_eq!(0, callback_invoke_count_first.load(Ordering::SeqCst));
    assert!(callback_invoke_count_second.load(Ordering::SeqCst) > 0);
  }

  #[test]
  fn es_snapshot() {
    #[derive(Default)]
    struct ModsLoader;

    impl ModuleLoader for ModsLoader {
      fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
      ) -> Result<ModuleSpecifier, Error> {
        assert_eq!(specifier, "file:///main.js");
        assert_eq!(referrer, ".");
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
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
      runtime.load_main_module(&specifier, Some(source_code)),
    )
    .unwrap();

    let _ = runtime.mod_evaluate(module_id);
    futures::executor::block_on(runtime.run_event_loop(false)).unwrap();

    let _snapshot = runtime.snapshot();
  }

  #[test]
  fn test_error_without_stack() {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    // SyntaxError
    let result = runtime.execute_script(
      "error_without_stack.js",
      r#"
function main() {
  console.log("asdf);
}

main();
"#,
    );
    let expected_error = r#"Uncaught SyntaxError: Invalid or unexpected token
    at error_without_stack.js:3:15"#;
    assert_eq!(result.unwrap_err().to_string(), expected_error);
  }

  #[test]
  fn test_error_stack() {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    let result = runtime.execute_script(
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
        .execute_script(
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

      match runtime.poll_event_loop(cx, false) {
        Poll::Ready(Err(e)) => {
          assert_eq!(e.to_string(), expected_error);
        }
        _ => panic!(),
      };
    })
  }

  #[test]
  fn test_pump_message_loop() {
    run_in_task(|cx| {
      let mut runtime = JsRuntime::new(RuntimeOptions::default());
      runtime
        .execute_script(
          "pump_message_loop.js",
          r#"
function assertEquals(a, b) {
  if (a === b) return;
  throw a + " does not equal " + b;
}
const sab = new SharedArrayBuffer(16);
const i32a = new Int32Array(sab);
globalThis.resolved = false;

(function() {
  const result = Atomics.waitAsync(i32a, 0, 0);
  result.value.then(
    (value) => { assertEquals("ok", value); globalThis.resolved = true; },
    () => { assertUnreachable();
  });
})();

const notify_return_value = Atomics.notify(i32a, 0, 1);
assertEquals(1, notify_return_value);
"#,
        )
        .unwrap();

      match runtime.poll_event_loop(cx, false) {
        Poll::Ready(Ok(())) => {}
        _ => panic!(),
      };

      // noop script, will resolve promise from first script
      runtime
        .execute_script("pump_message_loop2.js", r#"assertEquals(1, 1);"#)
        .unwrap();

      // check that promise from `Atomics.waitAsync` has been resolved
      runtime
        .execute_script(
          "pump_message_loop3.js",
          r#"assertEquals(globalThis.resolved, true);"#,
        )
        .unwrap();
    })
  }

  #[test]
  fn test_core_js_stack_frame() {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    // Call non-existent op so we get error from `core.js`
    let error = runtime
      .execute_script(
        "core_js_stack_frame.js",
        "Deno.core.opAsync('non_existent');",
      )
      .unwrap_err();
    let error_string = error.to_string();
    // Test that the script specifier is a URL: `deno:<repo-relative path>`.
    assert!(error_string.contains("deno:core/01_core.js"));
  }

  #[test]
  fn test_v8_platform() {
    let options = RuntimeOptions {
      v8_platform: Some(v8::new_default_platform(0, false).make_shared()),
      ..Default::default()
    };
    let mut runtime = JsRuntime::new(options);
    runtime.execute_script("<none>", "").unwrap();
  }

  #[ignore] // TODO(@littledivy): Fast API ops when snapshot is not loaded.
  #[test]
  fn test_is_proxy() {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    let all_true: v8::Global<v8::Value> = runtime
      .execute_script(
        "is_proxy.js",
        r#"
      (function () {
        const o = { a: 1, b: 2};
        const p = new Proxy(o, {});
        return Deno.core.ops.op_is_proxy(p) && !Deno.core.ops.op_is_proxy(o) && !Deno.core.ops.op_is_proxy(42);
      })()
    "#,
      )
      .unwrap();
    let mut scope = runtime.handle_scope();
    let all_true = v8::Local::<v8::Value>::new(&mut scope, &all_true);
    assert!(all_true.is_true());
  }

  #[tokio::test]
  async fn test_async_opstate_borrow() {
    struct InnerState(u64);

    #[op]
    async fn op_async_borrow(
      op_state: Rc<RefCell<OpState>>,
    ) -> Result<(), Error> {
      let n = {
        let op_state = op_state.borrow();
        let inner_state = op_state.borrow::<InnerState>();
        inner_state.0
      };
      // Future must be Poll::Pending on first call
      tokio::time::sleep(std::time::Duration::from_millis(1)).await;
      if n != 42 {
        unreachable!();
      }
      Ok(())
    }

    let extension = Extension::builder()
      .ops(vec![op_async_borrow::decl()])
      .state(|state| {
        state.put(InnerState(42));
        Ok(())
      })
      .build();

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![extension],
      ..Default::default()
    });

    runtime
      .execute_script(
        "op_async_borrow.js",
        "Deno.core.opAsync('op_async_borrow')",
      )
      .unwrap();
    runtime.run_event_loop(false).await.unwrap();
  }

  #[tokio::test]
  async fn test_sync_op_serialize_object_with_numbers_as_keys() {
    #[op]
    fn op_sync_serialize_object_with_numbers_as_keys(
      value: serde_json::Value,
    ) -> Result<(), Error> {
      assert_eq!(
        value.to_string(),
        r#"{"lines":{"100":{"unit":"m"},"200":{"unit":"cm"}}}"#
      );
      Ok(())
    }

    let extension = Extension::builder()
      .ops(vec![op_sync_serialize_object_with_numbers_as_keys::decl()])
      .build();

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![extension],
      ..Default::default()
    });

    runtime
      .execute_script(
        "op_sync_serialize_object_with_numbers_as_keys.js",
        r#"
Deno.core.ops.op_sync_serialize_object_with_numbers_as_keys({
  lines: {
    100: {
      unit: "m"
    },
    200: {
      unit: "cm"
    }
  }
})
"#,
      )
      .unwrap();
    runtime.run_event_loop(false).await.unwrap();
  }

  #[tokio::test]
  async fn test_async_op_serialize_object_with_numbers_as_keys() {
    #[op]
    async fn op_async_serialize_object_with_numbers_as_keys(
      value: serde_json::Value,
    ) -> Result<(), Error> {
      assert_eq!(
        value.to_string(),
        r#"{"lines":{"100":{"unit":"m"},"200":{"unit":"cm"}}}"#
      );
      Ok(())
    }

    let extension = Extension::builder()
      .ops(vec![op_async_serialize_object_with_numbers_as_keys::decl()])
      .build();

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![extension],
      ..Default::default()
    });

    runtime
      .execute_script(
        "op_async_serialize_object_with_numbers_as_keys.js",
        r#"
Deno.core.opAsync('op_async_serialize_object_with_numbers_as_keys', {
  lines: {
    100: {
      unit: "m"
    },
    200: {
      unit: "cm"
    }
  }
})
"#,
      )
      .unwrap();
    runtime.run_event_loop(false).await.unwrap();
  }

  #[tokio::test]
  async fn test_set_macrotask_callback_set_next_tick_callback() {
    #[op]
    async fn op_async_sleep() -> Result<(), Error> {
      // Future must be Poll::Pending on first call
      tokio::time::sleep(std::time::Duration::from_millis(1)).await;
      Ok(())
    }

    let extension = Extension::builder()
      .ops(vec![op_async_sleep::decl()])
      .build();

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![extension],
      ..Default::default()
    });

    runtime
      .execute_script(
        "macrotasks_and_nextticks.js",
        r#"
        (async function () {
          const results = [];
          Deno.core.ops.op_set_macrotask_callback(() => {
            results.push("macrotask");
            return true;
          });
          Deno.core.ops.op_set_next_tick_callback(() => {
            results.push("nextTick");
            Deno.core.ops.op_set_has_tick_scheduled(false);
          });

          Deno.core.ops.op_set_has_tick_scheduled(true);
          await Deno.core.opAsync('op_async_sleep');
          if (results[0] != "nextTick") {
            throw new Error(`expected nextTick, got: ${results[0]}`);
          }
          if (results[1] != "macrotask") {
            throw new Error(`expected macrotask, got: ${results[1]}`);
          }
        })();
        "#,
      )
      .unwrap();
    runtime.run_event_loop(false).await.unwrap();
  }

  #[tokio::test]
  async fn test_set_macrotask_callback_set_next_tick_callback_multiple() {
    let mut runtime = JsRuntime::new(Default::default());

    runtime
      .execute_script(
        "multiple_macrotasks_and_nextticks.js",
        r#"
        Deno.core.ops.op_set_macrotask_callback(() => { return true; });
        Deno.core.ops.op_set_macrotask_callback(() => { return true; });
        Deno.core.ops.op_set_next_tick_callback(() => {});
        Deno.core.ops.op_set_next_tick_callback(() => {});
        "#,
      )
      .unwrap();
    let isolate = runtime.v8_isolate();
    let state_rc = JsRuntime::state(isolate);
    let state = state_rc.borrow();
    assert_eq!(state.js_macrotask_cbs.len(), 2);
    assert_eq!(state.js_nexttick_cbs.len(), 2);
  }

  #[test]
  fn test_has_tick_scheduled() {
    use futures::task::ArcWake;

    static MACROTASK: AtomicUsize = AtomicUsize::new(0);
    static NEXT_TICK: AtomicUsize = AtomicUsize::new(0);

    #[op]
    fn op_macrotask() -> Result<(), AnyError> {
      MACROTASK.fetch_add(1, Ordering::Relaxed);
      Ok(())
    }

    #[op]
    fn op_next_tick() -> Result<(), AnyError> {
      NEXT_TICK.fetch_add(1, Ordering::Relaxed);
      Ok(())
    }

    let extension = Extension::builder()
      .ops(vec![op_macrotask::decl(), op_next_tick::decl()])
      .build();

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![extension],
      ..Default::default()
    });

    runtime
      .execute_script(
        "has_tick_scheduled.js",
        r#"
          Deno.core.ops.op_set_macrotask_callback(() => {
            Deno.core.ops.op_macrotask();
            return true; // We're done.
          });
          Deno.core.ops.op_set_next_tick_callback(() => Deno.core.ops.op_next_tick());
          Deno.core.ops.op_set_has_tick_scheduled(true);
          "#,
      )
      .unwrap();

    struct ArcWakeImpl(Arc<AtomicUsize>);
    impl ArcWake for ArcWakeImpl {
      fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.0.fetch_add(1, Ordering::Relaxed);
      }
    }

    let awoken_times = Arc::new(AtomicUsize::new(0));
    let waker =
      futures::task::waker(Arc::new(ArcWakeImpl(awoken_times.clone())));
    let cx = &mut Context::from_waker(&waker);

    assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));
    assert_eq!(1, MACROTASK.load(Ordering::Relaxed));
    assert_eq!(1, NEXT_TICK.load(Ordering::Relaxed));
    assert_eq!(awoken_times.swap(0, Ordering::Relaxed), 1);
    assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));
    assert_eq!(awoken_times.swap(0, Ordering::Relaxed), 1);
    assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));
    assert_eq!(awoken_times.swap(0, Ordering::Relaxed), 1);
    assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));
    assert_eq!(awoken_times.swap(0, Ordering::Relaxed), 1);

    let state_rc = JsRuntime::state(runtime.v8_isolate());
    state_rc.borrow_mut().has_tick_scheduled = false;
    assert!(matches!(
      runtime.poll_event_loop(cx, false),
      Poll::Ready(Ok(()))
    ));
    assert_eq!(awoken_times.load(Ordering::Relaxed), 0);
    assert!(matches!(
      runtime.poll_event_loop(cx, false),
      Poll::Ready(Ok(()))
    ));
    assert_eq!(awoken_times.load(Ordering::Relaxed), 0);
  }

  #[test]
  fn terminate_during_module_eval() {
    #[derive(Default)]
    struct ModsLoader;

    impl ModuleLoader for ModsLoader {
      fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
      ) -> Result<ModuleSpecifier, Error> {
        assert_eq!(specifier, "file:///main.js");
        assert_eq!(referrer, ".");
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        async move {
          Ok(ModuleSource {
            code: b"console.log('hello world');".to_vec().into_boxed_slice(),
            module_url_specified: "file:///main.js".to_string(),
            module_url_found: "file:///main.js".to_string(),
            module_type: ModuleType::JavaScript,
          })
        }
        .boxed_local()
      }
    }

    let loader = std::rc::Rc::new(ModsLoader::default());
    let mut runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });

    let specifier = crate::resolve_url("file:///main.js").unwrap();
    let source_code = "Deno.core.print('hello\\n')".to_string();

    let module_id = futures::executor::block_on(
      runtime.load_main_module(&specifier, Some(source_code)),
    )
    .unwrap();

    runtime.v8_isolate().terminate_execution();

    let mod_result =
      futures::executor::block_on(runtime.mod_evaluate(module_id)).unwrap();
    assert!(mod_result
      .unwrap_err()
      .to_string()
      .contains("JavaScript execution has been terminated"));
  }

  #[tokio::test]
  async fn test_set_promise_reject_callback() {
    static PROMISE_REJECT: AtomicUsize = AtomicUsize::new(0);

    #[op]
    fn op_promise_reject() -> Result<(), AnyError> {
      PROMISE_REJECT.fetch_add(1, Ordering::Relaxed);
      Ok(())
    }

    let extension = Extension::builder()
      .ops(vec![op_promise_reject::decl()])
      .build();

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![extension],
      ..Default::default()
    });

    runtime
      .execute_script(
        "promise_reject_callback.js",
        r#"
        // Note: |promise| is not the promise created below, it's a child.
        Deno.core.ops.op_set_promise_reject_callback((type, promise, reason) => {
          if (type !== /* PromiseRejectWithNoHandler */ 0) {
            throw Error("unexpected type: " + type);
          }
          if (reason.message !== "reject") {
            throw Error("unexpected reason: " + reason);
          }
          Deno.core.ops.op_store_pending_promise_exception(promise);
          Deno.core.ops.op_promise_reject();
        });

        new Promise((_, reject) => reject(Error("reject")));
        "#,
      )
      .unwrap();
    runtime.run_event_loop(false).await.unwrap_err();

    assert_eq!(1, PROMISE_REJECT.load(Ordering::Relaxed));

    runtime
      .execute_script(
        "promise_reject_callback.js",
        r#"
        {
          const prev = Deno.core.ops.op_set_promise_reject_callback((...args) => {
            prev(...args);
          });
        }

        new Promise((_, reject) => reject(Error("reject")));
        "#,
      )
      .unwrap();
    runtime.run_event_loop(false).await.unwrap_err();

    assert_eq!(2, PROMISE_REJECT.load(Ordering::Relaxed));
  }

  #[tokio::test]
  async fn test_set_promise_reject_callback_realms() {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    let global_realm = runtime.global_realm();
    let realm1 = runtime.create_realm().unwrap();
    let realm2 = runtime.create_realm().unwrap();

    let realm_expectations = &[
      (&global_realm, "global_realm", 42),
      (&realm1, "realm1", 140),
      (&realm2, "realm2", 720),
    ];

    // Set up promise reject callbacks.
    for (realm, realm_name, number) in realm_expectations {
      realm
        .execute_script(
          runtime.v8_isolate(),
          "",
          &format!(
            r#"
              globalThis.rejectValue = undefined;
              Deno.core.setPromiseRejectCallback((_type, _promise, reason) => {{
                globalThis.rejectValue = `{realm_name}/${{reason}}`;
              }});
              Deno.core.opAsync("op_void_async").then(() => Promise.reject({number}));
            "#,
            realm_name=realm_name,
            number=number
          ),
        )
        .unwrap();
    }

    runtime.run_event_loop(false).await.unwrap();

    for (realm, realm_name, number) in realm_expectations {
      let reject_value = realm
        .execute_script(runtime.v8_isolate(), "", "globalThis.rejectValue")
        .unwrap();
      let scope = &mut realm.handle_scope(runtime.v8_isolate());
      let reject_value = v8::Local::new(scope, reject_value);
      assert!(reject_value.is_string());
      let reject_value_string = reject_value.to_rust_string_lossy(scope);
      assert_eq!(reject_value_string, format!("{}/{}", realm_name, number));
    }
  }

  #[tokio::test]
  async fn test_set_promise_reject_callback_top_level_await() {
    static PROMISE_REJECT: AtomicUsize = AtomicUsize::new(0);

    #[op]
    fn op_promise_reject() -> Result<(), AnyError> {
      PROMISE_REJECT.fetch_add(1, Ordering::Relaxed);
      Ok(())
    }

    let extension = Extension::builder()
      .ops(vec![op_promise_reject::decl()])
      .build();

    #[derive(Default)]
    struct ModsLoader;

    impl ModuleLoader for ModsLoader {
      fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
      ) -> Result<ModuleSpecifier, Error> {
        assert_eq!(specifier, "file:///main.js");
        assert_eq!(referrer, ".");
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        let source = r#"
        Deno.core.ops.op_set_promise_reject_callback((type, promise, reason) => {
          Deno.core.ops.op_promise_reject();
        });

        throw new Error('top level throw');
        "#;

        async move {
          Ok(ModuleSource {
            code: source.as_bytes().to_vec().into_boxed_slice(),
            module_url_specified: "file:///main.js".to_string(),
            module_url_found: "file:///main.js".to_string(),
            module_type: ModuleType::JavaScript,
          })
        }
        .boxed_local()
      }
    }

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![extension],
      module_loader: Some(Rc::new(ModsLoader)),
      ..Default::default()
    });

    let id = runtime
      .load_main_module(&crate::resolve_url("file:///main.js").unwrap(), None)
      .await
      .unwrap();
    let receiver = runtime.mod_evaluate(id);
    runtime.run_event_loop(false).await.unwrap();
    receiver.await.unwrap().unwrap_err();

    assert_eq!(1, PROMISE_REJECT.load(Ordering::Relaxed));
  }

  #[test]
  fn test_op_return_serde_v8_error() {
    #[op]
    fn op_err() -> Result<std::collections::BTreeMap<u64, u64>, anyhow::Error> {
      Ok([(1, 2), (3, 4)].into_iter().collect()) // Maps can't have non-string keys in serde_v8
    }

    let ext = Extension::builder().ops(vec![op_err::decl()]).build();
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![ext],
      ..Default::default()
    });
    assert!(runtime
      .execute_script(
        "test_op_return_serde_v8_error.js",
        "Deno.core.ops.op_err()"
      )
      .is_err());
  }

  #[test]
  fn test_op_high_arity() {
    #[op]
    fn op_add_4(
      x1: i64,
      x2: i64,
      x3: i64,
      x4: i64,
    ) -> Result<i64, anyhow::Error> {
      Ok(x1 + x2 + x3 + x4)
    }

    let ext = Extension::builder().ops(vec![op_add_4::decl()]).build();
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![ext],
      ..Default::default()
    });
    let r = runtime
      .execute_script("test.js", "Deno.core.ops.op_add_4(1, 2, 3, 4)")
      .unwrap();
    let scope = &mut runtime.handle_scope();
    assert_eq!(r.open(scope).integer_value(scope), Some(10));
  }

  #[test]
  fn test_op_disabled() {
    #[op]
    fn op_foo() -> Result<i64, anyhow::Error> {
      Ok(42)
    }

    let ext = Extension::builder()
      .ops(vec![op_foo::decl().disable()])
      .build();
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![ext],
      ..Default::default()
    });
    let r = runtime
      .execute_script("test.js", "Deno.core.ops.op_foo()")
      .unwrap();
    let scope = &mut runtime.handle_scope();
    assert!(r.open(scope).is_undefined());
  }

  #[test]
  fn test_op_detached_buffer() {
    use serde_v8::DetachedBuffer;

    #[op]
    fn op_sum_take(b: DetachedBuffer) -> Result<u64, anyhow::Error> {
      Ok(b.as_ref().iter().clone().map(|x| *x as u64).sum())
    }

    #[op]
    fn op_boomerang(
      b: DetachedBuffer,
    ) -> Result<DetachedBuffer, anyhow::Error> {
      Ok(b)
    }

    let ext = Extension::builder()
      .ops(vec![op_sum_take::decl(), op_boomerang::decl()])
      .build();

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![ext],
      ..Default::default()
    });

    runtime
      .execute_script(
        "test.js",
        r#"
        const a1 = new Uint8Array([1,2,3]);
        const a1b = a1.subarray(0, 3);
        const a2 = new Uint8Array([5,10,15]);
        const a2b = a2.subarray(0, 3);


        if (!(a1.length > 0 && a1b.length > 0)) {
          throw new Error("a1 & a1b should have a length");
        }
        let sum = Deno.core.ops.op_sum_take(a1b);
        if (sum !== 6) {
          throw new Error(`Bad sum: ${sum}`);
        }
        if (a1.length > 0 || a1b.length > 0) {
          throw new Error("expecting a1 & a1b to be detached");
        }

        const a3 = Deno.core.ops.op_boomerang(a2b);
        if (a3.byteLength != 3) {
          throw new Error(`Expected a3.byteLength === 3, got ${a3.byteLength}`);
        }
        if (a3[0] !== 5 || a3[1] !== 10) {
          throw new Error(`Invalid a3: ${a3[0]}, ${a3[1]}`);
        }
        if (a2.byteLength > 0 || a2b.byteLength > 0) {
          throw new Error("expecting a2 & a2b to be detached, a3 re-attached");
        }

        const wmem = new WebAssembly.Memory({ initial: 1, maximum: 2 });
        const w32 = new Uint32Array(wmem.buffer);
        w32[0] = 1; w32[1] = 2; w32[2] = 3;
        const assertWasmThrow = (() => {
          try {
            let sum = Deno.core.ops.op_sum_take(w32.subarray(0, 2));
            return false;
          } catch(e) {
            return e.message.includes('ExpectedDetachable');
          }
        });
        if (!assertWasmThrow()) {
          throw new Error("expected wasm mem to not be detachable");
        }
      "#,
      )
      .unwrap();
  }

  #[test]
  fn test_op_unstable_disabling() {
    #[op]
    fn op_foo() -> Result<i64, anyhow::Error> {
      Ok(42)
    }

    #[op(unstable)]
    fn op_bar() -> Result<i64, anyhow::Error> {
      Ok(42)
    }

    let ext = Extension::builder()
      .ops(vec![op_foo::decl(), op_bar::decl()])
      .middleware(|op| if op.is_unstable { op.disable() } else { op })
      .build();
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![ext],
      ..Default::default()
    });
    runtime
      .execute_script(
        "test.js",
        r#"
        if (Deno.core.ops.op_foo() !== 42) {
          throw new Error("Exptected op_foo() === 42");
        }
        if (Deno.core.ops.op_bar() !== undefined) {
          throw new Error("Expected op_bar to be disabled")
        }
      "#,
      )
      .unwrap();
  }

  #[test]
  fn js_realm_simple() {
    let mut runtime = JsRuntime::new(Default::default());
    let main_context = runtime.global_context();
    let main_global = {
      let scope = &mut runtime.handle_scope();
      let local_global = main_context.open(scope).global(scope);
      v8::Global::new(scope, local_global)
    };

    let realm = runtime.create_realm().unwrap();
    assert_ne!(realm.context(), &main_context);
    assert_ne!(realm.global_object(runtime.v8_isolate()), main_global);

    let main_object = runtime.execute_script("", "Object").unwrap();
    let realm_object = realm
      .execute_script(runtime.v8_isolate(), "", "Object")
      .unwrap();
    assert_ne!(main_object, realm_object);
  }

  #[test]
  fn js_realm_init() {
    #[op]
    fn op_test() -> Result<String, Error> {
      Ok(String::from("Test"))
    }

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![Extension::builder().ops(vec![op_test::decl()]).build()],
      ..Default::default()
    });
    let realm = runtime.create_realm().unwrap();
    let ret = realm
      .execute_script(runtime.v8_isolate(), "", "Deno.core.ops.op_test()")
      .unwrap();

    let scope = &mut realm.handle_scope(runtime.v8_isolate());
    assert_eq!(ret, serde_v8::to_v8(scope, "Test").unwrap());
  }

  #[test]
  fn js_realm_init_snapshot() {
    let snapshot = {
      let runtime = JsRuntime::new(RuntimeOptions {
        will_snapshot: true,
        ..Default::default()
      });
      let snap: &[u8] = &*runtime.snapshot();
      Vec::from(snap).into_boxed_slice()
    };

    #[op]
    fn op_test() -> Result<String, Error> {
      Ok(String::from("Test"))
    }

    let mut runtime = JsRuntime::new(RuntimeOptions {
      startup_snapshot: Some(Snapshot::Boxed(snapshot)),
      extensions: vec![Extension::builder().ops(vec![op_test::decl()]).build()],
      ..Default::default()
    });
    let realm = runtime.create_realm().unwrap();
    let ret = realm
      .execute_script(runtime.v8_isolate(), "", "Deno.core.ops.op_test()")
      .unwrap();

    let scope = &mut realm.handle_scope(runtime.v8_isolate());
    assert_eq!(ret, serde_v8::to_v8(scope, "Test").unwrap());
  }

  #[test]
  fn js_realm_sync_ops() {
    // Test that returning a ZeroCopyBuf and throwing an exception from a sync
    // op result in objects with prototypes from the right realm. Note that we
    // don't test the result of returning structs, because they will be
    // serialized to objects with null prototype.

    #[op]
    fn op_test(fail: bool) -> Result<ZeroCopyBuf, Error> {
      if !fail {
        Ok(ZeroCopyBuf::empty())
      } else {
        Err(crate::error::type_error("Test"))
      }
    }

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![Extension::builder().ops(vec![op_test::decl()]).build()],
      get_error_class_fn: Some(&|error| {
        crate::error::get_custom_error_class(error).unwrap()
      }),
      ..Default::default()
    });
    let new_realm = runtime.create_realm().unwrap();

    // Test in both realms
    for realm in [runtime.global_realm(), new_realm].into_iter() {
      let ret = realm
        .execute_script(
          runtime.v8_isolate(),
          "",
          r#"
            const buf = Deno.core.ops.op_test(false);
            let err;
            try {
              Deno.core.ops.op_test(true);
            } catch(e) {
              err = e;
            }
            buf instanceof Uint8Array && buf.byteLength === 0 &&
            err instanceof TypeError && err.message === "Test"
          "#,
        )
        .unwrap();
      assert!(ret.open(runtime.v8_isolate()).is_true());
    }
  }

  #[tokio::test]
  async fn js_realm_async_ops() {
    // Test that returning a ZeroCopyBuf and throwing an exception from a async
    // op result in objects with prototypes from the right realm. Note that we
    // don't test the result of returning structs, because they will be
    // serialized to objects with null prototype.

    #[op]
    async fn op_test(fail: bool) -> Result<ZeroCopyBuf, Error> {
      if !fail {
        Ok(ZeroCopyBuf::empty())
      } else {
        Err(crate::error::type_error("Test"))
      }
    }

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![Extension::builder().ops(vec![op_test::decl()]).build()],
      get_error_class_fn: Some(&|error| {
        crate::error::get_custom_error_class(error).unwrap()
      }),
      ..Default::default()
    });

    let global_realm = runtime.global_realm();
    let new_realm = runtime.create_realm().unwrap();

    let mut rets = vec![];

    // Test in both realms
    for realm in [global_realm, new_realm].into_iter() {
      let ret = realm
        .execute_script(
          runtime.v8_isolate(),
          "",
          r#"
              (async function () {
                const buf = await Deno.core.opAsync("op_test", false);
                let err;
                try {
                  await Deno.core.opAsync("op_test", true);
                } catch(e) {
                  err = e;
                }
                return buf instanceof Uint8Array && buf.byteLength === 0 &&
                       err instanceof TypeError && err.message === "Test" ;
              })();
            "#,
        )
        .unwrap();
      rets.push((realm, ret));
    }

    runtime.run_event_loop(false).await.unwrap();

    for ret in rets {
      let scope = &mut ret.0.handle_scope(runtime.v8_isolate());
      let value = v8::Local::new(scope, ret.1);
      let promise = v8::Local::<v8::Promise>::try_from(value).unwrap();
      let result = promise.result(scope);

      assert!(result.is_boolean() && result.is_true());
    }
  }

  #[tokio::test]
  async fn js_realm_ref_unref_ops() {
    run_in_task(|cx| {
      // Never resolves.
      #[op]
      async fn op_pending() {
        futures::future::pending().await
      }

      let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![Extension::builder()
          .ops(vec![op_pending::decl()])
          .build()],
        ..Default::default()
      });
      let main_realm = runtime.global_realm();
      let other_realm = runtime.create_realm().unwrap();

      main_realm
        .execute_script(
          runtime.v8_isolate(),
          "",
          "var promise = Deno.core.opAsync('op_pending');",
        )
        .unwrap();
      other_realm
        .execute_script(
          runtime.v8_isolate(),
          "",
          "var promise = Deno.core.opAsync('op_pending');",
        )
        .unwrap();
      assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));

      main_realm
        .execute_script(
          runtime.v8_isolate(),
          "",
          r#"
        let promiseIdSymbol = Symbol.for("Deno.core.internalPromiseId");
        Deno.core.ops.op_unref_op(promise[promiseIdSymbol]);
      "#,
        )
        .unwrap();
      assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));

      other_realm
        .execute_script(
          runtime.v8_isolate(),
          "",
          r#"
        let promiseIdSymbol = Symbol.for("Deno.core.internalPromiseId");
        Deno.core.ops.op_unref_op(promise[promiseIdSymbol]);
      "#,
        )
        .unwrap();
      assert!(matches!(
        runtime.poll_event_loop(cx, false),
        Poll::Ready(Ok(()))
      ));
    });
  }
}
