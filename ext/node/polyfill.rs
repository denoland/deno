// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::ModuleSpecifier;

/// e.g. `is_builtin_node_module("assert")`
pub fn is_builtin_node_module(module_name: &str) -> bool {
  SUPPORTED_BUILTIN_NODE_MODULES
    .iter()
    .any(|m| *m == module_name)
}

/// Ex. returns `fs` for `node:fs`
pub fn get_module_name_from_builtin_node_module_specifier(
  specifier: &ModuleSpecifier,
) -> Option<&str> {
  if specifier.scheme() != "node" {
    return None;
  }

  let (_, specifier) = specifier.as_str().split_once(':')?;
  Some(specifier)
}

macro_rules! generate_builtin_node_module_lists {
  ($( $module_name:literal ,)+) => {
    pub static SUPPORTED_BUILTIN_NODE_MODULES: &[&str] = &[
      $(
        $module_name,
      )+
    ];

    pub static SUPPORTED_BUILTIN_NODE_MODULES_WITH_PREFIX: &[&str] = &[
      $(
        concat!("node:", $module_name),
      )+
    ];
  };
}

// NOTE(bartlomieju): keep this list in sync with `ext/node/polyfills/01_require.js`
generate_builtin_node_module_lists! {
  "assert",
  "assert/strict",
  "async_hooks",
  "buffer",
  "child_process",
  "cluster",
  "console",
  "constants",
  "crypto",
  "dgram",
  "diagnostics_channel",
  "dns",
  "dns/promises",
  "domain",
  "events",
  "fs",
  "fs/promises",
  "http",
  "http2",
  "https",
  "module",
  "net",
  "os",
  "path",
  "path/posix",
  "path/win32",
  "perf_hooks",
  "process",
  "punycode",
  "querystring",
  "repl",
  "readline",
  "readline/promises",
  "stream",
  "stream/consumers",
  "stream/promises",
  "stream/web",
  "string_decoder",
  "sys",
  "test",
  "timers",
  "timers/promises",
  "tls",
  "tty",
  "url",
  "util",
  "util/types",
  "v8",
  "vm",
  "worker_threads",
  "zlib",
}
