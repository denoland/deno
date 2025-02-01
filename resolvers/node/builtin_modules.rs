// Copyright 2018-2025 the Deno authors. MIT license.

pub trait IsBuiltInNodeModuleChecker: std::fmt::Debug {
  /// e.g. `is_builtin_node_module("assert")`
  fn is_builtin_node_module(&self, module_name: &str) -> bool;
}

/// An implementation of IsBuiltInNodeModuleChecker that uses
/// the list of built-in node_modules that are supported by Deno
/// in the `deno_node` crate (ext/node).
#[derive(Debug)]
pub struct DenoIsBuiltInNodeModuleChecker;

impl IsBuiltInNodeModuleChecker for DenoIsBuiltInNodeModuleChecker {
  #[inline(always)]
  fn is_builtin_node_module(&self, module_name: &str) -> bool {
    DENO_SUPPORTED_BUILTIN_NODE_MODULES
      .binary_search(&module_name)
      .is_ok()
  }
}

/// Collection of built-in node_modules supported by Deno.
pub static DENO_SUPPORTED_BUILTIN_NODE_MODULES: &[&str] = &[
  // NOTE(bartlomieju): keep this list in sync with `ext/node/polyfills/01_require.js`
  "_http_agent",
  "_http_common",
  "_http_outgoing",
  "_http_server",
  "_stream_duplex",
  "_stream_passthrough",
  "_stream_readable",
  "_stream_transform",
  "_stream_writable",
  "_tls_common",
  "_tls_wrap",
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
  "inspector",
  "inspector/promises",
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
  "readline/promises",
  "repl",
  "sqlite",
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
  "wasi",
  "worker_threads",
  "zlib",
];

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_builtins_are_sorted() {
    let mut builtins_list = DENO_SUPPORTED_BUILTIN_NODE_MODULES.to_vec();
    builtins_list.sort();
    assert_eq!(DENO_SUPPORTED_BUILTIN_NODE_MODULES, builtins_list);
  }
}
