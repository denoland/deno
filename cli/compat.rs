// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;

static SUPPORTED_MODULES: &[&str] = &[
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
  "dns",
  "domain",
  "events",
  "fs",
  "fs/promises",
  "http",
  "https",
  "module",
  "net",
  "os",
  "path",
  "path/posix",
  "path/win32",
  "perf_hooks",
  "process",
  "querystring",
  "readline",
  "stream",
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
  "zlib",
];

pub fn get_mapped_node_builtins() -> HashMap<String, String> {
  let mut mappings = HashMap::new();

  for module in SUPPORTED_MODULES {
    // TODO(bartlomieju): this is unversioned, and should be fixed to use latest stable?
    let module_url = format!("https://deno.land/std/node/{}.ts", module);
    mappings.insert(module.to_string(), module_url);
  }

  mappings
}
