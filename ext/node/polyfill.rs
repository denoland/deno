// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

pub fn is_builtin_node_module(module_name: &str) -> bool {
  SUPPORTED_BUILTIN_NODE_MODULES
    .iter()
    .any(|m| m.name == module_name)
}

pub struct NodeModulePolyfill {
  /// Name of the module like "assert" or "timers/promises"
  pub name: &'static str,
  pub ext_specifier: &'static str,
}

// NOTE(bartlomieju): keep this list in sync with `ext/node/polyfills/01_require.js`
pub static SUPPORTED_BUILTIN_NODE_MODULES: &[NodeModulePolyfill] = &[
  NodeModulePolyfill {
    name: "assert",
    ext_specifier: "ext:deno_node/assert.ts",
  },
  NodeModulePolyfill {
    name: "assert/strict",
    ext_specifier: "ext:deno_node/assert/strict.ts",
  },
  NodeModulePolyfill {
    name: "async_hooks",
    ext_specifier: "ext:deno_node/async_hooks.ts",
  },
  NodeModulePolyfill {
    name: "buffer",
    ext_specifier: "ext:deno_node/buffer.ts",
  },
  NodeModulePolyfill {
    name: "child_process",
    ext_specifier: "ext:deno_node/child_process.ts",
  },
  NodeModulePolyfill {
    name: "cluster",
    ext_specifier: "ext:deno_node/cluster.ts",
  },
  NodeModulePolyfill {
    name: "console",
    ext_specifier: "ext:deno_node/console.ts",
  },
  NodeModulePolyfill {
    name: "constants",
    ext_specifier: "ext:deno_node/constants.ts",
  },
  NodeModulePolyfill {
    name: "crypto",
    ext_specifier: "ext:deno_node/crypto.ts",
  },
  NodeModulePolyfill {
    name: "dgram",
    ext_specifier: "ext:deno_node/dgram.ts",
  },
  NodeModulePolyfill {
    name: "diagnostics_channel",
    ext_specifier: "ext:deno_node/diagnostics_channel.ts",
  },
  NodeModulePolyfill {
    name: "dns",
    ext_specifier: "ext:deno_node/dns.ts",
  },
  NodeModulePolyfill {
    name: "dns/promises",
    ext_specifier: "ext:deno_node/dns/promises.ts",
  },
  NodeModulePolyfill {
    name: "domain",
    ext_specifier: "ext:deno_node/domain.ts",
  },
  NodeModulePolyfill {
    name: "events",
    ext_specifier: "ext:deno_node/events.ts",
  },
  NodeModulePolyfill {
    name: "fs",
    ext_specifier: "ext:deno_node/fs.ts",
  },
  NodeModulePolyfill {
    name: "fs/promises",
    ext_specifier: "ext:deno_node/fs/promises.ts",
  },
  NodeModulePolyfill {
    name: "http",
    ext_specifier: "ext:deno_node/http.ts",
  },
  NodeModulePolyfill {
    name: "http2",
    ext_specifier: "ext:deno_node/http2.ts",
  },
  NodeModulePolyfill {
    name: "https",
    ext_specifier: "ext:deno_node/https.ts",
  },
  NodeModulePolyfill {
    name: "module",
    ext_specifier: "ext:deno_node/01_require.js",
  },
  NodeModulePolyfill {
    name: "net",
    ext_specifier: "ext:deno_node/net.ts",
  },
  NodeModulePolyfill {
    name: "os",
    ext_specifier: "ext:deno_node/os.ts",
  },
  NodeModulePolyfill {
    name: "path",
    ext_specifier: "ext:deno_node/path.ts",
  },
  NodeModulePolyfill {
    name: "path/posix",
    ext_specifier: "ext:deno_node/path/posix.ts",
  },
  NodeModulePolyfill {
    name: "path/win32",
    ext_specifier: "ext:deno_node/path/win32.ts",
  },
  NodeModulePolyfill {
    name: "perf_hooks",
    ext_specifier: "ext:deno_node/perf_hooks.ts",
  },
  NodeModulePolyfill {
    name: "process",
    ext_specifier: "ext:deno_node/process.ts",
  },
  NodeModulePolyfill {
    name: "punycode",
    ext_specifier: "ext:deno_node/punycode.ts",
  },
  NodeModulePolyfill {
    name: "querystring",
    ext_specifier: "ext:deno_node/querystring.ts",
  },
  NodeModulePolyfill {
    name: "readline",
    ext_specifier: "ext:deno_node/readline.ts",
  },
  NodeModulePolyfill {
    name: "stream",
    ext_specifier: "ext:deno_node/stream.ts",
  },
  NodeModulePolyfill {
    name: "stream/consumers",
    ext_specifier: "ext:deno_node/stream/consumers.mjs",
  },
  NodeModulePolyfill {
    name: "stream/promises",
    ext_specifier: "ext:deno_node/stream/promises.mjs",
  },
  NodeModulePolyfill {
    name: "stream/web",
    ext_specifier: "ext:deno_node/stream/web.ts",
  },
  NodeModulePolyfill {
    name: "string_decoder",
    ext_specifier: "ext:deno_node/string_decoder.ts",
  },
  NodeModulePolyfill {
    name: "sys",
    ext_specifier: "ext:deno_node/sys.ts",
  },
  NodeModulePolyfill {
    name: "timers",
    ext_specifier: "ext:deno_node/timers.ts",
  },
  NodeModulePolyfill {
    name: "timers/promises",
    ext_specifier: "ext:deno_node/timers/promises.ts",
  },
  NodeModulePolyfill {
    name: "tls",
    ext_specifier: "ext:deno_node/tls.ts",
  },
  NodeModulePolyfill {
    name: "tty",
    ext_specifier: "ext:deno_node/tty.ts",
  },
  NodeModulePolyfill {
    name: "url",
    ext_specifier: "ext:deno_node/url.ts",
  },
  NodeModulePolyfill {
    name: "util",
    ext_specifier: "ext:deno_node/util.ts",
  },
  NodeModulePolyfill {
    name: "util/types",
    ext_specifier: "ext:deno_node/util/types.ts",
  },
  NodeModulePolyfill {
    name: "v8",
    ext_specifier: "ext:deno_node/v8.ts",
  },
  NodeModulePolyfill {
    name: "vm",
    ext_specifier: "ext:deno_node/vm.ts",
  },
  NodeModulePolyfill {
    name: "worker_threads",
    ext_specifier: "ext:deno_node/worker_threads.ts",
  },
  NodeModulePolyfill {
    name: "zlib",
    ext_specifier: "ext:deno_node/zlib.ts",
  },
];
