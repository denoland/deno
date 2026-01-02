// Copyright 2018-2025 the Deno authors. MIT license.
// Utilities shared between `build.rs` and the rest of the crate.

use deno_core::Extension;
use deno_core::extension;

extension!(runtime,
  deps = [
    deno_webidl,
    deno_tls,
    deno_web,
    deno_fetch,
    deno_cache,
    deno_websocket,
    deno_webstorage,
    deno_crypto,
    deno_node,
    deno_ffi,
    deno_net,
    deno_napi,
    deno_http,
    deno_io,
    deno_fs,
    deno_bundle_runtime
  ],
  esm_entry_point = "ext:runtime/90_deno_ns.js",
  esm = [
    dir "js",
    "01_errors.js",
    "01_version.ts",
    "06_util.js",
    "10_permissions.js",
    "11_workers.js",
    "40_fs_events.js",
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
      ext.esm_files.to_mut().push(ExtensionFileSource::new("ext:deno_features/flags.js", deno_features::JS_SOURCE));
      ext.esm_files.to_mut().push(ExtensionFileSource::new("ext:runtime_main/js/99_main.js", ascii_str_include!("./js/99_main.js")));
      ext.esm_entry_point = Some("ext:runtime_main/js/99_main.js");
    }
  }
);
