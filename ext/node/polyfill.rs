// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

pub fn find_builtin_node_module(
  module_name: &str,
) -> Option<&NodeModulePolyfill> {
  SUPPORTED_BUILTIN_NODE_MODULES
    .iter()
    .find(|m| m.name == module_name)
}

pub fn is_builtin_node_module(module_name: &str) -> bool {
  find_builtin_node_module(module_name).is_some()
}

pub struct NodeModulePolyfill {
  /// Name of the module like "assert" or "timers/promises"
  pub name: &'static str,
  pub specifier: &'static str,
}

pub static SUPPORTED_BUILTIN_NODE_MODULES: &[NodeModulePolyfill] = &[
  NodeModulePolyfill {
    name: "assert",
    specifier: "internal:deno_node/assert.ts",
  },
  NodeModulePolyfill {
    name: "assert/strict",
    specifier: "internal:deno_node/assert/strict.ts",
  },
  NodeModulePolyfill {
    name: "async_hooks",
    specifier: "internal:deno_node/async_hooks.ts",
  },
  NodeModulePolyfill {
    name: "buffer",
    specifier: "internal:deno_node/buffer.ts",
  },
  NodeModulePolyfill {
    name: "child_process",
    specifier: "internal:deno_node/child_process.ts",
  },
  NodeModulePolyfill {
    name: "cluster",
    specifier: "internal:deno_node/cluster.ts",
  },
  NodeModulePolyfill {
    name: "console",
    specifier: "internal:deno_node/console.ts",
  },
  NodeModulePolyfill {
    name: "constants",
    specifier: "internal:deno_node/constants.ts",
  },
  NodeModulePolyfill {
    name: "crypto",
    specifier: "internal:deno_node/crypto.ts",
  },
  NodeModulePolyfill {
    name: "dgram",
    specifier: "internal:deno_node/dgram.ts",
  },
  NodeModulePolyfill {
    name: "dns",
    specifier: "internal:deno_node/dns.ts",
  },
  NodeModulePolyfill {
    name: "dns/promises",
    specifier: "internal:deno_node/dns/promises.ts",
  },
  NodeModulePolyfill {
    name: "domain",
    specifier: "internal:deno_node/domain.ts",
  },
  NodeModulePolyfill {
    name: "events",
    specifier: "internal:deno_node/events.ts",
  },
  NodeModulePolyfill {
    name: "fs",
    specifier: "internal:deno_node/fs.ts",
  },
  NodeModulePolyfill {
    name: "fs/promises",
    specifier: "internal:deno_node/fs/promises.ts",
  },
  NodeModulePolyfill {
    name: "http",
    specifier: "internal:deno_node/http.ts",
  },
  NodeModulePolyfill {
    name: "https",
    specifier: "internal:deno_node/https.ts",
  },
  NodeModulePolyfill {
    name: "module",
    specifier: "internal:deno_node_loading/module_es_shim.js",
  },
  NodeModulePolyfill {
    name: "net",
    specifier: "internal:deno_node/net.ts",
  },
  NodeModulePolyfill {
    name: "os",
    specifier: "internal:deno_node/os.ts",
  },
  NodeModulePolyfill {
    name: "path",
    specifier: "internal:deno_node/path.ts",
  },
  NodeModulePolyfill {
    name: "path/posix",
    specifier: "internal:deno_node/path/posix.ts",
  },
  NodeModulePolyfill {
    name: "path/win32",
    specifier: "internal:deno_node/path/win32.ts",
  },
  NodeModulePolyfill {
    name: "perf_hooks",
    specifier: "internal:deno_node/perf_hooks.ts",
  },
  NodeModulePolyfill {
    name: "process",
    specifier: "internal:deno_node/process.ts",
  },
  NodeModulePolyfill {
    name: "querystring",
    specifier: "internal:deno_node/querystring.ts",
  },
  NodeModulePolyfill {
    name: "readline",
    specifier: "internal:deno_node/readline.ts",
  },
  NodeModulePolyfill {
    name: "stream",
    specifier: "internal:deno_node/stream.ts",
  },
  NodeModulePolyfill {
    name: "stream/consumers",
    specifier: "internal:deno_node/stream/consumers.mjs",
  },
  NodeModulePolyfill {
    name: "stream/promises",
    specifier: "internal:deno_node/stream/promises.mjs",
  },
  NodeModulePolyfill {
    name: "stream/web",
    specifier: "internal:deno_node/stream/web.ts",
  },
  NodeModulePolyfill {
    name: "string_decoder",
    specifier: "internal:deno_node/string_decoder.ts",
  },
  NodeModulePolyfill {
    name: "sys",
    specifier: "internal:deno_node/sys.ts",
  },
  NodeModulePolyfill {
    name: "timers",
    specifier: "internal:deno_node/timers.ts",
  },
  NodeModulePolyfill {
    name: "timers/promises",
    specifier: "internal:deno_node/timers/promises.ts",
  },
  NodeModulePolyfill {
    name: "tls",
    specifier: "internal:deno_node/tls.ts",
  },
  NodeModulePolyfill {
    name: "tty",
    specifier: "internal:deno_node/tty.ts",
  },
  NodeModulePolyfill {
    name: "url",
    specifier: "internal:deno_node/url.ts",
  },
  NodeModulePolyfill {
    name: "util",
    specifier: "internal:deno_node/util.ts",
  },
  NodeModulePolyfill {
    name: "util/types",
    specifier: "internal:deno_node/util/types.ts",
  },
  NodeModulePolyfill {
    name: "v8",
    specifier: "internal:deno_node/v8.ts",
  },
  NodeModulePolyfill {
    name: "vm",
    specifier: "internal:deno_node/vm.ts",
  },
  NodeModulePolyfill {
    name: "worker_threads",
    specifier: "internal:deno_node/worker_threads.ts",
  },
  NodeModulePolyfill {
    name: "zlib",
    specifier: "internal:deno_node/zlib.ts",
  },
];
