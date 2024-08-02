// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Utilities shared between `build.rs` and the rest of the crate.

use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceMapOption;
use deno_core::error::AnyError;
use deno_core::extension;
use deno_core::Extension;
use deno_core::ModuleCodeString;
use deno_core::ModuleName;
use deno_core::SourceMapData;
use std::path::Path;

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
    "01_async_context.js",
    "06_util.js",
    "10_permissions.js",
    "11_workers.js",
    "13_buffer.js",
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

pub fn maybe_transpile_source(
  name: ModuleName,
  source: ModuleCodeString,
) -> Result<(ModuleCodeString, Option<SourceMapData>), AnyError> {
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
  })?;
  let transpiled_source = parsed
    .transpile(
      &deno_ast::TranspileOptions {
        imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
        ..Default::default()
      },
      &deno_ast::EmitOptions {
        source_map: if cfg!(debug_assertions) {
          SourceMapOption::Separate
        } else {
          SourceMapOption::None
        },
        ..Default::default()
      },
    )?
    .into_source();

  let maybe_source_map: Option<SourceMapData> =
    transpiled_source.source_map.map(|sm| sm.into());
  let source_text = String::from_utf8(transpiled_source.source)?;

  Ok((source_text.into(), maybe_source_map))
}
