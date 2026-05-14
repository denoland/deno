// Copyright 2018-2026 the Deno authors. MIT license.

//! Tests for the `create_sibling_context` primitive — the per-module
//! isolation foundation referenced in
//! <https://github.com/denoland/orchid/issues/64>.

use crate::JsRuntime;
use crate::RuntimeOptions;
use crate::create_sibling_context;

fn run_in_scope<R>(
  runtime: &mut JsRuntime,
  f: impl FnOnce(&mut v8::PinScope) -> R,
) -> R {
  let context = runtime.main_context();
  let isolate = runtime.v8_isolate();
  v8::scope!(let scope, isolate);
  let context = v8::Local::new(scope, &context);
  let scope = &mut v8::ContextScope::new(scope, context);
  f(scope)
}

/// The sibling context must have its own global object — a value set on
/// the main realm's `globalThis` is NOT visible from the sibling.
#[test]
fn sibling_has_distinct_global_this() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());

  runtime
    .execute_script("setup", "globalThis.MAIN_ONLY = 'main';")
    .unwrap();

  run_in_scope(&mut runtime, |scope| {
    let sibling = create_sibling_context(scope);
    let sibling_scope = &mut v8::ContextScope::new(scope, sibling);
    let global = sibling.global(sibling_scope);
    let key = v8::String::new(sibling_scope, "MAIN_ONLY").unwrap();
    let value = global.get(sibling_scope, key.into()).unwrap();
    assert!(
      value.is_undefined(),
      "sibling realm leaked main realm's globalThis.MAIN_ONLY"
    );
  });
}

/// Same in reverse: a value set in the sibling realm must NOT leak back
/// into the main realm.
#[test]
fn sibling_assignments_do_not_leak_to_main() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());

  run_in_scope(&mut runtime, |scope| {
    let sibling = create_sibling_context(scope);
    let sibling_scope = &mut v8::ContextScope::new(scope, sibling);
    let source = v8::String::new(
      sibling_scope,
      "globalThis.SIBLING_ONLY = 'sibling'; globalThis.SIBLING_ONLY",
    )
    .unwrap();
    let script =
      v8::Script::compile(sibling_scope, source, None).expect("compile");
    let result = script.run(sibling_scope).expect("run");
    let result_str = result.to_rust_string_lossy(sibling_scope);
    assert_eq!(result_str, "sibling");
  });

  let result = runtime
    .execute_script("check", "globalThis.SIBLING_ONLY ?? 'missing'")
    .unwrap();
  run_in_scope(&mut runtime, |scope| {
    let value = v8::Local::new(scope, &result);
    assert_eq!(value.to_rust_string_lossy(scope), "missing");
  });
}

/// The sibling context's embedder slots point at the main realm's
/// `ContextState` and `ModuleMap`. Built-in `Deno.core` is exposed by
/// `JsRuntime` setup on the main realm only — but ops *do* dispatch via
/// the shared state, which we exercise indirectly here by confirming the
/// sibling can read main-realm functions (security tokens match).
#[test]
fn sibling_and_main_share_security_token() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());

  runtime
    .execute_script(
      "setup",
      "globalThis.SHARED_FN = (x) => x + 1; globalThis.SHARED_FN.tag = 'main';",
    )
    .unwrap();

  run_in_scope(&mut runtime, |scope| {
    // Grab the function from the main realm.
    let main_context = scope.get_current_context();
    let main_global = main_context.global(scope);
    let key = v8::String::new(scope, "SHARED_FN").unwrap();
    let shared_fn = main_global.get(scope, key.into()).unwrap();
    assert!(shared_fn.is_function());

    // Plant it on the sibling realm's globalThis. Cross-context object
    // access only works if security tokens match — if the assertion
    // below holds, the sibling primitive set the tokens correctly.
    let sibling = create_sibling_context(scope);
    let sibling_scope = &mut v8::ContextScope::new(scope, sibling);
    let sibling_global = sibling.global(sibling_scope);
    let key = v8::String::new(sibling_scope, "MAIN_FN").unwrap();
    sibling_global.set(sibling_scope, key.into(), shared_fn);

    let source =
      v8::String::new(sibling_scope, "MAIN_FN(41)").expect("v8 string");
    let script =
      v8::Script::compile(sibling_scope, source, None).expect("compile");
    let result = script.run(sibling_scope).expect("run");
    assert_eq!(result.integer_value(sibling_scope).unwrap(), 42);
  });
}
