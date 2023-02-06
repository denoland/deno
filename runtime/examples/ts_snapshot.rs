// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
//! This example shows how to use swc to transpile TypeScript and JSX/TSX
//! modules.
//!
//! It will only transpile, not typecheck (like Deno's `--no-check` flag).

use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;

use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceTextInfo;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Error;
use deno_core::futures::FutureExt;
use deno_core::include_js_files;
use deno_core::resolve_import;
use deno_core::resolve_path;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSourceFuture;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::ResolutionKind;
use deno_core::RuntimeOptions;
use deno_core::Snapshot;

fn snapshot_load_cb(name: &'static str, code: &'static str) -> String {
  let media_type = MediaType::from(Path::new(name));
  let (_module_type, should_transpile) = match MediaType::from(Path::new(name))
  {
    MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
      (ModuleType::JavaScript, false)
    }
    MediaType::Jsx => (ModuleType::JavaScript, true),
    MediaType::TypeScript
    | MediaType::Mts
    | MediaType::Cts
    | MediaType::Dts
    | MediaType::Dmts
    | MediaType::Dcts
    | MediaType::Tsx => (ModuleType::JavaScript, true),
    _ => panic!("Unknown extension"),
  };

  if should_transpile {
    let parsed = deno_ast::parse_module(ParseParams {
      specifier: name.to_string(),
      text_info: SourceTextInfo::from_string(code.to_string()),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .unwrap();
    parsed.transpile(&Default::default()).unwrap().text
  } else {
    code.to_string()
  }
}

fn main() -> Result<(), Error> {
  let ext = Extension::builder("ts")
    .esm(include_js_files!(
      prefix "internal:ts/",
      "01_ts.ts"
    ))
    .build();

  let js_runtime = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    extensions_with_js: vec![ext],
    snapshot_load_cb: Some(Box::new(snapshot_load_cb)),
    ..Default::default()
  });

  let snapshot = js_runtime.snapshot();

  let mut js_runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(Snapshot::JustCreated(snapshot)),
    ..Default::default()
  });

  js_runtime
    .execute_script("", "Deno.core.print(globalThis.foo)")
    .unwrap();

  Ok(())
}
