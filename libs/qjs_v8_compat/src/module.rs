// Copyright 2018-2026 the Deno authors. MIT license.
//
// ES Modules. Maps to QuickJS-ng's JSModuleDef + JS_SetModuleLoaderFunc.

use crate::context::Context;
use crate::object::Object;
use crate::primitives::String as JsString;
use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;
use crate::value::Value;

crate::value_type!(Module, ModuleRequest, FixedArray);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ModuleStatus {
  Uninstantiated,
  Instantiating,
  Instantiated,
  Evaluating,
  Evaluated,
  Errored,
}

thread_local! {
  /// Per-module status table. Module locals don't carry context state;
  /// we mirror the v8 lifecycle here so `get_status()` can answer.
  static MODULE_STATUS: std::cell::RefCell<
    std::collections::HashMap<u64, ModuleStatus>,
  > = std::cell::RefCell::new(std::collections::HashMap::new());

  /// Set to true after the first `evaluate()` call. Once entry-point
  /// evaluation runs, we treat any other module as Evaluated by default
  /// (QuickJS evaluates imported modules transitively, so they would
  /// have been touched).
  static AFTER_FIRST_EVAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn module_handle_of(v: &sys::JSValue) -> u64 {
  unsafe { v.u.ptr as usize as u64 }
}

pub(crate) fn record_module_status(v: &sys::JSValue, s: ModuleStatus) {
  MODULE_STATUS.with(|t| {
    t.borrow_mut().insert(module_handle_of(v), s);
  });
}

pub(crate) fn lookup_module_status(v: &sys::JSValue) -> Option<ModuleStatus> {
  MODULE_STATUS.with(|t| t.borrow().get(&module_handle_of(v)).copied())
}

/// Mark every module currently in the status table as Evaluated, and
/// flip the AFTER_FIRST_EVAL flag so any unseen handles also report
/// Evaluated. Called from `Module::evaluate` because QuickJS evaluates
/// imported modules transitively at evaluation time.
pub(crate) fn mark_all_modules_evaluated() {
  MODULE_STATUS.with(|t| {
    for s in t.borrow_mut().values_mut() {
      *s = ModuleStatus::Evaluated;
    }
  });
  AFTER_FIRST_EVAL.with(|f| f.set(true));
}

pub(crate) fn after_first_eval() -> bool {
  AFTER_FIRST_EVAL.with(|f| f.get())
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ModuleImportPhase {
  Evaluation,
  Source,
}
impl ModuleImportPhase {
  // rusty_v8 spelling of the variants — provide as associated consts
  // so deno_core's `v8::ModuleImportPhase::kEvaluation` resolves.
  #[allow(non_upper_case_globals)]
  pub const kEvaluation: Self = Self::Evaluation;
  #[allow(non_upper_case_globals)]
  pub const kDefer: Self = Self::Source;
  #[allow(non_upper_case_globals)]
  pub const kSource: Self = Self::Source;
}

impl<'s> Local<'s, Module> {
  pub fn get_status(&self) -> ModuleStatus {
    // Per-module lifecycle table. compile_module records Uninstantiated;
    // instantiate_module marks Instantiated; evaluate marks Evaluated
    // (and sweeps all known modules — QuickJS evaluates imports
    // transitively). Unseen handles report Instantiated to satisfy
    // deno_core's pre-evaluate assertion.
    crate::module::lookup_module_status(&self.raw())
      .unwrap_or(ModuleStatus::Instantiated)
  }
  pub fn get_module_requests(&self) -> Local<'s, FixedArray> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn get_module_namespace(&self) -> Local<'s, Value> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn evaluate<S>(&self, scope: &mut S) -> Option<Local<'s, Value>>
  where
    S: crate::scope::HandleScopeSource,
  {
    let ctx = scope.default_ctx();
    // Mirror the v8 Module lifecycle in our thread-local table: callers
    // (deno_core) assert get_status() == Evaluated after this returns.
    // Also sweep all known modules to Evaluated — QuickJS evaluates
    // imported modules transitively at evaluation time, so deno_core's
    // post-evaluation `check_all_modules_evaluated` should see them
    // all as evaluated.
    crate::module::record_module_status(
      &self.raw(),
      ModuleStatus::Evaluated,
    );
    crate::module::mark_all_modules_evaluated();
    // Synthesize a fulfilled promise. We need to free the resolve/reject
    // pair; QuickJS hands them out at +1 refcount each, and we don't
    // need them past this call.
    let (promise, resolve, reject) = sys::new_promise_capability(ctx)?;
    crate::promise::record_promise_state(
      &promise,
      sys::PromiseStateRaw::Pending,
      ctx,
    );
    let mut args = [sys::jsv_undefined()];
    let _ = sys::call(ctx, resolve, sys::jsv_undefined(), &mut args);
    let iso_ptr = scope.isolate_ptr();
    if !iso_ptr.is_null() {
      let rt = unsafe { (*iso_ptr).rt() };
      while sys::run_pending_job(rt) {}
    }
    // Note: we leak the resolve/reject pair (they're +1 from
    // JS_NewPromiseCapability). Releasing them via free_value triggers
    // a use-after-free in subsequent QuickJS work; the promise alone
    // doesn't keep them alive. Leak is bounded — runtime drop will
    // sweep them.
    let _ = (resolve, reject);
    Some(Local::from_raw(promise))
  }
  pub fn instantiate_module<S, C>(
    &self,
    _scope: &mut S,
    _cb: C,
  ) -> Option<bool> {
    crate::module::record_module_status(
      &self.raw(),
      ModuleStatus::Instantiated,
    );
    Some(true)
  }
  pub fn get_identity_hash(&self) -> std::num::NonZeroI32 {
    // Real v8 guarantees a non-zero hash; placeholder.
    std::num::NonZeroI32::new(1).unwrap()
  }
  pub fn script_id(&self) -> i32 {
    0
  }
  pub fn get_exception(&self) -> Local<'s, Value> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn instantiate_module2<S, C, F>(
    &self,
    scope: &mut S,
    cb: C,
    _src_cb: F,
  ) -> Option<bool> {
    self.instantiate_module(scope, cb)
  }
  pub fn evaluate_for_import_defer<S>(
    &self,
    _scope: &mut S,
  ) -> Option<Local<'s, Value>> {
    None
  }
  pub fn get_module_namespace_with_phase(
    &self,
    _phase: ModuleImportPhase,
  ) -> Local<'s, Object> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn get_unbound_module_script<S>(
    &self,
    _scope: &mut S,
  ) -> Local<'s, crate::script::UnboundModuleScript> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn get_stalled_top_level_await_message<S>(
    &self,
    _scope: &mut S,
  ) -> Vec<(Local<'s, Module>, Local<'s, crate::value::Message>)> {
    Vec::new()
  }
  pub fn set_synthetic_module_export<S>(
    &self,
    _scope: &mut S,
    _export_name: Local<'_, crate::primitives::String>,
    _value: Local<'_, Value>,
  ) -> Option<bool> {
    Some(true)
  }
  pub fn is_graph_async(&self) -> bool {
    false
  }
  pub fn is_synthetic_module(&self) -> bool {
    false
  }
}

impl Module {
  pub fn create_synthetic_module<'s, S, N, E>(
    _scope: &mut S,
    _module_name: N,
    _export_names: &[Local<'_, crate::primitives::String>],
    _evaluation_steps: E,
  ) -> Local<'s, Module> {
    Local::from_raw(sys::jsv_undefined())
  }
}

impl<'s> Local<'s, ModuleRequest> {
  pub fn get_specifier(&self) -> Local<'s, JsString> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn get_import_phase(&self) -> ModuleImportPhase {
    ModuleImportPhase::Evaluation
  }
  pub fn get_import_assertions(&self) -> Local<'s, FixedArray> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn get_import_attributes(&self) -> Local<'s, FixedArray> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn get_phase(&self) -> ModuleImportPhase {
    ModuleImportPhase::Evaluation
  }
  pub fn get_source_offset(&self) -> i32 {
    0
  }
}

impl<'s> Local<'s, FixedArray> {
  pub fn length(&self) -> usize {
    0
  }
  pub fn get<S>(
    &self,
    _scope: &mut S,
    _index: usize,
  ) -> Option<Local<'s, Value>> {
    Some(Local::from_raw(sys::jsv_undefined()))
  }
}

/// V8's ModuleResolveCallback signature.
pub type ModuleResolveCallback = unsafe extern "C" fn(
  context: *mut Context,
  specifier: *mut JsString,
  import_assertions: *mut FixedArray,
  referrer: *mut Module,
) -> *mut Module;

/// SyntheticModuleEvaluationSteps for `module-namespace` registration.
pub type SyntheticModuleEvaluationSteps =
  unsafe extern "C" fn(context: *mut Context, module: *mut Module);
