// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Path;

use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceTextInfo;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::Extension;
use deno_core::ExtensionFileSource;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use deno_core::Snapshot;

fn transpile_ts_for_snapshotting(
  file_source: &ExtensionFileSource,
) -> Result<String, AnyError> {
  let media_type = MediaType::from(Path::new(&file_source.specifier));

  let should_transpile = match media_type {
    MediaType::JavaScript => false,
    MediaType::TypeScript => true,
    _ => panic!("Unsupported media type for snapshotting {media_type:?}"),
  };

  if !should_transpile {
    return Ok(file_source.code.to_string());
  }

  let parsed = deno_ast::parse_module(ParseParams {
    specifier: file_source.specifier.to_string(),
    text_info: SourceTextInfo::from_string(file_source.code.to_string()),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })?;
  let transpiled_source = parsed.transpile(&Default::default())?;
  Ok(transpiled_source.text)
}

fn main() -> Result<(), AnyError> {
  let ext = Extension::builder("ts_snapshot")
    .esm(include_js_files!("01_ts.ts",))
    .build();

  let js_runtime = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    extensions_with_js: vec![ext],
    snapshot_load_cb: Some(Box::new(transpile_ts_for_snapshotting)),
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
