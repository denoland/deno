// Copyright 2018-2026 the Deno authors. MIT license.
//
// Function, FunctionTemplate, FunctionCallbackInfo, ReturnValue.

use crate::object::Object;
use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;
use crate::value::Value;

crate::value_type!(Function);

impl Function {
  /// Mirror of `v8::Function::builder(callback)`.
  pub fn builder(
    callback: FunctionCallback,
  ) -> crate::v8::FunctionBuilder<Function> {
    crate::v8::FunctionBuilder::<Function>::new(callback)
  }
}

/// V8's `FunctionCallback` signature. We mirror it byte-for-byte so the
/// op2 macro expansions compile against either backend.
pub type FunctionCallback = unsafe extern "C" fn(*const FunctionCallbackInfo);

pub type PropertyCallback = unsafe extern "C" fn();

/// Adapter trait V8 uses to convert various function pointer flavors to
/// `FunctionCallback`. We mirror it. Implemented for FunctionCallback
/// itself and for Rust function pointers with the op2-generated shape;
/// the conversion is type-only (we never actually invoke the trampoline
/// — QuickJS doesn't dispatch via v8's C ABI), so the convert just
/// returns a stub.
pub trait MapFnTo<T> {
  fn map_fn_to(self) -> T;
}

impl MapFnTo<FunctionCallback> for FunctionCallback {
  fn map_fn_to(self) -> FunctionCallback {
    self
  }
}

// Op2-generated callback shapes:
// `fn(&mut PinScope, FunctionCallbackArguments, ReturnValue)`
// FunctionCallback is `unsafe extern "C" fn(*const FunctionCallbackInfo)`.
unsafe extern "C" fn map_fn_to_stub(_info: *const FunctionCallbackInfo) {}

impl<F> MapFnTo<FunctionCallback> for F
where
  F: Fn(
    &mut crate::scope::PinScope<'_, '_>,
    FunctionCallbackArguments<'_>,
    ReturnValue<'_>,
  ),
{
  fn map_fn_to(self) -> FunctionCallback {
    map_fn_to_stub
  }
}

/// `FunctionCallbackInfo` carries (this, argv, argc). On QuickJS the
/// equivalent shape is `(this_val, argc, argv)`; we plant the same
/// layout in memory so op2 generated code can reach in.
#[repr(C)]
pub struct FunctionCallbackInfo {
  // Pointer to argument vector and length. Same layout as v8::internal.
  pub(crate) implicit_args: *mut sys::JSValue,
  pub(crate) values: *mut sys::JSValue,
  pub(crate) length: i32,
}

/// Argument accessor wrapper.
pub struct FunctionCallbackArguments<'s> {
  info: *const FunctionCallbackInfo,
  _scope: std::marker::PhantomData<&'s ()>,
}

impl<'s> FunctionCallbackArguments<'s> {
  pub unsafe fn from_raw(info: *const FunctionCallbackInfo) -> Self {
    Self {
      info,
      _scope: std::marker::PhantomData,
    }
  }
  /// Mirrors rusty_v8's `FunctionCallbackArguments::from_function_callback_info`.
  /// The op2-generated code constructs args by handing in the raw info
  /// pointer without an `unsafe` block, so we accept the raw pointer in
  /// safe context here even though dereferencing it is up to the caller.
  pub fn from_function_callback_info(
    info: *const FunctionCallbackInfo,
  ) -> Self {
    Self {
      info,
      _scope: std::marker::PhantomData,
    }
  }
  pub fn length(&self) -> i32 {
    unsafe { (*self.info).length }
  }
  pub fn get(&self, idx: i32) -> Local<'s, Value> {
    let raw = unsafe {
      if idx < 0 || idx >= (*self.info).length {
        sys::jsv_undefined()
      } else {
        *((*self.info).values.offset(idx as isize))
      }
    };
    Local::from_raw(raw)
  }
  pub fn this(&self) -> Local<'s, Object> {
    // implicit_args[0] = this in V8's layout. We mirror.
    let raw = unsafe { *(*self.info).implicit_args };
    Local::from_raw(raw)
  }
  pub fn holder(&self) -> Local<'s, Object> {
    self.this()
  }
  pub fn data(&self) -> Local<'s, Value> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn new_target(&self) -> Local<'s, Value> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn is_construct_call(&self) -> bool {
    false
  }
}

/// `ReturnValue<T>` — a stash slot for the function's return.
pub struct ReturnValue<'s, T = Value> {
  slot: *mut sys::JSValue,
  _t: std::marker::PhantomData<&'s T>,
}

impl<'s, T> ReturnValue<'s, T> {
  /// Mirrors rusty_v8's `ReturnValue::from_function_callback_info`.
  /// Safe wrapper to match op2's call sites (which don't use `unsafe`).
  pub fn from_function_callback_info(
    info: *const FunctionCallbackInfo,
  ) -> Self {
    let slot = unsafe { (*info).implicit_args };
    Self {
      slot,
      _t: std::marker::PhantomData,
    }
  }
  pub fn set<'a>(&mut self, value: Local<'a, T>) {
    unsafe { *self.slot = value.raw() }
  }
  pub fn set_undefined(&mut self) {
    unsafe { *self.slot = sys::jsv_undefined() }
  }
  pub fn set_null(&mut self) {
    unsafe { *self.slot = sys::jsv_null() }
  }
  pub fn set_bool(&mut self, b: bool) {
    unsafe { *self.slot = sys::jsv_bool(b) }
  }
  pub fn set_int32(&mut self, v: i32) {
    unsafe { *self.slot = sys::jsv_int32(v) }
  }
  pub fn set_double(&mut self, v: f64) {
    unsafe { *self.slot = sys::jsv_float64(v) }
  }
  pub fn set_empty_string(&mut self) {
    self.set_null();
  }
}

impl<'s> Local<'s, Function> {
  pub fn call(
    &self,
    _scope: &mut HandleScope<'s>,
    _recv: Local<'s, Value>,
    _args: &[Local<'s, Value>],
  ) -> Option<Local<'s, Value>> {
    None
  }
  pub fn new_instance<S>(
    &self,
    _scope: &mut S,
    _args: &[Local<'s, Value>],
  ) -> Option<Local<'s, Object>> {
    None
  }
  pub fn set_name(&self, _name: Local<'_, crate::primitives::String>) {}
  pub fn create_code_cache(&self) -> Option<Box<crate::external::CachedData>> {
    None
  }
}

impl Function {
  pub fn new<'s, S, F>(
    _scope: &mut S,
    _callback: F,
  ) -> Option<Local<'s, Function>>
  where
    F: crate::function::MapFnTo<crate::function::FunctionCallback>,
  {
    None
  }
}

/// Side-effect markers V8 lets you attach to functions.
#[derive(Copy, Clone)]
pub enum SideEffectType {
  HasSideEffect,
  HasNoSideEffect,
}

#[derive(Copy, Clone)]
pub enum ConstructorBehavior {
  Allow,
  Throw,
}

#[derive(Copy, Clone)]
pub enum FunctionCodeHandling {
  Keep,
  Clear,
}
