// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

pub fn is_builtin_node_module(module_name: &str) -> bool {
  SUPPORTED_BUILTIN_NODE_MODULES
    .iter()
    .any(|m| m.specifier.strip_prefix("node:").unwrap() == module_name)
}

pub struct NodeModulePolyfill {
  /// Name of the module like "assert" or "timers/promises"
  pub specifier: &'static str,
  pub ext_specifier: &'static str,
}

// NOTE(bartlomieju): keep this list in sync with `ext/node/polyfills/01_require.js`
pub static SUPPORTED_BUILTIN_NODE_MODULES: &[NodeModulePolyfill] = &[
  NodeModulePolyfill {
    specifier: "node:assert",
    ext_specifier: "ext:deno_node/assert.ts",
  },
  NodeModulePolyfill {
    specifier: "node:assert/strict",
    ext_specifier: "ext:deno_node/assert/strict.ts",
  },
  NodeModulePolyfill {
    specifier: "node:async_hooks",
    ext_specifier: "ext:deno_node/async_hooks.ts",
  },
  NodeModulePolyfill {
    specifier: "node:buffer",
    ext_specifier: "ext:deno_node/buffer.ts",
  },
  NodeModulePolyfill {
    specifier: "node:child_process",
    ext_specifier: "ext:deno_node/child_process.ts",
  },
  NodeModulePolyfill {
    specifier: "node:cluster",
    ext_specifier: "ext:deno_node/cluster.ts",
  },
  NodeModulePolyfill {
    specifier: "node:console",
    ext_specifier: "ext:deno_node/console.ts",
  },
  NodeModulePolyfill {
    specifier: "node:constants",
    ext_specifier: "ext:deno_node/constants.ts",
  },
  NodeModulePolyfill {
    specifier: "node:crypto",
    ext_specifier: "ext:deno_node/crypto.ts",
  },
  NodeModulePolyfill {
    specifier: "node:dgram",
    ext_specifier: "ext:deno_node/dgram.ts",
  },
  NodeModulePolyfill {
    specifier: "node:diagnostics_channel",
    ext_specifier: "ext:deno_node/diagnostics_channel.ts",
  },
  NodeModulePolyfill {
    specifier: "node:dns",
    ext_specifier: "ext:deno_node/dns.ts",
  },
  NodeModulePolyfill {
    specifier: "node:dns/promises",
    ext_specifier: "ext:deno_node/dns/promises.ts",
  },
  NodeModulePolyfill {
    specifier: "node:domain",
    ext_specifier: "ext:deno_node/domain.ts",
  },
  NodeModulePolyfill {
    specifier: "node:events",
    ext_specifier: "ext:deno_node/events.ts",
  },
  NodeModulePolyfill {
    specifier: "node:fs",
    ext_specifier: "ext:deno_node/fs.ts",
  },
  NodeModulePolyfill {
    specifier: "node:fs/promises",
    ext_specifier: "ext:deno_node/fs/promises.ts",
  },
  NodeModulePolyfill {
    specifier: "node:http",
    ext_specifier: "ext:deno_node/http.ts",
  },
  NodeModulePolyfill {
    specifier: "node:http2",
    ext_specifier: "ext:deno_node/http2.ts",
  },
  NodeModulePolyfill {
    specifier: "node:https",
    ext_specifier: "ext:deno_node/https.ts",
  },
  NodeModulePolyfill {
    specifier: "node:module",
    ext_specifier: "ext:deno_node/01_require.js",
  },
  NodeModulePolyfill {
    specifier: "node:net",
    ext_specifier: "ext:deno_node/net.ts",
  },
  NodeModulePolyfill {
    specifier: "node:os",
    ext_specifier: "ext:deno_node/os.ts",
  },
  NodeModulePolyfill {
    specifier: "node:path",
    ext_specifier: "ext:deno_node/path.ts",
  },
  NodeModulePolyfill {
    specifier: "node:path/posix",
    ext_specifier: "ext:deno_node/path/posix.ts",
  },
  NodeModulePolyfill {
    specifier: "node:path/win32",
    ext_specifier: "ext:deno_node/path/win32.ts",
  },
  NodeModulePolyfill {
    specifier: "node:perf_hooks",
    ext_specifier: "ext:deno_node/perf_hooks.ts",
  },
  NodeModulePolyfill {
    specifier: "node:process",
    ext_specifier: "ext:deno_node/process.ts",
  },
  NodeModulePolyfill {
    specifier: "node:punycode",
    ext_specifier: "ext:deno_node/punycode.ts",
  },
  NodeModulePolyfill {
    specifier: "node:querystring",
    ext_specifier: "ext:deno_node/querystring.ts",
  },
  NodeModulePolyfill {
    specifier: "node:readline",
    ext_specifier: "ext:deno_node/readline.ts",
  },
  NodeModulePolyfill {
    specifier: "node:stream",
    ext_specifier: "ext:deno_node/stream.ts",
  },
  NodeModulePolyfill {
    specifier: "node:stream/consumers",
    ext_specifier: "ext:deno_node/stream/consumers.mjs",
  },
  NodeModulePolyfill {
    specifier: "node:stream/promises",
    ext_specifier: "ext:deno_node/stream/promises.mjs",
  },
  NodeModulePolyfill {
    specifier: "node:stream/web",
    ext_specifier: "ext:deno_node/stream/web.ts",
  },
  NodeModulePolyfill {
    specifier: "node:string_decoder",
    ext_specifier: "ext:deno_node/string_decoder.ts",
  },
  NodeModulePolyfill {
    specifier: "node:sys",
    ext_specifier: "ext:deno_node/sys.ts",
  },
  NodeModulePolyfill {
    specifier: "node:timers",
    ext_specifier: "ext:deno_node/timers.ts",
  },
  NodeModulePolyfill {
    specifier: "node:timers/promises",
    ext_specifier: "ext:deno_node/timers/promises.ts",
  },
  NodeModulePolyfill {
    specifier: "node:tls",
    ext_specifier: "ext:deno_node/tls.ts",
  },
  NodeModulePolyfill {
    specifier: "node:tty",
    ext_specifier: "ext:deno_node/tty.ts",
  },
  NodeModulePolyfill {
    specifier: "node:url",
    ext_specifier: "ext:deno_node/url.ts",
  },
  NodeModulePolyfill {
    specifier: "node:util",
    ext_specifier: "ext:deno_node/util.ts",
  },
  NodeModulePolyfill {
    specifier: "node:util/types",
    ext_specifier: "ext:deno_node/util/types.ts",
  },
  NodeModulePolyfill {
    specifier: "node:v8",
    ext_specifier: "ext:deno_node/v8.ts",
  },
  NodeModulePolyfill {
    specifier: "node:vm",
    ext_specifier: "ext:deno_node/vm.ts",
  },
  NodeModulePolyfill {
    specifier: "node:worker_threads",
    ext_specifier: "ext:deno_node/worker_threads.ts",
  },
  NodeModulePolyfill {
    specifier: "node:zlib",
    ext_specifier: "ext:deno_node/zlib.ts",
  },
];
