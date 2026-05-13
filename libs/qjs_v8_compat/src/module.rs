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

impl<'s> Local<'s, Module> {
  pub fn get_status(&self) -> ModuleStatus {
    ModuleStatus::Uninstantiated
  }
  pub fn get_module_requests<'a>(
    &self,
    _scope: &mut HandleScope<'s>,
  ) -> Local<'s, FixedArray> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn get_module_namespace(
    &self,
    _scope: &mut HandleScope<'s>,
  ) -> Local<'s, Object> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn evaluate(
    &self,
    _scope: &mut HandleScope<'s>,
  ) -> Option<Local<'s, Value>> {
    None
  }
  pub fn instantiate_module(
    &self,
    _scope: &mut HandleScope<'s>,
    _cb: ModuleResolveCallback,
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
}

impl<'s> Local<'s, FixedArray> {
  pub fn length(&self) -> i32 {
    0
  }
  pub fn get(
    &self,
    _scope: &mut HandleScope<'s>,
    _index: i32,
  ) -> Local<'s, Value> {
    Local::from_raw(sys::jsv_undefined())
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
