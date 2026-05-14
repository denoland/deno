// Copyright 2018-2026 the Deno authors. MIT license.
//
// Dual-mode backend.
//
// When `--features link_quickjs` is on, every `sys::` call dispatches to the
// real C ABI declared in `ffi`. Otherwise it dispatches to a pure-Rust mock
// arena that simulates QuickJS's refcounting semantics so we can validate
// the GC translation without a linked QuickJS.
//
// The mock is intentionally *not* a JS engine — it stores only enough state
// to verify that every `JS_NewX` is balanced by exactly one `JS_FreeValue`
// (and any `JS_DupValue`s in between are balanced by matching frees). That
// is the invariant the compat layer must preserve.

use core::ffi::c_void;

pub use crate::ffi::JS_TAG_BIG_INT;
pub use crate::ffi::JS_TAG_BOOL;
pub use crate::ffi::JS_TAG_EXCEPTION;
pub use crate::ffi::JS_TAG_FLOAT64;
pub use crate::ffi::JS_TAG_INT;
pub use crate::ffi::JS_TAG_NULL;
pub use crate::ffi::JS_TAG_OBJECT;
pub use crate::ffi::JS_TAG_STRING;
pub use crate::ffi::JS_TAG_SYMBOL;
pub use crate::ffi::JS_TAG_UNDEFINED;
pub use crate::ffi::JSValue;
pub use crate::ffi::JSValueUnion;
pub use crate::ffi::jsv_bool;
pub use crate::ffi::jsv_exception;
pub use crate::ffi::jsv_float64;
pub use crate::ffi::jsv_int32;
pub use crate::ffi::jsv_is_bigint;
pub use crate::ffi::jsv_is_bool;
pub use crate::ffi::jsv_is_exception;
pub use crate::ffi::jsv_is_float64;
pub use crate::ffi::jsv_is_int;
pub use crate::ffi::jsv_is_null;
pub use crate::ffi::jsv_is_number;
pub use crate::ffi::jsv_is_object;
pub use crate::ffi::jsv_is_string;
pub use crate::ffi::jsv_is_symbol;
pub use crate::ffi::jsv_is_undefined;
pub use crate::ffi::jsv_null;
pub use crate::ffi::jsv_undefined;

/// QuickJS-ng `JS_WRITE_OBJ_BYTECODE` flag. Mirrored here so callers don't
/// need to reach into `ffi`.
pub const JS_WRITE_OBJ_BYTECODE: i32 = 1 << 0;
pub const JS_WRITE_OBJ_REFERENCE: i32 = 1 << 3;
pub const JS_READ_OBJ_BYTECODE: i32 = 1 << 0;
pub const JS_READ_OBJ_REFERENCE: i32 = 1 << 3;

/// PromiseState mirrors QuickJS-ng's `JS_PROMISE_*` constants and V8's
/// `v8::Promise::PromiseState` simultaneously. Values match QuickJS's
/// `JS_PromiseState` return values: 0 Pending, 1 Fulfilled, 2 Rejected.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum PromiseStateRaw {
  Pending = 0,
  Fulfilled = 1,
  Rejected = 2,
}

#[cfg(feature = "link_quickjs")]
mod backend {
  use core::ffi::c_char;

  use super::*;
  use crate::ffi;

  pub type Runtime = *mut ffi::JSRuntime;
  pub type Context = *mut ffi::JSContext;

  pub fn new_runtime() -> Runtime {
    unsafe { ffi::JS_NewRuntime() }
  }
  pub fn free_runtime(rt: Runtime) {
    unsafe { ffi::JS_FreeRuntime(rt) }
  }
  pub fn new_context(rt: Runtime) -> Context {
    unsafe { ffi::JS_NewContext(rt) }
  }
  pub fn free_context(ctx: Context) {
    unsafe { ffi::JS_FreeContext(ctx) }
  }

  pub fn dup_value(ctx: Context, v: JSValue) -> JSValue {
    unsafe { ffi::JS_DupValue(ctx, v) }
  }
  pub fn free_value(ctx: Context, v: JSValue) {
    unsafe { ffi::JS_FreeValue(ctx, v) }
  }

  pub fn new_bool(ctx: Context, b: bool) -> JSValue {
    unsafe { ffi::JS_NewBool(ctx, if b { 1 } else { 0 }) }
  }
  pub fn new_int32(ctx: Context, v: i32) -> JSValue {
    unsafe { ffi::JS_NewInt32(ctx, v) }
  }
  pub fn new_float64(ctx: Context, v: f64) -> JSValue {
    unsafe { ffi::JS_NewFloat64(ctx, v) }
  }
  pub fn new_string(ctx: Context, s: &str) -> JSValue {
    unsafe { ffi::JS_NewStringLen(ctx, s.as_ptr() as *const c_char, s.len()) }
  }
  pub fn new_object(ctx: Context) -> JSValue {
    unsafe { ffi::JS_NewObject(ctx) }
  }
  pub fn new_array(ctx: Context) -> JSValue {
    unsafe { ffi::JS_NewArray(ctx) }
  }

  pub fn to_bool(ctx: Context, v: JSValue) -> bool {
    unsafe { ffi::JS_ToBool(ctx, v) != 0 }
  }
  pub fn to_int32(ctx: Context, v: JSValue) -> Option<i32> {
    let mut out = 0i32;
    let r = unsafe { ffi::JS_ToInt32(ctx, &mut out, v) };
    (r == 0).then_some(out)
  }
  pub fn to_float64(ctx: Context, v: JSValue) -> Option<f64> {
    let mut out = 0.0f64;
    let r = unsafe { ffi::JS_ToFloat64(ctx, &mut out, v) };
    (r == 0).then_some(out)
  }
  pub fn to_string_lossy(ctx: Context, v: JSValue) -> Option<String> {
    unsafe {
      let mut len = 0usize;
      let p = ffi::JS_ToCStringLen(ctx, &mut len, v);
      if p.is_null() {
        return None;
      }
      let slice = std::slice::from_raw_parts(p as *const u8, len);
      let s = std::str::from_utf8(slice).map(|s| s.to_owned()).ok();
      ffi::JS_FreeCString(ctx, p);
      s
    }
  }

  pub fn get_global_object(ctx: Context) -> JSValue {
    unsafe { ffi::JS_GetGlobalObject(ctx) }
  }
  pub fn eval(ctx: Context, src: &str, filename: &str, flags: i32) -> JSValue {
    // QuickJS-ng's JS_Eval contract: "input must be zero terminated i.e.
    // input[input_len] = '\0'". A Rust `&str.as_ptr()` is NOT
    // zero-terminated, so the parser reads past the end into garbage
    // and surfaces bogus syntax errors. Copy into a CString.
    let src_c = match std::ffi::CString::new(src) {
      Ok(s) => s,
      Err(_) => return jsv_undefined(), // src contains interior NUL
    };
    let fname = std::ffi::CString::new(filename).unwrap();
    unsafe {
      ffi::JS_Eval(
        ctx,
        src_c.as_ptr(),
        src.len(),
        fname.as_ptr(),
        flags,
      )
    }
  }
  pub fn run_pending_job(rt: Runtime) -> bool {
    let mut pctx: Context = core::ptr::null_mut();
    unsafe { ffi::JS_ExecutePendingJob(rt, &mut pctx) > 0 }
  }
  pub fn set_runtime_opaque(rt: Runtime, p: *mut c_void) {
    unsafe { ffi::JS_SetRuntimeOpaque(rt, p) }
  }
  pub fn get_runtime_opaque(rt: Runtime) -> *mut c_void {
    unsafe { ffi::JS_GetRuntimeOpaque(rt) }
  }

  // Per-Context embedder slots. V8 lets the embedder stash arbitrary
  // pointers indexed by integer; deno_core uses slots 0/1 for context
  // state and module map. Backed by a thread-local map keyed by the
  // raw JSContext pointer.
  thread_local! {
    static CONTEXT_SLOTS: std::cell::RefCell<
      std::collections::HashMap<usize, Vec<*mut c_void>>
    > = std::cell::RefCell::new(std::collections::HashMap::new());
  }
  pub fn set_context_embedder_slot(ctx: Context, index: usize, value: *mut c_void) {
    CONTEXT_SLOTS.with(|m| {
      let mut m = m.borrow_mut();
      let entry = m.entry(ctx as usize).or_default();
      if entry.len() <= index {
        entry.resize(index + 1, core::ptr::null_mut());
      }
      entry[index] = value;
    });
  }
  pub fn get_context_embedder_slot(ctx: Context, index: usize) -> *mut c_void {
    CONTEXT_SLOTS.with(|m| {
      let m = m.borrow();
      m.get(&(ctx as usize))
        .and_then(|v| v.get(index).copied())
        .unwrap_or(core::ptr::null_mut())
    })
  }

  pub fn get_property_str(ctx: Context, obj: JSValue, key: &str) -> JSValue {
    let k = std::ffi::CString::new(key).unwrap();
    unsafe { ffi::JS_GetPropertyStr(ctx, obj, k.as_ptr()) }
  }
  pub fn set_property_str(
    ctx: Context,
    obj: JSValue,
    key: &str,
    val: JSValue,
  ) -> bool {
    let k = std::ffi::CString::new(key).unwrap();
    unsafe { ffi::JS_SetPropertyStr(ctx, obj, k.as_ptr(), val) > 0 }
  }
  pub fn has_property_str(ctx: Context, obj: JSValue, key: &str) -> bool {
    let k = std::ffi::CString::new(key).unwrap();
    unsafe { ffi::JS_HasPropertyStr(ctx, obj, k.as_ptr()) > 0 }
  }
  pub fn delete_property_str(ctx: Context, obj: JSValue, key: &str) -> bool {
    let k = std::ffi::CString::new(key).unwrap();
    unsafe { ffi::JS_DeletePropertyStr(ctx, obj, k.as_ptr(), 0) > 0 }
  }
  pub fn get_indexed(ctx: Context, obj: JSValue, idx: u32) -> JSValue {
    unsafe { ffi::JS_GetPropertyUint32(ctx, obj, idx) }
  }
  pub fn set_indexed(
    ctx: Context,
    obj: JSValue,
    idx: u32,
    val: JSValue,
  ) -> bool {
    unsafe { ffi::JS_SetPropertyUint32(ctx, obj, idx, val) > 0 }
  }

  pub fn new_promise_capability(
    ctx: Context,
  ) -> Option<(JSValue, JSValue, JSValue)> {
    let mut funcs: [JSValue; 2] = [jsv_undefined(); 2];
    let promise =
      unsafe { ffi::JS_NewPromiseCapability(ctx, funcs.as_mut_ptr()) };
    if jsv_is_exception(&promise) {
      return None;
    }
    Some((promise, funcs[0], funcs[1]))
  }
  pub fn promise_state(ctx: Context, p: JSValue) -> super::PromiseStateRaw {
    let s = unsafe { ffi::JS_PromiseState(ctx, p) };
    match s {
      1 => super::PromiseStateRaw::Fulfilled,
      2 => super::PromiseStateRaw::Rejected,
      _ => super::PromiseStateRaw::Pending,
    }
  }
  pub fn promise_result(ctx: Context, p: JSValue) -> JSValue {
    unsafe { ffi::JS_PromiseResult(ctx, p) }
  }
  pub fn call(
    ctx: Context,
    func: JSValue,
    this_: JSValue,
    args: &mut [JSValue],
  ) -> JSValue {
    unsafe {
      ffi::JS_Call(ctx, func, this_, args.len() as i32, args.as_mut_ptr())
    }
  }
  pub fn has_pending_exception(ctx: Context) -> bool {
    unsafe { ffi::JS_HasException(ctx) != 0 }
  }
  pub fn take_pending_exception(ctx: Context) -> Option<JSValue> {
    let exc = unsafe { ffi::JS_GetException(ctx) };
    // JS_GetException returns JS_NULL when there's nothing to take, but
    // some QuickJS-ng builds also surface JS_UNINITIALIZED. Treat both
    // as "no exception".
    if jsv_is_null(&exc) || exc.tag == ffi::JS_TAG_UNINITIALIZED {
      None
    } else {
      Some(exc)
    }
  }
  pub fn throw(ctx: Context, exc: JSValue) -> JSValue {
    unsafe { ffi::JS_Throw(ctx, exc) }
  }
  pub fn eval_function(ctx: Context, fun: JSValue) -> JSValue {
    unsafe { ffi::JS_EvalFunction(ctx, fun) }
  }
  pub fn write_bytecode(ctx: Context, v: JSValue) -> Option<Vec<u8>> {
    let mut len = 0usize;
    let p = unsafe {
      ffi::JS_WriteObject(ctx, &mut len, v, super::JS_WRITE_OBJ_BYTECODE)
    };
    if p.is_null() {
      return None;
    }
    let slice = unsafe { std::slice::from_raw_parts(p, len) };
    let out = slice.to_vec();
    // QuickJS-ng allocates the buffer via js_malloc; we should free it via
    // js_free. But the public sys layer doesn't expose js_free yet, so we
    // accept the small leak per write here. TODO: js_free wiring.
    let _ = p;
    Some(out)
  }
  pub fn read_bytecode(ctx: Context, bytes: &[u8]) -> JSValue {
    unsafe {
      ffi::JS_ReadObject(
        ctx,
        bytes.as_ptr(),
        bytes.len(),
        super::JS_READ_OBJ_BYTECODE,
      )
    }
  }
}

#[cfg(not(feature = "link_quickjs"))]
mod backend {
  //! Mock backend: a thread-local arena of refcounted "JSValue"s, used for
  //! testing the compat layer's refcount discipline without QuickJS linked.
  //!
  //! Every `new_*` returns a JSValue whose `u.ptr` is a key into a per-runtime
  //! `RefCell<HashMap<u64, (Tag, refcount, bytes)>>`. `dup_value` bumps the
  //! refcount; `free_value` decrements and drops on zero. If the program
  //! exits with any nonzero refcount, the test fixtures notice and fail.

  use std::sync::Mutex;
  use std::sync::atomic::AtomicUsize;
  use std::sync::atomic::Ordering;

  use super::*;
  use crate::arena::Arena;

  static RT_COUNTER: AtomicUsize = AtomicUsize::new(1);

  // Public so type aliases `Runtime = *mut MockRuntime` don't leak a
  // private type out of `sys`. The body remains effectively opaque.
  pub struct MockRuntime {
    id: usize,
    arena: Mutex<Arena>,
    opaque: Mutex<usize>,
    pending_jobs: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
    /// At most one pending exception per runtime — matches the QuickJS C
    /// API where `JS_GetException` takes whatever is pending. The Mutex
    /// stores the raw JSValue *and* owns one refcount on it; on take it
    /// transfers that refcount to the caller.
    pending_exception: Mutex<Option<JSValue>>,
    /// Bytecode cache (mock): handle -> serialized form. Used by the
    /// stage-2 snapshot pathway tests.
    bytecode: Mutex<std::collections::HashMap<u64, Vec<u8>>>,
  }

  // Pointers are usize-sized handles into a leaked Box<MockRuntime>. This
  // mirrors the C API where the pointer outlives any individual call.
  pub type Runtime = *mut MockRuntime;
  pub type Context = *mut MockRuntime;

  pub fn new_runtime() -> Runtime {
    let rt = Box::new(MockRuntime {
      id: RT_COUNTER.fetch_add(1, Ordering::SeqCst),
      arena: Mutex::new(Arena::new()),
      opaque: Mutex::new(0),
      pending_jobs: Mutex::new(Vec::new()),
      pending_exception: Mutex::new(None),
      bytecode: Mutex::new(std::collections::HashMap::new()),
    });
    Box::into_raw(rt)
  }
  pub fn free_runtime(rt: Runtime) {
    if rt.is_null() {
      return;
    }
    // Drop any still-pending exception so its refcount goes to zero before
    // the arena leak check fires.
    {
      let pe = unsafe { (*rt).pending_exception.lock().unwrap().take() };
      if let Some(v) = pe {
        if is_refcounted(&v) {
          unsafe { (*rt).arena.lock().unwrap().free(handle_of(&v)) };
        }
      }
    }
    // Verify arena is empty — leaks should be visible.
    let owned = unsafe { Box::from_raw(rt) };
    let arena = owned.arena.lock().unwrap();
    if !arena.is_empty() {
      panic!(
        "qjs_v8_compat mock: runtime freed with {} live JSValues — \
         refcount discipline broken",
        arena.live_count()
      );
    }
  }
  pub fn new_context(rt: Runtime) -> Context {
    // Mock context = mock runtime — no separate realm tracking yet.
    rt
  }
  pub fn free_context(_ctx: Context) {}

  fn arena_of(ctx: Context) -> std::sync::MutexGuard<'static, Arena> {
    // SAFETY: the Runtime pointer is the leaked Box ptr; we just took a
    // shared borrow of an internal Mutex which is OK across the FFI bound.
    unsafe { (*ctx).arena.lock().unwrap() }
  }

  pub fn dup_value(ctx: Context, v: JSValue) -> JSValue {
    // Tagged primitives have no refcount.
    if !is_refcounted(&v) {
      return v;
    }
    arena_of(ctx).dup(handle_of(&v));
    v
  }
  pub fn free_value(ctx: Context, v: JSValue) {
    if !is_refcounted(&v) || ctx.is_null() {
      return;
    }
    arena_of(ctx).free(handle_of(&v));
  }

  fn is_refcounted(v: &JSValue) -> bool {
    matches!(
      v.tag,
      JS_TAG_OBJECT | JS_TAG_STRING | JS_TAG_SYMBOL | JS_TAG_BIG_INT
    )
  }
  fn handle_of(v: &JSValue) -> u64 {
    unsafe { v.u.ptr as usize as u64 }
  }
  fn make_handle(tag: i64, h: u64) -> JSValue {
    JSValue {
      u: JSValueUnion {
        ptr: h as usize as *mut _,
      },
      tag,
    }
  }

  pub fn new_bool(_ctx: Context, b: bool) -> JSValue {
    jsv_bool(b)
  }
  pub fn new_int32(_ctx: Context, v: i32) -> JSValue {
    jsv_int32(v)
  }
  pub fn new_float64(_ctx: Context, v: f64) -> JSValue {
    jsv_float64(v)
  }
  pub fn new_string(ctx: Context, s: &str) -> JSValue {
    let h = arena_of(ctx).alloc_string(s);
    make_handle(JS_TAG_STRING, h)
  }
  pub fn new_object(ctx: Context) -> JSValue {
    let h = arena_of(ctx).alloc_object();
    make_handle(JS_TAG_OBJECT, h)
  }
  pub fn new_array(ctx: Context) -> JSValue {
    let h = arena_of(ctx).alloc_array();
    make_handle(JS_TAG_OBJECT, h)
  }

  pub fn to_bool(_ctx: Context, v: JSValue) -> bool {
    match v.tag {
      JS_TAG_BOOL | JS_TAG_INT => unsafe { v.u.int32 != 0 },
      JS_TAG_FLOAT64 => unsafe { v.u.float64 != 0.0 && !v.u.float64.is_nan() },
      JS_TAG_NULL | JS_TAG_UNDEFINED => false,
      JS_TAG_STRING => {
        // Empty string is falsy.
        let s = to_string_lossy(_ctx, v).unwrap_or_default();
        !s.is_empty()
      }
      _ => true,
    }
  }
  pub fn to_int32(_ctx: Context, v: JSValue) -> Option<i32> {
    match v.tag {
      JS_TAG_INT => Some(unsafe { v.u.int32 }),
      JS_TAG_FLOAT64 => Some(unsafe { v.u.float64 } as i32),
      JS_TAG_BOOL => Some(unsafe { v.u.int32 }),
      _ => None,
    }
  }
  pub fn to_float64(_ctx: Context, v: JSValue) -> Option<f64> {
    match v.tag {
      JS_TAG_INT => Some(unsafe { v.u.int32 } as f64),
      JS_TAG_FLOAT64 => Some(unsafe { v.u.float64 }),
      JS_TAG_BOOL => Some(unsafe { v.u.int32 } as f64),
      _ => None,
    }
  }
  pub fn to_string_lossy(ctx: Context, v: JSValue) -> Option<String> {
    match v.tag {
      JS_TAG_STRING => arena_of(ctx).string_value(handle_of(&v)),
      JS_TAG_INT => Some(unsafe { v.u.int32 }.to_string()),
      JS_TAG_FLOAT64 => Some(unsafe { v.u.float64 }.to_string()),
      JS_TAG_BOOL => Some(
        if unsafe { v.u.int32 } != 0 {
          "true"
        } else {
          "false"
        }
        .to_string(),
      ),
      JS_TAG_NULL => Some("null".to_string()),
      JS_TAG_UNDEFINED => Some("undefined".to_string()),
      _ => None,
    }
  }

  pub fn get_global_object(ctx: Context) -> JSValue {
    let h = arena_of(ctx).alloc_object();
    make_handle(JS_TAG_OBJECT, h)
  }
  pub fn eval(
    _ctx: Context,
    _src: &str,
    _filename: &str,
    _flags: i32,
  ) -> JSValue {
    // The mock doesn't actually execute JS; eval is a no-op that returns
    // `undefined`. Tests that need real eval use the `link_quickjs` build.
    jsv_undefined()
  }
  pub fn run_pending_job(rt: Runtime) -> bool {
    let job = unsafe { (*rt).pending_jobs.lock().unwrap().pop() };
    if let Some(j) = job {
      j();
      true
    } else {
      false
    }
  }
  pub fn set_runtime_opaque(rt: Runtime, p: *mut c_void) {
    unsafe { *(*rt).opaque.lock().unwrap() = p as usize };
  }
  pub fn get_runtime_opaque(rt: Runtime) -> *mut c_void {
    let v = unsafe { *(*rt).opaque.lock().unwrap() };
    v as *mut c_void
  }

  thread_local! {
    static CONTEXT_SLOTS: std::cell::RefCell<
      std::collections::HashMap<usize, Vec<*mut c_void>>
    > = std::cell::RefCell::new(std::collections::HashMap::new());
  }
  pub fn set_context_embedder_slot(ctx: Context, index: usize, value: *mut c_void) {
    CONTEXT_SLOTS.with(|m| {
      let mut m = m.borrow_mut();
      let entry = m.entry(ctx as usize).or_default();
      if entry.len() <= index {
        entry.resize(index + 1, core::ptr::null_mut());
      }
      entry[index] = value;
    });
  }
  pub fn get_context_embedder_slot(ctx: Context, index: usize) -> *mut c_void {
    CONTEXT_SLOTS.with(|m| {
      let m = m.borrow();
      m.get(&(ctx as usize))
        .and_then(|v| v.get(index).copied())
        .unwrap_or(core::ptr::null_mut())
    })
  }

  // ---- Property access ------------------------------------------------
  //
  // The mock arena stores properties in `MockJSValue::named` (a `HashMap`).
  // When we read a value out, the JSValue we hand back has a *new*
  // refcount taken on it — the caller is responsible for either pushing
  // it onto a scope's owned vec or freeing it explicitly. This matches
  // QuickJS's `JS_GetPropertyStr` which returns a +1-refcount value.

  pub fn get_property_str(ctx: Context, obj: JSValue, key: &str) -> JSValue {
    if !is_refcounted(&obj) {
      return jsv_undefined();
    }
    let h = handle_of(&obj);
    let mut arena = arena_of(ctx);
    let Some(val_h) = arena.get_named(h, key) else {
      return jsv_undefined();
    };
    let tag = match arena.tag(val_h) {
      Some(crate::arena::MockTag::String) => JS_TAG_STRING,
      Some(crate::arena::MockTag::Symbol) => JS_TAG_SYMBOL,
      Some(crate::arena::MockTag::BigInt) => JS_TAG_BIG_INT,
      _ => JS_TAG_OBJECT,
    };
    // Bump refcount — the returned JSValue owns one ref.
    arena.dup(val_h);
    make_handle(tag, val_h)
  }
  pub fn set_property_str(
    ctx: Context,
    obj: JSValue,
    key: &str,
    val: JSValue,
  ) -> bool {
    if !is_refcounted(&obj) {
      return false;
    }
    let obj_h = handle_of(&obj);
    let mut arena = arena_of(ctx);
    if is_refcounted(&val) {
      let val_h = handle_of(&val);
      // If a previous value exists at this key, drop our refcount on it.
      if let Some(prev) = arena.get_named(obj_h, key) {
        arena.free(prev);
      }
      arena.set_named(obj_h, key, val_h);
      // `set_property_str` transfers ownership of `val`'s refcount to the
      // property slot. The caller's local must be invalidated; we don't
      // need to mutate it here because the higher-level wrapper has
      // already done so by passing the JSValue.
    } else {
      // Primitive: encode into the named slot using a sentinel. We don't
      // support that yet (mock tests use refcounted values), so fall back
      // to ignoring primitive sets for now.
    }
    true
  }
  pub fn has_property_str(ctx: Context, obj: JSValue, key: &str) -> bool {
    if !is_refcounted(&obj) {
      return false;
    }
    arena_of(ctx).get_named(handle_of(&obj), key).is_some()
  }
  pub fn delete_property_str(ctx: Context, obj: JSValue, key: &str) -> bool {
    if !is_refcounted(&obj) {
      return false;
    }
    let obj_h = handle_of(&obj);
    let mut arena = arena_of(ctx);
    if let Some(prev) = arena.get_named(obj_h, key) {
      arena.free(prev);
      // Remove the key from the named map.
      arena.delete_named(obj_h, key);
      true
    } else {
      false
    }
  }
  pub fn get_indexed(ctx: Context, obj: JSValue, idx: u32) -> JSValue {
    if !is_refcounted(&obj) {
      return jsv_undefined();
    }
    let h = handle_of(&obj);
    let mut arena = arena_of(ctx);
    let Some(val_h) = arena.get_indexed(h, idx as usize) else {
      return jsv_undefined();
    };
    if val_h == 0 {
      return jsv_undefined();
    }
    let tag = match arena.tag(val_h) {
      Some(crate::arena::MockTag::String) => JS_TAG_STRING,
      Some(crate::arena::MockTag::Symbol) => JS_TAG_SYMBOL,
      Some(crate::arena::MockTag::BigInt) => JS_TAG_BIG_INT,
      _ => JS_TAG_OBJECT,
    };
    arena.dup(val_h);
    make_handle(tag, val_h)
  }
  pub fn set_indexed(
    ctx: Context,
    obj: JSValue,
    idx: u32,
    val: JSValue,
  ) -> bool {
    if !is_refcounted(&obj) || !is_refcounted(&val) {
      return false;
    }
    let obj_h = handle_of(&obj);
    let val_h = handle_of(&val);
    let mut arena = arena_of(ctx);
    if let Some(prev) = arena.get_indexed(obj_h, idx as usize) {
      if prev != 0 {
        arena.free(prev);
      }
    }
    arena.set_indexed(obj_h, idx as usize, val_h);
    true
  }

  // ---- Promise capability --------------------------------------------
  //
  // In the mock we model a Promise as a refcounted object that carries
  // three slots on its named map:
  //   "[[PromiseState]]"  -> handle to an int (we encode state as the
  //                          handle of a one-byte string payload "0"|"1"|"2")
  //   "[[PromiseValue]]"  -> handle to the resolved/rejected value (or 0)
  // Two resolving functions (resolve, reject) are also stored, modelled as
  // fresh function-tagged values whose .label encodes their target.

  pub fn new_promise_capability(
    ctx: Context,
  ) -> Option<(JSValue, JSValue, JSValue)> {
    let mut arena = arena_of(ctx);
    let promise_h = arena.alloc_promise();
    let state_h = arena.alloc_string("0");
    arena.set_named(promise_h, "[[PromiseState]]", state_h);
    // resolve / reject — modelled as function values whose .label encodes
    // a back-pointer to the promise as "resolve:<h>" or "reject:<h>". We
    // stash a duplicate refcount on each function in a named slot on the
    // promise, so the arena's recursive free reaches them when the
    // promise is dropped. The returned JSValue carries its own refcount.
    let resolve_h = arena.alloc_function(&format!("resolve:{promise_h}"));
    let reject_h = arena.alloc_function(&format!("reject:{promise_h}"));
    arena.dup(resolve_h);
    arena.dup(reject_h);
    arena.set_named(promise_h, "[[Resolve]]", resolve_h);
    arena.set_named(promise_h, "[[Reject]]", reject_h);
    Some((
      make_handle(JS_TAG_OBJECT, promise_h),
      make_handle(JS_TAG_OBJECT, resolve_h),
      make_handle(JS_TAG_OBJECT, reject_h),
    ))
  }
  pub fn promise_state(ctx: Context, p: JSValue) -> super::PromiseStateRaw {
    if !is_refcounted(&p) {
      return super::PromiseStateRaw::Pending;
    }
    let h = handle_of(&p);
    let arena = arena_of(ctx);
    let Some(state_h) = arena.get_named(h, "[[PromiseState]]") else {
      return super::PromiseStateRaw::Pending;
    };
    match arena.string_value(state_h).as_deref() {
      Some("1") => super::PromiseStateRaw::Fulfilled,
      Some("2") => super::PromiseStateRaw::Rejected,
      _ => super::PromiseStateRaw::Pending,
    }
  }
  pub fn promise_result(ctx: Context, p: JSValue) -> JSValue {
    if !is_refcounted(&p) {
      return jsv_undefined();
    }
    let h = handle_of(&p);
    let mut arena = arena_of(ctx);
    let Some(val_h) = arena.get_named(h, "[[PromiseValue]]") else {
      return jsv_undefined();
    };
    arena.dup(val_h);
    let tag = match arena.tag(val_h) {
      Some(crate::arena::MockTag::String) => JS_TAG_STRING,
      _ => JS_TAG_OBJECT,
    };
    make_handle(tag, val_h)
  }
  /// Mock-only: transition a promise to a settled state. Called by the
  /// resolve/reject wrappers in `promise.rs`. Takes ownership of `value`'s
  /// refcount: we either store it (if not already stored) or free it.
  pub(crate) fn mock_settle(
    ctx: Context,
    promise: JSValue,
    state: super::PromiseStateRaw,
    value: JSValue,
  ) {
    if !is_refcounted(&promise) {
      if is_refcounted(&value) {
        arena_of(ctx).free(handle_of(&value));
      }
      return;
    }
    let h = handle_of(&promise);
    let mut arena = arena_of(ctx);
    // Replace state slot.
    if let Some(prev) = arena.get_named(h, "[[PromiseState]]") {
      arena.free(prev);
    }
    let tag = match state {
      super::PromiseStateRaw::Pending => "0",
      super::PromiseStateRaw::Fulfilled => "1",
      super::PromiseStateRaw::Rejected => "2",
    };
    let s = arena.alloc_string(tag);
    arena.set_named(h, "[[PromiseState]]", s);
    // Replace value slot.
    if let Some(prev) = arena.get_named(h, "[[PromiseValue]]") {
      arena.free(prev);
    }
    if is_refcounted(&value) {
      arena.set_named(h, "[[PromiseValue]]", handle_of(&value));
    }
  }

  pub fn call(
    _ctx: Context,
    _func: JSValue,
    _this_: JSValue,
    _args: &mut [JSValue],
  ) -> JSValue {
    // The mock backend doesn't actually invoke JS functions; we just
    // return `undefined`. Real call dispatch is in the linked-quickjs
    // backend.
    jsv_undefined()
  }

  // ---- Pending exception ---------------------------------------------

  pub fn has_pending_exception(ctx: Context) -> bool {
    unsafe { (*ctx).pending_exception.lock().unwrap().is_some() }
  }
  pub fn take_pending_exception(ctx: Context) -> Option<JSValue> {
    let v = unsafe { (*ctx).pending_exception.lock().unwrap().take() };
    // Caller now owns the refcount.
    v
  }
  /// Mock-only: install a pending exception. The runtime takes one
  /// refcount on `exc` (matching QuickJS's `JS_Throw`, which transfers
  /// ownership). If an exception is already pending we drop ours.
  pub(crate) fn mock_throw(ctx: Context, exc: JSValue) -> JSValue {
    let mut slot = unsafe { (*ctx).pending_exception.lock().unwrap() };
    if slot.is_some() && is_refcounted(&exc) {
      arena_of(ctx).free(handle_of(&exc));
    } else {
      *slot = Some(exc);
    }
    jsv_exception()
  }
  pub fn throw(ctx: Context, exc: JSValue) -> JSValue {
    mock_throw(ctx, exc)
  }
  pub fn eval_function(_ctx: Context, _fun: JSValue) -> JSValue {
    // The mock backend has no actual eval; return undefined.
    jsv_undefined()
  }

  // ---- Bytecode (mock serializer) -------------------------------------
  //
  // We serialize as a tiny tagged stream: [tag:i64][payload-len:u32][bytes...]
  // for strings, [tag:i64][named-count:u32][(key-len:u32, key, val-tag, val)...]
  // for objects. This is enough to round-trip simple values in tests and
  // mirrors the wire shape of JS_WriteObject without replicating QuickJS's
  // bytecode format.

  pub fn write_bytecode(ctx: Context, v: JSValue) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    write_value(ctx, v, &mut out, 0).then_some(out)
  }
  pub fn read_bytecode(ctx: Context, bytes: &[u8]) -> JSValue {
    let mut pos = 0;
    read_value(ctx, bytes, &mut pos).unwrap_or_else(jsv_exception)
  }

  fn write_value(
    ctx: Context,
    v: JSValue,
    out: &mut Vec<u8>,
    depth: u32,
  ) -> bool {
    if depth > 32 {
      return false;
    }
    out.extend_from_slice(&v.tag.to_le_bytes());
    match v.tag {
      JS_TAG_INT | JS_TAG_BOOL => {
        out.extend_from_slice(&unsafe { v.u.int32 }.to_le_bytes());
        true
      }
      JS_TAG_FLOAT64 => {
        out.extend_from_slice(&unsafe { v.u.float64 }.to_le_bytes());
        true
      }
      JS_TAG_NULL | JS_TAG_UNDEFINED => true,
      JS_TAG_STRING => {
        let h = handle_of(&v);
        let s = arena_of(ctx).string_value(h).unwrap_or_default();
        out.extend_from_slice(&(s.len() as u32).to_le_bytes());
        out.extend_from_slice(s.as_bytes());
        true
      }
      JS_TAG_OBJECT => {
        let h = handle_of(&v);
        let named: Vec<(String, u64)> = {
          let arena = arena_of(ctx);
          arena.entries_named(h).into_iter().collect()
        };
        out.extend_from_slice(&(named.len() as u32).to_le_bytes());
        for (k, child_h) in named {
          out.extend_from_slice(&(k.len() as u32).to_le_bytes());
          out.extend_from_slice(k.as_bytes());
          // Encode child by recursion. We need a JSValue; reconstruct from
          // its tag.
          let arena = arena_of(ctx);
          let child_tag = match arena.tag(child_h) {
            Some(crate::arena::MockTag::String) => JS_TAG_STRING,
            Some(crate::arena::MockTag::Symbol) => JS_TAG_SYMBOL,
            Some(crate::arena::MockTag::BigInt) => JS_TAG_BIG_INT,
            _ => JS_TAG_OBJECT,
          };
          drop(arena);
          let child = make_handle(child_tag, child_h);
          if !write_value(ctx, child, out, depth + 1) {
            return false;
          }
        }
        true
      }
      _ => false,
    }
  }

  fn read_u32(bytes: &[u8], pos: &mut usize) -> Option<u32> {
    if *pos + 4 > bytes.len() {
      return None;
    }
    let v = u32::from_le_bytes(bytes[*pos..*pos + 4].try_into().ok()?);
    *pos += 4;
    Some(v)
  }
  fn read_i64(bytes: &[u8], pos: &mut usize) -> Option<i64> {
    if *pos + 8 > bytes.len() {
      return None;
    }
    let v = i64::from_le_bytes(bytes[*pos..*pos + 8].try_into().ok()?);
    *pos += 8;
    Some(v)
  }
  fn read_f64(bytes: &[u8], pos: &mut usize) -> Option<f64> {
    if *pos + 8 > bytes.len() {
      return None;
    }
    let v = f64::from_le_bytes(bytes[*pos..*pos + 8].try_into().ok()?);
    *pos += 8;
    Some(v)
  }
  fn read_i32(bytes: &[u8], pos: &mut usize) -> Option<i32> {
    if *pos + 4 > bytes.len() {
      return None;
    }
    let v = i32::from_le_bytes(bytes[*pos..*pos + 4].try_into().ok()?);
    *pos += 4;
    Some(v)
  }

  fn read_value(
    ctx: Context,
    bytes: &[u8],
    pos: &mut usize,
  ) -> Option<JSValue> {
    let tag = read_i64(bytes, pos)?;
    match tag {
      JS_TAG_INT => Some(jsv_int32(read_i32(bytes, pos)?)),
      JS_TAG_BOOL => Some(jsv_bool(read_i32(bytes, pos)? != 0)),
      JS_TAG_FLOAT64 => Some(jsv_float64(read_f64(bytes, pos)?)),
      JS_TAG_NULL => Some(jsv_null()),
      JS_TAG_UNDEFINED => Some(jsv_undefined()),
      JS_TAG_STRING => {
        let len = read_u32(bytes, pos)? as usize;
        if *pos + len > bytes.len() {
          return None;
        }
        let s = std::str::from_utf8(&bytes[*pos..*pos + len]).ok()?;
        *pos += len;
        let h = arena_of(ctx).alloc_string(s);
        Some(make_handle(JS_TAG_STRING, h))
      }
      JS_TAG_OBJECT => {
        let count = read_u32(bytes, pos)?;
        let obj_h = arena_of(ctx).alloc_object();
        for _ in 0..count {
          let klen = read_u32(bytes, pos)? as usize;
          if *pos + klen > bytes.len() {
            return None;
          }
          let key = std::str::from_utf8(&bytes[*pos..*pos + klen])
            .ok()?
            .to_owned();
          *pos += klen;
          let child = read_value(ctx, bytes, pos)?;
          if is_refcounted(&child) {
            arena_of(ctx).set_named(obj_h, &key, handle_of(&child));
          }
        }
        Some(make_handle(JS_TAG_OBJECT, obj_h))
      }
      _ => None,
    }
  }

  pub fn bytecode_store(rt: Runtime, h: u64, bytes: Vec<u8>) {
    unsafe { (*rt).bytecode.lock().unwrap().insert(h, bytes) };
  }
  pub fn bytecode_load(rt: Runtime, h: u64) -> Option<Vec<u8>> {
    unsafe { (*rt).bytecode.lock().unwrap().get(&h).cloned() }
  }
}

pub use backend::*;
