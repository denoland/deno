// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Utilities shared between `build.rs` and the rest of the crate.

use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceTextInfo;
use deno_core::error::AnyError;
use deno_core::extension;
use deno_core::Extension;
use deno_core::ExtensionFileSource;
use deno_core::ExtensionFileSourceCode;
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
    "06_util.js",
    "10_permissions.js",
    "11_workers.js",
    "13_buffer.js",
    "30_os.js",
    "40_fs_events.js",
    "40_http.js",
    "40_process.js",
    "40_signals.js",
    "40_tty.js",
    "41_prompt.js",
    "90_deno_ns.js",
    "98_global_scope.js"
  ],
  customizer = |ext: &mut Extension| {
    #[cfg(not(feature = "exclude_runtime_main_js"))]
    {
      ext.esm_files.to_mut().push(ExtensionFileSource {
        specifier: "ext:runtime_main/js/99_main.js",
        code: ExtensionFileSourceCode::IncludedInBinary(
          include_str!("./js/99_main.js"),
        ),
      });
      ext.esm_entry_point = Some("ext:runtime_main/js/99_main.js");
    }
  }
);

#[allow(dead_code)]
pub fn maybe_transpile_source(
  source: &mut ExtensionFileSource,
) -> Result<(), AnyError> {
  // Always transpile `node:` built-in modules, since they might be TypeScript.
  let media_type = if source.specifier.starts_with("node:") {
    MediaType::TypeScript
  } else {
    MediaType::from_path(Path::new(&source.specifier))
  };

  match media_type {
    MediaType::TypeScript => {}
    MediaType::JavaScript => return Ok(()),
    MediaType::Mjs => return Ok(()),
    _ => panic!(
      "Unsupported media type for snapshotting {media_type:?} for file {}",
      source.specifier
    ),
  }
  let code = source.load()?;

  let parsed = deno_ast::parse_module(ParseParams {
    specifier: source.specifier.to_string(),
    text_info: SourceTextInfo::from_string(code.as_str().to_owned()),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })?;
  let transpiled_source = parsed.transpile(&deno_ast::EmitOptions {
    imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
    inline_source_map: false,
    ..Default::default()
  })?;

  source.code =
    ExtensionFileSourceCode::Computed(transpiled_source.text.into());
  Ok(())
}
