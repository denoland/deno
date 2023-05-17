// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;

// TODO(bartlomieju): seems super wasteful to parse the specifier each time
pub fn resolve_builtin_node_module(module_name: &str) -> Result<Url, AnyError> {
  if let Some(module) = find_builtin_node_module(module_name) {
    return Ok(ModuleSpecifier::parse(module.specifier).unwrap());
  }

  Err(generic_error(format!(
    "Unknown built-in \"node:\" module: {module_name}"
  )))
}

fn find_builtin_node_module(module_name: &str) -> Option<&NodeModulePolyfill> {
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
    specifier: "ext:deno_node/assert.ts",
  },
  NodeModulePolyfill {
    name: "assert/strict",
    specifier: "ext:deno_node/assert/strict.ts",
  },
  NodeModulePolyfill {
    name: "async_hooks",
    specifier: "ext:deno_node/async_hooks.ts",
  },
  NodeModulePolyfill {
    name: "buffer",
    specifier: "ext:deno_node/buffer.ts",
  },
  NodeModulePolyfill {
    name: "child_process",
    specifier: "ext:deno_node/child_process.ts",
  },
  NodeModulePolyfill {
    name: "cluster",
    specifier: "ext:deno_node/cluster.ts",
  },
  NodeModulePolyfill {
    name: "console",
    specifier: "ext:deno_node/console.ts",
  },
  NodeModulePolyfill {
    name: "constants",
    specifier: "ext:deno_node/constants.ts",
  },
  NodeModulePolyfill {
    name: "crypto",
    specifier: "ext:deno_node/crypto.ts",
  },
  NodeModulePolyfill {
    name: "dgram",
    specifier: "ext:deno_node/dgram.ts",
  },
  NodeModulePolyfill {
    name: "dns",
    specifier: "ext:deno_node/dns.ts",
  },
  NodeModulePolyfill {
    name: "dns/promises",
    specifier: "ext:deno_node/dns/promises.ts",
  },
  NodeModulePolyfill {
    name: "domain",
    specifier: "ext:deno_node/domain.ts",
  },
  NodeModulePolyfill {
    name: "events",
    specifier: "ext:deno_node/events.ts",
  },
  NodeModulePolyfill {
    name: "fs",
    specifier: "ext:deno_node/fs.ts",
  },
  NodeModulePolyfill {
    name: "fs/promises",
    specifier: "ext:deno_node/fs/promises.ts",
  },
  NodeModulePolyfill {
    name: "http",
    specifier: "ext:deno_node/http.ts",
  },
  NodeModulePolyfill {
    name: "http2",
    specifier: "ext:deno_node/http2.ts",
  },
  NodeModulePolyfill {
    name: "https",
    specifier: "ext:deno_node/https.ts",
  },
  NodeModulePolyfill {
    name: "module",
    specifier: "ext:deno_node/01_require.js",
  },
  NodeModulePolyfill {
    name: "net",
    specifier: "ext:deno_node/net.ts",
  },
  NodeModulePolyfill {
    name: "os",
    specifier: "ext:deno_node/os.ts",
  },
  NodeModulePolyfill {
    name: "path",
    specifier: "ext:deno_node/path.ts",
  },
  NodeModulePolyfill {
    name: "path/posix",
    specifier: "ext:deno_node/path/posix.ts",
  },
  NodeModulePolyfill {
    name: "path/win32",
    specifier: "ext:deno_node/path/win32.ts",
  },
  NodeModulePolyfill {
    name: "perf_hooks",
    specifier: "ext:deno_node/perf_hooks.ts",
  },
  NodeModulePolyfill {
    name: "process",
    specifier: "ext:deno_node/process.ts",
  },
  NodeModulePolyfill {
    name: "punycode",
    specifier: "ext:deno_node/punycode.ts",
  },
  NodeModulePolyfill {
    name: "querystring",
    specifier: "ext:deno_node/querystring.ts",
  },
  NodeModulePolyfill {
    name: "readline",
    specifier: "ext:deno_node/readline.ts",
  },
  NodeModulePolyfill {
    name: "stream",
    specifier: "ext:deno_node/stream.ts",
  },
  NodeModulePolyfill {
    name: "stream/consumers",
    specifier: "ext:deno_node/stream/consumers.mjs",
  },
  NodeModulePolyfill {
    name: "stream/promises",
    specifier: "ext:deno_node/stream/promises.mjs",
  },
  NodeModulePolyfill {
    name: "stream/web",
    specifier: "ext:deno_node/stream/web.ts",
  },
  NodeModulePolyfill {
    name: "string_decoder",
    specifier: "ext:deno_node/string_decoder.ts",
  },
  NodeModulePolyfill {
    name: "sys",
    specifier: "ext:deno_node/sys.ts",
  },
  NodeModulePolyfill {
    name: "timers",
    specifier: "ext:deno_node/timers.ts",
  },
  NodeModulePolyfill {
    name: "timers/promises",
    specifier: "ext:deno_node/timers/promises.ts",
  },
  NodeModulePolyfill {
    name: "tls",
    specifier: "ext:deno_node/tls.ts",
  },
  NodeModulePolyfill {
    name: "tty",
    specifier: "ext:deno_node/tty.ts",
  },
  NodeModulePolyfill {
    name: "url",
    specifier: "ext:deno_node/url.ts",
  },
  NodeModulePolyfill {
    name: "util",
    specifier: "ext:deno_node/util.ts",
  },
  NodeModulePolyfill {
    name: "util/types",
    specifier: "ext:deno_node/util/types.ts",
  },
  NodeModulePolyfill {
    name: "v8",
    specifier: "ext:deno_node/v8.ts",
  },
  NodeModulePolyfill {
    name: "vm",
    specifier: "ext:deno_node/vm.ts",
  },
  NodeModulePolyfill {
    name: "worker_threads",
    specifier: "ext:deno_node/worker_threads.ts",
  },
  NodeModulePolyfill {
    name: "zlib",
    specifier: "ext:deno_node/zlib.ts",
  },
];
