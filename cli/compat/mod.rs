// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod errors;
mod esm_resolver;

use crate::file_fetcher::FileFetcher;
use deno_ast::MediaType;
use deno_core::error::AnyError;
use deno_core::located_script_name;
use deno_core::url::Url;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use once_cell::sync::Lazy;

pub use esm_resolver::check_if_should_use_esm_loader;
pub use esm_resolver::NodeEsmResolver;

// WARNING: Ensure this is the only deno_std version reference as this
// is automatically updated by the version bump workflow.
pub(crate) static STD_URL_STR: &str = "https://deno.land/std@0.150.0/";

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

pub static GLOBAL_URL: Lazy<Url> =
  Lazy::new(|| Url::parse(&GLOBAL_URL_STR).unwrap());

static MODULE_URL_STR: Lazy<String> =
  Lazy::new(|| format!("{}node/module.ts", NODE_COMPAT_URL.as_str()));

pub static MODULE_URL: Lazy<Url> =
  Lazy::new(|| Url::parse(&MODULE_URL_STR).unwrap());

static COMPAT_IMPORT_URL: Lazy<Url> =
  Lazy::new(|| Url::parse("flags:compat").unwrap());

/// Provide imports into a module graph when the compat flag is true.
pub fn get_node_imports() -> Vec<(Url, Vec<String>)> {
  vec![(COMPAT_IMPORT_URL.clone(), vec![GLOBAL_URL_STR.clone()])]
}

fn try_resolve_builtin_module(specifier: &str) -> Option<Url> {
  if SUPPORTED_MODULES.contains(&specifier) {
    let ext = match specifier {
      "stream/promises" => "mjs",
      _ => "ts",
    };
    let module_url =
      format!("{}node/{}.{}", NODE_COMPAT_URL.as_str(), specifier, ext);
    Some(Url::parse(&module_url).unwrap())
  } else {
    None
  }
}

pub fn load_cjs_module(
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

pub fn add_global_require(
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
  text.replace('\\', r"\\").replace('\'', r"\'")
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

/// Translates given CJS module into ESM. This function will perform static
/// analysis on the file to find defined exports and reexports.
///
/// For all discovered reexports the analysis will be performed recursively.
///
/// If successful a source code for equivalent ES module is returned.
pub async fn translate_cjs_to_esm(
  file_fetcher: &FileFetcher,
  specifier: &ModuleSpecifier,
  code: String,
  media_type: MediaType,
) -> Result<String, AnyError> {
  let parsed_source = deno_ast::parse_script(deno_ast::ParseParams {
    specifier: specifier.to_string(),
    text_info: deno_ast::SourceTextInfo::new(code.into()),
    media_type,
    capture_tokens: true,
    scope_analysis: false,
    maybe_syntax: None,
  })?;
  let analysis = parsed_source.analyze_cjs();

  let mut source = vec![
    r#"import { createRequire } from "node:module";"#.to_string(),
    r#"const require = createRequire(import.meta.url);"#.to_string(),
  ];

  // if there are reexports, handle them first
  for (idx, reexport) in analysis.reexports.iter().enumerate() {
    // Firstly, resolve relate reexport specifier
    let resolved_reexport = node_resolver::resolve(
      reexport,
      &specifier.to_file_path().unwrap(),
      // FIXME(bartlomieju): check if these conditions are okay, probably
      // should be `deno-require`, because `deno` is already used in `esm_resolver.rs`
      &["deno", "require", "default"],
    )?;
    let reexport_specifier =
      ModuleSpecifier::from_file_path(&resolved_reexport).unwrap();
    // Secondly, read the source code from disk
    let reexport_file = file_fetcher.get_source(&reexport_specifier).unwrap();
    // Now perform analysis again
    {
      let parsed_source = deno_ast::parse_script(deno_ast::ParseParams {
        specifier: reexport_specifier.to_string(),
        text_info: deno_ast::SourceTextInfo::new(reexport_file.source),
        media_type: reexport_file.media_type,
        capture_tokens: true,
        scope_analysis: false,
        maybe_syntax: None,
      })?;
      let analysis = parsed_source.analyze_cjs();

      source.push(format!(
        "const reexport{} = require(\"{}\");",
        idx, reexport
      ));

      for export in analysis.exports.iter().filter(|e| e.as_str() != "default")
      {
        // TODO(bartlomieju): Node actually checks if a given export exists in `exports` object,
        // but it might not be necessary here since our analysis is more detailed?
        source.push(format!(
          "export const {} = reexport{}.{};",
          export, idx, export
        ));
      }
    }
  }

  source.push(format!(
    "const mod = require(\"{}\");",
    specifier
      .to_file_path()
      .unwrap()
      .to_str()
      .unwrap()
      .replace('\\', "\\\\")
      .replace('\'', "\\\'")
      .replace('\"', "\\\"")
  ));
  source.push("export default mod;".to_string());

  for export in analysis.exports.iter().filter(|e| e.as_str() != "default") {
    // TODO(bartlomieju): Node actually checks if a given export exists in `exports` object,
    // but it might not be necessary here since our analysis is more detailed?
    source.push(format!("export const {} = mod.{};", export, export));
  }

  let translated_source = source.join("\n");
  Ok(translated_source)
}
