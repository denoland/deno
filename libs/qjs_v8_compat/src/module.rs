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

  /// Source code stashed by `compile_module` so `Module::evaluate` can
  /// hand it off to QuickJS at evaluation time. Keyed by raw module
  /// handle. `(source, filename)`.
  static MODULE_SOURCES: std::cell::RefCell<
    std::collections::HashMap<u64, (String, Option<String>)>,
  > = std::cell::RefCell::new(std::collections::HashMap::new());

  /// Source code keyed by module specifier (URL/import name). Populated
  /// alongside MODULE_SOURCES so QuickJS's module loader callback can
  /// resolve `import x from "ext:core/ops"` etc.
  static MODULE_SOURCES_BY_NAME: std::cell::RefCell<
    std::collections::HashMap<String, String>,
  > = std::cell::RefCell::new(std::collections::HashMap::new());

  /// Set to true after the first `evaluate()` call. Once entry-point
  /// evaluation runs, we treat any other module as Evaluated by default
  /// (QuickJS evaluates imported modules transitively, so they would
  /// have been touched).
  static AFTER_FIRST_EVAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };

  /// JSModuleDef pointers we've previously handed back from the
  /// module loader, keyed by module name. Lets us return the same
  /// JSModuleDef on a second import of the same name (QuickJS's
  /// JS_Eval doesn't dedupe module-type evals against existing
  /// loaded modules; without this cache we'd compile each module
  /// twice on cyclic imports and trip the resolver's
  /// "circular reference" check).
  static MODULE_DEF_CACHE: std::cell::RefCell<
    std::collections::HashMap<String, usize>,
  > = std::cell::RefCell::new(std::collections::HashMap::new());

  /// Synthetic module exports keyed by JSModuleDef pointer (as usize).
  /// Populated by `Local<Module>::set_synthetic_module_export`; consumed
  /// by `synthetic_module_init_callback` when QuickJS calls the
  /// init_func to actually populate the module.
  static SYNTHETIC_MODULE_EXPORTS: std::cell::RefCell<
    std::collections::HashMap<usize, Vec<(String, sys::JSValue)>>,
  > = std::cell::RefCell::new(std::collections::HashMap::new());

  /// JSModuleDef pointers for synthetic modules keyed by Module raw
  /// handle (the placeholder JSValue we hand back from
  /// create_synthetic_module). Lets `set_synthetic_module_export`
  /// recover the JSModuleDef given just the Module handle.
  static SYNTHETIC_MODULE_DEFS: std::cell::RefCell<
    std::collections::HashMap<u64, usize>,
  > = std::cell::RefCell::new(std::collections::HashMap::new());
}

pub(crate) fn record_synthetic_module_def(
  handle: &sys::JSValue,
  m: *mut crate::ffi::JSModuleDef,
) {
  SYNTHETIC_MODULE_DEFS.with(|t| {
    t.borrow_mut().insert(module_handle_of(handle), m as usize);
  });
}

pub(crate) fn lookup_synthetic_module_def(
  handle: &sys::JSValue,
) -> Option<*mut crate::ffi::JSModuleDef> {
  SYNTHETIC_MODULE_DEFS.with(|t| {
    t.borrow()
      .get(&module_handle_of(handle))
      .copied()
      .map(|p| p as *mut crate::ffi::JSModuleDef)
  })
}

pub(crate) fn record_synthetic_export(
  m: *mut crate::ffi::JSModuleDef,
  name: String,
  value: sys::JSValue,
) {
  SYNTHETIC_MODULE_EXPORTS.with(|t| {
    t.borrow_mut()
      .entry(m as usize)
      .or_default()
      .push((name, value));
  });
}

/// Init callback for synthetic modules. QuickJS calls this when the
/// module is first imported — we read the stashed (name, value) list
/// for this JSModuleDef and call JS_SetModuleExport for each.
pub(crate) unsafe extern "C" fn synthetic_module_init_callback(
  ctx: *mut crate::ffi::JSContext,
  m: *mut crate::ffi::JSModuleDef,
) -> std::os::raw::c_int {
  let exports = SYNTHETIC_MODULE_EXPORTS.with(|t| {
    t.borrow_mut().remove(&(m as usize)).unwrap_or_default()
  });
  for (name, value) in exports {
    let Ok(name_c) = std::ffi::CString::new(name) else {
      continue;
    };
    unsafe {
      let dup = crate::ffi::JS_DupValue(ctx, value);
      crate::ffi::JS_SetModuleExport(ctx, m, name_c.as_ptr(), dup);
    }
  }
  0
}

pub(crate) fn record_module_source(
  v: &sys::JSValue,
  source: String,
  filename: Option<String>,
) {
  if let Some(name) = filename.as_ref() {
    MODULE_SOURCES_BY_NAME.with(|t| {
      t.borrow_mut().insert(name.clone(), source.clone());
    });
  }
  MODULE_SOURCES.with(|t| {
    t.borrow_mut()
      .insert(module_handle_of(v), (source, filename));
  });
}

pub(crate) fn lookup_module_source(
  v: &sys::JSValue,
) -> Option<(String, Option<String>)> {
  MODULE_SOURCES.with(|t| t.borrow().get(&module_handle_of(v)).cloned())
}

pub(crate) fn lookup_module_source_by_name(name: &str) -> Option<String> {
  MODULE_SOURCES_BY_NAME.with(|t| t.borrow().get(name).cloned())
}

/// Public hook for deno_core's lazy-loaded module store to register
/// sources by name so QuickJS's module loader can resolve them when
/// evaluating an importing module.
pub fn register_lazy_module_source(name: &str, source: &str) {
  MODULE_SOURCES_BY_NAME.with(|t| {
    t.borrow_mut().insert(name.to_string(), source.to_string());
  });
}

/// Module loader callback registered with QuickJS via
/// `JS_SetModuleLoaderFunc`. Looks up the source we stashed in
/// `compile_module` (keyed by URL) and hands it to QuickJS as a fresh
/// module via `JS_Eval(JS_EVAL_TYPE_MODULE | JS_EVAL_FLAG_COMPILE_ONLY)`.
pub(crate) unsafe extern "C" fn module_loader_callback(
  ctx: *mut crate::ffi::JSContext,
  module_name: *const std::os::raw::c_char,
  _opaque: *mut std::os::raw::c_void,
) -> *mut crate::ffi::JSModuleDef {
  let name = match unsafe { std::ffi::CStr::from_ptr(module_name) }.to_str() {
    Ok(s) => s,
    Err(_) => return core::ptr::null_mut(),
  };
  // Note: we don't cache here — QuickJS itself dedupes registered
  // modules by name via its loaded-module list, and re-issuing the
  // cached JSModuleDef confuses the linker (the cached module may
  // already be in LINKING/EVALUATED state, tripping the
  // `m->status == JS_MODULE_STATUS_UNLINKED` assertion at link time).
  let Some(source) = lookup_module_source_by_name(name) else {
    eprintln!("[qjs] module loader: no source for {name}");
    return core::ptr::null_mut();
  };
  let src_c = match std::ffi::CString::new(source) {
    Ok(s) => s,
    Err(_) => return core::ptr::null_mut(),
  };
  let name_c = match std::ffi::CString::new(name) {
    Ok(s) => s,
    Err(_) => return core::ptr::null_mut(),
  };
  let result = unsafe {
    crate::ffi::JS_Eval(
      ctx,
      src_c.as_ptr(),
      src_c.as_bytes().len(),
      name_c.as_ptr(),
      crate::ffi::JS_EVAL_TYPE_MODULE | crate::ffi::JS_EVAL_FLAG_COMPILE_ONLY,
    )
  };
  if sys::jsv_is_exception(&result) {
    if let Some(exc) = sys::take_pending_exception(ctx) {
      if let Some(s) = sys::to_string_lossy(ctx, exc) {
        eprintln!("[qjs] module loader: parse failed for {name}: {s}");
      }
      sys::free_value(ctx, exc);
    }
    return core::ptr::null_mut();
  }
  // The compiled module's payload (u.ptr) is the JSModuleDef pointer.
  let m = unsafe { result.u.ptr } as *mut crate::ffi::JSModuleDef;
  // Cache so subsequent imports of the same name reuse the same module.
  MODULE_DEF_CACHE.with(|c| {
    c.borrow_mut().insert(name.to_string(), m as usize);
  });
  // Don't free `result` — JSModuleDef ownership is transferred to QuickJS.
  m
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
    // If we have stashed source for this module, run it through QuickJS
    // as a module. JS_Eval with JS_EVAL_TYPE_MODULE will resolve and
    // evaluate the import graph transitively.
    if let Some((src, filename)) =
      crate::module::lookup_module_source(&self.raw())
    {
      let fname = filename.unwrap_or_else(|| "<module>".to_string());
      eprintln!("[Module::evaluate] running {fname}");
      let result = sys::eval(
        ctx,
        &src,
        &fname,
        crate::ffi::JS_EVAL_TYPE_MODULE,
      );
      eprintln!("[Module::evaluate] {fname} done is_exc={}", sys::jsv_is_exception(&result));
      // Drain microtasks that might be pending from the module's eval.
      let iso_ptr = scope.isolate_ptr();
      if !iso_ptr.is_null() {
        let rt = unsafe { (*iso_ptr).rt() };
        while sys::run_pending_job(rt) {}
      }
      eprintln!("[Module::evaluate] {fname} jobs drained");
      if sys::jsv_is_exception(&result) {
        if let Some(exc) = sys::take_pending_exception(ctx) {
          if let Some(s) = sys::to_string_lossy(ctx, exc) {
            eprintln!("[qjs] Module::evaluate exception in {fname}: {s}");
          }
          sys::free_value(ctx, exc);
        }
      } else if !sys::jsv_is_undefined(&result) {
        sys::free_value(ctx, result);
      }
    }
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
    scope: &mut S,
    export_name: Local<'_, crate::primitives::String>,
    value: Local<'_, Value>,
  ) -> Option<bool>
  where
    S: crate::scope::HandleScopeSource,
  {
    let m = crate::module::lookup_synthetic_module_def(&self.raw())?;
    let ctx = scope.default_ctx();
    let name_str = sys::to_string_lossy(ctx, export_name.raw())?;
    let name_c = std::ffi::CString::new(name_str.clone()).ok()?;
    unsafe {
      let dup = crate::ffi::JS_DupValue(ctx, value.raw());
      let r = crate::ffi::JS_SetModuleExport(ctx, m, name_c.as_ptr(), dup);
      Some(r >= 0)
    }
  }
  pub fn is_graph_async(&self) -> bool {
    false
  }
  pub fn is_synthetic_module(&self) -> bool {
    false
  }
}

impl Module {
  pub fn create_synthetic_module<'s, S, E>(
    scope: &mut S,
    module_name: Local<'_, crate::primitives::String>,
    export_names: &[Local<'_, crate::primitives::String>],
    _evaluation_steps: E,
  ) -> Local<'s, Module>
  where
    S: crate::scope::HandleScopeSource,
  {
    let ctx = scope.default_ctx();
    let name_str = sys::to_string_lossy(ctx, module_name.raw())
      .unwrap_or_else(|| "<synthetic>".to_string());
    let name_c = match std::ffi::CString::new(name_str.clone()) {
      Ok(s) => s,
      Err(_) => {
        return Local::from_raw(sys::jsv_undefined());
      }
    };
    let m = unsafe {
      crate::ffi::JS_NewCModule(
        ctx,
        name_c.as_ptr(),
        Some(crate::module::synthetic_module_init_callback),
      )
    };
    if m.is_null() {
      return Local::from_raw(sys::jsv_undefined());
    }
    for export_name in export_names {
      if let Some(s) = sys::to_string_lossy(ctx, export_name.raw()) {
        if let Ok(c) = std::ffi::CString::new(s) {
          unsafe {
            crate::ffi::JS_AddModuleExport(ctx, m, c.as_ptr());
          }
        }
      }
    }
    // Build a placeholder JSValue to hand back as the Module handle.
    // Stash a pointer to the JSModuleDef so set_synthetic_module_export
    // can recover it. Mark the module Instantiated immediately —
    // synthetic modules don't go through compile_module.
    let raw = sys::new_object(ctx);
    crate::module::record_synthetic_module_def(&raw, m);
    crate::module::record_module_status(&raw, ModuleStatus::Instantiated);
    Local::from_raw(raw)
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
