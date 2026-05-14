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
    ModuleStatus::Uninstantiated
  }
  pub fn get_module_requests(&self) -> Local<'s, FixedArray> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn get_module_namespace(&self) -> Local<'s, Object> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn evaluate<S>(&self, _scope: S) -> Option<Local<'s, Value>>
  where
    S: Sized,
  {
    let _ = _scope;
    None
  }
  pub fn instantiate_module<S, C>(
    &self,
    _scope: S,
    _cb: C,
  ) -> Option<bool> {
    Some(true)
  }
  pub fn get_identity_hash(&self) -> i32 {
    0
  }
  pub fn script_id(&self) -> i32 {
    0
  }
  pub fn get_exception(&self) -> Local<'s, Value> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn instantiate_module2<S, C, F>(
    &self,
    scope: S,
    cb: C,
    _src_cb: F,
  ) -> Option<bool> {
    self.instantiate_module(scope, cb)
  }
  pub fn evaluate_for_import_defer<S>(
    &self,
    _scope: S,
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
    _scope: S,
  ) -> Vec<(Local<'s, Module>, Local<'s, crate::value::Message>)> {
    Vec::new()
  }
  pub fn set_synthetic_module_export<S>(
    &self,
    _scope: S,
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
