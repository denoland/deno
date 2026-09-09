// Copyright 2018-2026 the Deno authors. MIT license.

//! `ext:core/ops` exports only the names extension sources import
//! (deno_core_revamp#41, final step).

use crate::JsRuntime;
use crate::JsRuntimeForSnapshot;
use crate::RuntimeOptions;
use crate::op2;

#[op2(fast)]
#[smi]
fn op_exp_used(#[smi] a: i32) -> i32 {
  a + 1
}

#[op2(fast)]
#[smi]
fn op_exp_unused(#[smi] a: i32) -> i32 {
  a + 2
}

#[op2]
#[string]
fn op_exp_aliased(#[string] s: String) -> String {
  format!("{s}!")
}

deno_core::extension!(
  exp_ops,
  ops = [op_exp_used, op_exp_unused, op_exp_aliased],
  esm_entry_point = "ext:exp/main.js",
  esm = ["ext:exp/main.js" = {
    source = r#"
        import { op_exp_used, op_exp_aliased as alias } from "ext:core/ops";
        globalThis.expUsed = op_exp_used(41);
        globalThis.expAliased = alias("hi");
      "#
  },],
);

/// Reads the export list of the live `ext:core/ops` module by importing a
/// namespace from a module the scanner never saw. `lazy_load_es_module_with_code`
/// instantiates against the synthetic module that already exists, so a name
/// missing from the export list fails here rather than being `undefined`.
fn try_import(
  runtime: &mut JsRuntime,
  specifier: &'static str,
  code: &'static str,
) -> Result<(), String> {
  runtime
    .lazy_load_es_module_with_code(specifier, code)
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[test]
fn exports_only_imported_names() {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![exp_ops::init()],
    ..Default::default()
  });

  // The statically imported op ran at extension-init time.
  runtime
    .execute_script(
      "check.js",
      r#"
      if (globalThis.expUsed !== 42) throw new Error("expUsed=" + globalThis.expUsed);
      if (globalThis.expAliased !== "hi!") throw new Error("expAliased=" + globalThis.expAliased);
      "#,
    )
    .unwrap();

  // Imported names have export cells...
  try_import(
    &mut runtime,
    "ext:exp/probe_ok.js",
    r#"import { op_exp_used, op_exp_aliased } from "ext:core/ops";
       if (typeof op_exp_used !== "function") throw new Error("not a fn");
       if (typeof op_exp_aliased !== "function") throw new Error("not a fn");"#,
  )
  .unwrap();

  // ...and a name nothing imports does not. The failure is loud and it names
  // the op.
  let err = try_import(
    &mut runtime,
    "ext:exp/probe_missing.js",
    r#"import { op_exp_unused } from "ext:core/ops";"#,
  )
  .unwrap_err();
  assert!(
    err.contains("op_exp_unused"),
    "expected the error to name the missing export, got: {err}"
  );
  assert!(
    err.contains("does not provide an export"),
    "expected a module instantiation SyntaxError, got: {err}"
  );

  // But the op itself is perfectly callable through the `Deno.core.ops`
  // interceptor -- shrinking the export list does not disable an op.
  runtime
    .execute_script(
      "call_unused.js",
      r#"
      const v = Deno.core.ops.op_exp_unused(40);
      if (v !== 42) throw new Error("got " + v);
      "#,
    )
    .unwrap();
}

#[test]
fn export_all_escape_hatch_restores_every_name() {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![exp_ops::init()],
    export_all_ops_from_virtual_module: true,
    ..Default::default()
  });

  try_import(
    &mut runtime,
    "ext:exp/probe_missing.js",
    r#"import { op_exp_unused } from "ext:core/ops";
       if (op_exp_unused(40) !== 42) throw new Error("bad");"#,
  )
  .unwrap();
}

deno_core::extension!(
  exp_ops_namespace,
  ops = [op_exp_used, op_exp_unused],
  esm_entry_point = "ext:expns/main.js",
  esm = ["ext:expns/main.js" = {
    source = r#"
        import * as ops from "ext:core/ops";
        globalThis.expNs = ops.op_exp_used(41);
      "#
  },],
);

/// A namespace import needs every name, and the scanner cannot know which ones
/// are read off the namespace object. It must fall back to exporting all of
/// them rather than silently handing back a half-populated namespace.
#[test]
fn namespace_import_falls_back_to_exporting_everything() {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![exp_ops_namespace::init()],
    ..Default::default()
  });
  runtime
    .execute_script(
      "check.js",
      "if (globalThis.expNs !== 42) throw new Error('expNs=' + globalThis.expNs);",
    )
    .unwrap();
  try_import(
    &mut runtime,
    "ext:expns/probe.js",
    r#"import { op_exp_unused } from "ext:core/ops";
       if (op_exp_unused(40) !== 42) throw new Error("bad");"#,
  )
  .unwrap();
}

deno_core::extension!(
  exp_ops_lazy,
  ops = [op_exp_used, op_exp_unused],
  lazy_loaded_esm = ["ext:exp/lazy.js" = {
    source = r#"
        import { op_exp_used } from "ext:core/ops";
        export const value = op_exp_used(41);
      "#
  },],
);

/// A `lazy_loaded_esm` module instantiates long after the snapshot was built,
/// against the export list baked into the blob. Its imports have to be in the
/// union, which is why the scan covers the lazy sources too.
#[test]
fn lazy_loaded_esm_imports_survive_a_snapshot() {
  let _snapshot_lock = super::snapshot_test_lock();
  let snapshot = {
    let runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
      extensions: vec![exp_ops_lazy::init()],
      ..Default::default()
    });
    runtime.snapshot()
  };
  let snapshot = Box::leak(snapshot);

  let mut runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    extensions: vec![exp_ops_lazy::init()],
    ..Default::default()
  });

  runtime
    .execute_script(
      "load_lazy.js",
      r#"
      const mod = Deno.core.createLazyLoader("ext:exp/lazy.js")();
      if (mod.value !== 42) throw new Error("got " + mod.value);
      "#,
    )
    .unwrap();
}

deno_core::extension!(
  exp_ops_residual,
  ops = [op_exp_used, op_exp_unused],
  lazy_loaded_esm = ["ext:exp/residual.js" = {
    source = r#"
        import { op_exp_used } from "ext:core/ops";
        export const value = op_exp_used(41);
      "#
  },],
);

/// The same module, but reaching the runtime through
/// `residual_lazy_esm_sources` (the shape a real embedder's build script emits
/// for lazy files the snapshot did not consume). Those sources are known at
/// snapshot-build time, so they are scanned there.
#[test]
fn residual_lazy_esm_imports_are_in_the_export_set() {
  let _snapshot_lock = super::snapshot_test_lock();
  const RESIDUAL: &[(&str, &str)] = &[(
    "ext:exp/residual.js",
    r#"
      import { op_exp_used } from "ext:core/ops";
      export const value = op_exp_used(41);
    "#,
  )];

  let snapshot = {
    let runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
      extensions: vec![exp_ops_residual::init()],
      residual_lazy_esm_sources: RESIDUAL,
      ..Default::default()
    });
    runtime.snapshot()
  };
  let snapshot = Box::leak(snapshot);

  let mut runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    extensions: vec![exp_ops_residual::init()],
    residual_lazy_esm_sources: RESIDUAL,
    ..Default::default()
  });

  runtime
    .execute_script(
      "load_residual.js",
      r#"
      const mod = Deno.core.createLazyLoader("ext:exp/residual.js")();
      if (mod.value !== 42) throw new Error("got " + mod.value);
      "#,
    )
    .unwrap();
}
