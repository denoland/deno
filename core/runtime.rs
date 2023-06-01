// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::bindings;
use crate::bindings::BindingsMode;
use crate::error::generic_error;
use crate::error::to_v8_type_error;
use crate::error::JsError;
use crate::extensions::OpDecl;
use crate::extensions::OpEventLoopFn;
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
use crate::modules::ModuleName;
use crate::ops::*;
use crate::realm::ContextState;
use crate::realm::JsRealm;
use crate::realm::JsRealmInner;
use crate::snapshot_util;
use crate::snapshot_util::SnapshotOptions;
use crate::snapshot_util::SnapshottedData;
use crate::source_map::SourceMapCache;
use crate::source_map::SourceMapGetter;
use crate::Extension;
use crate::ModuleType;
use crate::NoopModuleLoader;
use crate::OpMiddlewareFn;
use crate::OpResult;
use crate::OpState;
use crate::PromiseId;
use anyhow::Context as AnyhowContext;
use anyhow::Error;
use futures::channel::oneshot;
use futures::future::poll_fn;
use futures::future::Future;
use futures::future::FutureExt;
use futures::future::MaybeDone;
use futures::stream::StreamExt;
use futures::task::noop_waker;
use futures::task::AtomicWaker;
use smallvec::SmallVec;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ops::DerefMut;
use std::option::Option;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Once;
use std::task::Context;
use std::task::Poll;
use v8::CreateParams;

const STATE_DATA_OFFSET: u32 = 0;
const MODULE_MAP_DATA_OFFSET: u32 = 1;

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

/// ManuallyDrop<Rc<...>> is clone, but it returns a ManuallyDrop<Rc<...>> which is a massive
/// memory-leak footgun.
struct ManuallyDropRc<T>(ManuallyDrop<Rc<T>>);

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
struct InnerIsolateState {
  snapshotting: bool,
  state: ManuallyDropRc<RefCell<JsRuntimeState>>,
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
      if self.snapshotting {
        // Create the snapshot and just drop it.
        eprintln!("WARNING: v8::OwnedIsolate for snapshot was leaked");
      } else {
        ManuallyDrop::drop(&mut self.v8_isolate);
      }
    }
  }
}

/// A single execution context of JavaScript. Corresponds roughly to the "Web
/// Worker" concept in the DOM.
////
/// The JsRuntimeImpl future completes when there is an error or when all
/// pending ops have completed.
///
/// API consumers will want to use either the [`JsRuntime`] or [`JsRuntimeForSnapshot`]
/// type aliases.
pub struct JsRuntimeImpl<const FOR_SNAPSHOT: bool = false> {
  inner: InnerIsolateState,
  module_map: Rc<RefCell<ModuleMap>>,
  allocations: IsolateAllocations,
  extensions: Vec<Extension>,
  event_loop_middlewares: Vec<Box<OpEventLoopFn>>,
  bindings_mode: BindingsMode,
  // Marks if this is considered the top-level runtime. Used only be inspector.
  is_main: bool,
}

/// The runtime type that most users will use when not creating a snapshot.
pub struct JsRuntime(JsRuntimeImpl<false>);

impl Deref for JsRuntime {
  type Target = JsRuntimeImpl<false>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for JsRuntime {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

/// The runtime type used for snapshot creation.
pub struct JsRuntimeForSnapshot(JsRuntimeImpl<true>);

impl Deref for JsRuntimeForSnapshot {
  type Target = JsRuntimeImpl<true>;

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

/// Internal state for JsRuntimeImpl which is stored in one of v8::Isolate's
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
  pub(crate) have_unpolled_ops: bool,
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
  waker: AtomicWaker,
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

  /// JsRuntimeImpl extensions, not to be confused with ES modules.
  /// Only ops registered by extensions will be initialized. If you need
  /// to execute JS code from extensions, pass source files in `js` or `esm`
  /// option on `ExtensionBuilder`.
  ///
  /// If you are creating a runtime from a snapshot take care not to include
  /// JavaScript sources in the extensions.
  pub extensions: Vec<Extension>,

  /// V8 snapshot that should be loaded on startup.
  pub startup_snapshot: Option<Snapshot>,

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

trait JsRuntimeInternalTrait {
  fn create_raw_isolate(
    refs: &'static v8::ExternalReferences,
    params: Option<CreateParams>,
    snapshot: SnapshotOptions,
  ) -> v8::OwnedIsolate;
}

impl<const FOR_SNAPSHOT: bool> JsRuntimeImpl<FOR_SNAPSHOT> {
  fn init_v8(v8_platform: Option<v8::SharedRef<v8::Platform>>) {
    static DENO_INIT: Once = Once::new();
    static DENO_PREDICTABLE: AtomicBool = AtomicBool::new(false);
    static DENO_PREDICTABLE_SET: AtomicBool = AtomicBool::new(false);

    let predictable = FOR_SNAPSHOT || cfg!(test);
    if DENO_PREDICTABLE_SET.load(Ordering::SeqCst) {
      let current = DENO_PREDICTABLE.load(Ordering::SeqCst);
      assert_eq!(current, predictable, "V8 may only be initialized once in either snapshotting or non-snapshotting mode. Either snapshotting or non-snapshotting mode may be used in a single process, not both.");
      DENO_PREDICTABLE_SET.store(true, Ordering::SeqCst);
      DENO_PREDICTABLE.store(predictable, Ordering::SeqCst);
    }

    DENO_INIT.call_once(move || v8_init(v8_platform, predictable));
  }

  fn new_runtime(
    mut options: RuntimeOptions,
    snapshot_options: SnapshotOptions,
    maybe_load_callback: Option<ExtModuleLoaderCb>,
  ) -> JsRuntimeImpl<FOR_SNAPSHOT>
  where
    JsRuntimeImpl<FOR_SNAPSHOT>: JsRuntimeInternalTrait,
  {
    let (op_state, ops) = Self::create_opstate(&mut options, &snapshot_options);
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
      waker: AtomicWaker::new(),
      have_unpolled_ops: false,
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

    let bindings_mode = match snapshot_options {
      SnapshotOptions::None => bindings::BindingsMode::New,
      SnapshotOptions::Create => bindings::BindingsMode::New,
      SnapshotOptions::Load(_) => bindings::BindingsMode::LoadedFinal,
      SnapshotOptions::CreateFromExisting(_) => bindings::BindingsMode::Loaded,
    };
    let snapshotting = snapshot_options.will_snapshot();

    let (mut isolate, global_context, snapshotted_data) = Self::create_isolate(
      refs,
      options.create_params.take(),
      snapshot_options,
    );
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
      bindings_mode,
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

    let mut js_runtime = JsRuntimeImpl {
      inner: InnerIsolateState {
        snapshotting,
        state: ManuallyDropRc(ManuallyDrop::new(state_rc)),
        v8_isolate: ManuallyDrop::new(isolate),
      },
      bindings_mode,
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
    js_runtime
  }

  /// Create a new [`v8::OwnedIsolate`] and its global [`v8::Context`] from optional parameters and snapshot.
  fn create_isolate(
    refs: &'static v8::ExternalReferences,
    params: Option<CreateParams>,
    snapshot: SnapshotOptions,
  ) -> (
    v8::OwnedIsolate,
    v8::Global<v8::Context>,
    Option<SnapshottedData>,
  )
  where
    JsRuntimeImpl<FOR_SNAPSHOT>: JsRuntimeInternalTrait,
  {
    let has_snapshot = snapshot.loaded();
    let mut isolate = Self::create_raw_isolate(refs, params, snapshot);

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

    let (context, snapshotted_data) = {
      let scope = &mut v8::HandleScope::new(&mut isolate);
      let context = v8::Context::new(scope);

      // Get module map data from the snapshot
      let snapshotted_data = if has_snapshot {
        Some(snapshot_util::get_snapshotted_data(scope, context))
      } else {
        None
      };

      (v8::Global::new(scope, context), snapshotted_data)
    };

    (isolate, context, snapshotted_data)
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
  /// [`RuntimeOptions::extensions`] when the [`JsRuntimeImpl`] was
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
        self.bindings_mode,
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
      for extension in &extensions {
        let maybe_esm_entry_point = extension.get_esm_entry_point();

        if let Some(esm_files) = extension.get_esm_sources() {
          for file_source in esm_files {
            self
              .load_side_module(
                &ModuleSpecifier::parse(file_source.specifier)?,
                None,
              )
              .await?;
          }
        }

        if let Some(entry_point) = maybe_esm_entry_point {
          esm_entrypoints.push(entry_point);
        }

        if let Some(js_files) = extension.get_js_sources() {
          for file_source in js_files {
            realm.execute_script(
              self.v8_isolate(),
              file_source.specifier,
              file_source.load()?,
            )?;
          }
        }

        if extension.is_core {
          self.init_cbs(realm);
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

    // Flatten ops, apply middlware & override disabled ops
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
  fn create_opstate(
    options: &mut RuntimeOptions,
    snapshot_options: &SnapshotOptions,
  ) -> (OpState, Vec<OpDecl>) {
    // Add built-in extension
    if snapshot_options.loaded() {
      options
        .extensions
        .insert(0, crate::ops_builtin::core::init_ops());
    } else {
      options
        .extensions
        .insert(0, crate::ops_builtin::core::init_ops_and_esm());
    }

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
    let module_handle = self
      .module_map
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
      state.waker.register(cx.waker());
    }

    if has_inspector {
      // We poll the inspector first.
      let _ = self.inspector().borrow().poll_sessions(Some(cx)).unwrap();
    }

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
    if state.have_unpolled_ops
      || pending_state.has_pending_background_tasks
      || pending_state.has_tick_scheduled
      || maybe_scheduling
    {
      state.waker.wake();
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
        let messages = find_stalled_top_level_await(scope);
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
        let messages = find_stalled_top_level_await(scope);
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
        state.waker.wake();
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

impl JsRuntime {
  /// Only constructor, configuration is done through `options`.
  pub fn new(mut options: RuntimeOptions) -> JsRuntime {
    JsRuntimeImpl::<false>::init_v8(options.v8_platform.take());

    let snapshot_options = snapshot_util::SnapshotOptions::new_from(
      options.startup_snapshot.take(),
      false,
    );

    JsRuntime(JsRuntimeImpl::<false>::new_runtime(
      options,
      snapshot_options,
      None,
    ))
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
}

impl JsRuntimeForSnapshot {
  pub fn new(
    mut options: RuntimeOptions,
    runtime_snapshot_options: RuntimeSnapshotOptions,
  ) -> JsRuntimeForSnapshot {
    JsRuntimeImpl::<true>::init_v8(options.v8_platform.take());

    let snapshot_options = snapshot_util::SnapshotOptions::new_from(
      options.startup_snapshot.take(),
      true,
    );

    JsRuntimeForSnapshot(JsRuntimeImpl::<true>::new_runtime(
      options,
      snapshot_options,
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

impl JsRuntimeInternalTrait for JsRuntimeImpl<true> {
  fn create_raw_isolate(
    refs: &'static v8::ExternalReferences,
    _params: Option<CreateParams>,
    snapshot: SnapshotOptions,
  ) -> v8::OwnedIsolate {
    snapshot_util::create_snapshot_creator(refs, snapshot)
  }
}

impl JsRuntimeInternalTrait for JsRuntimeImpl<false> {
  fn create_raw_isolate(
    refs: &'static v8::ExternalReferences,
    params: Option<CreateParams>,
    snapshot: SnapshotOptions,
  ) -> v8::OwnedIsolate {
    let mut params = params
      .unwrap_or_default()
      .embedder_wrapper_type_info_offsets(
        V8_WRAPPER_TYPE_INDEX,
        V8_WRAPPER_OBJECT_INDEX,
      )
      .external_references(&**refs);

    if let Some(snapshot) = snapshot.snapshot() {
      params = match snapshot {
        Snapshot::Static(data) => params.snapshot_blob(data),
        Snapshot::JustCreated(data) => params.snapshot_blob(data),
        Snapshot::Boxed(data) => params.snapshot_blob(data),
      };
    }

    v8::Isolate::new(params)
  }
}

fn get_stalled_top_level_await_message_for_module(
  scope: &mut v8::HandleScope,
  module_id: ModuleId,
) -> Vec<v8::Global<v8::Message>> {
  let module_map = JsRuntime::module_map_from(scope);
  let module_map = module_map.borrow();
  let module_handle = module_map.handles.get(module_id).unwrap();

  let module = v8::Local::new(scope, module_handle);
  let stalled = module.get_stalled_top_level_await_message(scope);
  let mut messages = vec![];
  for (_, message) in stalled {
    messages.push(v8::Global::new(scope, message));
  }
  messages
}

fn find_stalled_top_level_await(
  scope: &mut v8::HandleScope,
) -> Vec<v8::Global<v8::Message>> {
  let module_map = JsRuntime::module_map_from(scope);
  let module_map = module_map.borrow();

  // First check if that's root module
  let root_module_id = module_map
    .info
    .iter()
    .filter(|m| m.main)
    .map(|m| m.id)
    .next();

  if let Some(root_module_id) = root_module_id {
    let messages =
      get_stalled_top_level_await_message_for_module(scope, root_module_id);
    if !messages.is_empty() {
      return messages;
    }
  }

  // It wasn't a top module, so iterate over all modules and try to find
  // any with stalled top level await
  for module_id in 0..module_map.handles.len() {
    let messages =
      get_stalled_top_level_await_message_for_module(scope, module_id);
    if !messages.is_empty() {
      return messages;
    }
  }

  unreachable!()
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
  // in `JsRuntimeImpl::allocations` and thus is guaranteed to outlive the isolate.
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

pub(crate) fn exception_to_err_result<T>(
  scope: &mut v8::HandleScope,
  exception: v8::Local<v8::Value>,
  in_promise: bool,
) -> Result<T, Error> {
  let state_rc = JsRuntime::state_from(scope);

  let was_terminating_execution = scope.is_execution_terminating();
  // Disable running microtasks for a moment. When upgrading to V8 v11.4
  // we discovered that canceling termination here will cause the queued
  // microtasks to run which breaks some tests.
  scope.set_microtasks_policy(v8::MicrotasksPolicy::Explicit);
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
    exception = if let Some(exception) = &state.dispatched_exception {
      v8::Local::new(scope, exception.clone())
    } else if was_terminating_execution && exception.is_null_or_undefined() {
      let message = v8::String::new(scope, "execution terminated").unwrap();
      v8::Exception::error(scope, message)
    } else {
      exception
    };
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
  scope.set_microtasks_policy(v8::MicrotasksPolicy::Auto);

  Err(js_error.into())
}

// Related to module loading
impl<const FOR_SNAPSHOT: bool> JsRuntimeImpl<FOR_SNAPSHOT> {
  pub(crate) fn instantiate_module(
    &mut self,
    id: ModuleId,
  ) -> Result<(), v8::Global<v8::Value>> {
    let module_map_rc = self.module_map.clone();
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
  /// Implementors must manually call [`JsRuntimeImpl::run_event_loop`] to drive
  /// module evaluation future.
  ///
  /// `Error` can usually be downcast to `JsError` and should be awaited and
  /// checked after [`JsRuntimeImpl::run_event_loop`] completion.
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
    // the promise to pending promise rejection table - meaning JsRuntimeImpl will return
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

  /// Clear the module map, meant to be used after initializing extensions.
  /// Optionally pass a list of exceptions `(old_name, new_name)` representing
  /// specifiers which will be renamed and preserved in the module map.
  pub fn clear_module_map(
    &self,
    exceptions: impl Iterator<Item = (&'static str, &'static str)>,
  ) {
    let mut module_map = self.module_map.borrow_mut();
    let handles = exceptions
      .map(|(old_name, new_name)| {
        (module_map.get_handle_by_name(old_name).unwrap(), new_name)
      })
      .collect::<Vec<_>>();
    module_map.clear();
    for (handle, new_name) in handles {
      module_map.inject_handle(
        ModuleName::from_static(new_name),
        ModuleType::JavaScript,
        handle,
      )
    }
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
  /// User must call [`JsRuntimeImpl::mod_evaluate`] with returned `ModuleId`
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
  /// User must call [`JsRuntimeImpl::mod_evaluate`] with returned `ModuleId`
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
    // Now handle actual ops.
    {
      let mut state = self.inner.state.borrow_mut();
      state.have_unpolled_ops = false;
    }

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

      while let Poll::Ready(Some(item)) =
        context_state.pending_ops.poll_next_unpin(cx)
      {
        let (promise_id, op_id, mut resp) = item;
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

#[inline]
pub fn queue_fast_async_op<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  promise_id: PromiseId,
  op: impl Future<Output = Result<R, Error>> + 'static,
) {
  let runtime_state = match ctx.runtime_state.upgrade() {
    Some(rc_state) => rc_state,
    // at least 1 Rc is held by the JsRuntimeImpl.
    None => unreachable!(),
  };
  let get_class = {
    let state = RefCell::borrow(&ctx.state);
    state.tracker.track_async(ctx.id);
    state.get_error_class_fn
  };
  let fut = op
    .map(|result| crate::_ops::to_op_result(get_class, result))
    .boxed_local();
  let mut state = runtime_state.borrow_mut();
  ctx
    .context_state
    .borrow_mut()
    .pending_ops
    .push(OpCall::pending(ctx, promise_id, fut));
  state.have_unpolled_ops = true;
}

#[inline]
pub fn map_async_op1<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  op: impl Future<Output = Result<R, Error>> + 'static,
) -> MaybeDone<Pin<Box<dyn Future<Output = OpResult>>>> {
  let get_class = {
    let state = RefCell::borrow(&ctx.state);
    state.tracker.track_async(ctx.id);
    state.get_error_class_fn
  };

  let fut = op
    .map(|result| crate::_ops::to_op_result(get_class, result))
    .boxed_local();
  MaybeDone::Future(fut)
}

#[inline]
pub fn map_async_op2<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  op: impl Future<Output = R> + 'static,
) -> MaybeDone<Pin<Box<dyn Future<Output = OpResult>>>> {
  let state = RefCell::borrow(&ctx.state);
  state.tracker.track_async(ctx.id);

  let fut = op.map(|result| OpResult::Ok(result.into())).boxed_local();
  MaybeDone::Future(fut)
}

#[inline]
pub fn map_async_op3<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  op: Result<impl Future<Output = Result<R, Error>> + 'static, Error>,
) -> MaybeDone<Pin<Box<dyn Future<Output = OpResult>>>> {
  let get_class = {
    let state = RefCell::borrow(&ctx.state);
    state.tracker.track_async(ctx.id);
    state.get_error_class_fn
  };

  match op {
    Err(err) => MaybeDone::Done(OpResult::Err(OpError::new(get_class, err))),
    Ok(fut) => MaybeDone::Future(
      fut
        .map(|result| crate::_ops::to_op_result(get_class, result))
        .boxed_local(),
    ),
  }
}

#[inline]
pub fn map_async_op4<R: serde::Serialize + 'static>(
  ctx: &OpCtx,
  op: Result<impl Future<Output = R> + 'static, Error>,
) -> MaybeDone<Pin<Box<dyn Future<Output = OpResult>>>> {
  let get_class = {
    let state = RefCell::borrow(&ctx.state);
    state.tracker.track_async(ctx.id);
    state.get_error_class_fn
  };

  match op {
    Err(err) => MaybeDone::Done(OpResult::Err(OpError::new(get_class, err))),
    Ok(fut) => MaybeDone::Future(
      fut.map(|result| OpResult::Ok(result.into())).boxed_local(),
    ),
  }
}

pub fn queue_async_op<'s>(
  ctx: &OpCtx,
  scope: &'s mut v8::HandleScope,
  deferred: bool,
  promise_id: PromiseId,
  mut op: MaybeDone<Pin<Box<dyn Future<Output = OpResult>>>>,
) -> Option<v8::Local<'s, v8::Value>> {
  let runtime_state = match ctx.runtime_state.upgrade() {
    Some(rc_state) => rc_state,
    // at least 1 Rc is held by the JsRuntimeImpl.
    None => unreachable!(),
  };

  // An op's realm (as given by `OpCtx::realm_idx`) must match the realm in
  // which it is invoked. Otherwise, we might have cross-realm object exposure.
  // deno_core doesn't currently support such exposure, even though embedders
  // can cause them, so we panic in debug mode (since the check is expensive).
  // TODO(mmastrac): Restore this
  // debug_assert_eq!(
  //   runtime_state.borrow().context(ctx.realm_idx as usize, scope),
  //   Some(scope.get_current_context())
  // );

  // All ops are polled immediately
  let waker = noop_waker();
  let mut cx = Context::from_waker(&waker);

  // Note that MaybeDone returns () from the future
  let op_call = match op.poll_unpin(&mut cx) {
    Poll::Pending => {
      let MaybeDone::Future(fut) = op else {
        unreachable!()
      };
      OpCall::pending(ctx, promise_id, fut)
    }
    Poll::Ready(_) => {
      let mut op_result = Pin::new(&mut op).take_output().unwrap();
      // If the op is ready and is not marked as deferred we can immediately return
      // the result.
      if !deferred {
        ctx.state.borrow_mut().tracker.track_async_completed(ctx.id);
        return Some(op_result.to_v8(scope).unwrap());
      }

      OpCall::ready(ctx, promise_id, op_result)
    }
  };

  // Otherwise we will push it to the `pending_ops` and let it be polled again
  // or resolved on the next tick of the event loop.
  let mut state = runtime_state.borrow_mut();
  ctx.context_state.borrow_mut().pending_ops.push(op_call);
  state.have_unpolled_ops = true;
  None
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use crate::ascii_str;
  use crate::error::custom_error;
  use crate::error::AnyError;
  use crate::include_ascii_string;
  use crate::modules::AssertedModuleType;
  use crate::modules::ModuleInfo;
  use crate::modules::ModuleSource;
  use crate::modules::ModuleSourceFuture;
  use crate::modules::ModuleType;
  use crate::modules::ResolutionKind;
  use crate::modules::SymbolicModule;
  use crate::ZeroCopyBuf;
  use deno_ops::op;
  use std::pin::Pin;
  use std::rc::Rc;
  use std::sync::atomic::AtomicUsize;
  use std::sync::atomic::Ordering;
  use std::sync::Arc;

  // deno_ops macros generate code assuming deno_core in scope.
  mod deno_core {
    pub use crate::*;
  }

  #[derive(Copy, Clone)]
  pub enum Mode {
    Async,
    AsyncDeferred,
    AsyncZeroCopy(bool),
  }

  struct TestState {
    mode: Mode,
    dispatch_count: Arc<AtomicUsize>,
  }

  #[op]
  async fn op_test(
    rc_op_state: Rc<RefCell<OpState>>,
    control: u8,
    buf: Option<ZeroCopyBuf>,
  ) -> Result<u8, AnyError> {
    #![allow(clippy::await_holding_refcell_ref)] // False positive.
    let op_state_ = rc_op_state.borrow();
    let test_state = op_state_.borrow::<TestState>();
    test_state.dispatch_count.fetch_add(1, Ordering::Relaxed);
    let mode = test_state.mode;
    drop(op_state_);
    match mode {
      Mode::Async => {
        assert_eq!(control, 42);
        Ok(43)
      }
      Mode::AsyncDeferred => {
        tokio::task::yield_now().await;
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
    deno_core::extension!(
      test_ext,
      ops = [op_test],
      options = {
        mode: Mode,
        dispatch_count: Arc<AtomicUsize>,
      },
      state = |state, options| {
        state.put(TestState {
          mode: options.mode,
          dispatch_count: options.dispatch_count
        })
      }
    );
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops(mode, dispatch_count.clone())],
      get_error_class_fn: Some(&|error| {
        crate::error::get_custom_error_class(error).unwrap()
      }),
      ..Default::default()
    });

    runtime
      .execute_script_static(
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
    let (mut runtime, _dispatch_count) = setup(Mode::AsyncDeferred);
    runtime
      .execute_script_static(
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
      assert_eq!(realm.num_pending_ops(), 2);
      assert_eq!(realm.num_unrefed_ops(), 0);
    }
    runtime
      .execute_script_static(
        "filename.js",
        r#"
        Deno.core.ops.op_unref_op(p1[promiseIdSymbol]);
        Deno.core.ops.op_unref_op(p2[promiseIdSymbol]);
        "#,
      )
      .unwrap();
    {
      let realm = runtime.global_realm();
      assert_eq!(realm.num_pending_ops(), 2);
      assert_eq!(realm.num_unrefed_ops(), 2);
    }
    runtime
      .execute_script_static(
        "filename.js",
        r#"
        Deno.core.ops.op_ref_op(p1[promiseIdSymbol]);
        Deno.core.ops.op_ref_op(p2[promiseIdSymbol]);
        "#,
      )
      .unwrap();
    {
      let realm = runtime.global_realm();
      assert_eq!(realm.num_pending_ops(), 2);
      assert_eq!(realm.num_unrefed_ops(), 0);
    }
  }

  #[test]
  fn test_dispatch() {
    let (mut runtime, dispatch_count) = setup(Mode::Async);
    runtime
      .execute_script_static(
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
      .execute_script_static(
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
      .execute_script_static(
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
      .execute_script_static(
        "filename.js",
        r#"
        const { op_test } = Deno.core.generateAsyncOpHandler("op_test");
        let zero_copy_a = new Uint8Array([0]);
        op_test(null, zero_copy_a);
        "#,
      )
      .unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn test_execute_script_return_value() {
    let mut runtime = JsRuntime::new(Default::default());
    let value_global =
      runtime.execute_script_static("a.js", "a = 1 + 2").unwrap();
    {
      let scope = &mut runtime.handle_scope();
      let value = value_global.open(scope);
      assert_eq!(value.integer_value(scope).unwrap(), 3);
    }
    let value_global = runtime
      .execute_script_static("b.js", "b = 'foobar'")
      .unwrap();
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
    let mut runtime = JsRuntime::new(Default::default());
    poll_fn(move |cx| {
      let value_global = runtime
        .execute_script_static("a.js", "Promise.resolve(1 + 2)")
        .unwrap();
      let v = runtime.poll_value(&value_global, cx);
      {
        let scope = &mut runtime.handle_scope();
        assert!(
          matches!(v, Poll::Ready(Ok(v)) if v.open(scope).integer_value(scope).unwrap() == 3)
        );
      }

      let value_global = runtime
        .execute_script_static(
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
        .execute_script_static("a.js", "Promise.reject(new Error('fail'))")
        .unwrap();
      let v = runtime.poll_value(&value_global, cx);
      assert!(
        matches!(v, Poll::Ready(Err(e)) if e.downcast_ref::<JsError>().unwrap().exception_message == "Uncaught Error: fail")
      );

      let value_global = runtime
        .execute_script_static("a.js", "new Promise(resolve => {})")
        .unwrap();
      let v = runtime.poll_value(&value_global, cx);
      matches!(v, Poll::Ready(Err(e)) if e.to_string() == "Promise resolution is still pending but the event loop has already resolved.");
      Poll::Ready(())
    }).await;
  }

  #[tokio::test]
  async fn test_resolve_value() {
    let mut runtime = JsRuntime::new(Default::default());
    let value_global = runtime
      .execute_script_static("a.js", "Promise.resolve(1 + 2)")
      .unwrap();
    let result_global = runtime.resolve_value(value_global).await.unwrap();
    {
      let scope = &mut runtime.handle_scope();
      let value = result_global.open(scope);
      assert_eq!(value.integer_value(scope).unwrap(), 3);
    }

    let value_global = runtime
      .execute_script_static(
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
      .execute_script_static("a.js", "Promise.reject(new Error('fail'))")
      .unwrap();
    let err = runtime.resolve_value(value_global).await.unwrap_err();
    assert_eq!(
      "Uncaught Error: fail",
      err.downcast::<JsError>().unwrap().exception_message
    );

    let value_global = runtime
      .execute_script_static("a.js", "new Promise(resolve => {})")
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
    let promise = runtime.execute_script_static("infinite_wasm_loop.js",
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
      .execute_script_static(
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
      .execute_script_static("simple.js", "1 + 1")
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
    match isolate.execute_script_static("infinite_loop.js", "for(;;) {}") {
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
      .execute_script_static("simple.js", "1 + 1")
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
    let r = runtime.execute_script_static("i.js", src);
    let e = r.unwrap_err();
    let js_error = e.downcast::<JsError>().unwrap();
    let frame = js_error.frames.first().unwrap();
    assert_eq!(frame.column_number, Some(12));
  }

  #[tokio::test]
  async fn test_encode_decode() {
    let (mut runtime, _dispatch_count) = setup(Mode::Async);
    poll_fn(move |cx| {
      runtime
        .execute_script(
          "encode_decode_test.js",
          // Note: We make this to_owned because it contains non-ASCII chars
          include_str!("encode_decode_test.js").to_owned().into(),
        )
        .unwrap();
      if let Poll::Ready(Err(_)) = runtime.poll_event_loop(cx, false) {
        unreachable!();
      }
      Poll::Ready(())
    })
    .await;
  }

  #[tokio::test]
  async fn test_serialize_deserialize() {
    let (mut runtime, _dispatch_count) = setup(Mode::Async);
    poll_fn(move |cx| {
      runtime
        .execute_script(
          "serialize_deserialize_test.js",
          include_ascii_string!("serialize_deserialize_test.js"),
        )
        .unwrap();
      if let Poll::Ready(Err(_)) = runtime.poll_event_loop(cx, false) {
        unreachable!();
      }
      Poll::Ready(())
    })
    .await;
  }

  #[tokio::test]
  async fn test_error_builder() {
    #[op]
    fn op_err() -> Result<(), Error> {
      Err(custom_error("DOMExceptionOperationError", "abc"))
    }

    pub fn get_error_class_name(_: &Error) -> &'static str {
      "DOMExceptionOperationError"
    }

    deno_core::extension!(test_ext, ops = [op_err]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      get_error_class_fn: Some(&get_error_class_name),
      ..Default::default()
    });
    poll_fn(move |cx| {
      runtime
        .execute_script_static(
          "error_builder_test.js",
          include_str!("error_builder_test.js"),
        )
        .unwrap();
      if let Poll::Ready(Err(_)) = runtime.poll_event_loop(cx, false) {
        unreachable!();
      }
      Poll::Ready(())
    })
    .await;
  }

  /// Ensure that putting the inspector into OpState doesn't cause crashes. The only valid place we currently allow
  /// the inspector to be stashed without cleanup is the OpState, and this should not actually cause crashes.
  #[test]
  fn inspector() {
    let mut runtime = JsRuntime::new(RuntimeOptions {
      inspector: true,
      ..Default::default()
    });
    // This was causing a crash
    runtime.op_state().borrow_mut().put(runtime.inspector());
    runtime.execute_script_static("check.js", "null").unwrap();
  }

  #[test]
  fn will_snapshot() {
    let snapshot = {
      let mut runtime =
        JsRuntimeForSnapshot::new(Default::default(), Default::default());
      runtime.execute_script_static("a.js", "a = 1 + 2").unwrap();
      runtime.snapshot()
    };

    let snapshot = Snapshot::JustCreated(snapshot);
    let mut runtime2 = JsRuntime::new(RuntimeOptions {
      startup_snapshot: Some(snapshot),
      ..Default::default()
    });
    runtime2
      .execute_script_static("check.js", "if (a != 3) throw Error('x')")
      .unwrap();
  }

  #[test]
  fn will_snapshot2() {
    let startup_data = {
      let mut runtime =
        JsRuntimeForSnapshot::new(Default::default(), Default::default());
      runtime
        .execute_script_static("a.js", "let a = 1 + 2")
        .unwrap();
      runtime.snapshot()
    };

    let snapshot = Snapshot::JustCreated(startup_data);
    let mut runtime = JsRuntimeForSnapshot::new(
      RuntimeOptions {
        startup_snapshot: Some(snapshot),
        ..Default::default()
      },
      Default::default(),
    );

    let startup_data = {
      runtime
        .execute_script_static("check_a.js", "if (a != 3) throw Error('x')")
        .unwrap();
      runtime.execute_script_static("b.js", "b = 2 + 3").unwrap();
      runtime.snapshot()
    };

    let snapshot = Snapshot::JustCreated(startup_data);
    {
      let mut runtime = JsRuntime::new(RuntimeOptions {
        startup_snapshot: Some(snapshot),
        ..Default::default()
      });
      runtime
        .execute_script_static("check_b.js", "if (b != 5) throw Error('x')")
        .unwrap();
      runtime
        .execute_script_static("check2.js", "if (!Deno.core) throw Error('x')")
        .unwrap();
    }
  }

  #[test]
  fn test_snapshot_callbacks() {
    let snapshot = {
      let mut runtime =
        JsRuntimeForSnapshot::new(Default::default(), Default::default());
      runtime
        .execute_script_static(
          "a.js",
          r#"
          Deno.core.setMacrotaskCallback(() => {
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
      .execute_script_static("check.js", "if (a != 3) throw Error('x')")
      .unwrap();
  }

  #[test]
  fn test_from_boxed_snapshot() {
    let snapshot = {
      let mut runtime =
        JsRuntimeForSnapshot::new(Default::default(), Default::default());
      runtime.execute_script_static("a.js", "a = 1 + 2").unwrap();
      let snap: &[u8] = &runtime.snapshot();
      Vec::from(snap).into_boxed_slice()
    };

    let snapshot = Snapshot::Boxed(snapshot);
    let mut runtime2 = JsRuntime::new(RuntimeOptions {
      startup_snapshot: Some(snapshot),
      ..Default::default()
    });
    runtime2
      .execute_script_static("check.js", "if (a != 3) throw Error('x')")
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
        _kind: ResolutionKind,
      ) -> Result<ModuleSpecifier, Error> {
        assert_eq!(specifier, "file:///main.js");
        assert_eq!(referrer, ".");
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
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
    let source_code = ascii_str!(
      r#"
      export const a = "b";
      export default 1 + 2;
      "#
    );

    let module_id = futures::executor::block_on(
      runtime.load_main_module(&specifier, Some(source_code)),
    )
    .unwrap();

    #[allow(clippy::let_underscore_future)]
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
      v8::Isolate::create_params().heap_limits(0, 5 * 1024 * 1024);
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
      .execute_script_static(
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
      v8::Isolate::create_params().heap_limits(0, 5 * 1024 * 1024);
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
      .execute_script_static(
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
        _kind: ResolutionKind,
      ) -> Result<ModuleSpecifier, Error> {
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        eprintln!("load() should not be called");
        unreachable!()
      }
    }

    fn create_module<const FOR_SNAPSHOT: bool>(
      runtime: &mut JsRuntimeImpl<FOR_SNAPSHOT>,
      i: usize,
      main: bool,
    ) -> ModuleInfo {
      let specifier = crate::resolve_url(&format!("file:///{i}.js")).unwrap();
      let prev = i - 1;
      let source_code = format!(
        r#"
        import {{ f{prev} }} from "file:///{prev}.js";
        export function f{i}() {{ return f{prev}() }}
        "#
      )
      .into();

      let id = if main {
        futures::executor::block_on(
          runtime.load_main_module(&specifier, Some(source_code)),
        )
        .unwrap()
      } else {
        futures::executor::block_on(
          runtime.load_side_module(&specifier, Some(source_code)),
        )
        .unwrap()
      };
      assert_eq!(i, id);

      #[allow(clippy::let_underscore_future)]
      let _ = runtime.mod_evaluate(id);
      futures::executor::block_on(runtime.run_event_loop(false)).unwrap();

      ModuleInfo {
        id,
        main,
        name: specifier.into(),
        requests: vec![crate::modules::ModuleRequest {
          specifier: format!("file:///{prev}.js"),
          asserted_module_type: AssertedModuleType::JavaScriptOrWasm,
        }],
        module_type: ModuleType::JavaScript,
      }
    }

    fn assert_module_map<const FOR_SNAPSHOT: bool>(
      runtime: &mut JsRuntimeImpl<FOR_SNAPSHOT>,
      modules: &Vec<ModuleInfo>,
    ) {
      let module_map = runtime.module_map.borrow();
      assert_eq!(module_map.handles.len(), modules.len());
      assert_eq!(module_map.info.len(), modules.len());
      assert_eq!(
        module_map.by_name(AssertedModuleType::Json).len()
          + module_map
            .by_name(AssertedModuleType::JavaScriptOrWasm)
            .len(),
        modules.len()
      );

      assert_eq!(module_map.next_load_id, (modules.len() + 1) as ModuleLoadId);

      for info in modules {
        assert!(module_map.handles.get(info.id).is_some());
        assert_eq!(module_map.info.get(info.id).unwrap(), info);
        assert_eq!(
          module_map
            .by_name(AssertedModuleType::JavaScriptOrWasm)
            .get(&info.name)
            .unwrap(),
          &SymbolicModule::Mod(info.id)
        );
      }
    }

    #[op]
    fn op_test() -> Result<String, Error> {
      Ok(String::from("test"))
    }

    let loader = Rc::new(ModsLoader::default());
    let mut runtime = JsRuntimeForSnapshot::new(
      RuntimeOptions {
        module_loader: Some(loader.clone()),
        extensions: vec![Extension::builder("text_ext")
          .ops(vec![op_test::decl()])
          .build()],
        ..Default::default()
      },
      Default::default(),
    );

    let specifier = crate::resolve_url("file:///0.js").unwrap();
    let source_code =
      ascii_str!(r#"export function f0() { return "hello world" }"#);
    let id = futures::executor::block_on(
      runtime.load_side_module(&specifier, Some(source_code)),
    )
    .unwrap();

    #[allow(clippy::let_underscore_future)]
    let _ = runtime.mod_evaluate(id);
    futures::executor::block_on(runtime.run_event_loop(false)).unwrap();

    let mut modules = vec![];
    modules.push(ModuleInfo {
      id,
      main: false,
      name: specifier.into(),
      requests: vec![],
      module_type: ModuleType::JavaScript,
    });

    modules.extend((1..200).map(|i| create_module(&mut runtime, i, false)));

    assert_module_map(&mut runtime, &modules);

    let snapshot = runtime.snapshot();

    let mut runtime2 = JsRuntimeForSnapshot::new(
      RuntimeOptions {
        module_loader: Some(loader.clone()),
        startup_snapshot: Some(Snapshot::JustCreated(snapshot)),
        extensions: vec![Extension::builder("text_ext")
          .ops(vec![op_test::decl()])
          .build()],
        ..Default::default()
      },
      Default::default(),
    );

    assert_module_map(&mut runtime2, &modules);

    modules.extend((200..400).map(|i| create_module(&mut runtime2, i, false)));
    modules.push(create_module(&mut runtime2, 400, true));

    assert_module_map(&mut runtime2, &modules);

    let snapshot2 = runtime2.snapshot();

    let mut runtime3 = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      startup_snapshot: Some(Snapshot::JustCreated(snapshot2)),
      extensions: vec![Extension::builder("text_ext")
        .ops(vec![op_test::decl()])
        .build()],
      ..Default::default()
    });

    assert_module_map(&mut runtime3, &modules);

    let source_code = r#"(async () => {
      const mod = await import("file:///400.js");
      return mod.f400() + " " + Deno.core.ops.op_test();
    })();"#;
    let val = runtime3.execute_script_static(".", source_code).unwrap();
    let val = futures::executor::block_on(runtime3.resolve_value(val)).unwrap();
    {
      let scope = &mut runtime3.handle_scope();
      let value = v8::Local::new(scope, val);
      let str_ = value.to_string(scope).unwrap().to_rust_string_lossy(scope);
      assert_eq!(str_, "hello world test");
    }
  }

  #[test]
  fn test_error_without_stack() {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    // SyntaxError
    let result = runtime.execute_script_static(
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
    let result = runtime.execute_script_static(
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
    at main (error_stack.js:8:3)
    at error_stack.js:10:1"#;
    assert_eq!(result.unwrap_err().to_string(), expected_error);
  }

  #[tokio::test]
  async fn test_error_async_stack() {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    poll_fn(move |cx| {
      runtime
        .execute_script_static(
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
    at async error_async_stack.js:9:5"#;

      match runtime.poll_event_loop(cx, false) {
        Poll::Ready(Err(e)) => {
          assert_eq!(e.to_string(), expected_error);
        }
        _ => panic!(),
      };
      Poll::Ready(())
    })
    .await;
  }

  #[tokio::test]
  async fn test_error_context() {
    use anyhow::anyhow;

    #[op]
    fn op_err_sync() -> Result<(), Error> {
      Err(anyhow!("original sync error").context("higher-level sync error"))
    }

    #[op]
    async fn op_err_async() -> Result<(), Error> {
      Err(anyhow!("original async error").context("higher-level async error"))
    }

    deno_core::extension!(test_ext, ops = [op_err_sync, op_err_async]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });

    poll_fn(move |cx| {
      runtime
        .execute_script_static(
          "test_error_context_sync.js",
          r#"
let errMessage;
try {
  Deno.core.ops.op_err_sync();
} catch (err) {
  errMessage = err.message;
}
if (errMessage !== "higher-level sync error: original sync error") {
  throw new Error("unexpected error message from op_err_sync: " + errMessage);
}
"#,
        )
        .unwrap();

      let promise = runtime
        .execute_script_static(
          "test_error_context_async.js",
          r#"

(async () => {
  let errMessage;
  try {
    await Deno.core.opAsync("op_err_async");
  } catch (err) {
    errMessage = err.message;
  }
  if (errMessage !== "higher-level async error: original async error") {
    throw new Error("unexpected error message from op_err_async: " + errMessage);
  }
})()
"#,
        )
        .unwrap();

      match runtime.poll_value(&promise, cx) {
        Poll::Ready(Ok(_)) => {}
        Poll::Ready(Err(err)) => panic!("{err:?}"),
        _ => panic!(),
      }
      Poll::Ready(())
    }).await;
  }

  #[tokio::test]
  async fn test_pump_message_loop() {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    poll_fn(move |cx| {
      runtime
        .execute_script_static(
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
        .execute_script_static(
          "pump_message_loop2.js",
          r#"assertEquals(1, 1);"#,
        )
        .unwrap();

      // check that promise from `Atomics.waitAsync` has been resolved
      runtime
        .execute_script_static(
          "pump_message_loop3.js",
          r#"assertEquals(globalThis.resolved, true);"#,
        )
        .unwrap();
      Poll::Ready(())
    })
    .await;
  }

  #[test]
  fn test_v8_platform() {
    let options = RuntimeOptions {
      v8_platform: Some(v8::new_default_platform(0, false).make_shared()),
      ..Default::default()
    };
    let mut runtime = JsRuntime::new(options);
    runtime.execute_script_static("<none>", "").unwrap();
  }

  #[ignore] // TODO(@littledivy): Fast API ops when snapshot is not loaded.
  #[test]
  fn test_is_proxy() {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    let all_true: v8::Global<v8::Value> = runtime
      .execute_script_static(
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

    deno_core::extension!(
      test_ext,
      ops = [op_async_borrow],
      state = |state| state.put(InnerState(42))
    );
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });

    runtime
      .execute_script_static(
        "op_async_borrow.js",
        "Deno.core.opAsync(\"op_async_borrow\")",
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

    deno_core::extension!(
      test_ext,
      ops = [op_sync_serialize_object_with_numbers_as_keys]
    );
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });

    runtime
      .execute_script_static(
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

    deno_core::extension!(
      test_ext,
      ops = [op_async_serialize_object_with_numbers_as_keys]
    );
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });

    runtime
      .execute_script_static(
        "op_async_serialize_object_with_numbers_as_keys.js",
        r#"

Deno.core.opAsync("op_async_serialize_object_with_numbers_as_keys", {
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

    deno_core::extension!(test_ext, ops = [op_async_sleep]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });

    runtime
      .execute_script_static(
        "macrotasks_and_nextticks.js",
        r#"

        (async function () {
          const results = [];
          Deno.core.setMacrotaskCallback(() => {
            results.push("macrotask");
            return true;
          });
          Deno.core.setNextTickCallback(() => {
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

    deno_core::extension!(test_ext, ops = [op_macrotask, op_next_tick]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });

    runtime
      .execute_script_static(
        "has_tick_scheduled.js",
        r#"
          Deno.core.setMacrotaskCallback(() => {
            Deno.core.ops.op_macrotask();
            return true; // We're done.
          });
          Deno.core.setNextTickCallback(() => Deno.core.ops.op_next_tick());
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

    runtime.inner.state.borrow_mut().has_tick_scheduled = false;
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
        _kind: ResolutionKind,
      ) -> Result<ModuleSpecifier, Error> {
        assert_eq!(specifier, "file:///main.js");
        assert_eq!(referrer, ".");
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        async move {
          Ok(ModuleSource::for_test(
            "console.log('hello world');",
            "file:///main.js",
          ))
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
    let source_code = ascii_str!("Deno.core.print('hello\\n')");

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
  async fn test_unhandled_rejection_order() {
    let mut runtime = JsRuntime::new(Default::default());
    runtime
      .execute_script_static(
        "",
        r#"
        for (let i = 0; i < 100; i++) {
          Promise.reject(i);
        }
        "#,
      )
      .unwrap();
    let err = runtime.run_event_loop(false).await.unwrap_err();
    assert_eq!(err.to_string(), "Uncaught (in promise) 0");
  }

  #[tokio::test]
  async fn test_set_promise_reject_callback() {
    static PROMISE_REJECT: AtomicUsize = AtomicUsize::new(0);

    #[op]
    fn op_promise_reject() -> Result<(), AnyError> {
      PROMISE_REJECT.fetch_add(1, Ordering::Relaxed);
      Ok(())
    }

    deno_core::extension!(test_ext, ops = [op_promise_reject]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });

    runtime
      .execute_script_static(
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
          Deno.core.ops.op_store_pending_promise_rejection(promise);
          Deno.core.ops.op_promise_reject();
        });
        new Promise((_, reject) => reject(Error("reject")));
        "#,
      )
      .unwrap();
    runtime.run_event_loop(false).await.unwrap_err();

    assert_eq!(1, PROMISE_REJECT.load(Ordering::Relaxed));

    runtime
      .execute_script_static(
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
          format!(
            r#"

              globalThis.rejectValue = undefined;
              Deno.core.setPromiseRejectCallback((_type, _promise, reason) => {{
                globalThis.rejectValue = `{realm_name}/${{reason}}`;
              }});
              Deno.core.opAsync("op_void_async").then(() => Promise.reject({number}));
            "#
          ).into()
        )
        .unwrap();
    }

    runtime.run_event_loop(false).await.unwrap();

    for (realm, realm_name, number) in realm_expectations {
      let reject_value = realm
        .execute_script_static(
          runtime.v8_isolate(),
          "",
          "globalThis.rejectValue",
        )
        .unwrap();
      let scope = &mut realm.handle_scope(runtime.v8_isolate());
      let reject_value = v8::Local::new(scope, reject_value);
      assert!(reject_value.is_string());
      let reject_value_string = reject_value.to_rust_string_lossy(scope);
      assert_eq!(reject_value_string, format!("{realm_name}/{number}"));
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

    deno_core::extension!(test_ext, ops = [op_promise_reject]);

    #[derive(Default)]
    struct ModsLoader;

    impl ModuleLoader for ModsLoader {
      fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
      ) -> Result<ModuleSpecifier, Error> {
        assert_eq!(specifier, "file:///main.js");
        assert_eq!(referrer, ".");
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        let code = r#"
        Deno.core.ops.op_set_promise_reject_callback((type, promise, reason) => {
          Deno.core.ops.op_promise_reject();
        });
        throw new Error('top level throw');
        "#;

        async move { Ok(ModuleSource::for_test(code, "file:///main.js")) }
          .boxed_local()
      }
    }

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
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

    deno_core::extension!(test_ext, ops = [op_err]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });
    assert!(runtime
      .execute_script_static(
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

    deno_core::extension!(test_ext, ops = [op_add_4]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });
    let r = runtime
      .execute_script_static("test.js", "Deno.core.ops.op_add_4(1, 2, 3, 4)")
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

    fn ops() -> Vec<OpDecl> {
      vec![op_foo::decl().disable()]
    }

    deno_core::extension!(test_ext, ops_fn = ops);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });
    let err = runtime
      .execute_script_static("test.js", "Deno.core.ops.op_foo()")
      .unwrap_err();
    assert!(err
      .to_string()
      .contains("TypeError: Deno.core.ops.op_foo is not a function"));
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

    deno_core::extension!(test_ext, ops = [op_sum_take, op_boomerang]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });

    runtime
      .execute_script_static(
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
            return e.message.includes('invalid type; expected: detachable');
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

    deno_core::extension!(
      test_ext,
      ops = [op_foo, op_bar],
      middleware = |op| if op.is_unstable { op.disable() } else { op }
    );
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });
    runtime
      .execute_script_static(
        "test.js",
        r#"
        if (Deno.core.ops.op_foo() !== 42) {
          throw new Error("Exptected op_foo() === 42");
        }
        if (typeof Deno.core.ops.op_bar !== "undefined") {
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

    let main_object = runtime.execute_script_static("", "Object").unwrap();
    let realm_object = realm
      .execute_script_static(runtime.v8_isolate(), "", "Object")
      .unwrap();
    assert_ne!(main_object, realm_object);
  }

  #[test]
  fn js_realm_init() {
    #[op]
    fn op_test() -> Result<String, Error> {
      Ok(String::from("Test"))
    }

    deno_core::extension!(test_ext, ops = [op_test]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });
    let realm = runtime.create_realm().unwrap();
    let ret = realm
      .execute_script_static(
        runtime.v8_isolate(),
        "",
        "Deno.core.ops.op_test()",
      )
      .unwrap();

    let scope = &mut realm.handle_scope(runtime.v8_isolate());
    assert_eq!(ret, serde_v8::to_v8(scope, "Test").unwrap());
  }

  #[test]
  fn js_realm_init_snapshot() {
    let snapshot = {
      let runtime =
        JsRuntimeForSnapshot::new(Default::default(), Default::default());
      let snap: &[u8] = &runtime.snapshot();
      Vec::from(snap).into_boxed_slice()
    };

    #[op]
    fn op_test() -> Result<String, Error> {
      Ok(String::from("Test"))
    }

    deno_core::extension!(
      test_ext,
      ops = [op_test],
      customizer = |ext: &mut deno_core::ExtensionBuilder| {
        ext.force_op_registration();
      },
    );
    let mut runtime = JsRuntime::new(RuntimeOptions {
      startup_snapshot: Some(Snapshot::Boxed(snapshot)),
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });
    let realm = runtime.create_realm().unwrap();
    let ret = realm
      .execute_script_static(
        runtime.v8_isolate(),
        "",
        "Deno.core.ops.op_test()",
      )
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

    deno_core::extension!(test_ext, ops = [op_test]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      get_error_class_fn: Some(&|error| {
        crate::error::get_custom_error_class(error).unwrap()
      }),
      ..Default::default()
    });
    let new_realm = runtime.create_realm().unwrap();

    // Test in both realms
    for realm in [runtime.global_realm(), new_realm].into_iter() {
      let ret = realm
        .execute_script_static(
          runtime.v8_isolate(),
          "",
          r#"
            const buf = Deno.core.ops.op_test(false);
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

    deno_core::extension!(test_ext, ops = [op_test]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
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
        .execute_script_static(
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
  async fn js_realm_gc() {
    static INVOKE_COUNT: AtomicUsize = AtomicUsize::new(0);
    struct PendingFuture {}

    impl Future for PendingFuture {
      type Output = ();
      fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        Poll::Pending
      }
    }

    impl Drop for PendingFuture {
      fn drop(&mut self) {
        assert_eq!(INVOKE_COUNT.fetch_sub(1, Ordering::SeqCst), 1);
      }
    }

    // Never resolves.
    #[op]
    async fn op_pending() {
      assert_eq!(INVOKE_COUNT.fetch_add(1, Ordering::SeqCst), 0);
      PendingFuture {}.await
    }

    deno_core::extension!(test_ext, ops = [op_pending]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });

    // Detect a drop in OpState
    let opstate_drop_detect = Rc::new(());
    runtime
      .op_state()
      .borrow_mut()
      .put(opstate_drop_detect.clone());
    assert_eq!(Rc::strong_count(&opstate_drop_detect), 2);

    let other_realm = runtime.create_realm().unwrap();
    other_realm
      .execute_script(
        runtime.v8_isolate(),
        "future",
        ModuleCode::from_static("Deno.core.opAsync('op_pending')"),
      )
      .unwrap();
    while INVOKE_COUNT.load(Ordering::SeqCst) == 0 {
      poll_fn(|cx: &mut Context| runtime.poll_event_loop(cx, false))
        .await
        .unwrap();
    }
    drop(other_realm);
    while INVOKE_COUNT.load(Ordering::SeqCst) == 1 {
      poll_fn(|cx| runtime.poll_event_loop(cx, false))
        .await
        .unwrap();
    }

    drop(runtime);

    // Make sure the OpState was dropped properly when the runtime dropped
    assert_eq!(Rc::strong_count(&opstate_drop_detect), 1);
  }

  #[tokio::test]
  async fn js_realm_ref_unref_ops() {
    // Never resolves.
    #[op]
    async fn op_pending() {
      futures::future::pending().await
    }

    deno_core::extension!(test_ext, ops = [op_pending]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });

    poll_fn(move |cx| {
      let main_realm = runtime.global_realm();
      let other_realm = runtime.create_realm().unwrap();

      main_realm
        .execute_script_static(
          runtime.v8_isolate(),
          "",
          r#"

            var promise = Deno.core.opAsync("op_pending");
          "#,
        )
        .unwrap();
      other_realm
        .execute_script_static(
          runtime.v8_isolate(),
          "",
          r#"

            var promise = Deno.core.opAsync("op_pending");
          "#,
        )
        .unwrap();
      assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));

      main_realm
        .execute_script_static(
          runtime.v8_isolate(),
          "",
          r#"
            let promiseIdSymbol = Symbol.for("Deno.core.internalPromiseId");
            Deno.core.unrefOp(promise[promiseIdSymbol]);
          "#,
        )
        .unwrap();
      assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));

      other_realm
        .execute_script_static(
          runtime.v8_isolate(),
          "",
          r#"
            let promiseIdSymbol = Symbol.for("Deno.core.internalPromiseId");
            Deno.core.unrefOp(promise[promiseIdSymbol]);
          "#,
        )
        .unwrap();
      assert!(matches!(
        runtime.poll_event_loop(cx, false),
        Poll::Ready(Ok(()))
      ));
      Poll::Ready(())
    })
    .await;
  }

  #[test]
  fn test_array_by_copy() {
    // Verify that "array by copy" proposal is enabled (https://github.com/tc39/proposal-change-array-by-copy)
    let mut runtime = JsRuntime::new(Default::default());
    assert!(runtime
      .execute_script_static(
        "test_array_by_copy.js",
        "const a = [1, 2, 3];
        const b = a.toReversed();
        if (!(a[0] === 1 && a[1] === 2 && a[2] === 3)) {
          throw new Error('Expected a to be intact');
        }
        if (!(b[0] === 3 && b[1] === 2 && b[2] === 1)) {
          throw new Error('Expected b to be reversed');
        }",
      )
      .is_ok());
  }

  #[cfg(debug_assertions)]
  #[test]
  #[should_panic(expected = "Found ops with duplicate names:")]
  fn duplicate_op_names() {
    mod a {
      use super::*;

      #[op]
      fn op_test() -> Result<String, Error> {
        Ok(String::from("Test"))
      }
    }

    #[op]
    fn op_test() -> Result<String, Error> {
      Ok(String::from("Test"))
    }

    deno_core::extension!(test_ext, ops = [a::op_test, op_test]);
    JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });
  }

  #[test]
  fn ops_in_js_have_proper_names() {
    #[op]
    fn op_test_sync() -> Result<String, Error> {
      Ok(String::from("Test"))
    }

    #[op]
    async fn op_test_async() -> Result<String, Error> {
      Ok(String::from("Test"))
    }

    deno_core::extension!(test_ext, ops = [op_test_sync, op_test_async]);
    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });

    let src = r#"
    if (Deno.core.ops.op_test_sync.name !== "op_test_sync") {
      throw new Error();
    }

    if (Deno.core.ops.op_test_async.name !== "op_test_async") {
      throw new Error();
    }

    const { op_test_async } = Deno.core.generateAsyncOpHandler("op_test_async");
    if (op_test_async.name !== "op_test_async") {
      throw new Error();
    }
    "#;
    runtime.execute_script_static("test", src).unwrap();
  }
}
