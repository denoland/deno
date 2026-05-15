// Copyright 2018-2026 the Deno authors. MIT license.
//
// Hand-written FFI declarations for the QuickJS-ng C API.
//
// We do this by hand rather than bindgen to keep the build hermetic: the
// declarations are checked at compile time against the libquickjs symbol
// table when `--features link_quickjs` is on, and otherwise they're just
// inert `extern "C"` forward decls that never resolve at link time.
//
// The header version we target is quickjs-ng 0.10+. Where the API diverges
// between original quickjs and quickjs-ng we use the -ng spelling.

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

use core::ffi::c_char;
use core::ffi::c_int;
use core::ffi::c_void;

// ----- Opaque types -----------------------------------------------------

#[repr(C)]
pub struct JSRuntime {
  _private: [u8; 0],
}

#[repr(C)]
pub struct JSContext {
  _private: [u8; 0],
}

#[repr(C)]
pub struct JSModuleDef {
  _private: [u8; 0],
}

#[repr(C)]
pub struct JSClass {
  _private: [u8; 0],
}

pub type JSClassID = u32;
pub type JSAtom = u32;

// ----- JSValue layout ---------------------------------------------------
//
// QuickJS-ng's `JSValue` is a 16-byte tagged union (`int64 tag; union { ... }`)
// on 64-bit hosts. The `JS_VALUE_GET_*` macros decode it. We reproduce the
// layout faithfully so it can be passed by value across the FFI boundary.

#[repr(C)]
#[derive(Copy, Clone)]
pub union JSValueUnion {
  pub int32: i32,
  pub float64: f64,
  pub ptr: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct JSValue {
  pub u: JSValueUnion,
  pub tag: i64,
}

impl JSValue {
  pub fn as_ptr(&self) -> *const Self {
    self as *const Self
  }
}

// Tag constants — match quickjs-ng `quickjs.h` enum exactly.
pub const JS_TAG_FIRST: i64 = -9;
pub const JS_TAG_BIG_INT: i64 = -9;
pub const JS_TAG_SYMBOL: i64 = -8;
pub const JS_TAG_STRING: i64 = -7;
pub const JS_TAG_STRING_ROPE: i64 = -6;
pub const JS_TAG_MODULE: i64 = -3;
pub const JS_TAG_FUNCTION_BYTECODE: i64 = -2;
pub const JS_TAG_OBJECT: i64 = -1;
pub const JS_TAG_INT: i64 = 0;
pub const JS_TAG_BOOL: i64 = 1;
pub const JS_TAG_NULL: i64 = 2;
pub const JS_TAG_UNDEFINED: i64 = 3;
pub const JS_TAG_UNINITIALIZED: i64 = 4;
pub const JS_TAG_CATCH_OFFSET: i64 = 5;
pub const JS_TAG_EXCEPTION: i64 = 6;
pub const JS_TAG_SHORT_BIG_INT: i64 = 7;
pub const JS_TAG_FLOAT64: i64 = 8;

#[inline]
pub const fn make_value(tag: i64, u: JSValueUnion) -> JSValue {
  JSValue { u, tag }
}

#[inline]
pub fn jsv_undefined() -> JSValue {
  make_value(JS_TAG_UNDEFINED, JSValueUnion { int32: 0 })
}
#[inline]
pub fn jsv_null() -> JSValue {
  make_value(JS_TAG_NULL, JSValueUnion { int32: 0 })
}
#[inline]
pub fn jsv_bool(b: bool) -> JSValue {
  make_value(
    JS_TAG_BOOL,
    JSValueUnion {
      int32: if b { 1 } else { 0 },
    },
  )
}
#[inline]
pub fn jsv_int32(v: i32) -> JSValue {
  make_value(JS_TAG_INT, JSValueUnion { int32: v })
}
#[inline]
pub fn jsv_float64(v: f64) -> JSValue {
  make_value(JS_TAG_FLOAT64, JSValueUnion { float64: v })
}
#[inline]
pub fn jsv_exception() -> JSValue {
  make_value(JS_TAG_EXCEPTION, JSValueUnion { int32: 0 })
}

#[inline]
pub fn jsv_is_undefined(v: &JSValue) -> bool {
  v.tag == JS_TAG_UNDEFINED
}
#[inline]
pub fn jsv_is_null(v: &JSValue) -> bool {
  v.tag == JS_TAG_NULL
}
#[inline]
pub fn jsv_is_bool(v: &JSValue) -> bool {
  v.tag == JS_TAG_BOOL
}
#[inline]
pub fn jsv_is_int(v: &JSValue) -> bool {
  v.tag == JS_TAG_INT
}
#[inline]
pub fn jsv_is_float64(v: &JSValue) -> bool {
  v.tag == JS_TAG_FLOAT64
}
#[inline]
pub fn jsv_is_number(v: &JSValue) -> bool {
  v.tag == JS_TAG_INT || v.tag == JS_TAG_FLOAT64
}
#[inline]
pub fn jsv_is_string(v: &JSValue) -> bool {
  v.tag == JS_TAG_STRING
}
#[inline]
pub fn jsv_is_symbol(v: &JSValue) -> bool {
  v.tag == JS_TAG_SYMBOL
}
#[inline]
pub fn jsv_is_object(v: &JSValue) -> bool {
  v.tag == JS_TAG_OBJECT
}
#[inline]
pub fn jsv_is_bigint(v: &JSValue) -> bool {
  v.tag == JS_TAG_BIG_INT
}
#[inline]
pub fn jsv_is_exception(v: &JSValue) -> bool {
  v.tag == JS_TAG_EXCEPTION
}

/// Extract the pointer payload of a tagged-pointer JSValue (`JS_TAG_OBJECT`,
/// `JS_TAG_MODULE`, `JS_TAG_FUNCTION_BYTECODE`, `JS_TAG_STRING`, etc).
///
/// Returns `null` for non-pointer tags (numbers, booleans, undefined).
#[inline]
pub fn jsv_get_ptr(v: &JSValue) -> *mut c_void {
  // Safe: for pointer-tagged values the union's `ptr` field is the live
  // discriminant; for non-pointer tags the read still returns a defined
  // bit pattern (we just don't promise it's a valid pointer).
  unsafe { v.u.ptr }
}

// Eval flags (quickjs.h).
pub const JS_EVAL_TYPE_GLOBAL: c_int = 0;
pub const JS_EVAL_TYPE_MODULE: c_int = 1;
pub const JS_EVAL_TYPE_DIRECT: c_int = 2;
pub const JS_EVAL_TYPE_INDIRECT: c_int = 3;
pub const JS_EVAL_TYPE_MASK: c_int = 3;
pub const JS_EVAL_FLAG_STRICT: c_int = 1 << 3;
pub const JS_EVAL_FLAG_COMPILE_ONLY: c_int = 1 << 5;
pub const JS_EVAL_FLAG_BACKTRACE_BARRIER: c_int = 1 << 6;
pub const JS_EVAL_FLAG_ASYNC: c_int = 1 << 7;

// Property flags (subset).
pub const JS_PROP_CONFIGURABLE: c_int = 1 << 0;
pub const JS_PROP_WRITABLE: c_int = 1 << 1;
pub const JS_PROP_ENUMERABLE: c_int = 1 << 2;
pub const JS_PROP_C_W_E: c_int =
  JS_PROP_CONFIGURABLE | JS_PROP_WRITABLE | JS_PROP_ENUMERABLE;
pub const JS_PROP_THROW: c_int = 1 << 14;
pub const JS_PROP_THROW_STRICT: c_int = 1 << 15;

// Promise hook events.
pub const JS_PROMISE_HOOK_INIT: c_int = 0;
pub const JS_PROMISE_HOOK_BEFORE: c_int = 1;
pub const JS_PROMISE_HOOK_AFTER: c_int = 2;
pub const JS_PROMISE_HOOK_RESOLVE: c_int = 3;

// Callback signatures.
pub type JSCFunction = unsafe extern "C" fn(
  ctx: *mut JSContext,
  this_val: JSValue,
  argc: c_int,
  argv: *mut JSValue,
) -> JSValue;

pub type JSCFunctionData = unsafe extern "C" fn(
  ctx: *mut JSContext,
  this_val: JSValue,
  argc: c_int,
  argv: *mut JSValue,
  magic: c_int,
  func_data: *mut JSValue,
) -> JSValue;

pub type JSModuleInitFunc = unsafe extern "C" fn(
  ctx: *mut JSContext,
  m: *mut JSModuleDef,
) -> c_int;
pub type JSModuleNormalizeFunc = unsafe extern "C" fn(
  ctx: *mut JSContext,
  module_base_name: *const c_char,
  module_name: *const c_char,
  opaque: *mut c_void,
) -> *mut c_char;

pub type JSModuleLoaderFunc = unsafe extern "C" fn(
  ctx: *mut JSContext,
  module_name: *const c_char,
  opaque: *mut c_void,
) -> *mut JSModuleDef;

pub type JSHostPromiseRejectionTracker = unsafe extern "C" fn(
  ctx: *mut JSContext,
  promise: JSValue,
  reason: JSValue,
  is_handled: c_int,
  opaque: *mut c_void,
);

// ----- Runtime/context lifecycle ---------------------------------------

unsafe extern "C" {
  pub fn JS_NewRuntime() -> *mut JSRuntime;
  pub fn JS_FreeRuntime(rt: *mut JSRuntime);
  pub fn JS_SetRuntimeOpaque(rt: *mut JSRuntime, opaque: *mut c_void);
  pub fn JS_GetRuntimeOpaque(rt: *mut JSRuntime) -> *mut c_void;
  pub fn JS_SetMemoryLimit(rt: *mut JSRuntime, limit: usize);
  pub fn JS_SetMaxStackSize(rt: *mut JSRuntime, stack_size: usize);
  pub fn JS_SetGCThreshold(rt: *mut JSRuntime, gc_threshold: usize);
  pub fn JS_RunGC(rt: *mut JSRuntime);
  pub fn JS_IsJobPending(rt: *mut JSRuntime) -> bool;
  pub fn JS_ExecutePendingJob(
    rt: *mut JSRuntime,
    pctx: *mut *mut JSContext,
  ) -> c_int;

  pub fn JS_NewContext(rt: *mut JSRuntime) -> *mut JSContext;
  pub fn JS_NewContextRaw(rt: *mut JSRuntime) -> *mut JSContext;
  pub fn JS_FreeContext(ctx: *mut JSContext);
  pub fn JS_GetRuntime(ctx: *mut JSContext) -> *mut JSRuntime;
  pub fn JS_SetContextOpaque(ctx: *mut JSContext, opaque: *mut c_void);
  pub fn JS_GetContextOpaque(ctx: *mut JSContext) -> *mut c_void;
  pub fn JS_GetGlobalObject(ctx: *mut JSContext) -> JSValue;

  // Value refcount.
  pub fn JS_FreeValue(ctx: *mut JSContext, v: JSValue);
  pub fn JS_FreeValueRT(rt: *mut JSRuntime, v: JSValue);
  pub fn JS_DupValue(ctx: *mut JSContext, v: JSValue) -> JSValue;
  pub fn JS_DupValueRT(rt: *mut JSRuntime, v: JSValue) -> JSValue;

  // Primitive constructors.
  pub fn JS_NewBool(ctx: *mut JSContext, val: c_int) -> JSValue;
  pub fn JS_NewInt32(ctx: *mut JSContext, val: i32) -> JSValue;
  pub fn JS_NewUint32(ctx: *mut JSContext, val: u32) -> JSValue;
  pub fn JS_NewInt64(ctx: *mut JSContext, val: i64) -> JSValue;
  pub fn JS_NewFloat64(ctx: *mut JSContext, val: f64) -> JSValue;
  pub fn JS_NewString(ctx: *mut JSContext, str: *const c_char) -> JSValue;
  pub fn JS_NewStringLen(
    ctx: *mut JSContext,
    str: *const c_char,
    len: usize,
  ) -> JSValue;
  pub fn JS_NewAtomString(ctx: *mut JSContext, str: *const c_char) -> JSValue;
  pub fn JS_NewSymbol(
    ctx: *mut JSContext,
    description: *const c_char,
    is_global: c_int,
  ) -> JSValue;
  pub fn JS_NewBigInt64(ctx: *mut JSContext, val: i64) -> JSValue;
  pub fn JS_NewBigUint64(ctx: *mut JSContext, val: u64) -> JSValue;

  // Number extraction.
  pub fn JS_ToBool(ctx: *mut JSContext, v: JSValue) -> c_int;
  pub fn JS_ToInt32(ctx: *mut JSContext, pres: *mut i32, v: JSValue) -> c_int;
  pub fn JS_ToInt64(ctx: *mut JSContext, pres: *mut i64, v: JSValue) -> c_int;
  pub fn JS_ToFloat64(ctx: *mut JSContext, pres: *mut f64, v: JSValue)
  -> c_int;
  // The exported symbol is `JS_ToCStringLen2`. `JS_ToCString` and
  // `JS_ToCStringLen` are `static inline` wrappers in quickjs.h — see
  // `JS_ToCString` / `JS_ToCStringLen` Rust wrappers below.
  pub fn JS_ToCStringLen2(
    ctx: *mut JSContext,
    plen: *mut usize,
    v: JSValue,
    cesu8: bool,
  ) -> *const c_char;
  pub fn JS_FreeCString(ctx: *mut JSContext, ptr: *const c_char);

  // Objects, properties, calls.
  pub fn JS_NewObject(ctx: *mut JSContext) -> JSValue;
  pub fn JS_NewObjectClass(ctx: *mut JSContext, class_id: c_int) -> JSValue;
  pub fn JS_NewArray(ctx: *mut JSContext) -> JSValue;
  pub fn JS_IsArray(ctx: *mut JSContext, v: JSValue) -> c_int;
  pub fn JS_IsFunction(ctx: *mut JSContext, v: JSValue) -> c_int;
  pub fn JS_IsConstructor(ctx: *mut JSContext, v: JSValue) -> c_int;
  pub fn JS_GetPropertyStr(
    ctx: *mut JSContext,
    this_obj: JSValue,
    prop: *const c_char,
  ) -> JSValue;
  pub fn JS_GetPropertyUint32(
    ctx: *mut JSContext,
    this_obj: JSValue,
    idx: u32,
  ) -> JSValue;
  pub fn JS_SetPropertyStr(
    ctx: *mut JSContext,
    this_obj: JSValue,
    prop: *const c_char,
    val: JSValue,
  ) -> c_int;
  pub fn JS_SetPropertyUint32(
    ctx: *mut JSContext,
    this_obj: JSValue,
    idx: u32,
    val: JSValue,
  ) -> c_int;
  // QuickJS-ng exports the atom-based forms only; the str-based forms are
  // provided as Rust wrappers below.
  pub fn JS_HasProperty(
    ctx: *mut JSContext,
    this_obj: JSValue,
    prop: JSAtom,
  ) -> c_int;
  pub fn JS_DeleteProperty(
    ctx: *mut JSContext,
    this_obj: JSValue,
    prop: JSAtom,
    flags: c_int,
  ) -> c_int;
  pub fn JS_Call(
    ctx: *mut JSContext,
    func_obj: JSValue,
    this_obj: JSValue,
    argc: c_int,
    argv: *mut JSValue,
  ) -> JSValue;
  pub fn JS_CallConstructor(
    ctx: *mut JSContext,
    func_obj: JSValue,
    argc: c_int,
    argv: *mut JSValue,
  ) -> JSValue;
  // The exported symbol is `JS_NewCFunction2`. `JS_NewCFunction` is a
  // `static inline` wrapper in quickjs.h — see Rust wrapper below.
  pub fn JS_NewCFunction2(
    ctx: *mut JSContext,
    func: JSCFunction,
    name: *const c_char,
    length: c_int,
    cproto: c_int, // JSCFunctionEnum, 0 = JS_CFUNC_generic
    magic: c_int,
  ) -> JSValue;
  pub fn JS_NewCFunctionData(
    ctx: *mut JSContext,
    func: JSCFunctionData,
    length: c_int,
    magic: c_int,
    data_len: c_int,
    data: *mut JSValue,
  ) -> JSValue;

  // Eval/script.
  pub fn JS_Eval(
    ctx: *mut JSContext,
    input: *const c_char,
    input_len: usize,
    filename: *const c_char,
    eval_flags: c_int,
  ) -> JSValue;
  pub fn JS_EvalThis(
    ctx: *mut JSContext,
    this_obj: JSValue,
    input: *const c_char,
    input_len: usize,
    filename: *const c_char,
    eval_flags: c_int,
  ) -> JSValue;
  pub fn JS_EvalFunction(ctx: *mut JSContext, fun_obj: JSValue) -> JSValue;

  // Bytecode (for the snapshot Option-A path).
  pub fn JS_WriteObject(
    ctx: *mut JSContext,
    psize: *mut usize,
    obj: JSValue,
    flags: c_int,
  ) -> *mut u8;
  pub fn JS_ReadObject(
    ctx: *mut JSContext,
    buf: *const u8,
    buf_len: usize,
    flags: c_int,
  ) -> JSValue;

  // Promises.
  pub fn JS_NewPromiseCapability(
    ctx: *mut JSContext,
    resolving_funcs: *mut JSValue, // [resolve, reject]
  ) -> JSValue;
  pub fn JS_PromiseState(ctx: *mut JSContext, promise: JSValue) -> c_int;
  pub fn JS_PromiseResult(ctx: *mut JSContext, promise: JSValue) -> JSValue;
  pub fn JS_IsPromise(v: JSValue) -> c_int;
  pub fn JS_SetHostPromiseRejectionTracker(
    rt: *mut JSRuntime,
    cb: Option<JSHostPromiseRejectionTracker>,
    opaque: *mut c_void,
  );

  // Modules.
  pub fn JS_SetModuleLoaderFunc(
    rt: *mut JSRuntime,
    normalize: Option<JSModuleNormalizeFunc>,
    loader: Option<JSModuleLoaderFunc>,
    opaque: *mut c_void,
  );
  pub fn JS_GetModuleName(ctx: *mut JSContext, m: *mut JSModuleDef) -> JSAtom;
  pub fn JS_GetModuleNamespace(
    ctx: *mut JSContext,
    m: *mut JSModuleDef,
  ) -> JSValue;
  pub fn JS_NewCModule(
    ctx: *mut JSContext,
    name_str: *const c_char,
    func: Option<JSModuleInitFunc>,
  ) -> *mut JSModuleDef;
  pub fn JS_GetPrototype(ctx: *mut JSContext, val: JSValue) -> JSValue;
  pub fn JS_AddModuleExport(
    ctx: *mut JSContext,
    m: *mut JSModuleDef,
    name_str: *const c_char,
  ) -> c_int;
  pub fn JS_SetModuleExport(
    ctx: *mut JSContext,
    m: *mut JSModuleDef,
    export_name: *const c_char,
    val: JSValue,
  ) -> c_int;

  // Exception handling.
  pub fn JS_Throw(ctx: *mut JSContext, obj: JSValue) -> JSValue;
  pub fn JS_GetException(ctx: *mut JSContext) -> JSValue;
  pub fn JS_HasException(ctx: *mut JSContext) -> c_int;
  pub fn JS_ResetUncatchableError(ctx: *mut JSContext);
  pub fn JS_ThrowTypeError(
    ctx: *mut JSContext,
    fmt: *const c_char,
    ...
  ) -> JSValue;
  pub fn JS_ThrowReferenceError(
    ctx: *mut JSContext,
    fmt: *const c_char,
    ...
  ) -> JSValue;
  pub fn JS_ThrowSyntaxError(
    ctx: *mut JSContext,
    fmt: *const c_char,
    ...
  ) -> JSValue;
  pub fn JS_ThrowRangeError(
    ctx: *mut JSContext,
    fmt: *const c_char,
    ...
  ) -> JSValue;
  pub fn JS_ThrowInternalError(
    ctx: *mut JSContext,
    fmt: *const c_char,
    ...
  ) -> JSValue;
  pub fn JS_ThrowOutOfMemory(ctx: *mut JSContext) -> JSValue;

  // Atoms.
  pub fn JS_NewAtom(ctx: *mut JSContext, str: *const c_char) -> JSAtom;
  // QuickJS-ng allocator. The module loader / normalizer contract requires
  // returned strings to come from `js_malloc`/`js_strdup` because QuickJS
  // will free them via `js_free`. Plain libc allocators won't work — the
  // free side must match the malloc side.
  pub fn js_malloc(ctx: *mut JSContext, size: usize) -> *mut c_void;
  pub fn js_free(ctx: *mut JSContext, ptr: *mut c_void);
  pub fn js_strdup(ctx: *mut JSContext, s: *const c_char) -> *mut c_char;

  pub fn JS_NewAtomLen(
    ctx: *mut JSContext,
    str: *const c_char,
    len: usize,
  ) -> JSAtom;
  pub fn JS_FreeAtom(ctx: *mut JSContext, v: JSAtom);
  pub fn JS_AtomToString(ctx: *mut JSContext, atom: JSAtom) -> JSValue;
  pub fn JS_AtomToValue(ctx: *mut JSContext, atom: JSAtom) -> JSValue;
}

// ---- inline-wrapper FFI shims ------------------------------------------
//
// QuickJS-ng exposes some entry points only as `static inline` in
// `quickjs.h`, so they don't appear in the linker's symbol table. We
// re-implement the inline wrapper in Rust over the exported underlying
// symbol. Names match the C API spelling so the rest of the crate (and
// users who follow the QuickJS docs) can call the familiar function.

pub const JS_CFUNC_GENERIC: c_int = 0;
pub const JS_CFUNC_GENERIC_MAGIC: c_int = 1;

/// Equivalent of the inline `JS_NewCFunction` in `quickjs.h`:
/// `JS_NewCFunction2(ctx, func, name, length, JS_CFUNC_generic, 0)`.
///
/// # Safety
///
/// `ctx` must be a live `JSContext`; `name` must be a NUL-terminated string
/// or `NULL`; `func` must be ABI-compatible with `JSCFunction`.
#[inline]
pub unsafe fn JS_NewCFunction(
  ctx: *mut JSContext,
  func: JSCFunction,
  name: *const c_char,
  length: c_int,
) -> JSValue {
  unsafe { JS_NewCFunction2(ctx, func, name, length, JS_CFUNC_GENERIC, 0) }
}

/// Equivalent of the inline `JS_ToCString` in `quickjs.h`:
/// `JS_ToCStringLen2(ctx, NULL, val, false)`. Caller must release the
/// returned pointer with `JS_FreeCString`.
///
/// # Safety
///
/// `ctx` must be a live `JSContext`; `v` must be a JSValue owned (or
/// borrowed) by the caller for the duration of this call.
#[inline]
pub unsafe fn JS_ToCString(ctx: *mut JSContext, v: JSValue) -> *const c_char {
  unsafe { JS_ToCStringLen2(ctx, std::ptr::null_mut(), v, false) }
}

/// Equivalent of the inline `JS_ToCStringLen` in `quickjs.h`:
/// `JS_ToCStringLen2(ctx, plen, val, false)`.
///
/// # Safety
///
/// Same requirements as [`JS_ToCString`]; `plen` must be a writable
/// `usize` or `NULL`.
#[inline]
pub unsafe fn JS_ToCStringLen(
  ctx: *mut JSContext,
  plen: *mut usize,
  v: JSValue,
) -> *const c_char {
  unsafe { JS_ToCStringLen2(ctx, plen, v, false) }
}

/// Wrapper for the str-keyed `JS_HasProperty` lookup that was inline in
/// classic QuickJS but is not present in QuickJS-ng. Internally creates a
/// transient atom for the property name.
///
/// # Safety
///
/// `ctx` must be a live `JSContext`; `prop` must be a NUL-terminated
/// string; `this_obj` must be a JSValue owned (or borrowed) by the caller.
#[inline]
pub unsafe fn JS_HasPropertyStr(
  ctx: *mut JSContext,
  this_obj: JSValue,
  prop: *const c_char,
) -> c_int {
  unsafe {
    let atom = JS_NewAtom(ctx, prop);
    let r = JS_HasProperty(ctx, this_obj, atom);
    JS_FreeAtom(ctx, atom);
    r
  }
}

/// Wrapper for the str-keyed `JS_DeleteProperty` that QuickJS-ng dropped.
///
/// # Safety
///
/// Same requirements as [`JS_HasPropertyStr`].
#[inline]
pub unsafe fn JS_DeletePropertyStr(
  ctx: *mut JSContext,
  this_obj: JSValue,
  prop: *const c_char,
  flags: c_int,
) -> c_int {
  unsafe {
    let atom = JS_NewAtom(ctx, prop);
    let r = JS_DeleteProperty(ctx, this_obj, atom, flags);
    JS_FreeAtom(ctx, atom);
    r
  }
}
