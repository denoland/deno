// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;

// TODO(bartlomieju): this is unversioned, and should be fixed to use latest stable?
static DENO_STD_URL: &'static str = "https://deno.land/std/node/";

static SUPPORTED_MODULES: &[&'static str] = &[
  "assert",
  "buffer",
  "child_process",
  "console",
  "constants",
  "crypto",
  "events",
  "fs",
  "module",
  "os",
  "path",
  "perf_hooks",
  "process",
  "querystring",
  "stream",
  "string_decoder",
  "sys",
  "timers",
  "tty",
  "url",
  "util",
];

pub fn get_mapped_node_builtins() -> HashMap<String, String> {
  let mut mappings = HashMap::new();

  for module in SUPPORTED_MODULES {
    let module_url = format!("{}{}", DENO_STD_URL, module);
    mappings.insert(module.to_string(), module_url);
  }

  mappings
}
