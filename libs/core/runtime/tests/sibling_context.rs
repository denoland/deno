// Copyright 2018-2026 the Deno authors. MIT license.

//! Tests for the per-module isolation primitives — the foundation for the
//! foolproof per-module-permissions design tracked in
//! <https://github.com/denoland/orchid/issues/64>.

use std::collections::HashMap;

use crate::JsRuntime;
use crate::RuntimeOptions;
use crate::compile_module_in;
use crate::create_module_namespace_bridge;
use crate::create_sibling_context;
use crate::evaluate_module_in;
use crate::instantiate_module_in;
use crate::module_namespace_in;

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

/// Security tokens match between main and sibling, so cross-realm object
/// access works (required for synthetic-module bridges).
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
    let main_context = scope.get_current_context();
    let main_global = main_context.global(scope);
    let key = v8::String::new(scope, "SHARED_FN").unwrap();
    let shared_fn = main_global.get(scope, key.into()).unwrap();
    assert!(shared_fn.is_function());

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

/// End-to-end: a *denied* module is compiled, instantiated, and evaluated
/// in a sibling realm with no `Deno` binding installed. A trusted main
/// module imports its exports through a synthetic-module bridge. The
/// trusted side observes the export values; the denied module's attempt
/// to touch `Deno` resolves to `undefined` because the sibling realm's
/// `globalThis` has no such property.
#[test]
fn cross_realm_module_namespace_bridge() {
  let mut runtime = JsRuntime::new(RuntimeOptions::default());

  let bridge_global = run_in_scope(&mut runtime, |scope| {
    // 1. Spin up a sibling realm. No `Deno` is installed on it, so a
    //    module loaded into this realm has *no* lexical or global
    //    access to ops.
    let sibling = create_sibling_context(scope);

    // 2. Compile a "denied" ES module in the sibling realm. The body
    //    captures the runtime value of `globalThis.Deno` and a constant
    //    so the test can assert both: the export shape AND the absence
    //    of `Deno` from the denied module's view.
    let denied_module = compile_module_in(
      scope,
      sibling,
      "denied.js",
      r#"
        export const greeting = "hello from denied realm";
        export const deno_visible = typeof globalThis.Deno !== "undefined";
      "#,
    )
    .expect("compile denied module");

    // 3. Instantiate it. No imports, so an empty resolution table is fine.
    instantiate_module_in(scope, sibling, &denied_module, HashMap::new())
      .expect("instantiate denied module");

    // 4. Evaluate it. After this, the namespace's exports are live.
    evaluate_module_in(scope, sibling, &denied_module)
      .expect("evaluate denied module");

    // 5. Snapshot the namespace and build a synthetic bridge in the
    //    main realm. The bridge holds v8::Globals from the sibling
    //    realm; security tokens match, so cross-realm transfer is fine.
    let namespace = module_namespace_in(scope, sibling, &denied_module);
    let main_context = scope.get_current_context();
    create_module_namespace_bridge(
      scope,
      main_context,
      "denied-bridge",
      sibling,
      &namespace,
    )
  });

  // 6. Compile a trusted module in the main realm that imports from the
  //    bridge. V8 resolves the import via the explicit resolution
  //    table we hand to `instantiate_module_in`.
  let result_global = run_in_scope(&mut runtime, |scope| {
    let main_context = scope.get_current_context();
    let main_module = compile_module_in(
      scope,
      main_context,
      "main.js",
      r#"
        import { greeting, deno_visible } from "denied";
        globalThis.__test_result__ = { greeting, deno_visible };
      "#,
    )
    .expect("compile main module");

    let mut resolutions = HashMap::new();
    resolutions.insert("denied".to_string(), bridge_global);
    instantiate_module_in(scope, main_context, &main_module, resolutions)
      .expect("instantiate main");
    evaluate_module_in(scope, main_context, &main_module)
      .expect("evaluate main")
  });

  // 7. Confirm the trusted realm saw the foreign-realm exports and that
  //    the denied module observed no `Deno` global.
  let result = runtime
    .execute_script("check", "JSON.stringify(globalThis.__test_result__)")
    .unwrap();
  run_in_scope(&mut runtime, |scope| {
    let value = v8::Local::new(scope, &result);
    let json = value.to_rust_string_lossy(scope);
    assert_eq!(
      json, r#"{"greeting":"hello from denied realm","deno_visible":false}"#,
      "expected the trusted realm to see the bridge exports and the denied realm to have no Deno global"
    );
  });

  // Suppress unused-result lint on the evaluation promise.
  let _ = result_global;
}

/// `JsRuntime::install_isolated_module` registers a bridge under the
/// supplied specifier in the main `ModuleMap`. A subsequent main-realm
/// module that statically imports the specifier sees the foreign
/// realm's exports, while the isolated body observes a stripped
/// globalThis (no `Deno`).
#[tokio::test]
async fn install_isolated_module_end_to_end() {
  use crate::ascii_str;
  use crate::modules::StaticModuleLoader;
  use crate::url::Url;

  let main_specifier = Url::parse("file:///main.js").unwrap();
  let main_source = ascii_str!(
    r#"
    import { greeting, has_deno } from "file:///denied.js";
    globalThis.__test_result__ = { greeting, has_deno };
    "#
  );

  let loader = StaticModuleLoader::with(main_specifier.clone(), main_source);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(std::rc::Rc::new(loader)),
    ..Default::default()
  });

  // Install the denied module in a sibling realm with no Deno binding.
  runtime
    .install_isolated_module(
      "file:///denied.js",
      r#"
        export const greeting = "from-isolated";
        export const has_deno = typeof globalThis.Deno !== "undefined";
      "#
      .to_string(),
    )
    .expect("install_isolated_module");

  // Load and run the main module. V8's resolver finds the bridge in
  // ModuleMap when "denied" is requested.
  let module_id = runtime
    .load_main_es_module(&main_specifier)
    .await
    .expect("load main module");
  let _evaluate = runtime.mod_evaluate(module_id);
  runtime
    .run_event_loop(Default::default())
    .await
    .expect("event loop");

  let result = runtime
    .execute_script("check", "JSON.stringify(globalThis.__test_result__)")
    .unwrap();
  run_in_scope(&mut runtime, |scope| {
    let value = v8::Local::new(scope, &result);
    let json = value.to_rust_string_lossy(scope);
    assert_eq!(
      json, r#"{"greeting":"from-isolated","has_deno":false}"#,
      "expected the trusted realm to see the bridge exports and the denied realm to have no Deno global"
    );
  });
}
