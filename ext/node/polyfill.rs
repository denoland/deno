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

pub enum NodeModulePolyfillSpecifier {
  /// An internal module specifier, like "internal:deno_node/assert.ts". The
  /// module must be either embedded in the binary or snapshotted.
  Embedded(&'static str),

  /// Specifier relative to the root of `deno_std` repo, like "node/assert.ts"
  StdNode(&'static str),
}

pub struct NodeModulePolyfill {
  /// Name of the module like "assert" or "timers/promises"
  pub name: &'static str,
  pub specifier: NodeModulePolyfillSpecifier,
}

pub static SUPPORTED_BUILTIN_NODE_MODULES: &[NodeModulePolyfill] = &[
  NodeModulePolyfill {
    name: "assert",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/assert.ts"),
  },
  NodeModulePolyfill {
    name: "assert/strict",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/assert/strict.ts"),
  },
  NodeModulePolyfill {
    name: "async_hooks",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/async_hooks.ts"),
  },
  NodeModulePolyfill {
    name: "buffer",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/buffer.ts"),
  },
  NodeModulePolyfill {
    name: "child_process",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/child_process.ts"),
  },
  NodeModulePolyfill {
    name: "cluster",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/cluster.ts"),
  },
  NodeModulePolyfill {
    name: "console",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/console.ts"),
  },
  NodeModulePolyfill {
    name: "constants",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/constants.ts"),
  },
  NodeModulePolyfill {
    name: "crypto",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/crypto.ts"),
  },
  NodeModulePolyfill {
    name: "dgram",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/dgram.ts"),
  },
  NodeModulePolyfill {
    name: "dns",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/dns.ts"),
  },
  NodeModulePolyfill {
    name: "dns/promises",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/dns/promises.ts"),
  },
  NodeModulePolyfill {
    name: "domain",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/domain.ts"),
  },
  NodeModulePolyfill {
    name: "events",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/events.ts"),
  },
  NodeModulePolyfill {
    name: "fs",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/fs.ts"),
  },
  NodeModulePolyfill {
    name: "fs/promises",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/fs/promises.ts"),
  },
  NodeModulePolyfill {
    name: "http",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/http.ts"),
  },
  NodeModulePolyfill {
    name: "https",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/https.ts"),
  },
  NodeModulePolyfill {
    name: "module",
    specifier: NodeModulePolyfillSpecifier::Embedded(
      "internal:deno_node/module_es_shim.js",
    ),
  },
  NodeModulePolyfill {
    name: "net",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/net.ts"),
  },
  NodeModulePolyfill {
    name: "os",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/os.ts"),
  },
  NodeModulePolyfill {
    name: "path",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/path.ts"),
  },
  NodeModulePolyfill {
    name: "path/posix",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/path/posix.ts"),
  },
  NodeModulePolyfill {
    name: "path/win32",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/path/win32.ts"),
  },
  NodeModulePolyfill {
    name: "perf_hooks",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/perf_hooks.ts"),
  },
  NodeModulePolyfill {
    name: "process",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/process.ts"),
  },
  NodeModulePolyfill {
    name: "querystring",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/querystring.ts"),
  },
  NodeModulePolyfill {
    name: "readline",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/readline.ts"),
  },
  NodeModulePolyfill {
    name: "stream",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/stream.ts"),
  },
  NodeModulePolyfill {
    name: "stream/consumers",
    specifier: NodeModulePolyfillSpecifier::StdNode(
      "node/stream/consumers.mjs",
    ),
  },
  NodeModulePolyfill {
    name: "stream/promises",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/stream/promises.mjs"),
  },
  NodeModulePolyfill {
    name: "stream/web",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/stream/web.ts"),
  },
  NodeModulePolyfill {
    name: "string_decoder",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/string_decoder.ts"),
  },
  NodeModulePolyfill {
    name: "sys",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/sys.ts"),
  },
  NodeModulePolyfill {
    name: "timers",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/timers.ts"),
  },
  NodeModulePolyfill {
    name: "timers/promises",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/timers/promises.ts"),
  },
  NodeModulePolyfill {
    name: "tls",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/tls.ts"),
  },
  NodeModulePolyfill {
    name: "tty",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/tty.ts"),
  },
  NodeModulePolyfill {
    name: "url",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/url.ts"),
  },
  NodeModulePolyfill {
    name: "util",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/util.ts"),
  },
  NodeModulePolyfill {
    name: "util/types",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/util/types.ts"),
  },
  NodeModulePolyfill {
    name: "v8",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/v8.ts"),
  },
  NodeModulePolyfill {
    name: "vm",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/vm.ts"),
  },
  NodeModulePolyfill {
    name: "worker_threads",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/worker_threads.ts"),
  },
  NodeModulePolyfill {
    name: "zlib",
    specifier: NodeModulePolyfillSpecifier::StdNode("node/zlib.ts"),
  },
];
