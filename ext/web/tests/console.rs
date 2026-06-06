// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;

fn console_runtime(extra: Vec<Extension>) -> JsRuntime {
  let mut extensions = vec![
    deno_webidl::deno_webidl::init(),
    deno_web::deno_web::init(
      Default::default(),
      None,
      Default::default(),
      Default::default(),
    ),
  ];
  extensions.extend(extra);
  JsRuntime::new(RuntimeOptions {
    extensions,
    ..Default::default()
  })
}

#[test]
fn console_inspect_error_works_without_node() {
  let mut runtime = console_runtime(vec![]);
  runtime
    .execute_script(
      "test.js",
      r#"
        const console = Deno.core.loadExtScript("ext:deno_web/01_console.js");
        console.inspect(new Error("error"), { colors: true });
      "#,
    )
    .unwrap();
}

#[test]
fn console_inspect_error_dims_node_builtin_frames() {
  deno_core::extension!(
    node_module_stub,
    lazy_loaded_esm = ["node:module" = {
      source = r#"export function isBuiltin(name) { return name === "fs"; }"#
    }],
  );
  let mut runtime = console_runtime(vec![node_module_stub::init()]);
  runtime
    .execute_script(
      "test.js",
      r#"
        const console = Deno.core.loadExtScript("ext:deno_web/01_console.js");
        const err = new Error("error");
        err.stack = "Error: error\n" +
          "    at fn (node:fs:10:5)\n" +
          "    at userland (file:///main.ts:1:1)";
        const out = console.inspect(err, { colors: true });
        if (!out.includes("\x1b[90m    at fn (node:fs:10:5)\x1b[39m")) {
          throw new Error(
            "node builtin frame was not dimmed; got: " + JSON.stringify(out),
          );
        }
        if (out.includes("\x1b[90m    at userland")) {
          throw new Error(
            "userland frame should not be dimmed; got: " + JSON.stringify(out),
          );
        }
      "#,
    )
    .unwrap();
}
