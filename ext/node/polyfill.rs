// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/// e.g. `is_builtin_node_module("assert")`
pub fn is_builtin_node_module(module_name: &str) -> bool {
  SUPPORTED_BUILTIN_NODE_MODULES
    .iter()
    .any(|m| *m == module_name)
}

// NOTE(bartlomieju): keep this list in sync with `ext/node/polyfills/01_require.js`
pub static SUPPORTED_BUILTIN_NODE_MODULES: &[&str] = &[
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
  "readline",
  "stream",
  "stream/consumers",
  "stream/promises",
  "stream/web",
  "string_decoder",
  "sys",
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
];
