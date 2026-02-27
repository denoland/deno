// Copyright 2018-2025 the Deno authors. MIT license.

use self::runtime::CreateSnapshotOptions;
use self::runtime::create_snapshot;
use crate::modules::ModuleInfo;
use crate::modules::RequestedModuleType;
use crate::runtime::NO_OF_BUILTIN_MODULES;
use crate::*;
use deno_error::JsErrorBox;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn will_snapshot() {
  let snapshot = {
    let mut runtime = JsRuntimeForSnapshot::new(Default::default());
    runtime.execute_script("a.js", "a = 1 + 2").unwrap();
    runtime.snapshot()
  };

  let snapshot = Box::leak(snapshot);
  let mut runtime2 = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });
  runtime2
    .execute_script("check.js", "if (a != 3) throw Error('x')")
    .unwrap();
}

#[test]
fn will_snapshot2() {
  let startup_data = {
    let mut runtime = JsRuntimeForSnapshot::new(Default::default());
    runtime.execute_script("a.js", "let a = 1 + 2").unwrap();
    runtime.snapshot()
  };

  let snapshot = Box::leak(startup_data);
  let mut runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });

  let startup_data = {
    runtime
      .execute_script("check_a.js", "if (a != 3) throw Error('x')")
      .unwrap();
    runtime.execute_script("b.js", "b = 2 + 3").unwrap();
    runtime.snapshot()
  };

  let snapshot = Box::leak(startup_data);
  {
    let mut runtime = JsRuntime::new(RuntimeOptions {
      startup_snapshot: Some(snapshot),
      ..Default::default()
    });
    runtime
      .execute_script("check_b.js", "if (b != 5) throw Error('x')")
      .unwrap();
    runtime
      .execute_script("check2.js", "if (!Deno.core) throw Error('x')")
      .unwrap();
  }
}

#[test]
fn test_snapshot_callbacks() {
  let snapshot = {
    let mut runtime = JsRuntimeForSnapshot::new(Default::default());
    runtime
      .execute_script(
        "a.js",
        r#"
        Deno.core.setMacrotaskCallback(() => {
          return true;
        });
        Deno.core.ops.op_set_format_exception_callback(()=> {
          return null;
        })
        Deno.core.setUnhandledPromiseRejectionHandler(() => {
          return false;
        });
        a = 1 + 2;
    "#,
      )
      .unwrap();
    runtime.snapshot()
  };

  let snapshot = Box::leak(snapshot);
  let mut runtime2 = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });
  runtime2
    .execute_script("check.js", "if (a != 3) throw Error('x')")
    .unwrap();
}

#[test]
fn test_from_snapshot() {
  let snapshot = {
    let mut runtime = JsRuntimeForSnapshot::new(Default::default());
    runtime.execute_script("a.js", "a = 1 + 2").unwrap();
    runtime.snapshot()
  };

  let snapshot = Box::leak(snapshot);
  let mut runtime2 = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });
  runtime2
    .execute_script("check.js", "if (a != 3) throw Error('x')")
    .unwrap();
}

/// Smoke test for create_snapshot.
#[test]
fn test_snapshot_creator() {
  let output = create_snapshot(
    CreateSnapshotOptions {
      cargo_manifest_dir: "",
      startup_snapshot: None,
      skip_op_registration: false,
      extension_transpiler: None,
      extensions: vec![],
      with_runtime_cb: Some(Box::new(|runtime| {
        runtime.execute_script("a.js", "a = 1 + 2").unwrap();
      })),
    },
    None,
  )
  .unwrap();

  let snapshot = Box::leak(output.output);

  let mut runtime2 = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });
  runtime2
    .execute_script("check.js", "if (a != 3) throw Error('x')")
    .unwrap();
}

#[test]
fn test_snapshot_creator_warmup() {
  let counter = Rc::new(RefCell::new(0));

  let c = counter.clone();
  let output = create_snapshot(
    CreateSnapshotOptions {
      cargo_manifest_dir: "",
      startup_snapshot: None,
      skip_op_registration: false,
      extensions: vec![],
      extension_transpiler: None,
      with_runtime_cb: Some(Box::new(move |runtime| {
        c.replace_with(|&mut c| c + 1);

        runtime.execute_script("a.js", "a = 1 + 2").unwrap();
      })),
    },
    Some("const b = 'Hello'"),
  )
  .unwrap();

  // `with_runtime_cb` executes twice, once for snapshot creation
  // and one for the warmup.
  assert_eq!(*counter.borrow(), 2);

  let snapshot = Box::leak(output.output);

  let mut runtime2 = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });
  runtime2
    .execute_script("check.js", "if (a != 3) throw Error('x')")
    .unwrap();
}

#[test]
fn es_snapshot() {
  fn create_module(
    runtime: &mut JsRuntime,
    i: usize,
    main: bool,
  ) -> ModuleInfo {
    let specifier = crate::resolve_url(&format!("file:///{i}.js")).unwrap();
    let prev = i - 1;
    let source_code = format!(
      r#"
      import {{ f{prev} }} from "file:///{prev}.js";
      export function f{i}() {{ return f{prev}() }}
      "#
    );

    let id = if main {
      futures::executor::block_on(
        runtime.load_main_es_module_from_code(&specifier, source_code),
      )
      .unwrap()
    } else {
      futures::executor::block_on(
        runtime.load_side_es_module_from_code(&specifier, source_code),
      )
      .unwrap()
    };
    assert_eq!(i + NO_OF_BUILTIN_MODULES, id);

    #[allow(clippy::let_underscore_future)]
    let _ = runtime.mod_evaluate(id);
    futures::executor::block_on(runtime.run_event_loop(Default::default()))
      .unwrap();

    ModuleInfo {
      id,
      main,
      name: specifier.into(),
      requests: vec![crate::modules::ModuleRequest {
        reference: crate::modules::ModuleReference {
          specifier: ModuleSpecifier::parse(&format!("file:///{prev}.js"))
            .unwrap(),
          requested_module_type: RequestedModuleType::None,
        },
        specifier_key: Some(format!("file:///{prev}.js")),
        referrer_source_offset: Some(25 + prev.to_string().len() as i32),
        phase: crate::modules::ModuleImportPhase::Evaluation,
      }],
      module_type: ModuleType::JavaScript,
    }
  }

  #[allow(clippy::unnecessary_wraps)]
  #[op2]
  #[string]
  fn op_test() -> Result<String, JsErrorBox> {
    Ok(String::from("test"))
  }
  let mut runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
    extensions: vec![Extension {
      name: "test_ext",
      ops: Cow::Borrowed(&[DECL]),
      ..Default::default()
    }],
    ..Default::default()
  });

  let specifier = crate::resolve_url("file:///0.js").unwrap();
  let id = futures::executor::block_on(runtime.load_side_es_module_from_code(
    &specifier,
    r#"export function f0() { return "hello world" }"#,
  ))
  .unwrap();

  #[allow(clippy::let_underscore_future)]
  let _ = runtime.mod_evaluate(id);
  futures::executor::block_on(runtime.run_event_loop(Default::default()))
    .unwrap();

  let mut modules = vec![];
  modules.push(ModuleInfo {
    id,
    main: false,
    name: specifier.into(),
    requests: vec![],
    module_type: ModuleType::JavaScript,
  });

  modules.extend((1..200).map(|i| create_module(&mut runtime, i, false)));

  runtime.module_map().assert_module_map(&modules);

  let snapshot = runtime.snapshot();
  let snapshot = Box::leak(snapshot);

  let mut runtime2 = JsRuntimeForSnapshot::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    extensions: vec![Extension {
      name: "test_ext",
      ops: Cow::Borrowed(&[DECL]),
      ..Default::default()
    }],
    ..Default::default()
  });

  runtime2.module_map().assert_module_map(&modules);

  modules.extend((200..400).map(|i| create_module(&mut runtime2, i, false)));
  modules.push(create_module(&mut runtime2, 400, true));

  runtime2.module_map().assert_module_map(&modules);

  let snapshot2 = runtime2.snapshot();
  let snapshot2 = Box::leak(snapshot2);

  const DECL: OpDecl = op_test();
  let mut runtime3 = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot2),
    extensions: vec![Extension {
      name: "test_ext",
      ops: Cow::Borrowed(&[DECL]),
      ..Default::default()
    }],
    ..Default::default()
  });

  runtime3.module_map().assert_module_map(&modules);

  let source_code = r#"(async () => {
    const mod = await import("file:///400.js");
    return mod.f400() + " " + Deno.core.ops.op_test();
  })();"#;
  let val = runtime3.execute_script(".", source_code).unwrap();
  #[allow(deprecated)]
  let val = futures::executor::block_on(runtime3.resolve_value(val)).unwrap();
  {
    deno_core::scope!(scope, runtime3);
    let value = v8::Local::new(scope, val);
    let str_ = value.to_string(scope).unwrap().to_rust_string_lossy(scope);
    assert_eq!(str_, "hello world test");
  }
}

#[test]
pub(crate) fn es_snapshot_without_runtime_module_loader() {
  let startup_data = {
    deno_core::extension!(
      module_snapshot,
      esm_entry_point = "ext:module_snapshot/test.js",
      esm = ["ext:module_snapshot/test.js" =
        { source = "globalThis.TEST = 'foo'; export const TEST = 'bar';" },]
    );

    let runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
      extensions: vec![module_snapshot::init()],
      ..Default::default()
    });

    runtime.snapshot()
  };

  let snapshot = Box::leak(startup_data);

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: None,
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });
  let realm = runtime.main_realm();

  // Make sure the module was evaluated.
  {
    deno_core::scope!(scope, runtime);
    let global_test: v8::Local<v8::String> =
      JsRuntime::eval(scope, "globalThis.TEST").unwrap();
    assert_eq!(
      serde_v8::to_utf8(global_test.to_string(scope).unwrap(), scope),
      String::from("foo"),
    );
  }

  // Dynamic imports of ext: from non-ext: modules are not allowed.
  let dyn_import_promise = realm
    .execute_script(
      runtime.v8_isolate(),
      "",
      "import('ext:module_snapshot/test.js')",
    )
    .unwrap();
  #[allow(deprecated)]
  let dyn_import_result =
    futures::executor::block_on(runtime.resolve_value(dyn_import_promise));
  assert_eq!(
    dyn_import_result.err().unwrap().to_string().as_str(),
    r#"Uncaught (in promise) TypeError: Importing ext: modules is only allowed from ext: and node: modules. Tried to import ext:module_snapshot/test.js from (no referrer)"#
  );

  // But not a new one
  let dyn_import_promise = realm
    .execute_script(
      runtime.v8_isolate(),
      "",
      "import('ext:module_snapshot/test2.js')",
    )
    .unwrap();
  #[allow(deprecated)]
  let dyn_import_result =
    futures::executor::block_on(runtime.resolve_value(dyn_import_promise));
  assert!(dyn_import_result.is_err());
  assert_eq!(
    dyn_import_result.err().unwrap().to_string().as_str(),
    r#"Uncaught (in promise) TypeError: Importing ext: modules is only allowed from ext: and node: modules. Tried to import ext:module_snapshot/test2.js from (no referrer)"#
  );
}

#[test]
pub fn snapshot_with_additional_extensions() {
  #[op2]
  #[string]
  fn op_before() -> String {
    "before".to_owned()
  }

  #[op2]
  #[string]
  fn op_after() -> String {
    "after".to_owned()
  }

  deno_core::extension!(
    before_snapshot,
    ops = [op_before],
    esm_entry_point = "ext:module_snapshot/before.js",
    esm = ["ext:module_snapshot/before.js" =
      // If this throws, we accidentally tried to evaluate this module twice
      { source = "if (globalThis.before) { throw 'twice?' } globalThis.before = () => { globalThis.BEFORE = Deno.core.ops.op_before(); };" },]
  );
  deno_core::extension!(
    after_snapshot,
    ops = [op_after],
    esm_entry_point = "ext:module_snapshot/after.js",
    esm = ["ext:module_snapshot/after.js" = {
      source =
        "globalThis.before(); globalThis.AFTER = Deno.core.ops.op_after();"
    },]
  );

  let snapshot = {
    let runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
      extensions: vec![before_snapshot::init()],
      ..Default::default()
    });

    Box::leak(runtime.snapshot())
  };

  let mut runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    extensions: vec![before_snapshot::init(), after_snapshot::init()],
    ..Default::default()
  });

  // Make sure the module was evaluated.
  {
    deno_core::scope!(scope, runtime);
    let global_test: v8::Local<v8::String> =
      JsRuntime::eval(scope, "globalThis.BEFORE + '/' + globalThis.AFTER")
        .unwrap();
    assert_eq!(
      serde_v8::to_utf8(global_test.to_string(scope).unwrap(), scope),
      String::from("before/after"),
    );
  }
}
