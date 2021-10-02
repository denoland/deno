// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;

// TODO(bartlomieju): this is unversioned, and should be fixed to use latest stable?
static DENO_STD_URL: &str = "https://deno.land/std/node/";

static SUPPORTED_MODULES: &[&str] = &[
  "assert",
  "assert/strict",
  "buffer",
  "child_process",
  "console",
  "constants",
  "crypto",
  "events",
  "fs",
  "fs/promises",
  "module",
  "os",
  "path",
  "path/posix",
  "path/win32",
  "perf_hooks",
  "process",
  "querystring",
  "stream",
  "stream/promises",
  "stream/web",
  "string_decoder",
  "sys",
  "timers",
  "timers/promises",
  "tty",
  "url",
  "util",
  "util/types",
];

pub fn get_mapped_node_builtins() -> HashMap<String, String> {
  let mut mappings = HashMap::new();

  for module in SUPPORTED_MODULES {
    let module_url = format!("{}{}.ts", DENO_STD_URL, module);
    mappings.insert(module.to_string(), module_url);
  }

  mappings
}
