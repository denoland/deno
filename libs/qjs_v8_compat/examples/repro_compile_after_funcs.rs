// Minimal repro: create lots of JSCFunctions, then compile a script.
// Mirrors what deno_core does during JsRuntime::new() to bisect where
// the QuickJS-side memory corruption originates.

use core::ffi::c_int;
use qjs_v8_compat::ffi;
use qjs_v8_compat::sys::JSValue;

unsafe extern "C" fn noop_cfn(
  _ctx: *mut ffi::JSContext,
  _this: JSValue,
  _argc: c_int,
  _argv: *mut JSValue,
) -> JSValue {
  qjs_v8_compat::sys::jsv_undefined()
}

fn main() {
  unsafe {
    let rt = ffi::JS_NewRuntime();
    let ctx = ffi::JS_NewContext(rt);

    // Create as many JSCFunctions as deno_core does (~80 ops).
    let n_funcs: i32 = std::env::var("N_FUNCS").ok().and_then(|s| s.parse().ok()).unwrap_or(80);
    let mut funcs = Vec::new();
    for i in 0..n_funcs {
      let f = ffi::JS_NewCFunction(ctx, noop_cfn, core::ptr::null(), i % 16);
      funcs.push(f);
    }
    eprintln!("[repro] created {} JSCFunctions", funcs.len());

    // Compile a small script.
    let src = r#"
const x = 1;
const y = 2;
const z = x + y;
"#;
    let fname = std::ffi::CString::new("<repro>").unwrap();
    let r = ffi::JS_Eval(
      ctx,
      src.as_ptr() as *const _,
      src.len(),
      fname.as_ptr(),
      ffi::JS_EVAL_TYPE_GLOBAL,
    );
    eprintln!("[repro] eval result tag={}", r.tag);
    if qjs_v8_compat::sys::jsv_is_exception(&r) {
      let exc = ffi::JS_GetException(ctx);
      let mut len = 0usize;
      let p = ffi::JS_ToCStringLen(ctx, &mut len, exc);
      if !p.is_null() {
        let bytes = std::slice::from_raw_parts(p as *const u8, len);
        eprintln!("[repro] small exception: {}", std::str::from_utf8_unchecked(bytes));
        ffi::JS_FreeCString(ctx, p);
      }
      ffi::JS_FreeValue(ctx, exc);
    }

    // Compile a bigger script.
    let big_src = include_str!("../../core/02_timers.js");
    eprintln!("[repro] big src len={}", big_src.len());
    let r2 = ffi::JS_Eval(
      ctx,
      big_src.as_ptr() as *const _,
      big_src.len(),
      fname.as_ptr(),
      ffi::JS_EVAL_TYPE_GLOBAL | ffi::JS_EVAL_FLAG_COMPILE_ONLY,
    );
    eprintln!("[repro] big eval result tag={}", r2.tag);
    if qjs_v8_compat::sys::jsv_is_exception(&r2) {
      let exc = ffi::JS_GetException(ctx);
      let mut len = 0usize;
      let p = ffi::JS_ToCStringLen(ctx, &mut len, exc);
      if !p.is_null() {
        let bytes = std::slice::from_raw_parts(p as *const u8, len);
        eprintln!("[repro] exception: {}", std::str::from_utf8_unchecked(bytes));
        ffi::JS_FreeCString(ctx, p);
      }
      ffi::JS_FreeValue(ctx, exc);
    }

    ffi::JS_FreeContext(ctx);
    ffi::JS_FreeRuntime(rt);
  }
  eprintln!("[repro] done");
}
