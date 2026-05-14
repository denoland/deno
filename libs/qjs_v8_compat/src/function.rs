// Copyright 2018-2026 the Deno authors. MIT license.
//
// Function, FunctionTemplate, FunctionCallbackInfo, ReturnValue.

use crate::object::Object;
use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;
use crate::value::Value;

crate::value_type!(Function);

/// V8's `FunctionCallback` signature. We mirror it byte-for-byte so the
/// op2 macro expansions compile against either backend.
pub type FunctionCallback = unsafe extern "C" fn(*const FunctionCallbackInfo);

pub type PropertyCallback = unsafe extern "C" fn();

/// Adapter trait V8 uses to convert various function pointer flavors to
/// `FunctionCallback`. We mirror it.
pub trait MapFnTo<T> {
  fn map_fn_to(self) -> T;
}

impl MapFnTo<FunctionCallback> for FunctionCallback {
  fn map_fn_to(self) -> FunctionCallback {
    self
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
  /// The op2-generated code constructs args by handing in the raw info pointer.
  ///
  /// # Safety
  ///
  /// `info` must point to a valid `FunctionCallbackInfo` whose lifetime
  /// covers `'s`.
  pub unsafe fn from_function_callback_info(
    info: *const FunctionCallbackInfo,
  ) -> Self {
    unsafe { Self::from_raw(info) }
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
  /// The op2-generated code constructs the return slot from the raw info
  /// pointer; the slot lives at `implicit_args[V8_RETURN_VALUE_INDEX]`,
  /// which on V8 is implicit_args[0]; we mirror that layout.
  ///
  /// # Safety
  ///
  /// `info` must point to a valid `FunctionCallbackInfo` whose lifetime
  /// covers `'s`.
  pub unsafe fn from_function_callback_info(
    info: *const FunctionCallbackInfo,
  ) -> Self {
    unsafe {
      Self {
        slot: (*info).implicit_args,
        _t: std::marker::PhantomData,
      }
    }
  }
  pub fn set(&mut self, value: Local<'s, T>) {
    unsafe { *self.slot = value.raw }
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
  pub fn new_instance(
    &self,
    _scope: &mut HandleScope<'s>,
    _args: &[Local<'s, Value>],
  ) -> Option<Local<'s, Object>> {
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
