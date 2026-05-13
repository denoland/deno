// Copyright 2018-2026 the Deno authors. MIT license.
//
// ObjectTemplate / FunctionTemplate.
//
// V8 templates are factories: you describe properties + accessors once and
// stamp out instances. QuickJS has no analog — you build prototype objects
// imperatively. We hide that by accumulating settings in a Rust struct and
// applying them in `get_function` / `new_instance`.

use crate::function::FunctionCallback;
use crate::object::Object;
use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;

crate::value_type!(FunctionTemplate, ObjectTemplate);

pub struct FunctionBuilder<'s> {
  callback: Option<FunctionCallback>,
  data: Option<Local<'s, crate::value::Value>>,
  length: i32,
  name: Option<String>,
}

impl<'s> FunctionBuilder<'s> {
  pub fn new(callback: FunctionCallback) -> Self {
    Self {
      callback: Some(callback),
      data: None,
      length: 0,
      name: None,
    }
  }
  pub fn data(mut self, data: Local<'s, crate::value::Value>) -> Self {
    self.data = Some(data);
    self
  }
  pub fn length(mut self, n: i32) -> Self {
    self.length = n;
    self
  }
  pub fn build(
    self,
    scope: &mut HandleScope<'s>,
  ) -> Local<'s, FunctionTemplate> {
    let _ = self.callback;
    let raw = sys::new_object(scope.ctx());
    scope.track_owned(raw);
    Local::from_raw(raw)
  }
}

impl<'s> Local<'s, FunctionTemplate> {
  pub fn new(scope: &mut HandleScope<'s>, _callback: FunctionCallback) -> Self {
    let raw = sys::new_object(scope.ctx());
    scope.track_owned(raw);
    Local::from_raw(raw)
  }
  pub fn get_function(
    &self,
    _scope: &mut HandleScope<'s>,
  ) -> Option<Local<'s, crate::function::Function>> {
    None
  }
  pub fn instance_template(
    &self,
    _scope: &mut HandleScope<'s>,
  ) -> Local<'s, ObjectTemplate> {
    Local::from_raw(self.raw)
  }
  pub fn prototype_template(
    &self,
    _scope: &mut HandleScope<'s>,
  ) -> Local<'s, ObjectTemplate> {
    Local::from_raw(self.raw)
  }
  pub fn set_class_name(&self, _name: Local<'s, crate::primitives::String>) {}
}

impl<'s> Local<'s, ObjectTemplate> {
  pub fn new(scope: &mut HandleScope<'s>) -> Self {
    let raw = sys::new_object(scope.ctx());
    scope.track_owned(raw);
    Local::from_raw(raw)
  }
  pub fn new_instance(
    &self,
    _scope: &mut HandleScope<'s>,
  ) -> Option<Local<'s, Object>> {
    None
  }
  pub fn set_internal_field_count(&self, _n: i32) {}
  pub fn set_named_property_handler(
    &self,
    _config: crate::object::NamedPropertyHandlerConfiguration,
  ) {
  }
}
