// Copyright 2018-2025 the Deno authors. MIT license.

use super::exception_state::ExceptionState;
#[cfg(test)]
use super::op_driver::OpDriver;
use crate::_ops::OpMethodDecl;
use crate::ModuleSourceCode;
use crate::SourceCodeCacheInfo;
use crate::cppgc::FunctionTemplateData;
use crate::error::CoreError;
use crate::error::CreateCodeCacheError;
use crate::error::JsError;
use crate::error::exception_to_err;
use crate::error::exception_to_err_result;
use crate::event_loop::EventLoopPhases;
use crate::module_specifier::ModuleSpecifier;
use crate::modules::IntoModuleCodeString;
use crate::modules::IntoModuleName;
use crate::modules::ModuleCodeString;
use crate::modules::ModuleId;
use crate::modules::ModuleMap;
use crate::modules::ModuleName;
use crate::modules::script_origin;
use crate::ops::ExternalOpsTracker;
use crate::ops::OpCtx;
use crate::reactor::DefaultReactor;
use crate::stats::RuntimeActivityTraces;
use crate::tasks::V8TaskSpawnerFactory;
use crate::uv_compat::UvLoopInner;
use crate::web_timeout::WebTimers;
use futures::stream::StreamExt;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::BuildHasherDefault;
use std::hash::Hasher;
use std::rc::Rc;
use std::sync::Arc;

pub const CONTEXT_STATE_SLOT_INDEX: i32 = 1;
pub const MODULE_MAP_SLOT_INDEX: i32 = 2;

// Hasher used for `unrefed_ops`. Since these are rolling i32, there's no
// need to actually hash them.
#[derive(Default)]
pub(crate) struct IdentityHasher(u64);

impl Hasher for IdentityHasher {
  fn write_i32(&mut self, i: i32) {
    self.0 = i as u64;
  }

  fn finish(&self) -> u64 {
    self.0
  }

  fn write(&mut self, _bytes: &[u8]) {
    unreachable!()
  }
}

/// We may wish to experiment with alternative drivers in the future.
pub(crate) type OpDriverImpl = super::op_driver::FuturesUnorderedDriver;

pub(crate) type UnrefedOps =
  Rc<RefCell<HashSet<i32, BuildHasherDefault<IdentityHasher>>>>;

#[derive(Debug, Default)]
pub(crate) struct ImmediateInfo {
  pub count: u32,
  pub ref_count: u32,
  pub has_outstanding: bool,
}

pub struct ContextState {
  pub(crate) task_spawner_factory: Arc<V8TaskSpawnerFactory>,
  pub(crate) timers: WebTimers<(v8::Global<v8::Function>, u32), DefaultReactor>,
  // Per-phase JS callbacks (replacing monolithic eventLoopTick)
  pub(crate) js_resolve_ops_cb: RefCell<Option<v8::Global<v8::Function>>>,
  pub(crate) js_drain_next_tick_and_macrotasks_cb:
    RefCell<Option<v8::Global<v8::Function>>>,
  pub(crate) js_handle_rejections_cb: RefCell<Option<v8::Global<v8::Function>>>,
  pub(crate) js_set_timer_depth_cb: RefCell<Option<v8::Global<v8::Function>>>,
  pub(crate) js_report_exception_cb: RefCell<Option<v8::Global<v8::Function>>>,
  pub(crate) run_immediate_callbacks_cb:
    RefCell<Option<v8::Global<v8::Function>>>,
  pub(crate) js_wasm_streaming_cb: RefCell<Option<v8::Global<v8::Function>>>,
  pub(crate) wasm_instance_fn: RefCell<Option<v8::Global<v8::Function>>>,
  pub(crate) unrefed_ops: UnrefedOps,
  pub(crate) activity_traces: RuntimeActivityTraces,
  pub(crate) pending_ops: Rc<OpDriverImpl>,
  // We don't explicitly re-read this prop but need the slice to live alongside
  // the context
  pub(crate) op_ctxs: Box<[OpCtx]>,
  pub(crate) op_method_decls: Vec<OpMethodDecl>,
  pub(crate) methods_ctx_offset: usize,
  pub(crate) isolate: Option<v8::UnsafeRawIsolatePtr>,
  pub(crate) exception_state: Rc<ExceptionState>,
  pub(crate) has_next_tick_scheduled: Cell<bool>,
  pub(crate) immediate_info: RefCell<ImmediateInfo>,
  pub(crate) external_ops_tracker: ExternalOpsTracker,
  pub(crate) ext_import_meta_proto: RefCell<Option<v8::Global<v8::Object>>>,
  /// Phase-specific state for the libuv-style event loop.
  pub(crate) event_loop_phases: RefCell<EventLoopPhases>,
  /// Pointer to the `UvLoopInner` for the libuv compat layer.
  /// Set via [`JsRuntime::register_uv_loop`] when a `uv_loop_t` is
  /// associated with this context.
  ///
  /// # Safety
  /// The pointee is heap-allocated by `uv_loop_init` (boxed) and lives until
  /// `uv_loop_close` destroys it. The caller of `register_uv_loop` must
  /// guarantee the `uv_loop_t` outlives this `ContextState`. Both
  /// `UvLoopInner` and `ContextState` are `!Send` -- all access is on the
  /// event loop thread.
  pub(crate) uv_loop_inner: Cell<Option<*const UvLoopInner>>,
  /// Raw pointer to the `uv_loop_t` handle, used to set `loop_.data`
  /// to the current `v8::Context` at the start of each event loop tick
  /// so that libuv-style C callbacks can retrieve the context.
  ///
  /// # Safety
  /// Same lifetime requirements as `uv_loop_inner` above.
  pub(crate) uv_loop_ptr: Cell<Option<*mut crate::uv_compat::uv_loop_t>>,
}

impl ContextState {
  pub(crate) fn new(
    op_driver: Rc<OpDriverImpl>,
    isolate_ptr: v8::UnsafeRawIsolatePtr,
    op_ctxs: Box<[OpCtx]>,
    op_method_decls: Vec<OpMethodDecl>,
    methods_ctx_offset: usize,
    external_ops_tracker: ExternalOpsTracker,
    unrefed_ops: UnrefedOps,
  ) -> Self {
    Self {
      isolate: Some(isolate_ptr),
      exception_state: Default::default(),
      has_next_tick_scheduled: Default::default(),
      immediate_info: Default::default(),
      js_resolve_ops_cb: Default::default(),
      js_drain_next_tick_and_macrotasks_cb: Default::default(),
      js_handle_rejections_cb: Default::default(),
      js_set_timer_depth_cb: Default::default(),
      js_report_exception_cb: Default::default(),
      run_immediate_callbacks_cb: Default::default(),
      js_wasm_streaming_cb: Default::default(),
      wasm_instance_fn: Default::default(),
      activity_traces: Default::default(),
      op_ctxs,
      op_method_decls,
      methods_ctx_offset,
      pending_ops: op_driver,
      task_spawner_factory: Default::default(),
      timers: Default::default(),
      unrefed_ops,
      external_ops_tracker,
      ext_import_meta_proto: Default::default(),
      event_loop_phases: Default::default(),
      uv_loop_inner: Cell::new(None),
      uv_loop_ptr: Cell::new(None),
    }
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
/// Every method of [`JsRealm`] will panic if you call it with a reference to a
/// [`v8::Isolate`] other than the one that corresponds to the current context.
///
/// In other words, the [`v8::Isolate`] parameter for all the related [`JsRealm`] methods
/// must be extracted from the pre-existing [`JsRuntime`].
///
/// # Lifetime of the realm
///
/// As long as the corresponding isolate is alive, a [`JsRealm`] instance will
/// keep the underlying V8 context alive even if it would have otherwise been
/// garbage collected.
#[derive(Clone)]
#[repr(transparent)]
pub(crate) struct JsRealm(pub(crate) JsRealmInner);

#[derive(Clone)]
pub(crate) struct JsRealmInner {
  pub(crate) context_state: Rc<ContextState>,
  context: v8::Global<v8::Context>,
  pub(crate) module_map: Rc<ModuleMap>,
  pub(crate) function_templates: Rc<RefCell<FunctionTemplateData>>,
}

impl JsRealmInner {
  pub(crate) fn new(
    context_state: Rc<ContextState>,
    context: v8::Global<v8::Context>,
    module_map: Rc<ModuleMap>,
    function_templates: Rc<RefCell<FunctionTemplateData>>,
  ) -> Self {
    Self {
      context_state,
      context: context.clone(),
      module_map,
      function_templates,
    }
  }

  #[inline(always)]
  pub fn context(&self) -> &v8::Global<v8::Context> {
    &self.context
  }

  #[inline(always)]
  pub(crate) fn state(&self) -> Rc<ContextState> {
    self.context_state.clone()
  }

  #[inline(always)]
  pub(crate) fn module_map(&self) -> Rc<ModuleMap> {
    self.module_map.clone()
  }

  #[inline(always)]
  pub(crate) fn function_templates(&self) -> Rc<RefCell<FunctionTemplateData>> {
    self.function_templates.clone()
  }

  pub fn destroy(self) {
    let state = self.state();
    let raw_ptr = self.state().isolate.unwrap();
    // SAFETY: We know the isolate outlives the realm
    let mut isolate = unsafe { v8::Isolate::from_raw_isolate_ptr(raw_ptr) };
    v8::scope!(let scope, &mut isolate);
    // These globals will prevent snapshots from completing, take them
    state.exception_state.prepare_to_destroy();
    std::mem::take(&mut *state.js_resolve_ops_cb.borrow_mut());
    std::mem::take(
      &mut *state.js_drain_next_tick_and_macrotasks_cb.borrow_mut(),
    );
    std::mem::take(&mut *state.js_handle_rejections_cb.borrow_mut());
    std::mem::take(&mut *state.js_set_timer_depth_cb.borrow_mut());
    std::mem::take(&mut *state.js_report_exception_cb.borrow_mut());
    std::mem::take(&mut *state.run_immediate_callbacks_cb.borrow_mut());
    std::mem::take(&mut *state.js_wasm_streaming_cb.borrow_mut());

    {
      let ctx = self.context().open(scope);
      // SAFETY: Clear all embedder data
      unsafe {
        let ctx_state =
          ctx.get_aligned_pointer_from_embedder_data(CONTEXT_STATE_SLOT_INDEX);
        let _ = Rc::from_raw(ctx_state as *mut ContextState);

        let module_map =
          ctx.get_aligned_pointer_from_embedder_data(MODULE_MAP_SLOT_INDEX);
        // Explcitly destroy data in the module map, as there might be some pending
        // futures there and we want them dropped.
        let map = Rc::from_raw(module_map as *mut ModuleMap);
        map.destroy();

        ctx.set_aligned_pointer_in_embedder_data(
          CONTEXT_STATE_SLOT_INDEX,
          std::ptr::null_mut(),
        );
        ctx.set_aligned_pointer_in_embedder_data(
          MODULE_MAP_SLOT_INDEX,
          std::ptr::null_mut(),
        );
      }
      ctx.clear_all_slots();
      // Expect that this context is dead (we only check this in debug mode)
      // TODO(bartlomieju): This check fails for some tests, will need to fix this
      // debug_assert_eq!(Rc::strong_count(&module_map), 1, "ModuleMap still in use.");
    }

    // Expect that this context is dead (we only check this in debug mode)
    // TODO(mmastrac): This check fails for some tests, will need to fix this
    // debug_assert_eq!(Rc::strong_count(&self.context), 1, "Realm was still alive when we wanted to destroy it. Not dropped?");
  }
}

unsafe fn clone_rc_raw<T>(raw: *const T) -> Rc<T> {
  unsafe {
    Rc::increment_strong_count(raw);
    Rc::from_raw(raw)
  }
}
macro_rules! context_scope {
  ($scope: ident, $self: expr, $isolate: expr) => {
    v8::scope!($scope, $isolate);
    let context = v8::Local::new($scope, $self.context());
    let $scope = &mut v8::ContextScope::new($scope, context);
  };
}

pub(crate) use context_scope;

impl JsRealm {
  pub(crate) fn new(inner: JsRealmInner) -> Self {
    Self(inner)
  }

  #[inline(always)]
  pub(crate) fn state_from_scope(scope: &mut v8::PinScope) -> Rc<ContextState> {
    let context = scope.get_current_context();
    // SAFETY: slot is valid and set during realm creation
    unsafe {
      let rc = context
        .get_aligned_pointer_from_embedder_data(CONTEXT_STATE_SLOT_INDEX);
      clone_rc_raw(rc as *const ContextState)
    }
  }

  #[inline(always)]
  pub(crate) fn module_map_from(scope: &mut v8::PinScope) -> Rc<ModuleMap> {
    let context = scope.get_current_context();
    // SAFETY: slot is valid and set during realm creation
    unsafe {
      let rc =
        context.get_aligned_pointer_from_embedder_data(MODULE_MAP_SLOT_INDEX);
      clone_rc_raw(rc as *const ModuleMap)
    }
  }

  #[inline(always)]
  pub(crate) fn exception_state_from_scope(
    scope: &mut v8::PinScope,
  ) -> Rc<ExceptionState> {
    Self::state_from_scope(scope).exception_state.clone()
  }

  #[cfg(test)]
  #[inline(always)]
  pub fn num_pending_ops(&self) -> usize {
    self.0.context_state.pending_ops.len()
  }

  #[cfg(test)]
  #[inline(always)]
  pub fn num_unrefed_ops(&self) -> usize {
    self.0.context_state.unrefed_ops.borrow().len()
  }

  #[inline(always)]
  pub fn context(&self) -> &v8::Global<v8::Context> {
    self.0.context()
  }

  /// Executes traditional JavaScript code (traditional = not ES modules) in the
  /// realm's context.
  ///
  /// For info on the [`v8::Isolate`] parameter, check [`JsRealm#panics`].
  ///
  /// The `name` parameter can be a filepath or any other string. E.g.:
  ///
  ///   - "/some/file/path.js"
  ///   - "<anon>"
  ///   - "[native code]"
  ///
  /// The same `name` value can be used for multiple executions.
  pub fn execute_script(
    &self,
    isolate: &mut v8::Isolate,
    name: impl IntoModuleName,
    source_code: impl IntoModuleCodeString,
  ) -> Result<v8::Global<v8::Value>, Box<JsError>> {
    context_scope!(scope, self, isolate);

    let source = source_code.into_module_code().v8_string(scope).unwrap();
    let name = name.into_module_name().v8_string(scope).unwrap();
    let origin = script_origin(scope, name, false, None);

    v8::tc_scope!(let tc_scope, scope);

    let script = match v8::Script::compile(tc_scope, source, Some(&origin)) {
      Some(script) => script,
      None => {
        let exception = tc_scope.exception().unwrap();
        return exception_to_err_result(tc_scope, exception, false, false);
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
        exception_to_err_result(tc_scope, exception, false, false)
      }
    }
  }

  // TODO(nathanwhit): reduce duplication between this and `execute_script`, and
  // try to factor out the code cache logic to share with `op_eval_context`
  pub fn execute_script_with_cache(
    &self,
    isolate: &mut v8::Isolate,
    name: ModuleSpecifier,
    source_code: impl IntoModuleCodeString,
    get_cache: &dyn Fn(
      &ModuleSpecifier,
      &ModuleSourceCode,
    ) -> SourceCodeCacheInfo,
    cache_ready: &dyn Fn(ModuleSpecifier, u64, &[u8]),
  ) -> Result<v8::Global<v8::Value>, CoreError> {
    context_scope!(scope, self, isolate);

    let specifier = name.clone();
    let code = source_code.into_module_code();
    let source = ModuleSourceCode::String(code);
    let code_cache = get_cache(&name, &source);
    let ModuleSourceCode::String(source) = source else {
      unreachable!()
    };
    let name = name.into_module_name().v8_string(scope).unwrap();
    let source = source.v8_string(scope).unwrap();
    let origin = script_origin(scope, name, false, None);
    v8::tc_scope!(let tc_scope, scope);

    let (maybe_script, maybe_code_cache_hash) =
      if let Some(data) = &code_cache.data {
        let mut source = v8::script_compiler::Source::new_with_cached_data(
          source,
          Some(&origin),
          v8::CachedData::new(data),
        );
        let script = v8::script_compiler::compile(
          tc_scope,
          &mut source,
          v8::script_compiler::CompileOptions::ConsumeCodeCache,
          v8::script_compiler::NoCacheReason::NoReason,
        );
        // Check if the provided code cache is rejected by V8.
        let rejected = match source.get_cached_data() {
          Some(cached_data) => cached_data.rejected(),
          _ => true,
        };
        let maybe_code_cache_hash = if rejected {
          Some(code_cache.hash) // recreate the cache
        } else {
          None
        };
        (Some(script), maybe_code_cache_hash)
      } else {
        (None, Some(code_cache.hash))
      };

    let script = maybe_script
      .unwrap_or_else(|| v8::Script::compile(tc_scope, source, Some(&origin)));

    let script = match script {
      Some(script) => script,
      None => {
        let exception = tc_scope.exception().unwrap();
        return Ok(exception_to_err_result(tc_scope, exception, false, false)?);
      }
    };

    if let Some(code_cache_hash) = maybe_code_cache_hash {
      let unbound_script = script.get_unbound_script(tc_scope);
      let code_cache = unbound_script
        .create_code_cache()
        .ok_or_else(|| CreateCodeCacheError(specifier.clone()))?;
      cache_ready(specifier, code_cache_hash, &code_cache);
    }

    match script.run(tc_scope) {
      Some(value) => {
        let value_handle = v8::Global::new(tc_scope, value);
        Ok(value_handle)
      }
      None => {
        assert!(tc_scope.has_caught());
        let exception = tc_scope.exception().unwrap();
        Ok(exception_to_err_result(tc_scope, exception, false, false)?)
      }
    }
  }

  /// Returns the namespace object of a module.
  ///
  /// This is only available after module evaluation has completed.
  /// This function panics if module has not been instantiated.
  pub fn get_module_namespace(
    &self,
    isolate: &mut v8::Isolate,
    module_id: ModuleId,
  ) -> Result<v8::Global<v8::Object>, CoreError> {
    context_scope!(scope, self, isolate);
    self.0.module_map().get_module_namespace(scope, module_id)
  }

  pub(crate) fn instantiate_module(
    &self,
    scope: &mut v8::PinScope,
    id: ModuleId,
  ) -> Result<(), v8::Global<v8::Value>> {
    self.0.module_map().instantiate_module(scope, id)
  }

  pub(crate) fn modules_idle(&self) -> bool {
    self.0.module_map.dyn_module_evaluate_idle_counter.get() > 1
  }

  pub(crate) fn increment_modules_idle(&self) {
    let count = &self.0.module_map.dyn_module_evaluate_idle_counter;
    count.set(count.get() + 1)
  }

  /// Asynchronously load specified module and all of its dependencies.
  ///
  /// The module will be marked as "main", and because of that
  /// "import.meta.main" will return true when checked inside that module.
  ///
  /// User must call [`ModuleMap::mod_evaluate`] with returned `ModuleId`
  /// manually after load is finished.
  pub(crate) async fn load_main_es_module_from_code(
    &self,
    isolate: &mut v8::Isolate,
    specifier: &ModuleSpecifier,
    code: Option<ModuleCodeString>,
  ) -> Result<ModuleId, CoreError> {
    let module_map_rc = self.0.module_map();
    if let Some(code) = code {
      context_scope!(scope, self, isolate);
      // true for main module
      module_map_rc
        .new_es_module(
          scope,
          true,
          specifier.to_string().into(),
          code,
          false,
          None,
        )
        .map_err(|e| e.into_error(scope, false, false))?;
    }

    let mut load =
      ModuleMap::load_main(module_map_rc.clone(), specifier.to_string())
        .await?;

    while let Some(load_result) = load.next().await {
      let (request, info) = load_result?;
      context_scope!(scope, self, isolate);
      load
        .register_and_recurse(scope, &request, info)
        .map_err(|e| e.into_error(scope, false, false))?;
    }

    let root_id = load.root_module_id.expect("Root module should be loaded");
    context_scope!(scope, self, isolate);
    self.instantiate_module(scope, root_id).map_err(|e| {
      let exception = v8::Local::new(scope, e);
      exception_to_err(scope, exception, false, false)
    })?;
    Ok(root_id)
  }

  /// Asynchronously load specified ES module and all of its dependencies.
  ///
  /// This method is meant to be used when loading some utility code that
  /// might be later imported by the main module (ie. an entry point module).
  ///
  /// User must call [`ModuleMap::mod_evaluate`] with returned `ModuleId`
  /// manually after load is finished.
  // TODO(bartlomieju): create a separate method to execute code synchronously
  // from a loader? Would simplify JsRuntime code and not require running in
  // a `block_on`.
  pub(crate) async fn load_side_es_module_from_code(
    &self,
    isolate: &mut v8::Isolate,
    specifier: String,
    code: Option<ModuleCodeString>,
  ) -> Result<ModuleId, CoreError> {
    let module_map_rc = self.0.module_map();
    if let Some(code) = code {
      let specifier = specifier.to_owned();
      context_scope!(scope, self, isolate);
      // false for side module (not main module)
      module_map_rc
        .new_es_module(scope, false, specifier.into(), code, false, None)
        .map_err(|e| e.into_error(scope, false, false))?;
    }

    let mut load = ModuleMap::load_side(
      module_map_rc,
      specifier,
      crate::modules::SideModuleKind::Async,
      None,
    )
    .await?;

    while let Some(load_result) = load.next().await {
      let (request, info) = load_result?;
      context_scope!(scope, self, isolate);
      load
        .register_and_recurse(scope, &request, info)
        .map_err(|e| e.into_error(scope, false, false))?;
    }

    let root_id = load.root_module_id.expect("Root module should be loaded");
    context_scope!(scope, self, isolate);
    self.instantiate_module(scope, root_id).map_err(|e| {
      let exception = v8::Local::new(scope, e);
      exception_to_err(scope, exception, false, false)
    })?;
    Ok(root_id)
  }

  /// Load and evaluate an ES module provided the specifier and source code.
  ///
  /// The module should not have Top-Level Await (that is, it should be
  /// possible to evaluate it synchronously).
  ///
  /// It is caller's responsibility to ensure that not duplicate specifiers are
  /// passed to this method.
  pub(crate) fn lazy_load_es_module_with_code(
    &self,
    isolate: &mut v8::Isolate,
    module_specifier: ModuleName,
    code: ModuleCodeString,
  ) -> Result<v8::Global<v8::Value>, CoreError> {
    let module_map_rc = self.0.module_map();
    context_scope!(scope, self, isolate);
    module_map_rc.lazy_load_es_module_with_code(
      scope,
      module_specifier.as_str(),
      code,
      None,
    )
  }
}
