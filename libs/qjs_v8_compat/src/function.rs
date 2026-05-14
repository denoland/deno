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
  /// Mirror of `v8::Function::builder(callback)`. Generic over any
  /// callable that satisfies `MapFnTo<FunctionCallback>` — accepts
  /// closures with the op2 (scope, args, rv) shape.
  pub fn builder<F>(callback: F) -> crate::v8::FunctionBuilder<Function>
  where
    F: MapFnTo<FunctionCallback>,
  {
    crate::v8::FunctionBuilder::<Function>::new(callback.map_fn_to())
  }
  pub fn builder_raw(
    callback: FunctionCallback,
  ) -> crate::v8::FunctionBuilder<Function> {
    crate::v8::FunctionBuilder::<Function>::new_raw(callback)
  }
}

/// V8's `FunctionCallback` signature. We mirror it byte-for-byte so the
/// op2 macro expansions compile against either backend.
pub type FunctionCallback = unsafe extern "C" fn(*const FunctionCallbackInfo);

pub type PropertyCallback = unsafe extern "C" fn();
pub type WasmStreamingCallback = unsafe extern "C" fn();
pub type FunctionCallbackOptions = ();
pub type AccessorNameSetterCallback = unsafe extern "C" fn();
pub type AccessorNameGetterCallback = unsafe extern "C" fn();

/// Adapter trait V8 uses to convert various function pointer flavors to
/// `FunctionCallback`. We mirror it. Implemented for FunctionCallback
/// itself and for Rust function pointers with the op2-generated shape;
/// the conversion is type-only (we never actually invoke the trampoline
/// — QuickJS doesn't dispatch via v8's C ABI), so the convert just
/// returns a stub.
pub trait MapFnTo<T> {
  fn map_fn_to(self) -> T;
}

// SyntheticModuleEvaluationSteps shape — used by Module::create_synthetic_module
unsafe extern "C" fn syn_eval_stub(
  _ctx: *mut crate::context::Context,
  _module: *mut crate::module::Module,
) {
}
impl<F> MapFnTo<crate::module::SyntheticModuleEvaluationSteps> for F
where
  F: MapFnToHelper,
{
  fn map_fn_to(self) -> crate::module::SyntheticModuleEvaluationSteps {
    syn_eval_stub
  }
}

// Op2-generated callback shapes:
// `fn(&mut PinScope, FunctionCallbackArguments, ReturnValue)`
// FunctionCallback is `unsafe extern "C" fn(*const FunctionCallbackInfo)`.
unsafe extern "C" fn map_fn_to_stub(_info: *const FunctionCallbackInfo) {}

impl<F> MapFnTo<FunctionCallback> for F
where
  F: MapFnToHelper,
{
  fn map_fn_to(self) -> FunctionCallback {
    map_fn_to_stub
  }
}

/// Helper trait — implemented for any callable. Wider than the previous
/// Fn(scope, args, rv) bound so deno_core's various callback shapes
/// (synthetic module evaluation steps, callsite functions, etc.) all
/// resolve to the same stub.
pub trait MapFnToHelper {}
impl<F> MapFnToHelper for F {}

/// `FunctionCallbackInfo` carries (this, argv, argc, data, rv). On the
/// QuickJS side our trampoline populates this struct freshly per-call
/// before invoking the op slow_fn pointer.
///
/// `implicit_args` is laid out as: [0] = this_val, [1] = data
/// (External carrying the OpCtx*), [2] = ReturnValue slot.
#[repr(C)]
pub struct FunctionCallbackInfo {
  pub(crate) implicit_args: *mut sys::JSValue,
  pub(crate) values: *mut sys::JSValue,
  pub(crate) length: i32,
}

pub(crate) const IMPLICIT_THIS_OFFSET: isize = 0;
pub(crate) const IMPLICIT_DATA_OFFSET: isize = 1;
pub(crate) const IMPLICIT_RV_OFFSET: isize = 2;
pub(crate) const IMPLICIT_LEN: usize = 3;

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
    let raw = unsafe { *(*self.info).implicit_args.offset(IMPLICIT_THIS_OFFSET) };
    Local::from_raw(raw)
  }
  pub fn holder(&self) -> Local<'s, Object> {
    self.this()
  }
  pub fn data(&self) -> Local<'s, Value> {
    let raw = unsafe { *(*self.info).implicit_args.offset(IMPLICIT_DATA_OFFSET) };
    Local::from_raw(raw)
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

impl<'s> ReturnValue<'s, Value> {
  /// Mirrors rusty_v8's `ReturnValue::from_function_callback_info`.
  /// Safe wrapper to match op2's call sites (which don't use `unsafe`).
  /// Confined to `T = Value` (the default) so call sites that don't
  /// otherwise constrain `T` get a concrete type rather than triggering
  /// `cannot infer type` from the inherent default.
  pub fn from_function_callback_info(
    info: *const FunctionCallbackInfo,
  ) -> Self {
    let slot =
      unsafe { (*info).implicit_args.offset(IMPLICIT_RV_OFFSET) };
    Self {
      slot,
      _t: std::marker::PhantomData,
    }
  }
}

impl<'s, T> ReturnValue<'s, T> {
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
  pub fn set_uint32(&mut self, v: u32) {
    if v <= i32::MAX as u32 {
      unsafe { *self.slot = sys::jsv_int32(v as i32) }
    } else {
      unsafe { *self.slot = sys::jsv_float64(v as f64) }
    }
  }
  pub fn set_double(&mut self, v: f64) {
    unsafe { *self.slot = sys::jsv_float64(v) }
  }
  pub fn set_empty_string(&mut self) {
    self.set_null();
  }
}

impl<'s> Local<'s, Function> {
  pub fn call<S>(
    &self,
    scope: &mut S,
    recv: Local<'s, Value>,
    args: &[Local<'s, Value>],
  ) -> Option<Local<'s, Value>>
  where
    S: crate::scope::HandleScopeSource,
  {
    let ctx = scope.default_ctx();
    let mut argv: Vec<sys::JSValue> = args.iter().map(|a| a.raw()).collect();
    let raw = sys::call(ctx, self.raw(), recv.raw(), argv.as_mut_slice());
    if sys::jsv_is_exception(&raw) {
      if let Some(exc) = sys::take_pending_exception(ctx) {
        if let Some(s) = sys::to_string_lossy(ctx, exc) {
          eprintln!("[qjs] Function::call exception: {}", s);
        }
        sys::free_value(ctx, exc);
      }
      return None;
    }
    Some(Local::from_raw(raw))
  }
  pub fn new_instance<S>(
    &self,
    scope: &mut S,
    args: &[Local<'s, Value>],
  ) -> Option<Local<'s, Object>>
  where
    S: crate::scope::HandleScopeSource,
  {
    // Best-effort: treat as a plain call with no `this`. The caller
    // (cppgc machinery) just wants a fresh object back.
    let ctx = scope.default_ctx();
    let mut argv: Vec<sys::JSValue> = args.iter().map(|a| a.raw()).collect();
    let undef = sys::jsv_undefined();
    let raw = sys::call(ctx, self.raw(), undef, argv.as_mut_slice());
    if sys::jsv_is_exception(&raw) {
      // Fallback: return a fresh empty object so cppgc has something
      // to hold onto rather than panicking.
      let obj = sys::new_object(ctx);
      return Some(Local::from_raw(obj));
    }
    Some(Local::from_raw(raw))
  }
  pub fn set_name(&self, _name: Local<'_, crate::primitives::String>) {}
  pub fn create_code_cache(&self) -> Option<Box<crate::external::CachedData>> {
    None
  }
}

/// No-op trampoline for `Function::new` calls (where we don't have a
/// real V8 callback to bridge — currently used for `call_console` and
/// stub builtins like our console methods). Returns an empty object so
/// JS callers that do `result.foo = ...` don't blow up. Also prints
/// the first argument if it's a string so JS-side
/// `Deno.core.print(...)` produces visible output.
pub(crate) unsafe extern "C" fn function_new_trampoline(
  ctx: *mut crate::ffi::JSContext,
  _this: crate::sys::JSValue,
  argc: core::ffi::c_int,
  argv: *mut crate::sys::JSValue,
) -> crate::sys::JSValue {
  if argc > 0 && !argv.is_null() {
    let first = unsafe { *argv };
    if let Some(s) = crate::sys::to_string_lossy(ctx, first) {
      eprint!("{}", s);
    }
  }
  unsafe { crate::ffi::JS_NewObject(ctx) }
}

/// Bridge trampoline called by QuickJS for op2-generated functions.
/// Reads the slow_fn pointer and OpCtx External pointer from the
/// per-function `func_data` array, builds a v8-shaped
/// FunctionCallbackInfo on the stack, and dispatches.
///
/// `func_data[0].u.ptr` = slow_fn (`unsafe extern "C" fn(*const FunctionCallbackInfo)`)
/// `func_data[1].u.ptr` = OpCtx external pointer (carried in the
///   FunctionCallbackInfo's `data` slot for `args.data().value()` to
///   recover)
pub(crate) unsafe extern "C" fn op_bridge_trampoline(
  ctx: *mut crate::ffi::JSContext,
  this_val: crate::sys::JSValue,
  argc: core::ffi::c_int,
  argv: *mut crate::sys::JSValue,
  _magic: core::ffi::c_int,
  func_data: *mut crate::sys::JSValue,
) -> crate::sys::JSValue {
  let _ = (this_val, argc, argv, func_data);
  unsafe { crate::ffi::JS_NewObject(ctx) }
}

// Per-op (slow_fn, OpCtx) lookup table keyed by an index passed to the
// JSCFunctionMagic trampoline as `magic`. Used because
// JS_NewCFunctionData segfaults in our current build; we encode the
// op identity in `magic` instead.
thread_local! {
  static OP_DISPATCH_TABLE: std::cell::RefCell<
    Vec<(super::FunctionCallback, *mut std::ffi::c_void)>,
  > = const { std::cell::RefCell::new(Vec::new()) };
}

pub(crate) fn register_op_dispatch(
  cb: super::FunctionCallback,
  data: *mut std::ffi::c_void,
) -> core::ffi::c_int {
  OP_DISPATCH_TABLE.with(|t| {
    let mut t = t.borrow_mut();
    let idx = t.len() as core::ffi::c_int;
    t.push((cb, data));
    idx
  })
}

fn lookup_op_dispatch(
  idx: core::ffi::c_int,
) -> Option<(super::FunctionCallback, *mut std::ffi::c_void)> {
  OP_DISPATCH_TABLE.with(|t| t.borrow().get(idx as usize).copied())
}

/// JSCFunctionMagic trampoline — receives the op index as `magic`.
/// Currently returns an empty object; calling slow_fn directly with
/// our FunctionCallbackInfo segfaults (the op2-emitted code reads
/// fields beyond what we currently set up). Real dispatch is the
/// next milestone.
pub(crate) unsafe extern "C" fn op_bridge_trampoline_magic(
  ctx: *mut crate::ffi::JSContext,
  this_val: crate::sys::JSValue,
  argc: core::ffi::c_int,
  argv: *mut crate::sys::JSValue,
  magic: core::ffi::c_int,
) -> crate::sys::JSValue {
  let _ = (this_val, magic);
  // Print first arg (op_print's text) so JS-side console prints work.
  if argc > 0 && !argv.is_null() {
    let first = unsafe { *argv };
    if let Some(s) = crate::sys::to_string_lossy(ctx, first) {
      eprint!("{}", s);
    }
  }
  unsafe { crate::ffi::JS_NewObject(ctx) }
}

impl Function {
  pub fn new<'s, S, F>(
    scope: &mut S,
    _callback: F,
  ) -> Option<Local<'s, Function>>
  where
    S: crate::scope::HandleScopeSource,
    F: crate::function::MapFnTo<crate::function::FunctionCallback>,
  {
    // We use JS_NewCFunction to create a real callable function that
    // has a proper `length` property. The trampoline currently returns
    // undefined because the V8 FunctionCallback ABI isn't bridged yet.
    let ctx = scope.default_ctx();
    let raw = unsafe {
      crate::ffi::JS_NewCFunction(
        ctx,
        function_new_trampoline,
        core::ptr::null(),
        0,
      )
    };
    Some(Local::from_raw(raw))
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
