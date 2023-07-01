// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::bindings;
use super::jsrealm::JsRealmInner;
use super::snapshot_util;
use crate::error::exception_to_err_result;
use crate::error::generic_error;
use crate::error::to_v8_type_error;
use crate::error::GetErrorClassFn;
use crate::error::JsError;
use crate::extensions::OpDecl;
use crate::extensions::OpEventLoopFn;
use crate::include_js_files;
use crate::inspector::JsRuntimeInspector;
use crate::module_specifier::ModuleSpecifier;
use crate::modules::AssertedModuleType;
use crate::modules::ExtModuleLoader;
use crate::modules::ExtModuleLoaderCb;
use crate::modules::ModuleCode;
use crate::modules::ModuleError;
use crate::modules::ModuleId;
use crate::modules::ModuleLoadId;
use crate::modules::ModuleLoader;
use crate::modules::ModuleMap;
use crate::ops::*;
use crate::runtime::ContextState;
use crate::runtime::JsRealm;
use crate::source_map::SourceMapCache;
use crate::source_map::SourceMapGetter;
use crate::Extension;
use crate::ExtensionFileSource;
use crate::NoopModuleLoader;
use crate::OpMiddlewareFn;
use crate::OpResult;
use crate::OpState;
use crate::V8_WRAPPER_OBJECT_INDEX;
use crate::V8_WRAPPER_TYPE_INDEX;
use anyhow::Context as AnyhowContext;
use anyhow::Error;
use futures::channel::oneshot;
use futures::future::poll_fn;
use futures::stream::StreamExt;
use once_cell::sync::Lazy;
use smallvec::SmallVec;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ops::DerefMut;
use std::option::Option;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Once;
use std::task::Context;
use std::task::Poll;

const STATE_DATA_OFFSET: u32 = 0;
const MODULE_MAP_DATA_OFFSET: u32 = 1;

pub enum Snapshot {
  Static(&'static [u8]),
  JustCreated(v8::StartupData),
  Boxed(Box<[u8]>),
}

/// Objects that need to live as long as the isolate
#[derive(Default)]
pub(crate) struct IsolateAllocations {
  pub(crate) near_heap_limit_callback_data:
    Option<(Box<RefCell<dyn Any>>, v8::NearHeapLimitCallback)>,
}

/// ManuallyDrop<Rc<...>> is clone, but it returns a ManuallyDrop<Rc<...>> which is a massive
/// memory-leak footgun.
pub(crate) struct ManuallyDropRc<T>(ManuallyDrop<Rc<T>>);

impl<T> ManuallyDropRc<T> {
  pub fn clone(&self) -> Rc<T> {
    self.0.deref().clone()
  }
}

impl<T> Deref for ManuallyDropRc<T> {
  type Target = Rc<T>;
  fn deref(&self) -> &Self::Target {
    self.0.deref()
  }
}

impl<T> DerefMut for ManuallyDropRc<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.0.deref_mut()
  }
}

/// This struct contains the [`JsRuntimeState`] and [`v8::OwnedIsolate`] that are required
/// to do an orderly shutdown of V8. We keep these in a separate struct to allow us to control
/// the destruction more closely, as snapshots require the isolate to be destroyed by the
/// snapshot process, not the destructor.
///
/// The way rusty_v8 works w/snapshots is that the [`v8::OwnedIsolate`] gets consumed by a
/// [`v8::snapshot::SnapshotCreator`] that is stored in its annex. It's a bit awkward, because this
/// means we cannot let it drop (because we don't have it after a snapshot). On top of that, we have
/// to consume it in the snapshot creator because otherwise it panics.
///
/// This inner struct allows us to let the outer JsRuntime drop normally without a Drop impl, while we
/// control dropping more closely here using ManuallyDrop.
pub(crate) struct InnerIsolateState {
  will_snapshot: bool,
  pub(crate) state: ManuallyDropRc<RefCell<JsRuntimeState>>,
  v8_isolate: ManuallyDrop<v8::OwnedIsolate>,
}

impl InnerIsolateState {
  /// Clean out the opstate and take the inspector to prevent the inspector from getting destroyed
  /// after we've torn down the contexts. If the inspector is not correctly torn down, random crashes
  /// happen in tests (and possibly for users using the inspector).
  pub fn prepare_for_cleanup(&mut self) {
    let mut state = self.state.borrow_mut();
    let inspector = state.inspector.take();
    state.op_state.borrow_mut().clear();
    if let Some(inspector) = inspector {
      assert_eq!(
        Rc::strong_count(&inspector),
        1,
        "The inspector must be dropped before the runtime"
      );
    }
  }

  pub fn cleanup(&mut self) {
    self.prepare_for_cleanup();

    let state_ptr = self.v8_isolate.get_data(STATE_DATA_OFFSET);
    // SAFETY: We are sure that it's a valid pointer for whole lifetime of
    // the runtime.
    _ = unsafe { Rc::from_raw(state_ptr as *const RefCell<JsRuntimeState>) };

    let module_map_ptr = self.v8_isolate.get_data(MODULE_MAP_DATA_OFFSET);
    // SAFETY: We are sure that it's a valid pointer for whole lifetime of
    // the runtime.
    _ = unsafe { Rc::from_raw(module_map_ptr as *const RefCell<ModuleMap>) };

    self.state.borrow_mut().destroy_all_realms();

    debug_assert_eq!(Rc::strong_count(&self.state), 1);
  }

  pub fn prepare_for_snapshot(mut self) -> v8::OwnedIsolate {
    self.cleanup();
    // SAFETY: We're copying out of self and then immediately forgetting self
    let (state, isolate) = unsafe {
      (
        ManuallyDrop::take(&mut self.state.0),
        ManuallyDrop::take(&mut self.v8_isolate),
      )
    };
    std::mem::forget(self);
    drop(state);
    isolate
  }
}

impl Drop for InnerIsolateState {
  fn drop(&mut self) {
    self.cleanup();
    // SAFETY: We gotta drop these
    unsafe {
      ManuallyDrop::drop(&mut self.state.0);
      if self.will_snapshot {
        // Create the snapshot and just drop it.
        eprintln!("WARNING: v8::OwnedIsolate for snapshot was leaked");
      } else {
        ManuallyDrop::drop(&mut self.v8_isolate);
      }
    }
  }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum InitMode {
  /// We have no snapshot -- this is a pristine context.
  New,
  /// We are using a snapshot, thus certain initialization steps are skipped.
  FromSnapshot,
}

impl InitMode {
  fn from_options(options: &RuntimeOptions) -> Self {
    match options.startup_snapshot {
      None => Self::New,
      Some(_) => Self::FromSnapshot,
    }
  }
}

pub(crate) static BUILTIN_SOURCES: Lazy<Vec<ExtensionFileSource>> =
  Lazy::new(|| {
    include_js_files!(
      core
      "00_primordials.js",
      "01_core.js",
      "02_error.js",
    )
  });

/// A single execution context of JavaScript. Corresponds roughly to the "Web
/// Worker" concept in the DOM.
////
/// The JsRuntime future completes when there is an error or when all
/// pending ops have completed.
///
/// Use [`JsRuntimeForSnapshot`] to be able to create a snapshot.
pub struct JsRuntime {
  pub(crate) inner: InnerIsolateState,
  pub(crate) module_map: Rc<RefCell<ModuleMap>>,
  pub(crate) allocations: IsolateAllocations,
  extensions: Vec<Extension>,
  event_loop_middlewares: Vec<Box<OpEventLoopFn>>,
  init_mode: InitMode,
  // Marks if this is considered the top-level runtime. Used only be inspector.
  is_main: bool,
}

/// The runtime type used for snapshot creation.
pub struct JsRuntimeForSnapshot(JsRuntime);

impl Deref for JsRuntimeForSnapshot {
  type Target = JsRuntime;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for JsRuntimeForSnapshot {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
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

/// Internal state for JsRuntime which is stored in one of v8::Isolate's
/// embedder slots.
pub struct JsRuntimeState {
  global_realm: Option<JsRealm>,
  known_realms: Vec<JsRealmInner>,
  pub(crate) has_tick_scheduled: bool,
  pub(crate) pending_dyn_mod_evaluate: Vec<DynImportModEvaluate>,
  pub(crate) pending_mod_evaluate: Option<ModEvaluate>,
  /// A counter used to delay our dynamic import deadlock detection by one spin
  /// of the event loop.
  dyn_module_evaluate_idle_counter: u32,
  pub(crate) source_map_getter: Option<Rc<Box<dyn SourceMapGetter>>>,
  pub(crate) source_map_cache: Rc<RefCell<SourceMapCache>>,
  pub(crate) op_state: Rc<RefCell<OpState>>,
  pub(crate) shared_array_buffer_store: Option<SharedArrayBufferStore>,
  pub(crate) compiled_wasm_module_store: Option<CompiledWasmModuleStore>,
  /// The error that was passed to an `op_dispatch_exception` call.
  /// It will be retrieved by `exception_to_err_result` and used as an error
  /// instead of any other exceptions.
  // TODO(nayeemrmn): This is polled in `exception_to_err_result()` which is
  // flimsy. Try to poll it similarly to `pending_promise_rejections`.
  pub(crate) dispatched_exception: Option<v8::Global<v8::Value>>,
  pub(crate) inspector: Option<Rc<RefCell<JsRuntimeInspector>>>,
}

impl JsRuntimeState {
  pub(crate) fn destroy_all_realms(&mut self) {
    self.global_realm.take();
    for realm in self.known_realms.drain(..) {
      realm.destroy()
    }
  }

  pub(crate) fn remove_realm(
    &mut self,
    realm_context: &Rc<v8::Global<v8::Context>>,
  ) {
    self
      .known_realms
      .retain(|realm| !realm.is_same(realm_context));
  }
}

fn v8_init(
  v8_platform: Option<v8::SharedRef<v8::Platform>>,
  predictable: bool,
) {
  // Include 10MB ICU data file.
  #[repr(C, align(16))]
  struct IcuData([u8; 10541264]);
  static ICU_DATA: IcuData = IcuData(*include_bytes!("icudtl.dat"));
  v8::icu::set_common_data_72(&ICU_DATA.0).unwrap();

  let flags = concat!(
    " --wasm-test-streaming",
    " --harmony-import-assertions",
    " --no-validate-asm",
    " --turbo_fast_api_calls",
    " --harmony-change-array-by-copy",
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

  /// JsRuntime extensions, not to be confused with ES modules.
  /// Only ops registered by extensions will be initialized. If you need
  /// to execute JS code from extensions, pass source files in `js` or `esm`
  /// option on `ExtensionBuilder`.
  ///
  /// If you are creating a runtime from a snapshot take care not to include
  /// JavaScript sources in the extensions.
  pub extensions: Vec<Extension>,

  /// If provided, the module map will be cleared and left only with the specifiers
  /// in this list, with the new names provided. If not provided, the module map is
  /// left intact.
  pub rename_modules: Option<Vec<(&'static str, &'static str)>>,

  /// V8 snapshot that should be loaded on startup.
  pub startup_snapshot: Option<Snapshot>,

  /// Isolate creation parameters.
  pub create_params: Option<v8::CreateParams>,

  /// V8 platform instance to use. Used when Deno initializes V8
  /// (which it only does once), otherwise it's silently dropped.
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

  /// Start inspector instance to allow debuggers to connect.
  pub inspector: bool,

  /// Describe if this is the main runtime instance, used by debuggers in some
  /// situation - like disconnecting when program finishes running.
  pub is_main: bool,
}

#[derive(Default)]
pub struct RuntimeSnapshotOptions {
  /// An optional callback that will be called for each module that is loaded
  /// during snapshotting. This callback can be used to transpile source on the
  /// fly, during snapshotting, eg. to transpile TypeScript to JavaScript.
  pub snapshot_module_load_cb: Option<ExtModuleLoaderCb>,
}

impl JsRuntime {
  /// Only constructor, configuration is done through `options`.
  pub fn new(mut options: RuntimeOptions) -> JsRuntime {
    JsRuntime::init_v8(options.v8_platform.take(), cfg!(test));
    JsRuntime::new_inner(options, false, None)
  }

  pub(crate) fn state_from(
    isolate: &v8::Isolate,
  ) -> Rc<RefCell<JsRuntimeState>> {
    let state_ptr = isolate.get_data(STATE_DATA_OFFSET);
    let state_rc =
      // SAFETY: We are sure that it's a valid pointer for whole lifetime of
      // the runtime.
      unsafe { Rc::from_raw(state_ptr as *const RefCell<JsRuntimeState>) };
    let state = state_rc.clone();
    std::mem::forget(state_rc);
    state
  }

  pub(crate) fn module_map_from(
    isolate: &v8::Isolate,
  ) -> Rc<RefCell<ModuleMap>> {
    let module_map_ptr = isolate.get_data(MODULE_MAP_DATA_OFFSET);
    let module_map_rc =
      // SAFETY: We are sure that it's a valid pointer for whole lifetime of
      // the runtime.
      unsafe { Rc::from_raw(module_map_ptr as *const RefCell<ModuleMap>) };
    let module_map = module_map_rc.clone();
    std::mem::forget(module_map_rc);
    module_map
  }

  pub(crate) fn event_loop_pending_state_from_scope(
    scope: &mut v8::HandleScope,
  ) -> EventLoopPendingState {
    let state = JsRuntime::state_from(scope);
    let module_map = JsRuntime::module_map_from(scope);
    let state = EventLoopPendingState::new(
      scope,
      &mut state.borrow_mut(),
      &module_map.borrow(),
    );
    state
  }

  fn init_v8(
    v8_platform: Option<v8::SharedRef<v8::Platform>>,
    predictable: bool,
  ) {
    static DENO_INIT: Once = Once::new();
    static DENO_PREDICTABLE: AtomicBool = AtomicBool::new(false);
    static DENO_PREDICTABLE_SET: AtomicBool = AtomicBool::new(false);

    if DENO_PREDICTABLE_SET.load(Ordering::SeqCst) {
      let current = DENO_PREDICTABLE.load(Ordering::SeqCst);
      assert_eq!(current, predictable, "V8 may only be initialized once in either snapshotting or non-snapshotting mode. Either snapshotting or non-snapshotting mode may be used in a single process, not both.");
      DENO_PREDICTABLE_SET.store(true, Ordering::SeqCst);
      DENO_PREDICTABLE.store(predictable, Ordering::SeqCst);
    }

    DENO_INIT.call_once(move || v8_init(v8_platform, predictable));
  }

  fn new_inner(
    mut options: RuntimeOptions,
    will_snapshot: bool,
    maybe_load_callback: Option<ExtModuleLoaderCb>,
  ) -> JsRuntime {
    let init_mode = InitMode::from_options(&options);
    let (op_state, ops) = Self::create_opstate(&mut options);
    let op_state = Rc::new(RefCell::new(op_state));

    // Collect event-loop middleware
    let mut event_loop_middlewares =
      Vec::with_capacity(options.extensions.len());
    for extension in &mut options.extensions {
      if let Some(middleware) = extension.init_event_loop_middleware() {
        event_loop_middlewares.push(middleware);
      }
    }

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

    let state_rc = Rc::new(RefCell::new(JsRuntimeState {
      pending_dyn_mod_evaluate: vec![],
      pending_mod_evaluate: None,
      dyn_module_evaluate_idle_counter: 0,
      has_tick_scheduled: false,
      source_map_getter: options.source_map_getter.map(Rc::new),
      source_map_cache: Default::default(),
      shared_array_buffer_store: options.shared_array_buffer_store,
      compiled_wasm_module_store: options.compiled_wasm_module_store,
      op_state: op_state.clone(),
      dispatched_exception: None,
      // Some fields are initialized later after isolate is created
      inspector: None,
      global_realm: None,
      known_realms: Vec::with_capacity(1),
    }));

    let weak = Rc::downgrade(&state_rc);
    let context_state = Rc::new(RefCell::new(ContextState::default()));
    let op_ctxs = ops
      .into_iter()
      .enumerate()
      .map(|(id, decl)| {
        OpCtx::new(
          id as u16,
          context_state.clone(),
          Rc::new(decl),
          op_state.clone(),
          weak.clone(),
        )
      })
      .collect::<Vec<_>>()
      .into_boxed_slice();
    context_state.borrow_mut().op_ctxs = op_ctxs;
    context_state.borrow_mut().isolate = Some(isolate_ptr);

    let refs = bindings::external_references(&context_state.borrow().op_ctxs);
    // V8 takes ownership of external_references.
    let refs: &'static v8::ExternalReferences = Box::leak(Box::new(refs));

    let mut isolate = if will_snapshot {
      snapshot_util::create_snapshot_creator(
        refs,
        options.startup_snapshot.take(),
      )
    } else {
      let mut params = options
        .create_params
        .take()
        .unwrap_or_default()
        .embedder_wrapper_type_info_offsets(
          V8_WRAPPER_TYPE_INDEX,
          V8_WRAPPER_OBJECT_INDEX,
        )
        .external_references(&**refs);
      if let Some(snapshot) = options.startup_snapshot.take() {
        params = match snapshot {
          Snapshot::Static(data) => params.snapshot_blob(data),
          Snapshot::JustCreated(data) => params.snapshot_blob(data),
          Snapshot::Boxed(data) => params.snapshot_blob(data),
        };
      }
      v8::Isolate::new(params)
    };
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

    let (global_context, snapshotted_data) = {
      let scope = &mut v8::HandleScope::new(&mut isolate);
      let context = v8::Context::new(scope);

      // Get module map data from the snapshot
      let snapshotted_data = if init_mode == InitMode::FromSnapshot {
        Some(snapshot_util::get_snapshotted_data(scope, context))
      } else {
        None
      };

      (v8::Global::new(scope, context), snapshotted_data)
    };

    // SAFETY: this is first use of `isolate_ptr` so we are sure we're
    // not overwriting an existing pointer.
    isolate = unsafe {
      isolate_ptr.write(isolate);
      isolate_ptr.read()
    };

    let mut context_scope: v8::HandleScope =
      v8::HandleScope::with_context(&mut isolate, global_context.clone());
    let scope = &mut context_scope;
    let context = v8::Local::new(scope, global_context.clone());

    bindings::initialize_context(
      scope,
      context,
      &context_state.borrow().op_ctxs,
      init_mode,
    );

    context.set_slot(scope, context_state.clone());

    op_state.borrow_mut().put(isolate_ptr);
    let inspector = if options.inspector {
      Some(JsRuntimeInspector::new(scope, context, options.is_main))
    } else {
      None
    };

    let loader = options
      .module_loader
      .unwrap_or_else(|| Rc::new(NoopModuleLoader));

    {
      let global_realm = JsRealmInner::new(
        context_state,
        global_context,
        state_rc.clone(),
        true,
      );
      let mut state = state_rc.borrow_mut();
      state.global_realm = Some(JsRealm::new(global_realm.clone()));
      state.inspector = inspector;
      state.known_realms.push(global_realm);
    }
    scope.set_data(
      STATE_DATA_OFFSET,
      Rc::into_raw(state_rc.clone()) as *mut c_void,
    );
    let module_map_rc = Rc::new(RefCell::new(ModuleMap::new(loader)));
    if let Some(snapshotted_data) = snapshotted_data {
      let mut module_map = module_map_rc.borrow_mut();
      module_map.update_with_snapshotted_data(scope, snapshotted_data);
    }
    scope.set_data(
      MODULE_MAP_DATA_OFFSET,
      Rc::into_raw(module_map_rc.clone()) as *mut c_void,
    );

    drop(context_scope);

    let mut js_runtime = JsRuntime {
      inner: InnerIsolateState {
        will_snapshot,
        state: ManuallyDropRc(ManuallyDrop::new(state_rc)),
        v8_isolate: ManuallyDrop::new(isolate),
      },
      init_mode,
      allocations: IsolateAllocations::default(),
      event_loop_middlewares,
      extensions: options.extensions,
      module_map: module_map_rc,
      is_main: options.is_main,
    };

    let realm = js_runtime.global_realm();
    // TODO(mmastrac): We should thread errors back out of the runtime
    js_runtime
      .init_extension_js(&realm, maybe_load_callback)
      .unwrap();

    // If the user has requested that we rename modules
    if let Some(rename_modules) = options.rename_modules {
      js_runtime
        .module_map
        .borrow_mut()
        .clear_module_map(rename_modules.into_iter());
    }

    js_runtime
  }

  #[cfg(test)]
  #[inline]
  pub(crate) fn module_map(&self) -> &Rc<RefCell<ModuleMap>> {
    &self.module_map
  }

  #[inline]
  pub fn global_context(&self) -> v8::Global<v8::Context> {
    self
      .inner
      .state
      .borrow()
      .known_realms
      .get(0)
      .unwrap()
      .context()
      .clone()
  }

  #[inline]
  pub fn v8_isolate(&mut self) -> &mut v8::OwnedIsolate {
    &mut self.inner.v8_isolate
  }

  #[inline]
  pub fn inspector(&mut self) -> Rc<RefCell<JsRuntimeInspector>> {
    self.inner.state.borrow().inspector()
  }

  #[inline]
  pub fn global_realm(&mut self) -> JsRealm {
    let state = self.inner.state.borrow();
    state.global_realm.clone().unwrap()
  }

  /// Returns the extensions that this runtime is using (including internal ones).
  pub fn extensions(&self) -> &Vec<Extension> {
    &self.extensions
  }

  /// Creates a new realm (V8 context) in this JS execution context,
  /// pre-initialized with all of the extensions that were passed in
  /// [`RuntimeOptions::extensions`] when the [`JsRuntime`] was
  /// constructed.
  pub fn create_realm(&mut self) -> Result<JsRealm, Error> {
    let realm = {
      let context_state = Rc::new(RefCell::new(ContextState::default()));
      let op_ctxs: Box<[OpCtx]> = self
        .global_realm()
        .0
        .state()
        .borrow()
        .op_ctxs
        .iter()
        .map(|op_ctx| {
          OpCtx::new(
            op_ctx.id,
            context_state.clone(),
            op_ctx.decl.clone(),
            op_ctx.state.clone(),
            op_ctx.runtime_state.clone(),
          )
        })
        .collect();
      context_state.borrow_mut().op_ctxs = op_ctxs;
      context_state.borrow_mut().isolate = Some(self.v8_isolate() as _);

      let raw_ptr = self.v8_isolate() as *mut v8::OwnedIsolate;
      // SAFETY: Having the scope tied to self's lifetime makes it impossible to
      // reference JsRuntimeState::op_ctxs while the scope is alive. Here we
      // turn it into an unbound lifetime, which is sound because 1. it only
      // lives until the end of this block, and 2. the HandleScope only has
      // access to the isolate, and nothing else we're accessing from self does.
      let isolate = unsafe { raw_ptr.as_mut() }.unwrap();
      let scope = &mut v8::HandleScope::new(isolate);
      let context = v8::Context::new(scope);
      let scope = &mut v8::ContextScope::new(scope, context);

      let context = bindings::initialize_context(
        scope,
        context,
        &context_state.borrow().op_ctxs,
        self.init_mode,
      );
      context.set_slot(scope, context_state.clone());
      let realm = JsRealmInner::new(
        context_state,
        v8::Global::new(scope, context),
        self.inner.state.clone(),
        false,
      );
      let mut state = self.inner.state.borrow_mut();
      state.known_realms.push(realm.clone());
      JsRealm::new(realm)
    };

    self.init_extension_js(&realm, None)?;
    Ok(realm)
  }

  #[inline]
  pub fn handle_scope(&mut self) -> v8::HandleScope {
    self.global_realm().handle_scope(self.v8_isolate())
  }

  /// Initializes JS of provided Extensions in the given realm.
  fn init_extension_js(
    &mut self,
    realm: &JsRealm,
    maybe_load_callback: Option<ExtModuleLoaderCb>,
  ) -> Result<(), Error> {
    // Initialization of JS happens in phases:
    // 1. Iterate through all extensions:
    //  a. Execute all extension "script" JS files
    //  b. Load all extension "module" JS files (but do not execute them yet)
    // 2. Iterate through all extensions:
    //  a. If an extension has a `esm_entry_point`, execute it.

    // Take extensions temporarily so we can avoid have a mutable reference to self
    let extensions = std::mem::take(&mut self.extensions);

    // TODO(nayeemrmn): Module maps should be per-realm.
    let loader = self.module_map.borrow().loader.clone();
    let ext_loader = Rc::new(ExtModuleLoader::new(
      &extensions,
      maybe_load_callback.map(Rc::new),
    ));
    self.module_map.borrow_mut().loader = ext_loader;

    let mut esm_entrypoints = vec![];

    futures::executor::block_on(async {
      if self.init_mode == InitMode::New {
        for file_source in &*BUILTIN_SOURCES {
          realm.execute_script(
            self.v8_isolate(),
            file_source.specifier,
            file_source.load()?,
          )?;
        }
      }
      self.init_cbs(realm);

      for extension in &extensions {
        let maybe_esm_entry_point = extension.get_esm_entry_point();

        for file_source in extension.get_esm_sources() {
          self
            .load_side_module(
              &ModuleSpecifier::parse(file_source.specifier)?,
              None,
            )
            .await?;
        }

        if let Some(entry_point) = maybe_esm_entry_point {
          esm_entrypoints.push(entry_point);
        }

        for file_source in extension.get_js_sources() {
          realm.execute_script(
            self.v8_isolate(),
            file_source.specifier,
            file_source.load()?,
          )?;
        }
      }

      for specifier in esm_entrypoints {
        let mod_id = {
          self
            .module_map
            .borrow()
            .get_id(specifier, AssertedModuleType::JavaScriptOrWasm)
            .unwrap_or_else(|| {
              panic!("{} not present in the module map", specifier)
            })
        };
        let receiver = self.mod_evaluate(mod_id);
        self.run_event_loop(false).await?;
        receiver
          .await?
          .with_context(|| format!("Couldn't execute '{specifier}'"))?;
      }

      #[cfg(debug_assertions)]
      {
        let module_map_rc = self.module_map.clone();
        let mut scope = realm.handle_scope(self.v8_isolate());
        let module_map = module_map_rc.borrow();
        module_map.assert_all_modules_evaluated(&mut scope);
      }

      Ok::<_, anyhow::Error>(())
    })?;

    self.extensions = extensions;
    self.module_map.borrow_mut().loader = loader;
    Ok(())
  }

  /// Collects ops from extensions & applies middleware
  fn collect_ops(exts: &mut [Extension]) -> Vec<OpDecl> {
    for (ext, previous_exts) in
      exts.iter().enumerate().map(|(i, ext)| (ext, &exts[..i]))
    {
      ext.check_dependencies(previous_exts);
    }

    // Middleware
    let middleware: Vec<Box<OpMiddlewareFn>> = exts
      .iter_mut()
      .filter_map(|e| e.init_middleware())
      .collect();

    // macroware wraps an opfn in all the middleware
    let macroware = move |d| middleware.iter().fold(d, |d, m| m(d));

    // Flatten ops, apply middleware & override disabled ops
    let ops: Vec<_> = exts
      .iter_mut()
      .filter_map(|e| e.init_ops())
      .flatten()
      .map(|d| OpDecl {
        name: d.name,
        ..macroware(d)
      })
      .collect();

    // In debug build verify there are no duplicate ops.
    #[cfg(debug_assertions)]
    {
      let mut count_by_name = HashMap::new();

      for op in ops.iter() {
        count_by_name
          .entry(&op.name)
          .or_insert(vec![])
          .push(op.name.to_string());
      }

      let mut duplicate_ops = vec![];
      for (op_name, _count) in
        count_by_name.iter().filter(|(_k, v)| v.len() > 1)
      {
        duplicate_ops.push(op_name.to_string());
      }
      if !duplicate_ops.is_empty() {
        let mut msg = "Found ops with duplicate names:\n".to_string();
        for op_name in duplicate_ops {
          msg.push_str(&format!("  - {}\n", op_name));
        }
        msg.push_str("Op names need to be unique.");
        panic!("{}", msg);
      }
    }

    ops
  }

  /// Initializes ops of provided Extensions
  fn create_opstate(options: &mut RuntimeOptions) -> (OpState, Vec<OpDecl>) {
    // Add built-in extension
    options
      .extensions
      .insert(0, crate::ops_builtin::core::init_ops());

    let ops = Self::collect_ops(&mut options.extensions);

    let mut op_state = OpState::new(ops.len());

    if let Some(get_error_class_fn) = options.get_error_class_fn {
      op_state.get_error_class_fn = get_error_class_fn;
    }

    // Setup state
    for e in &mut options.extensions {
      // ops are already registered during in bindings::initialize_context();
      e.init_state(&mut op_state);
    }

    (op_state, ops)
  }

  pub fn eval<'s, T>(
    scope: &mut v8::HandleScope<'s>,
    code: &str,
  ) -> Option<v8::Local<'s, T>>
  where
    v8::Local<'s, T>: TryFrom<v8::Local<'s, v8::Value>, Error = v8::DataError>,
  {
    let scope = &mut v8::EscapableHandleScope::new(scope);
    let source = v8::String::new(scope, code).unwrap();
    let script = v8::Script::compile(scope, source, None).unwrap();
    let v = script.run(scope)?;
    scope.escape(v).try_into().ok()
  }

  /// Grabs a reference to core.js' eventLoopTick & buildCustomError
  fn init_cbs(&mut self, realm: &JsRealm) {
    let (event_loop_tick_cb, build_custom_error_cb) = {
      let scope = &mut realm.handle_scope(self.v8_isolate());
      let context = realm.context();
      let context_local = v8::Local::new(scope, context);
      let global = context_local.global(scope);
      let deno_str =
        v8::String::new_external_onebyte_static(scope, b"Deno").unwrap();
      let core_str =
        v8::String::new_external_onebyte_static(scope, b"core").unwrap();
      let event_loop_tick_str =
        v8::String::new_external_onebyte_static(scope, b"eventLoopTick")
          .unwrap();
      let build_custom_error_str =
        v8::String::new_external_onebyte_static(scope, b"buildCustomError")
          .unwrap();

      let deno_obj: v8::Local<v8::Object> = global
        .get(scope, deno_str.into())
        .unwrap()
        .try_into()
        .unwrap();
      let core_obj: v8::Local<v8::Object> = deno_obj
        .get(scope, core_str.into())
        .unwrap()
        .try_into()
        .unwrap();

      let event_loop_tick_cb: v8::Local<v8::Function> = core_obj
        .get(scope, event_loop_tick_str.into())
        .unwrap()
        .try_into()
        .unwrap();
      let build_custom_error_cb: v8::Local<v8::Function> = core_obj
        .get(scope, build_custom_error_str.into())
        .unwrap()
        .try_into()
        .unwrap();
      (
        v8::Global::new(scope, event_loop_tick_cb),
        v8::Global::new(scope, build_custom_error_cb),
      )
    };

    // Put global handles in the realm's ContextState
    let state_rc = realm.0.state();
    let mut state = state_rc.borrow_mut();
    state
      .js_event_loop_tick_cb
      .replace(Rc::new(event_loop_tick_cb));
    state
      .js_build_custom_error_cb
      .replace(Rc::new(build_custom_error_cb));
  }

  /// Returns the runtime's op state, which can be used to maintain ops
  /// and access resources between op calls.
  pub fn op_state(&mut self) -> Rc<RefCell<OpState>> {
    let state = self.inner.state.borrow();
    state.op_state.clone()
  }

  /// Executes traditional JavaScript code (traditional = not ES modules).
  ///
  /// The execution takes place on the current global context, so it is possible
  /// to maintain local JS state and invoke this method multiple times.
  ///
  /// `name` can be a filepath or any other string, but it is required to be 7-bit ASCII, eg.
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
    name: &'static str,
    source_code: ModuleCode,
  ) -> Result<v8::Global<v8::Value>, Error> {
    self
      .global_realm()
      .execute_script(self.v8_isolate(), name, source_code)
  }

  /// Executes traditional JavaScript code (traditional = not ES modules).
  ///
  /// The execution takes place on the current global context, so it is possible
  /// to maintain local JS state and invoke this method multiple times.
  ///
  /// `name` can be a filepath or any other string, but it is required to be 7-bit ASCII, eg.
  ///
  ///   - "/some/file/path.js"
  ///   - "<anon>"
  ///   - "[native code]"
  ///
  /// The same `name` value can be used for multiple executions.
  ///
  /// `Error` can usually be downcast to `JsError`.
  pub fn execute_script_static(
    &mut self,
    name: &'static str,
    source_code: &'static str,
  ) -> Result<v8::Global<v8::Value>, Error> {
    self.global_realm().execute_script(
      self.v8_isolate(),
      name,
      ModuleCode::from_static(source_code),
    )
  }

  /// Call a function. If it returns a promise, run the event loop until that
  /// promise is settled. If the promise rejects or there is an uncaught error
  /// in the event loop, return `Err(error)`. Or return `Ok(<await returned>)`.
  pub async fn call_and_await(
    &mut self,
    function: &v8::Global<v8::Function>,
  ) -> Result<v8::Global<v8::Value>, Error> {
    let promise = {
      let scope = &mut self.handle_scope();
      let cb = function.open(scope);
      let this = v8::undefined(scope).into();
      let promise = cb.call(scope, this, &[]);
      if promise.is_none() || scope.is_execution_terminating() {
        let undefined = v8::undefined(scope).into();
        return exception_to_err_result(scope, undefined, false);
      }
      v8::Global::new(scope, promise.unwrap())
    };
    self.resolve_value(promise).await
  }

  /// Returns the namespace object of a module.
  ///
  /// This is only available after module evaluation has completed.
  /// This function panics if module has not been instantiated.
  pub fn get_module_namespace(
    &mut self,
    module_id: ModuleId,
  ) -> Result<v8::Global<v8::Object>, Error> {
    self
      .module_map
      .clone()
      .borrow()
      .get_module_namespace(&mut self.handle_scope(), module_id)
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

  pub fn maybe_init_inspector(&mut self) {
    if self.inner.state.borrow().inspector.is_some() {
      return;
    }

    let context = self.global_context();
    let scope = &mut v8::HandleScope::with_context(
      self.inner.v8_isolate.as_mut(),
      context.clone(),
    );
    let context = v8::Local::new(scope, context);

    let mut state = self.inner.state.borrow_mut();
    state.inspector =
      Some(JsRuntimeInspector::new(scope, context, self.is_main));
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
    let has_inspector: bool;

    {
      let state = self.inner.state.borrow();
      has_inspector = state.inspector.is_some();
      state.op_state.borrow().waker.register(cx.waker());
    }

    if has_inspector {
      // We poll the inspector first.
      let _ = self.inspector().borrow().poll_sessions(Some(cx)).unwrap();
    }

    let module_map = self.module_map.clone();
    self.pump_v8_message_loop()?;

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
    }

    // Resolve async ops, run all next tick callbacks and macrotasks callbacks
    // and only then check for any promise exceptions (`unhandledrejection`
    // handlers are run in macrotasks callbacks so we need to let them run
    // first).
    self.do_js_event_loop_tick(cx)?;
    self.check_promise_rejections()?;

    // Event loop middlewares
    let mut maybe_scheduling = false;
    {
      let op_state = self.inner.state.borrow().op_state.clone();
      for f in &self.event_loop_middlewares {
        if f(op_state.clone(), cx) {
          maybe_scheduling = true;
        }
      }
    }

    // Top level module
    self.evaluate_pending_module();

    let pending_state = self.event_loop_pending_state();
    if !pending_state.is_pending() && !maybe_scheduling {
      if has_inspector {
        let inspector = self.inspector();
        let has_active_sessions = inspector.borrow().has_active_sessions();
        let has_blocking_sessions = inspector.borrow().has_blocking_sessions();

        if wait_for_inspector && has_active_sessions {
          // If there are no blocking sessions (eg. REPL) we can now notify
          // debugger that the program has finished running and we're ready
          // to exit the process once debugger disconnects.
          if !has_blocking_sessions {
            let context = self.global_context();
            let scope = &mut self.handle_scope();
            inspector.borrow_mut().context_destroyed(scope, context);
            println!("Program finished. Waiting for inspector to disconnect to exit the process...");
          }

          return Poll::Pending;
        }
      }

      return Poll::Ready(Ok(()));
    }

    let state = self.inner.state.borrow();

    // Check if more async ops have been dispatched
    // during this turn of event loop.
    // If there are any pending background tasks, we also wake the runtime to
    // make sure we don't miss them.
    // TODO(andreubotella) The event loop will spin as long as there are pending
    // background tasks. We should look into having V8 notify us when a
    // background task is done.
    if pending_state.has_pending_background_tasks
      || pending_state.has_tick_scheduled
      || maybe_scheduling
    {
      state.op_state.borrow().waker.wake();
    }

    drop(state);

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
        let scope = &mut self.handle_scope();
        let messages = module_map.borrow().find_stalled_top_level_await(scope);
        // We are gonna print only a single message to provide a nice formatting
        // with source line of offending promise shown. Once user fixed it, then
        // they will get another error message for the next promise (but this
        // situation is gonna be very rare, if ever happening).
        assert!(!messages.is_empty());
        let msg = v8::Local::new(scope, messages[0].clone());
        let js_error = JsError::from_v8_message(scope, msg);
        return Poll::Ready(Err(js_error.into()));
      }
    }

    if pending_state.has_pending_dyn_module_evaluation {
      if pending_state.has_pending_refed_ops
        || pending_state.has_pending_dyn_imports
        || pending_state.has_pending_background_tasks
        || pending_state.has_tick_scheduled
      {
        // pass, will be polled again
      } else if self.inner.state.borrow().dyn_module_evaluate_idle_counter >= 1
      {
        let scope = &mut self.handle_scope();
        let messages = module_map.borrow().find_stalled_top_level_await(scope);
        // We are gonna print only a single message to provide a nice formatting
        // with source line of offending promise shown. Once user fixed it, then
        // they will get another error message for the next promise (but this
        // situation is gonna be very rare, if ever happening).
        assert!(!messages.is_empty());
        let msg = v8::Local::new(scope, messages[0].clone());
        let js_error = JsError::from_v8_message(scope, msg);
        return Poll::Ready(Err(js_error.into()));
      } else {
        let mut state = self.inner.state.borrow_mut();
        // Delay the above error by one spin of the event loop. A dynamic import
        // evaluation may complete during this, in which case the counter will
        // reset.
        state.dyn_module_evaluate_idle_counter += 1;
        state.op_state.borrow().waker.wake();
      }
    }

    Poll::Pending
  }

  fn event_loop_pending_state(&mut self) -> EventLoopPendingState {
    let mut scope = v8::HandleScope::new(self.inner.v8_isolate.as_mut());
    EventLoopPendingState::new(
      &mut scope,
      &mut self.inner.state.borrow_mut(),
      &self.module_map.borrow(),
    )
  }
}

impl JsRuntimeForSnapshot {
  pub fn new(
    mut options: RuntimeOptions,
    runtime_snapshot_options: RuntimeSnapshotOptions,
  ) -> JsRuntimeForSnapshot {
    JsRuntime::init_v8(options.v8_platform.take(), true);
    JsRuntimeForSnapshot(JsRuntime::new_inner(
      options,
      true,
      runtime_snapshot_options.snapshot_module_load_cb,
    ))
  }

  /// Takes a snapshot and consumes the runtime.
  ///
  /// `Error` can usually be downcast to `JsError`.
  pub fn snapshot(mut self) -> v8::StartupData {
    // Ensure there are no live inspectors to prevent crashes.
    self.inner.prepare_for_cleanup();

    // Set the context to be snapshot's default context
    {
      let context = self.global_context();
      let mut scope = self.handle_scope();
      let local_context = v8::Local::new(&mut scope, context);
      scope.set_default_context(local_context);
    }

    // Serialize the module map and store its data in the snapshot.
    {
      let snapshotted_data = {
        // `self.module_map` points directly to the v8 isolate data slot, which
        // we must explicitly drop before destroying the isolate. We have to
        // take and drop this `Rc` before that.
        let module_map_rc = std::mem::take(&mut self.module_map);
        let module_map = module_map_rc.borrow();
        module_map.serialize_for_snapshotting(&mut self.handle_scope())
      };

      let context = self.global_context();
      let mut scope = self.handle_scope();
      snapshot_util::set_snapshotted_data(
        &mut scope,
        context,
        snapshotted_data,
      );
    }

    self
      .0
      .inner
      .prepare_for_snapshot()
      .create_blob(v8::FunctionCodeHandling::Keep)
      .unwrap()
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
  pub fn new(
    scope: &mut v8::HandleScope<()>,
    state: &mut JsRuntimeState,
    module_map: &ModuleMap,
  ) -> EventLoopPendingState {
    let mut num_unrefed_ops = 0;
    let mut num_pending_ops = 0;
    for realm in &state.known_realms {
      num_unrefed_ops += realm.num_unrefed_ops();
      num_pending_ops += realm.num_pending_ops();
    }

    EventLoopPendingState {
      has_pending_refed_ops: num_pending_ops > num_unrefed_ops,
      has_pending_dyn_imports: module_map.has_pending_dynamic_imports(),
      has_pending_dyn_module_evaluation: !state
        .pending_dyn_mod_evaluate
        .is_empty(),
      has_pending_module_evaluation: state.pending_mod_evaluate.is_some(),
      has_pending_background_tasks: scope.has_pending_background_tasks(),
      has_tick_scheduled: state.has_tick_scheduled,
    }
  }

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
    self.op_state.borrow().waker.wake();
  }
}

// Related to module loading
impl JsRuntime {
  pub(crate) fn instantiate_module(
    &mut self,
    id: ModuleId,
  ) -> Result<(), v8::Global<v8::Value>> {
    self
      .module_map
      .clone()
      .borrow_mut()
      .instantiate_module(&mut self.handle_scope(), id)
  }

  fn dynamic_import_module_evaluate(
    &mut self,
    load_id: ModuleLoadId,
    id: ModuleId,
  ) -> Result<(), Error> {
    let module_handle = self
      .module_map
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
    let global_realm =
      self.inner.state.borrow_mut().global_realm.clone().unwrap();
    let scope = &mut global_realm.handle_scope(&mut self.inner.v8_isolate);
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
      let promise_global = v8::Global::new(tc_scope, promise);
      let module_global = v8::Global::new(tc_scope, module);

      let dyn_import_mod_evaluate = DynImportModEvaluate {
        load_id,
        module_id: id,
        promise: promise_global,
        module: module_global,
      };

      self
        .inner
        .state
        .borrow_mut()
        .pending_dyn_mod_evaluate
        .push(dyn_import_mod_evaluate);
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
    let global_realm = self.global_realm();
    let state_rc = self.inner.state.clone();
    let module_map_rc = self.module_map.clone();
    let scope = &mut self.handle_scope();
    let tc_scope = &mut v8::TryCatch::new(scope);

    let module = module_map_rc
      .borrow()
      .get_handle(id)
      .map(|handle| v8::Local::new(tc_scope, handle))
      .expect("ModuleInfo not found");
    let mut status = module.get_status();
    assert_eq!(
      status,
      v8::ModuleStatus::Instantiated,
      "Module not instantiated {id}"
    );

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
      state_rc.borrow_mut().dispatched_exception.is_some();
    if has_dispatched_exception {
      // This will be overridden in `exception_to_err_result()`.
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
          global_realm
            .0
            .state()
            .borrow_mut()
            .pending_promise_rejections
            .retain(|(key, _)| key != &promise_global);
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
    let module_map_rc = self.module_map.clone();
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
    let state_rc = self.inner.state.clone();
    let module_map_rc = self.module_map.clone();
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
    if self
      .module_map
      .borrow()
      .preparing_dynamic_imports
      .is_empty()
    {
      return Poll::Ready(Ok(()));
    }

    loop {
      let poll_result = self
        .module_map
        .borrow_mut()
        .preparing_dynamic_imports
        .poll_next_unpin(cx);

      if let Poll::Ready(Some(prepare_poll)) = poll_result {
        let dyn_import_id = prepare_poll.0;
        let prepare_result = prepare_poll.1;

        match prepare_result {
          Ok(load) => {
            self
              .module_map
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
    if self.module_map.borrow().pending_dynamic_imports.is_empty() {
      return Poll::Ready(Ok(()));
    }

    loop {
      let poll_result = self
        .module_map
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
                info,
              );

              match register_result {
                Ok(()) => {
                  // Keep importing until it's fully drained
                  self
                    .module_map
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
    let maybe_module_evaluation =
      self.inner.state.borrow_mut().pending_mod_evaluate.take();

    if maybe_module_evaluation.is_none() {
      return;
    }

    let mut module_evaluation = maybe_module_evaluation.unwrap();
    let state_rc = self.inner.state.clone();
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
    let pending = std::mem::take(
      &mut self.inner.state.borrow_mut().pending_dyn_mod_evaluate,
    );
    if pending.is_empty() {
      return false;
    }
    let mut resolved_any = false;
    let mut still_pending = vec![];
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
    self.inner.state.borrow_mut().pending_dyn_mod_evaluate = still_pending;
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
    code: Option<ModuleCode>,
  ) -> Result<ModuleId, Error> {
    let module_map_rc = self.module_map.clone();
    if let Some(code) = code {
      let specifier = specifier.as_str().to_owned().into();
      let scope = &mut self.handle_scope();
      // true for main module
      module_map_rc
        .borrow_mut()
        .new_es_module(scope, true, specifier, code, false)
        .map_err(|e| match e {
          ModuleError::Exception(exception) => {
            let exception = v8::Local::new(scope, exception);
            exception_to_err_result::<()>(scope, exception, false).unwrap_err()
          }
          ModuleError::Other(error) => error,
        })?;
    }

    let mut load =
      ModuleMap::load_main(module_map_rc.clone(), &specifier).await?;

    while let Some(load_result) = load.next().await {
      let (request, info) = load_result?;
      let scope = &mut self.handle_scope();
      load.register_and_recurse(scope, &request, info).map_err(
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
    code: Option<ModuleCode>,
  ) -> Result<ModuleId, Error> {
    let module_map_rc = self.module_map.clone();
    if let Some(code) = code {
      let specifier = specifier.as_str().to_owned().into();
      let scope = &mut self.handle_scope();
      // false for side module (not main module)
      module_map_rc
        .borrow_mut()
        .new_es_module(scope, false, specifier, code, false)
        .map_err(|e| match e {
          ModuleError::Exception(exception) => {
            let exception = v8::Local::new(scope, exception);
            exception_to_err_result::<()>(scope, exception, false).unwrap_err()
          }
          ModuleError::Other(error) => error,
        })?;
    }

    let mut load =
      ModuleMap::load_side(module_map_rc.clone(), &specifier).await?;

    while let Some(load_result) = load.next().await {
      let (request, info) = load_result?;
      let scope = &mut self.handle_scope();
      load.register_and_recurse(scope, &request, info).map_err(
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

  fn check_promise_rejections(&mut self) -> Result<(), Error> {
    let state = self.inner.state.clone();
    let scope = &mut self.handle_scope();
    let state = state.borrow();
    for realm in &state.known_realms {
      realm.check_promise_rejections(scope)?;
    }
    Ok(())
  }

  // Polls pending ops and then runs `Deno.core.eventLoopTick` callback.
  fn do_js_event_loop_tick(&mut self, cx: &mut Context) -> Result<(), Error> {
    // Handle responses for each realm.
    let state = self.inner.state.clone();
    let isolate = &mut self.inner.v8_isolate;
    let realm_count = state.borrow().known_realms.len();
    for realm_idx in 0..realm_count {
      let realm = state.borrow().known_realms.get(realm_idx).unwrap().clone();
      let context_state = realm.state();
      let mut context_state = context_state.borrow_mut();
      let scope = &mut realm.handle_scope(isolate);

      // We return async responses to JS in unbounded batches (may change),
      // each batch is a flat vector of tuples:
      // `[promise_id1, op_result1, promise_id2, op_result2, ...]`
      // promise_id is a simple integer, op_result is an ops::OpResult
      // which contains a value OR an error, encoded as a tuple.
      // This batch is received in JS via the special `arguments` variable
      // and then each tuple is used to resolve or reject promises
      //
      // This can handle 15 promises futures in a single batch without heap
      // allocations.
      let mut args: SmallVec<[v8::Local<v8::Value>; 32]> =
        SmallVec::with_capacity(32);

      loop {
        let Poll::Ready(item) = context_state.pending_ops.poll_join_next(cx) else {
          break;
        };
        // TODO(mmastrac): If this task is really errored, things could be pretty bad
        let (promise_id, op_id, mut resp) = item.unwrap();
        state
          .borrow()
          .op_state
          .borrow()
          .tracker
          .track_async_completed(op_id);
        context_state.unrefed_ops.remove(&promise_id);
        args.push(v8::Integer::new(scope, promise_id).into());
        args.push(match resp.to_v8(scope) {
          Ok(v) => v,
          Err(e) => OpResult::Err(OpError::new(&|_| "TypeError", e.into()))
            .to_v8(scope)
            .unwrap(),
        });
      }

      let has_tick_scheduled =
        v8::Boolean::new(scope, self.inner.state.borrow().has_tick_scheduled);
      args.push(has_tick_scheduled.into());

      let js_event_loop_tick_cb_handle =
        context_state.js_event_loop_tick_cb.clone().unwrap();
      let tc_scope = &mut v8::TryCatch::new(scope);
      let js_event_loop_tick_cb = js_event_loop_tick_cb_handle.open(tc_scope);
      let this = v8::undefined(tc_scope).into();
      drop(context_state);
      js_event_loop_tick_cb.call(tc_scope, this, args.as_slice());

      if let Some(exception) = tc_scope.exception() {
        // TODO(@andreubotella): Returning here can cause async ops in other
        // realms to never resolve.
        return exception_to_err_result(tc_scope, exception, false);
      }

      if tc_scope.has_terminated() || tc_scope.is_execution_terminating() {
        return Ok(());
      }
    }

    Ok(())
  }
}
