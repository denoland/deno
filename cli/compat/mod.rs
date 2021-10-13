// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

mod errors;
mod esm_resolver;

use deno_core::url::Url;

pub use esm_resolver::NodeEsmResolver;

// TODO(bartlomieju): this needs to be bumped manually for
// each release, a better mechanism is preferable, but it's a quick and dirty
// solution to avoid printing `X-Deno-Warning` headers when the compat layer is
// downloaded
static STD_URL: &str = "https://deno.land/std@0.111.0/";
static GLOBAL_MODULE: &str = "global.ts";

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

lazy_static::lazy_static! {
  static ref GLOBAL_URL_STR: String = format!("{}node/{}", STD_URL, GLOBAL_MODULE);
  pub(crate) static ref GLOBAL_URL: Url = Url::parse(&GLOBAL_URL_STR).unwrap();
  static ref COMPAT_IMPORT_URL: Url = Url::parse("flags:compat").unwrap();
}

/// Provide imports into a module graph when the compat flag is true.
pub(crate) fn get_node_imports() -> Vec<(Url, Vec<String>)> {
  vec![(COMPAT_IMPORT_URL.clone(), vec![GLOBAL_URL_STR.clone()])]
}

pub(crate) fn try_resolve_builtin_module(specifier: &str) -> Option<Url> {
  if SUPPORTED_MODULES.contains(&specifier) {
    let module_url = format!("{}node/{}.ts", crate::compat::STD_URL, specifier);
    Some(Url::parse(&module_url).unwrap())
  } else {
    None
  }
}
