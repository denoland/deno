// Copyright 2018-2026 the Deno authors. MIT license.
//
// ES module loading through QuickJS-ng with a host-driven loader.
//
// What this proves: the loader callback hook (`JS_SetModuleLoaderFunc`)
// works against the real engine; modules compiled with
// `JS_EVAL_TYPE_MODULE | JS_EVAL_FLAG_COMPILE_ONLY` produce `JSModuleDef *`
// pointers that QuickJS will resolve `import` statements to; and a module
// that imports another module observably runs the imported code and gets
// the right values back.
//
// This is the synchronous loader path. QuickJS-ng's experimental
// `JS_LoadModuleAsync` (for true async loading) is the next layer; the
// synchronous path here is what `deno_core` would feed once the resolver
// has prefetched + transpiled all module sources, which is the same model
// Deno uses today.

#![cfg(feature = "link_quickjs")]

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CStr;
use std::ffi::CString;
use std::ffi::c_char;
use std::ffi::c_int;
use std::ffi::c_void;

use qjs_v8_compat::ffi;

// ---- the loader: a thread-local source map ------------------------------
//
// The loader callback is `extern "C"` and takes `*mut c_void` opaque, so
// in principle we'd thread our state through that pointer. For the test
// we use a thread-local instead: each test runs on its own thread and
// installs a fresh map before calling JS_Eval. This is identical to how
// `tests/real_engine.rs` collects callback output.

thread_local! {
  static SOURCES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
  static COLLECTOR: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

// Identity normalizer: preserve the spec exactly as the user wrote it in
// the `import`. Without this, QuickJS-ng's default normalizer strips
// `./` and resolves relative paths against the importing module's
// basename, which would force callers to know the normalization rules
// just to register a source.
unsafe extern "C" fn normalizer(
  ctx: *mut ffi::JSContext,
  _module_base_name: *const c_char,
  module_name: *const c_char,
  _opaque: *mut c_void,
) -> *mut c_char {
  // QuickJS owns the returned pointer and frees it via js_free, so the
  // string must be allocated by QuickJS's allocator. `js_strdup` does
  // exactly that.
  unsafe { ffi::js_strdup(ctx, module_name) }
}

unsafe extern "C" fn loader(
  ctx: *mut ffi::JSContext,
  module_name: *const c_char,
  _opaque: *mut c_void,
) -> *mut ffi::JSModuleDef {
  unsafe {
    let name = CStr::from_ptr(module_name).to_string_lossy().into_owned();
    let src = SOURCES.with(|s| s.borrow().get(&name).cloned());
    let Some(src) = src else {
      return std::ptr::null_mut();
    };

    // QuickJS-ng's parser reads one byte past `src_len` (lookahead) and
    // gets confused if it isn't NUL. Leak as a NUL-terminated buffer so
    // the lookahead always sees 0; pass `src_len` as the length so QuickJS
    // doesn't try to interpret the NUL as content.
    let mut bytes = src.into_bytes();
    bytes.push(0);
    let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    let src_ptr = leaked.as_ptr();
    let src_len = leaked.len() - 1;

    let fname_c = CString::new(name).unwrap();
    let val = ffi::JS_Eval(
      ctx,
      src_ptr as *const c_char,
      src_len,
      fname_c.as_ptr(),
      ffi::JS_EVAL_TYPE_MODULE | ffi::JS_EVAL_FLAG_COMPILE_ONLY,
    );
    if val.tag == ffi::JS_TAG_EXCEPTION {
      ffi::JS_FreeValue(ctx, ffi::JS_GetException(ctx));
      return std::ptr::null_mut();
    }
    // Per quickjs-libc.c's reference loader: the module is internally
    // referenced by QuickJS, so we extract the pointer and free the
    // surrounding JSValue (which decrements the wrapper's refcount, not
    // the module's).
    let m = ffi::jsv_get_ptr(&val) as *mut ffi::JSModuleDef;
    ffi::JS_FreeValue(ctx, val);
    m
  }
}

unsafe extern "C" fn collect_callback(
  ctx: *mut ffi::JSContext,
  _this: ffi::JSValue,
  argc: c_int,
  argv: *mut ffi::JSValue,
) -> ffi::JSValue {
  let mut parts = Vec::with_capacity(argc as usize);
  for i in 0..argc as isize {
    unsafe {
      let arg = *argv.offset(i);
      let p = ffi::JS_ToCString(ctx, arg);
      if !p.is_null() {
        parts.push(CStr::from_ptr(p).to_string_lossy().into_owned());
        ffi::JS_FreeCString(ctx, p);
      }
    }
  }
  COLLECTOR.with(|c| c.borrow_mut().push(parts.join(" ")));
  ffi::jsv_undefined()
}

fn run_module(
  sources: &[(&str, &str)],
  entry_name: &str,
  entry_src: &str,
) -> Vec<String> {
  SOURCES.with(|s| {
    let mut m = s.borrow_mut();
    m.clear();
    for (name, src) in sources {
      m.insert((*name).to_string(), (*src).to_string());
    }
  });
  COLLECTOR.with(|c| c.borrow_mut().clear());

  let entry_c = CString::new(entry_src).unwrap();
  let fname_c = CString::new(entry_name).unwrap();
  let collect_name = CString::new("collect").unwrap();

  unsafe {
    let rt = ffi::JS_NewRuntime();
    let ctx = ffi::JS_NewContext(rt);

    ffi::JS_SetModuleLoaderFunc(
      rt,
      Some(normalizer),
      Some(loader),
      std::ptr::null_mut(),
    );

    // Install the `collect` callback on the global so imported modules
    // can phone home with their results.
    let global = ffi::JS_GetGlobalObject(ctx);
    let collect_fn =
      ffi::JS_NewCFunction(ctx, collect_callback, collect_name.as_ptr(), 1);
    ffi::JS_SetPropertyStr(ctx, global, collect_name.as_ptr(), collect_fn);
    ffi::JS_FreeValue(ctx, global);

    // Compile + run the entry as a module. JS_EVAL_TYPE_MODULE returns a
    // promise-like result (the module's top-level evaluation); we drain
    // pending jobs to settle any microtasks the module scheduled.
    let val = ffi::JS_Eval(
      ctx,
      entry_c.as_ptr(),
      entry_src.len(),
      fname_c.as_ptr(),
      ffi::JS_EVAL_TYPE_MODULE,
    );
    if val.tag == ffi::JS_TAG_EXCEPTION {
      let exc = ffi::JS_GetException(ctx);
      let p = ffi::JS_ToCString(ctx, exc);
      let msg = if p.is_null() {
        "<no exception message>".to_string()
      } else {
        let s = CStr::from_ptr(p).to_string_lossy().into_owned();
        ffi::JS_FreeCString(ctx, p);
        s
      };
      ffi::JS_FreeValue(ctx, exc);
      ffi::JS_FreeContext(ctx);
      ffi::JS_FreeRuntime(rt);
      panic!("module entry threw: {msg}");
    }
    while ffi::JS_IsJobPending(rt) {
      let mut pctx = std::ptr::null_mut();
      let r = ffi::JS_ExecutePendingJob(rt, &mut pctx);
      if r <= 0 {
        break;
      }
    }
    ffi::JS_FreeValue(ctx, val);

    ffi::JS_FreeContext(ctx);
    ffi::JS_FreeRuntime(rt);
  }

  COLLECTOR.with(|c| c.borrow().clone())
}

#[test]
fn raw_module_eval_no_imports() {
  // No imports, just evaluate a tiny module that calls collect.
  let out = run_module(&[], "<simple>", "collect('hi from a module body');");
  assert_eq!(out, vec!["hi from a module body".to_string()]);
}

#[test]
fn raw_module_with_one_import() {
  // The smallest possible import: a one-export module imported and used.
  let out = run_module(
    &[("./x.js", "export const X = 7;")],
    "<simple>",
    "import { X } from './x.js'; collect('X=' + X);",
  );
  assert_eq!(out, vec!["X=7".to_string()]);
}

#[test]
fn import_function_name_property() {
  // Function.name is set from the source slice at parse time; this proves
  // the source-buffer lifetime is sufficient for that resolution to read
  // the right bytes (a regression we fixed by NUL-terminating the buffer
  // we pass to JS_Eval — QuickJS-ng's parser reads one byte past `len`).
  let out = run_module(
    &[("./fn.js", "export function myfn(x) { return x + 1; }")],
    "<simple>",
    "import { myfn } from './fn.js'; collect('name=' + myfn.name);",
  );
  assert_eq!(out, vec!["name=myfn".to_string()]);
}

#[test]
fn import_and_call_function() {
  // Import a function and invoke it across the module boundary.
  let out = run_module(
    &[("./fn.js", "export function myfn(x) { return x + 1; }")],
    "<simple>",
    "import { myfn } from './fn.js'; collect('val=' + myfn(10));",
  );
  assert_eq!(out, vec!["val=11".to_string()]);
}

#[test]
fn import_resolves_through_host_loader() {
  let out = run_module(
    &[(
      "./greet.js",
      "export function greet(name) { return 'hello, ' + name; }",
    )],
    "<entry>",
    "import { greet } from './greet.js'; collect('start'); collect(greet('quickjs')); collect('end');",
  );
  assert_eq!(
    out,
    vec![
      "start".to_string(),
      "hello, quickjs".to_string(),
      "end".to_string()
    ]
  );
}

#[test]
fn module_with_top_level_const_and_function() {
  let out = run_module(
    &[(
      "./math.js",
      r#"
        export const PI = 3;
        export function sq(x) { return x * x; }
        export function area(r) { return PI * sq(r); }
      "#,
    )],
    "<entry>",
    r#"
      import { area, PI } from './math.js';
      collect('PI=' + PI);
      collect('area5=' + area(5));
    "#,
  );
  assert_eq!(out, vec!["PI=3".to_string(), "area5=75".to_string()]);
}

#[test]
fn transitive_import_chain() {
  let out = run_module(
    &[
      (
        "./a.js",
        "import { b } from './b.js'; export const a = b + 1;",
      ),
      (
        "./b.js",
        "import { c } from './c.js'; export const b = c + 1;",
      ),
      ("./c.js", "export const c = 40;"),
    ],
    "<entry>",
    "import { a } from './a.js'; collect('chain=' + a);",
  );
  // c=40 → b=41 → a=42
  assert_eq!(out, vec!["chain=42".to_string()]);
}

#[test]
fn missing_module_throws() {
  // The loader returns null for unknown modules; QuickJS turns that into
  // an exception that propagates out of JS_Eval.
  let result = std::panic::catch_unwind(|| {
    run_module(&[], "<entry>", "import { x } from './nope.js'; collect(x);")
  });
  assert!(result.is_err(), "expected panic on missing module");
}

#[test]
fn import_then_call_with_microtasks() {
  // Imported function returns a Promise; verify draining settles it and
  // the `.then` callback runs before we collect results.
  let out = run_module(
    &[(
      "./async.js",
      r#"
        export function later(v) {
          return Promise.resolve(v).then(x => x * 10);
        }
      "#,
    )],
    "<entry>",
    r#"
      import { later } from './async.js';
      later(7).then(v => collect('async=' + v));
    "#,
  );
  assert_eq!(out, vec!["async=70".to_string()]);
}
