// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::deno_std::CURRENT_STD_URL;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::located_script_name;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::JsRuntime;
use deno_graph::source::ResolveResponse;
use deno_runtime::deno_node::get_closest_package_json;
use deno_runtime::deno_node::legacy_main_resolve;
use deno_runtime::deno_node::package_exports_resolve;
use deno_runtime::deno_node::package_imports_resolve;
use deno_runtime::deno_node::package_resolve;
use deno_runtime::deno_node::DenoDirNpmResolver;
use deno_runtime::deno_node::NodeModuleKind;
use deno_runtime::deno_node::PackageJson;
use deno_runtime::deno_node::DEFAULT_CONDITIONS;
use once_cell::sync::Lazy;
use path_clean::PathClean;
use regex::Regex;

use crate::file_fetcher::FileFetcher;
use crate::npm::GlobalNpmPackageResolver;
use crate::npm::NpmPackageReference;
use crate::npm::NpmPackageReq;
use crate::npm::NpmPackageResolver;

mod analyze;
pub mod errors;

pub use analyze::esm_code_with_node_globals;

pub struct NodeModulePolyfill {
  /// Name of the module like "assert" or "timers/promises"
  pub name: &'static str,

  /// Specifier relative to the root of `deno_std` repo, like "node/asser.ts"
  pub specifier: &'static str,
}

pub(crate) static SUPPORTED_MODULES: &[NodeModulePolyfill] = &[
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
    specifier: "node/module.ts",
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

pub(crate) static NODE_COMPAT_URL: Lazy<Url> = Lazy::new(|| {
  if let Ok(url_str) = std::env::var("DENO_NODE_COMPAT_URL") {
    let url = Url::parse(&url_str).expect(
      "Malformed DENO_NODE_COMPAT_URL value, make sure it's a file URL ending with a slash"
    );
    return url;
  }

  CURRENT_STD_URL.clone()
});

pub static MODULE_ALL_URL: Lazy<Url> =
  Lazy::new(|| NODE_COMPAT_URL.join("node/module_all.ts").unwrap());

pub fn try_resolve_builtin_module(specifier: &str) -> Option<Url> {
  for module in SUPPORTED_MODULES {
    if module.name == specifier {
      let module_url = NODE_COMPAT_URL.join(module.specifier).unwrap();
      return Some(module_url);
    }
  }

  None
}

static RESERVED_WORDS: Lazy<HashSet<&str>> = Lazy::new(|| {
  HashSet::from([
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "let",
    "enum",
    "implements",
    "interface",
    "package",
    "private",
    "protected",
    "public",
    "static",
  ])
});

pub async fn initialize_runtime(
  js_runtime: &mut JsRuntime,
) -> Result<(), AnyError> {
  let source_code = &format!(
    r#"(async function loadBuiltinNodeModules(moduleAllUrl) {{
      const moduleAll = await import(moduleAllUrl);
      Deno[Deno.internal].node.initialize(moduleAll.default);
    }})('{}');"#,
    MODULE_ALL_URL.as_str(),
  );

  let value =
    js_runtime.execute_script(&located_script_name!(), source_code)?;
  js_runtime.resolve_value(value).await?;
  Ok(())
}

pub async fn initialize_binary_command(
  js_runtime: &mut JsRuntime,
  binary_name: &str,
) -> Result<(), AnyError> {
  // overwrite what's done in deno_std in order to set the binary arg name
  let source_code = &format!(
    r#"(async function initializeBinaryCommand(binaryName) {{
      const process = Deno[Deno.internal].node.globalThis.process;
      Object.defineProperty(process.argv, "0", {{
        get: () => binaryName,
      }});
    }})('{}');"#,
    binary_name,
  );

  let value =
    js_runtime.execute_script(&located_script_name!(), source_code)?;
  js_runtime.resolve_value(value).await?;
  Ok(())
}

/// This function is an implementation of `defaultResolve` in
/// `lib/internal/modules/esm/resolve.js` from Node.
pub fn node_resolve(
  specifier: &str,
  referrer: &ModuleSpecifier,
  npm_resolver: &dyn DenoDirNpmResolver,
) -> Result<Option<ResolveResponse>, AnyError> {
  // Note: if we are here, then the referrer is an esm module

  // TODO(bartlomieju): skipped "policy" part as we don't plan to support it

  // NOTE(bartlomieju): this will force `ProcState` to use Node.js polyfill for
  // `module` from `ext/node/`.
  if specifier == "module" {
    return Ok(Some(ResolveResponse::Esm(
      Url::parse("node:module").unwrap(),
    )));
  }
  if let Some(resolved) = try_resolve_builtin_module(specifier) {
    return Ok(Some(ResolveResponse::Esm(resolved)));
  }

  if let Ok(url) = Url::parse(specifier) {
    if url.scheme() == "data" {
      return Ok(Some(ResolveResponse::Specifier(url)));
    }

    let protocol = url.scheme();

    if protocol == "node" {
      let split_specifier = url.as_str().split(':');
      let specifier = split_specifier.skip(1).collect::<String>();

      // NOTE(bartlomieju): this will force `ProcState` to use Node.js polyfill for
      // `module` from `ext/node/`.
      if specifier == "module" {
        return Ok(Some(ResolveResponse::Esm(
          Url::parse("node:module").unwrap(),
        )));
      }

      if let Some(resolved) = try_resolve_builtin_module(&specifier) {
        return Ok(Some(ResolveResponse::Esm(resolved)));
      } else {
        return Err(generic_error(format!("Unknown module {}", specifier)));
      }
    }

    if protocol != "file" && protocol != "data" {
      return Err(errors::err_unsupported_esm_url_scheme(&url));
    }

    // todo(dsherret): this seems wrong
    if referrer.scheme() == "data" {
      let url = referrer.join(specifier).map_err(AnyError::from)?;
      return Ok(Some(ResolveResponse::Specifier(url)));
    }
  }

  let conditions = DEFAULT_CONDITIONS;
  let url = module_resolve(specifier, referrer, conditions, npm_resolver)?;
  let url = match url {
    Some(url) => url,
    None => return Ok(None),
  };

  let resolve_response = url_to_resolve_response(url, npm_resolver)?;
  // TODO(bartlomieju): skipped checking errors for commonJS resolution and
  // "preserveSymlinksMain"/"preserveSymlinks" options.
  Ok(Some(resolve_response))
}

pub fn node_resolve_npm_reference(
  reference: &NpmPackageReference,
  npm_resolver: &GlobalNpmPackageResolver,
) -> Result<Option<ResolveResponse>, AnyError> {
  let package_folder = npm_resolver
    .resolve_package_from_deno_module(&reference.req)?
    .folder_path;
  let resolved_path = package_config_resolve(
    &reference
      .sub_path
      .as_ref()
      .map(|s| format!("./{}", s))
      .unwrap_or_else(|| ".".to_string()),
    &package_folder,
    npm_resolver,
    NodeModuleKind::Esm,
  )
  .with_context(|| {
    format!("Error resolving package config for '{}'.", reference)
  })?;

  let url = ModuleSpecifier::from_file_path(resolved_path).unwrap();
  let resolve_response = url_to_resolve_response(url, npm_resolver)?;
  // TODO(bartlomieju): skipped checking errors for commonJS resolution and
  // "preserveSymlinksMain"/"preserveSymlinks" options.
  Ok(Some(resolve_response))
}

pub fn node_resolve_binary_export(
  pkg_req: &NpmPackageReq,
  bin_name: Option<&str>,
  npm_resolver: &GlobalNpmPackageResolver,
) -> Result<ResolveResponse, AnyError> {
  let pkg = npm_resolver.resolve_package_from_deno_module(pkg_req)?;
  let package_folder = pkg.folder_path;
  let package_json_path = package_folder.join("package.json");
  let package_json = PackageJson::load(npm_resolver, package_json_path)?;
  let bin = match &package_json.bin {
    Some(bin) => bin,
    None => bail!(
      "package {} did not have a 'bin' property in its package.json",
      pkg.id
    ),
  };
  let bin_entry = match bin {
    Value::String(_) => {
      if bin_name.is_some() && bin_name.unwrap() != pkg_req.name {
        None
      } else {
        Some(bin)
      }
    }
    Value::Object(o) => {
      if let Some(bin_name) = bin_name {
        o.get(bin_name)
      } else if o.len() == 1 {
        o.values().next()
      } else {
        o.get(&pkg_req.name)
      }
    },
    _ => bail!("package {} did not have a 'bin' property with a string or object value in its package.json", pkg.id),
  };
  let bin_entry = match bin_entry {
    Some(e) => e,
    None => bail!(
      "package {} did not have a 'bin' entry for {} in its package.json",
      pkg.id,
      bin_name.unwrap_or(&pkg_req.name),
    ),
  };
  let bin_entry = match bin_entry {
    Value::String(s) => s,
    _ => bail!(
      "package {} had a non-string sub property of 'bin' in its package.json",
      pkg.id
    ),
  };

  let url =
    ModuleSpecifier::from_file_path(package_folder.join(bin_entry)).unwrap();

  let resolve_response = url_to_resolve_response(url, npm_resolver)?;
  // TODO(bartlomieju): skipped checking errors for commonJS resolution and
  // "preserveSymlinksMain"/"preserveSymlinks" options.
  Ok(resolve_response)
}

pub fn load_cjs_module_from_ext_node(
  js_runtime: &mut JsRuntime,
  module: &str,
  main: bool,
) -> Result<(), AnyError> {
  fn escape_for_single_quote_string(text: &str) -> String {
    text.replace('\\', r"\\").replace('\'', r"\'")
  }

  let source_code = &format!(
    r#"(function loadCjsModule(module) {{
      Deno[Deno.internal].require.Module._load(module, null, {main});
    }})('{module}');"#,
    main = main,
    module = escape_for_single_quote_string(module),
  );

  js_runtime.execute_script(&located_script_name!(), source_code)?;
  Ok(())
}

fn package_config_resolve(
  package_subpath: &str,
  package_dir: &Path,
  npm_resolver: &dyn DenoDirNpmResolver,
  referrer_kind: NodeModuleKind,
) -> Result<PathBuf, AnyError> {
  let package_json_path = package_dir.join("package.json");
  let referrer =
    ModuleSpecifier::from_directory_path(package_json_path.parent().unwrap())
      .unwrap();
  let package_config =
    PackageJson::load(npm_resolver, package_json_path.clone())?;
  if let Some(exports) = &package_config.exports {
    return package_exports_resolve(
      &package_json_path,
      package_subpath.to_string(),
      exports,
      &referrer,
      referrer_kind,
      DEFAULT_CONDITIONS,
      npm_resolver,
    );
  }
  if package_subpath == "." {
    return legacy_main_resolve(&package_config, referrer_kind);
  }

  Ok(package_dir.join(package_subpath))
}

fn url_to_resolve_response(
  url: ModuleSpecifier,
  npm_resolver: &dyn DenoDirNpmResolver,
) -> Result<ResolveResponse, AnyError> {
  Ok(if url.as_str().starts_with("http") {
    ResolveResponse::Esm(url)
  } else if url.as_str().ends_with(".js") {
    let package_config = get_closest_package_json(&url, npm_resolver)?;
    if package_config.typ == "module" {
      ResolveResponse::Esm(url)
    } else {
      ResolveResponse::CommonJs(url)
    }
  } else if url.as_str().ends_with(".cjs") {
    ResolveResponse::CommonJs(url)
  } else {
    ResolveResponse::Esm(url)
  })
}

fn finalize_resolution(
  resolved: ModuleSpecifier,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  // TODO(bartlomieju): this is not part of Node resolution algorithm
  // (as it doesn't support http/https); but I had to short circuit here
  // for remote modules because they are mainly used to polyfill `node` built
  // in modules. Another option would be to leave the resolved URLs
  // as `node:<module_name>` and do the actual remapping to std's polyfill
  // in module loader. I'm not sure which approach is better.
  if resolved.scheme().starts_with("http") {
    return Ok(resolved);
  }

  // todo(dsherret): cache
  let encoded_sep_re = Regex::new(r"%2F|%2C").unwrap();

  if encoded_sep_re.is_match(resolved.path()) {
    return Err(errors::err_invalid_module_specifier(
      resolved.path(),
      "must not include encoded \"/\" or \"\\\\\" characters",
      Some(to_file_path_string(base)),
    ));
  }

  let path = to_file_path(&resolved);

  // TODO(bartlomieju): currently not supported
  // if (getOptionValue('--experimental-specifier-resolution') === 'node') {
  //   ...
  // }

  let p_str = path.to_str().unwrap();
  let p = if p_str.ends_with('/') {
    p_str[p_str.len() - 1..].to_string()
  } else {
    p_str.to_string()
  };

  let (is_dir, is_file) = if let Ok(stats) = std::fs::metadata(&p) {
    (stats.is_dir(), stats.is_file())
  } else {
    (false, false)
  };
  if is_dir {
    return Err(errors::err_unsupported_dir_import(
      resolved.as_str(),
      base.as_str(),
    ));
  } else if !is_file {
    return Err(errors::err_module_not_found(
      resolved.as_str(),
      base.as_str(),
      "module",
    ));
  }

  Ok(resolved)
}

fn module_resolve(
  specifier: &str,
  referrer: &ModuleSpecifier,
  conditions: &[&str],
  npm_resolver: &dyn DenoDirNpmResolver,
) -> Result<Option<ModuleSpecifier>, AnyError> {
  // note: if we're here, the referrer is an esm module
  let url = if should_be_treated_as_relative_or_absolute_path(specifier) {
    let resolved_specifier = referrer.join(specifier)?;
    Some(resolved_specifier)
  } else if specifier.starts_with('#') {
    Some(
      package_imports_resolve(
        specifier,
        referrer,
        NodeModuleKind::Esm,
        conditions,
        npm_resolver,
      )
      .map(|p| ModuleSpecifier::from_file_path(p).unwrap())?,
    )
  } else if let Ok(resolved) = Url::parse(specifier) {
    Some(resolved)
  } else {
    Some(
      package_resolve(
        specifier,
        referrer,
        NodeModuleKind::Esm,
        conditions,
        npm_resolver,
      )
      .map(|p| ModuleSpecifier::from_file_path(p).unwrap())?,
    )
  };
  Ok(match url {
    Some(url) => Some(finalize_resolution(url, referrer)?),
    None => None,
  })
}

fn add_export(
  source: &mut Vec<String>,
  name: &str,
  initializer: &str,
  temp_var_count: &mut usize,
) {
  fn is_valid_var_decl(name: &str) -> bool {
    // it's ok to be super strict here
    name
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
  }

  // TODO(bartlomieju): Node actually checks if a given export exists in `exports` object,
  // but it might not be necessary here since our analysis is more detailed?
  if RESERVED_WORDS.contains(name) || !is_valid_var_decl(name) {
    *temp_var_count += 1;
    // we can't create an identifier with a reserved word or invalid identifier name,
    // so assign it to a temporary variable that won't have a conflict, then re-export
    // it as a string
    source.push(format!(
      "const __deno_export_{}__ = {};",
      temp_var_count, initializer
    ));
    source.push(format!(
      "export {{ __deno_export_{}__ as \"{}\" }};",
      temp_var_count, name
    ));
  } else {
    source.push(format!("export const {} = {};", name, initializer));
  }
}

/// Translates given CJS module into ESM. This function will perform static
/// analysis on the file to find defined exports and reexports.
///
/// For all discovered reexports the analysis will be performed recursively.
///
/// If successful a source code for equivalent ES module is returned.
pub fn translate_cjs_to_esm(
  file_fetcher: &FileFetcher,
  specifier: &ModuleSpecifier,
  code: String,
  media_type: MediaType,
  npm_resolver: &GlobalNpmPackageResolver,
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
  let root_exports = analysis
    .exports
    .iter()
    .map(|s| s.as_str())
    .collect::<HashSet<_>>();
  let mut temp_var_count = 0;

  let mut source = vec![
    r#"const require = Deno[Deno.internal].require.Module.createRequire(import.meta.url);"#.to_string(),
  ];

  // if there are reexports, handle them first
  for (idx, reexport) in analysis.reexports.iter().enumerate() {
    // Firstly, resolve relate reexport specifier
    // todo(dsherret): call module_resolve instead?
    let resolved_reexport = resolve(
      reexport,
      specifier,
      // FIXME(bartlomieju): check if these conditions are okay, probably
      // should be `deno-require`, because `deno` is already used in `esm_resolver.rs`
      &["deno", "require", "default"],
      npm_resolver,
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

      for export in analysis.exports.iter().filter(|e| {
        e.as_str() != "default" && !root_exports.contains(e.as_str())
      }) {
        add_export(
          &mut source,
          export,
          &format!("Deno[Deno.internal].require.bindExport(reexport{0}[\"{1}\"], reexport{0})", idx, export),
          &mut temp_var_count,
        );
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

  let mut had_default = false;
  for export in analysis.exports.iter() {
    if export.as_str() == "default" {
      if root_exports.contains("__esModule") {
        source.push(format!(
          "export default Deno[Deno.internal].require.bindExport(mod[\"{}\"], mod);",
          export,
        ));
        had_default = true;
      }
    } else {
      add_export(
        &mut source,
        export,
        &format!(
          "Deno[Deno.internal].require.bindExport(mod[\"{}\"], mod)",
          export
        ),
        &mut temp_var_count,
      );
    }
  }

  if !had_default {
    source.push("export default mod;".to_string());
  }

  let translated_source = source.join("\n");
  Ok(translated_source)
}

fn resolve_package_target_string(
  target: &str,
  subpath: Option<String>,
) -> String {
  if let Some(subpath) = subpath {
    target.replace('*', &subpath)
  } else {
    target.to_string()
  }
}

fn resolve(
  specifier: &str,
  referrer: &ModuleSpecifier,
  conditions: &[&str],
  npm_resolver: &dyn DenoDirNpmResolver,
) -> Result<PathBuf, AnyError> {
  if specifier.starts_with('/') {
    todo!();
  }

  let referrer_path = referrer.to_file_path().unwrap();
  if specifier.starts_with("./") || specifier.starts_with("../") {
    if let Some(parent) = referrer_path.parent() {
      return file_extension_probe(parent.join(specifier), &referrer_path);
    } else {
      todo!();
    }
  }

  // We've got a bare specifier or maybe bare_specifier/blah.js"

  let (_, package_subpath) = parse_specifier(specifier).unwrap();

  // todo(dsherret): use not_found error on not found here
  let module_dir =
    npm_resolver.resolve_package_folder_from_path(&referrer_path)?;

  let package_json_path = module_dir.join("package.json");
  if package_json_path.exists() {
    let package_json = PackageJson::load(npm_resolver, package_json_path)?;

    if let Some(map) = package_json.exports {
      if let Some((key, subpath)) = exports_resolve(&map, &package_subpath) {
        let value = map.get(&key).unwrap();
        let s = conditions_resolve(value, conditions);

        let t = resolve_package_target_string(&s, subpath);
        return Ok(module_dir.join(t).clean());
      } else {
        todo!()
      }
    }

    // old school
    if package_subpath != "." {
      let d = module_dir.join(package_subpath);
      if let Ok(m) = d.metadata() {
        if m.is_dir() {
          return Ok(d.join("index.js").clean());
        }
      }
      return file_extension_probe(d, &referrer_path);
    } else if let Some(main) = package_json.main {
      return Ok(module_dir.join(main).clean());
    } else {
      return Ok(module_dir.join("index.js").clean());
    }
  }

  Err(not_found(specifier, &referrer_path))
}

fn conditions_resolve(value: &Value, conditions: &[&str]) -> String {
  match value {
    Value::String(s) => s.to_string(),
    Value::Object(map) => {
      for condition in conditions {
        if let Some(x) = map.get(&condition.to_string()) {
          if let Value::String(s) = x {
            return s.to_string();
          } else {
            todo!()
          }
        }
      }
      todo!()
    }
    _ => todo!(),
  }
}

fn parse_specifier(specifier: &str) -> Option<(String, String)> {
  let mut separator_index = specifier.find('/');
  let mut valid_package_name = true;
  // let mut is_scoped = false;
  if specifier.is_empty() {
    valid_package_name = false;
  } else if specifier.starts_with('@') {
    // is_scoped = true;
    if let Some(index) = separator_index {
      separator_index = specifier[index + 1..].find('/');
    } else {
      valid_package_name = false;
    }
  }

  let package_name = if let Some(index) = separator_index {
    specifier[0..index].to_string()
  } else {
    specifier.to_string()
  };

  // Package name cannot have leading . and cannot have percent-encoding or separators.
  for ch in package_name.chars() {
    if ch == '%' || ch == '\\' {
      valid_package_name = false;
      break;
    }
  }

  if !valid_package_name {
    return None;
  }

  let package_subpath = if let Some(index) = separator_index {
    format!(".{}", specifier.chars().skip(index).collect::<String>())
  } else {
    ".".to_string()
  };

  Some((package_name, package_subpath))
}

fn exports_resolve(
  map: &Map<String, Value>,
  subpath: &str,
) -> Option<(String, Option<String>)> {
  if map.contains_key(subpath) {
    return Some((subpath.to_string(), None));
  }

  // best match
  let mut best_match = None;
  for key in map.keys() {
    if let Some(pattern_index) = key.find('*') {
      let key_sub = &key[0..pattern_index];
      if subpath.starts_with(key_sub) {
        if subpath.ends_with('/') {
          todo!()
        }
        let pattern_trailer = &key[pattern_index + 1..];

        if subpath.len() > key.len()
          && subpath.ends_with(pattern_trailer)
          // && pattern_key_compare(best_match, key) == 1
          && key.rfind('*') == Some(pattern_index)
        {
          let rest = subpath
            [pattern_index..(subpath.len() - pattern_trailer.len())]
            .to_string();
          best_match = Some((key, rest));
        }
      }
    }
  }

  if let Some((key, subpath_)) = best_match {
    return Some((key.to_string(), Some(subpath_)));
  }

  None
}

fn to_file_path(url: &ModuleSpecifier) -> PathBuf {
  url
    .to_file_path()
    .unwrap_or_else(|_| panic!("Provided URL was not file:// URL: {}", url))
}

fn to_file_path_string(url: &ModuleSpecifier) -> String {
  to_file_path(url).display().to_string()
}

fn should_be_treated_as_relative_or_absolute_path(specifier: &str) -> bool {
  if specifier.is_empty() {
    return false;
  }

  if specifier.starts_with('/') {
    return true;
  }

  is_relative_specifier(specifier)
}

// TODO(ry) We very likely have this utility function elsewhere in Deno.
fn is_relative_specifier(specifier: &str) -> bool {
  let specifier_len = specifier.len();
  let specifier_chars: Vec<_> = specifier.chars().collect();

  if !specifier_chars.is_empty() && specifier_chars[0] == '.' {
    if specifier_len == 1 || specifier_chars[1] == '/' {
      return true;
    }
    if specifier_chars[1] == '.'
      && (specifier_len == 2 || specifier_chars[2] == '/')
    {
      return true;
    }
  }
  false
}

fn file_extension_probe(
  mut p: PathBuf,
  referrer: &Path,
) -> Result<PathBuf, AnyError> {
  if p.exists() && !p.is_dir() {
    Ok(p.clean())
  } else {
    p.set_extension("js");
    if p.exists() && !p.is_dir() {
      Ok(p)
    } else {
      Err(not_found(&p.clean().to_string_lossy(), referrer))
    }
  }
}

fn not_found(path: &str, referrer: &Path) -> AnyError {
  let msg = format!(
    "[ERR_MODULE_NOT_FOUND] Cannot find module \"{}\" imported from \"{}\"",
    path,
    referrer.to_string_lossy()
  );
  std::io::Error::new(std::io::ErrorKind::NotFound, msg).into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_add_export() {
    let mut temp_var_count = 0;
    let mut source = vec![];

    let exports = vec!["static", "server", "app", "dashed-export"];
    for export in exports {
      add_export(&mut source, export, "init", &mut temp_var_count);
    }
    assert_eq!(
      source,
      vec![
        "const __deno_export_1__ = init;".to_string(),
        "export { __deno_export_1__ as \"static\" };".to_string(),
        "export const server = init;".to_string(),
        "export const app = init;".to_string(),
        "const __deno_export_2__ = init;".to_string(),
        "export { __deno_export_2__ as \"dashed-export\" };".to_string(),
      ]
    )
  }

  #[test]
  fn test_resolve_package_target_string() {
    assert_eq!(resolve_package_target_string("foo", None), "foo");
    assert_eq!(
      resolve_package_target_string("*foo", Some("bar".to_string())),
      "barfoo"
    );
  }
}
