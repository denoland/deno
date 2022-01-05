// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

mod errors;
mod esm_resolver;

use deno_core::error::AnyError;
use deno_core::located_script_name;
use deno_core::url::Url;
use deno_core::JsRuntime;
use once_cell::sync::Lazy;

pub use esm_resolver::check_if_should_use_esm_loader;
pub(crate) use esm_resolver::NodeEsmResolver;

// TODO(bartlomieju): this needs to be bumped manually for
// each release, a better mechanism is preferable, but it's a quick and dirty
// solution to avoid printing `X-Deno-Warning` headers when the compat layer is
// downloaded
static STD_URL_STR: &str = "https://deno.land/std@0.120.0/";

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

static NODE_COMPAT_URL: Lazy<String> = Lazy::new(|| {
  std::env::var("DENO_NODE_COMPAT_URL")
    .map(String::into)
    .ok()
    .unwrap_or_else(|| STD_URL_STR.to_string())
});

static GLOBAL_URL_STR: Lazy<String> =
  Lazy::new(|| format!("{}node/global.ts", NODE_COMPAT_URL.as_str()));

pub(crate) static GLOBAL_URL: Lazy<Url> =
  Lazy::new(|| Url::parse(&GLOBAL_URL_STR).unwrap());

static MODULE_URL_STR: Lazy<String> =
  Lazy::new(|| format!("{}node/module.ts", NODE_COMPAT_URL.as_str()));

pub(crate) static MODULE_URL: Lazy<Url> =
  Lazy::new(|| Url::parse(&MODULE_URL_STR).unwrap());

static COMPAT_IMPORT_URL: Lazy<Url> =
  Lazy::new(|| Url::parse("flags:compat").unwrap());

/// Provide imports into a module graph when the compat flag is true.
pub(crate) fn get_node_imports() -> Vec<(Url, Vec<String>)> {
  vec![(COMPAT_IMPORT_URL.clone(), vec![GLOBAL_URL_STR.clone()])]
}

fn try_resolve_builtin_module(specifier: &str) -> Option<Url> {
  if SUPPORTED_MODULES.contains(&specifier) {
    let module_url =
      format!("{}node/{}.ts", NODE_COMPAT_URL.as_str(), specifier);
    Some(Url::parse(&module_url).unwrap())
  } else {
    None
  }
}

pub(crate) fn load_cjs_module(
  js_runtime: &mut JsRuntime,
  module: &str,
  main: bool,
) -> Result<(), AnyError> {
  let source_code = &format!(
    r#"(async function loadCjsModule(module) {{
      const Module = await import("{module_loader}");
      Module.default._load(module, null, {main});
    }})('{module}');"#,
    module_loader = MODULE_URL_STR.as_str(),
    main = main,
    module = escape_for_single_quote_string(module),
  );

  js_runtime.execute_script(&located_script_name!(), source_code)?;
  Ok(())
}

pub(crate) fn add_global_require(
  js_runtime: &mut JsRuntime,
  main_module: &str,
) -> Result<(), AnyError> {
  let source_code = &format!(
    r#"(async function setupGlobalRequire(main) {{
      const Module = await import("{}");
      const require = Module.createRequire(main);
      globalThis.require = require;
    }})('{}');"#,
    MODULE_URL_STR.as_str(),
    escape_for_single_quote_string(main_module),
  );

  js_runtime.execute_script(&located_script_name!(), source_code)?;
  Ok(())
}

fn escape_for_single_quote_string(text: &str) -> String {
  text.replace(r"\", r"\\").replace("'", r"\'")
}

pub fn setup_builtin_modules(
  js_runtime: &mut JsRuntime,
) -> Result<(), AnyError> {
  let mut script = String::new();
  for module in SUPPORTED_MODULES {
    // skipping the modules that contains '/' as they are not available in NodeJS repl as well
    if !module.contains('/') {
      script = format!("{}const {} = require('{}');\n", script, module, module);
    }
  }

  js_runtime.execute_script("setup_node_builtins.js", &script)?;
  Ok(())
}
