// Copyright 2018-2025 the Deno authors. MIT license.
// Utilities shared between `build.rs` and the rest of the crate.

use std::path::Path;

use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceMapOption;
use deno_core::extension;
use deno_core::Extension;
use deno_core::ModuleCodeString;
use deno_core::ModuleName;
use deno_core::SourceMapData;
use deno_error::JsErrorBox;

extension!(runtime,
  deps = [
    deno_webidl,
    deno_console,
    deno_url,
    deno_tls,
    deno_web,
    deno_fetch,
    deno_cache,
    deno_websocket,
    deno_webstorage,
    deno_crypto,
    deno_broadcast_channel,
    deno_node,
    deno_ffi,
    deno_net,
    deno_napi,
    deno_http,
    deno_io,
    deno_fs
  ],
  esm_entry_point = "ext:runtime/90_deno_ns.js",
  esm = [
    dir "js",
    "01_errors.js",
    "01_version.ts",
    "06_util.js",
    "10_permissions.js",
    "11_workers.js",
    "30_os.js",
    "40_fs_events.js",
    "40_process.js",
    "40_signals.js",
    "40_tty.js",
    "41_prompt.js",
    "90_deno_ns.js",
    "98_global_scope_shared.js",
    "98_global_scope_window.js",
    "98_global_scope_worker.js"
  ],
  customizer = |ext: &mut Extension| {
    #[cfg(not(feature = "exclude_runtime_main_js"))]
    {
      use deno_core::ascii_str_include;
      use deno_core::ExtensionFileSource;
      ext.esm_files.to_mut().push(ExtensionFileSource::new("ext:runtime_main/js/99_main.js", ascii_str_include!("./js/99_main.js")));
      ext.esm_entry_point = Some("ext:runtime_main/js/99_main.js");
    }
  }
);

deno_error::js_error_wrapper!(
  deno_ast::ParseDiagnostic,
  JsParseDiagnostic,
  "Error"
);
deno_error::js_error_wrapper!(
  deno_ast::TranspileError,
  JsTranspileError,
  "Error"
);

pub fn maybe_transpile_source(
  name: ModuleName,
  source: ModuleCodeString,
) -> Result<(ModuleCodeString, Option<SourceMapData>), JsErrorBox> {
  // Always transpile `node:` built-in modules, since they might be TypeScript.
  let media_type = if name.starts_with("node:") {
    MediaType::TypeScript
  } else {
    MediaType::from_path(Path::new(&name))
  };

  match media_type {
    MediaType::TypeScript => {}
    MediaType::JavaScript => return Ok((source, None)),
    MediaType::Mjs => return Ok((source, None)),
    _ => panic!(
      "Unsupported media type for snapshotting {media_type:?} for file {}",
      name
    ),
  }

  let parsed = deno_ast::parse_module(ParseParams {
    specifier: deno_core::url::Url::parse(&name).unwrap(),
    text: source.into(),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })
  .map_err(|e| JsErrorBox::from_err(JsParseDiagnostic(e)))?;
  let transpiled_source = parsed
    .transpile(
      &deno_ast::TranspileOptions {
        imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
        ..Default::default()
      },
      &deno_ast::TranspileModuleOptions::default(),
      &deno_ast::EmitOptions {
        source_map: if cfg!(debug_assertions) {
          SourceMapOption::Separate
        } else {
          SourceMapOption::None
        },
        ..Default::default()
      },
    )
    .map_err(|e| JsErrorBox::from_err(JsTranspileError(e)))?
    .into_source();

  let maybe_source_map: Option<SourceMapData> = transpiled_source
    .source_map
    .map(|sm| sm.into_bytes().into());
  let source_text = transpiled_source.text;
  Ok((source_text.into(), maybe_source_map))
}
