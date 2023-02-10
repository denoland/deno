// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

pub fn find_builtin_node_module(
  specifier: &str,
) -> Option<&NodeModulePolyfill> {
  SUPPORTED_BUILTIN_NODE_MODULES
    .iter()
    .find(|m| m.name == specifier)
}

pub fn is_builtin_node_module(specifier: &str) -> bool {
  find_builtin_node_module(specifier).is_some()
}

pub struct NodeModulePolyfill {
  /// Name of the module like "assert" or "timers/promises"
  pub name: &'static str,

  /// Specifier relative to the root of `deno_std` repo, like "node/assert.ts"
  pub specifier: &'static str,
}

pub static SUPPORTED_BUILTIN_NODE_MODULES: &[NodeModulePolyfill] = &[
  NodeModulePolyfill {
    name: "assert",
    specifier: "node/assert.ts",
  },
  NodeModulePolyfill {
    name: "assert/strict",
    specifier: "node/assert/strict.ts",
  },
  NodeModulePolyfill {
    name: "async_hooks",
    specifier: "node/async_hooks.ts",
  },
  NodeModulePolyfill {
    name: "buffer",
    specifier: "node/buffer.ts",
  },
  NodeModulePolyfill {
    name: "child_process",
    specifier: "node/child_process.ts",
  },
  NodeModulePolyfill {
    name: "cluster",
    specifier: "node/cluster.ts",
  },
  NodeModulePolyfill {
    name: "console",
    specifier: "node/console.ts",
  },
  NodeModulePolyfill {
    name: "constants",
    specifier: "node/constants.ts",
  },
  NodeModulePolyfill {
    name: "crypto",
    specifier: "node/crypto.ts",
  },
  NodeModulePolyfill {
    name: "dgram",
    specifier: "node/dgram.ts",
  },
  NodeModulePolyfill {
    name: "dns",
    specifier: "node/dns.ts",
  },
  NodeModulePolyfill {
    name: "dns/promises",
    specifier: "node/dns/promises.ts",
  },
  NodeModulePolyfill {
    name: "domain",
    specifier: "node/domain.ts",
  },
  NodeModulePolyfill {
    name: "events",
    specifier: "node/events.ts",
  },
  NodeModulePolyfill {
    name: "fs",
    specifier: "node/fs.ts",
  },
  NodeModulePolyfill {
    name: "fs/promises",
    specifier: "node/fs/promises.ts",
  },
  NodeModulePolyfill {
    name: "http",
    specifier: "node/http.ts",
  },
  NodeModulePolyfill {
    name: "https",
    specifier: "node/https.ts",
  },
  NodeModulePolyfill {
    name: "module",
    // NOTE(bartlomieju): `module` is special, because we don't want to use
    // `deno_std/node/module.ts`, but instead use a special shim that we
    // provide in `ext/node`.
    specifier: "[USE `deno_node::MODULE_ES_SHIM` to get this module]",
  },
  NodeModulePolyfill {
    name: "net",
    specifier: "node/net.ts",
  },
  NodeModulePolyfill {
    name: "os",
    specifier: "node/os.ts",
  },
  NodeModulePolyfill {
    name: "path",
    specifier: "node/path.ts",
  },
  NodeModulePolyfill {
    name: "path/posix",
    specifier: "node/path/posix.ts",
  },
  NodeModulePolyfill {
    name: "path/win32",
    specifier: "node/path/win32.ts",
  },
  NodeModulePolyfill {
    name: "perf_hooks",
    specifier: "node/perf_hooks.ts",
  },
  NodeModulePolyfill {
    name: "process",
    specifier: "node/process.ts",
  },
  NodeModulePolyfill {
    name: "querystring",
    specifier: "node/querystring.ts",
  },
  NodeModulePolyfill {
    name: "readline",
    specifier: "node/readline.ts",
  },
  NodeModulePolyfill {
    name: "stream",
    specifier: "node/stream.ts",
  },
  NodeModulePolyfill {
    name: "stream/consumers",
    specifier: "node/stream/consumers.mjs",
  },
  NodeModulePolyfill {
    name: "stream/promises",
    specifier: "node/stream/promises.mjs",
  },
  NodeModulePolyfill {
    name: "stream/web",
    specifier: "node/stream/web.ts",
  },
  NodeModulePolyfill {
    name: "string_decoder",
    specifier: "node/string_decoder.ts",
  },
  NodeModulePolyfill {
    name: "sys",
    specifier: "node/sys.ts",
  },
  NodeModulePolyfill {
    name: "timers",
    specifier: "node/timers.ts",
  },
  NodeModulePolyfill {
    name: "timers/promises",
    specifier: "node/timers/promises.ts",
  },
  NodeModulePolyfill {
    name: "tls",
    specifier: "node/tls.ts",
  },
  NodeModulePolyfill {
    name: "tty",
    specifier: "node/tty.ts",
  },
  NodeModulePolyfill {
    name: "url",
    specifier: "node/url.ts",
  },
  NodeModulePolyfill {
    name: "util",
    specifier: "node/util.ts",
  },
  NodeModulePolyfill {
    name: "util/types",
    specifier: "node/util/types.ts",
  },
  NodeModulePolyfill {
    name: "v8",
    specifier: "node/v8.ts",
  },
  NodeModulePolyfill {
    name: "vm",
    specifier: "node/vm.ts",
  },
  NodeModulePolyfill {
    name: "worker_threads",
    specifier: "node/worker_threads.ts",
  },
  NodeModulePolyfill {
    name: "zlib",
    specifier: "node/zlib.ts",
  },
];
