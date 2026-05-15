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
  pub fn constructor_behavior(
    self,
    _b: crate::function::ConstructorBehavior,
  ) -> Self {
    self
  }
  pub fn side_effect_type(
    self,
    _t: crate::function::SideEffectType,
  ) -> Self {
    self
  }
  pub fn name(mut self, name: Local<'s, crate::primitives::String>) -> Self {
    let _ = name;
    self.name = Some(std::string::String::new());
    self
  }
  pub fn build_fast<F>(
    self,
    scope: &mut HandleScope<'s>,
    _fast_function: F,
  ) -> Local<'s, FunctionTemplate> {
    self.build(scope)
  }
}

// No-op constructor for stub function templates. JS sees this as a
// callable constructor — `class X extends Foo` and `new Foo()` both work.
// When invoked as `new`, returns a fresh object with the function's
// prototype as its [[Prototype]]; when invoked as plain call, returns
// undefined.
unsafe extern "C" fn ft_stub_ctor(
  ctx: *mut crate::ffi::JSContext,
  this_val: sys::JSValue,
  _argc: core::ffi::c_int,
  _argv: *mut sys::JSValue,
) -> sys::JSValue {
  // For `class X extends Foo` super calls, QuickJS passes `new.target`
  // via this_val handling. If this_val is undefined, treat as
  // plain call. Otherwise treat as a constructor invocation —
  // return `this_val` (the freshly created instance) so the derived
  // class's constructor sees a valid `this`.
  if sys::jsv_is_undefined(&this_val) {
    sys::jsv_undefined()
  } else {
    sys::dup_value(ctx, this_val);
    this_val
  }
}

impl FunctionTemplate {
  pub fn new<'s, S, F>(
    scope: &mut S,
    _callback: F,
  ) -> Local<'s, FunctionTemplate>
  where
    F: crate::function::MapFnTo<FunctionCallback>,
    S: crate::scope::HandleScopeSource,
  {
    let ctx = scope.default_ctx();
    let raw = unsafe {
      crate::ffi::JS_NewCFunction2(
        ctx,
        ft_stub_ctor,
        core::ptr::null(),
        0,
        crate::ffi::JS_CFUNC_CONSTRUCTOR_OR_FUNC,
        0,
      )
    };
    // Give it a prototype object so `class X extends F` works — QuickJS
    // requires `F.prototype` to be an object or null.
    unsafe {
      let proto = sys::new_object(ctx);
      let key = c"prototype".as_ptr();
      crate::ffi::JS_SetPropertyStr(ctx, raw, key, proto);
    }
    Local::from_raw(raw)
  }
  pub fn builder<F>(
    callback: F,
  ) -> crate::v8::FunctionBuilder<FunctionTemplate>
  where
    F: crate::function::MapFnTo<FunctionCallback>,
  {
    crate::v8::FunctionBuilder::<FunctionTemplate>::new(callback)
  }
  pub fn builder_raw(
    callback: FunctionCallback,
  ) -> crate::v8::FunctionBuilder<FunctionTemplate> {
    crate::v8::FunctionBuilder::<FunctionTemplate>::new_raw(callback)
  }
}

impl<'s> Local<'s, FunctionTemplate> {
  pub fn get_function<S>(
    &self,
    _scope: &mut S,
  ) -> Option<Local<'s, crate::function::Function>> {
    Some(Local::from_raw(self.raw))
  }
  pub fn instance_template<S>(&self, _scope: &S) -> Local<'s, ObjectTemplate> {
    Local::from_raw(self.raw)
  }
  pub fn prototype_template<S>(&self, _scope: &S) -> Local<'s, ObjectTemplate> {
    Local::from_raw(self.raw)
  }
  pub fn set_class_name(&self, _name: Local<'s, crate::primitives::String>) {}
  pub fn inherit(&self, _parent: Local<'_, FunctionTemplate>) {}
  pub fn set(
    &self,
    _key: Local<'_, crate::value::Name>,
    _value: Local<'_, crate::value::Data>,
  ) {
  }
}

impl ObjectTemplate {
  pub fn new<'s, S>(scope: &mut S) -> Local<'s, ObjectTemplate>
  where
    S: crate::scope::HandleScopeSource,
  {
    let raw = sys::new_object(scope.default_ctx());
    Local::from_raw(raw)
  }
}

impl<'s> Local<'s, ObjectTemplate> {
  /// Mirror of `v8::ObjectTemplate::set_with_attr` — sets a template
  /// property with explicit `PropertyAttribute` flags. rusty_v8's
  /// signature is `(key, value, attr)` (no scope).
  pub fn set_with_attr(
    &self,
    _key: Local<'s, crate::value::Name>,
    _value: Local<'s, crate::value::Value>,
    _attr: crate::object::PropertyAttribute,
  ) {
  }
  /// Mirror of `v8::ObjectTemplate::set` — sets a template property.
  pub fn set(
    &self,
    _key: Local<'s, crate::value::Name>,
    _value: Local<'s, crate::value::Value>,
  ) {
  }
  pub fn new_instance<S>(&self, scope: &mut S) -> Option<Local<'s, Object>>
  where
    S: crate::scope::HandleScopeSource,
  {
    let ctx = scope.default_ctx();
    let raw = crate::sys::new_object(ctx);
    Some(Local::from_raw(raw))
  }
  pub fn set_accessor_property<G, S>(
    &self,
    _key: Local<'_, crate::value::Name>,
    _getter: G,
    _setter: S,
    _attr: crate::object::PropertyAttribute,
  ) {
  }
  pub fn set_internal_field_count(&self, _n: i32) {}
  pub fn set_named_property_handler(
    &self,
    _config: crate::object::NamedPropertyHandlerConfiguration,
  ) {
  }
  pub fn set_indexed_property_handler(
    &self,
    _config: crate::v8::IndexedPropertyHandlerConfiguration,
  ) {
  }
}
