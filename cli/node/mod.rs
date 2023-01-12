// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;

use deno_ast::CjsAnalysis;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::located_script_name;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::JsRuntime;
use deno_runtime::deno_node::errors;
use deno_runtime::deno_node::get_closest_package_json;
use deno_runtime::deno_node::legacy_main_resolve;
use deno_runtime::deno_node::package_exports_resolve;
use deno_runtime::deno_node::package_imports_resolve;
use deno_runtime::deno_node::package_resolve;
use deno_runtime::deno_node::path_to_declaration_path;
use deno_runtime::deno_node::NodeModuleKind;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_node::PackageJson;
use deno_runtime::deno_node::PathClean;
use deno_runtime::deno_node::RequireNpmResolver;
use deno_runtime::deno_node::DEFAULT_CONDITIONS;
use deno_runtime::deno_node::NODE_GLOBAL_THIS_NAME;
use deno_runtime::permissions::PermissionsContainer;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::cache::NodeAnalysisCache;
use crate::deno_std::CURRENT_STD_URL;
use crate::file_fetcher::FileFetcher;
use crate::npm::NpmPackageReference;
use crate::npm::NpmPackageReq;
use crate::npm::NpmPackageResolver;

mod analyze;

pub use analyze::esm_code_with_node_globals;

#[derive(Debug)]
pub enum NodeResolution {
  Esm(ModuleSpecifier),
  CommonJs(ModuleSpecifier),
  BuiltIn(String),
}

impl NodeResolution {
  pub fn into_url(self) -> ModuleSpecifier {
    match self {
      Self::Esm(u) => u,
      Self::CommonJs(u) => u,
      Self::BuiltIn(specifier) => {
        if specifier.starts_with("node:") {
          ModuleSpecifier::parse(&specifier).unwrap()
        } else {
          ModuleSpecifier::parse(&format!("node:{}", specifier)).unwrap()
        }
      }
    }
  }

  pub fn into_specifier_and_media_type(
    resolution: Option<Self>,
  ) -> (ModuleSpecifier, MediaType) {
    match resolution {
      Some(NodeResolution::CommonJs(specifier)) => {
        let media_type = MediaType::from(&specifier);
        (
          specifier,
          match media_type {
            MediaType::JavaScript | MediaType::Jsx => MediaType::Cjs,
            MediaType::TypeScript | MediaType::Tsx => MediaType::Cts,
            MediaType::Dts => MediaType::Dcts,
            _ => media_type,
          },
        )
      }
      Some(NodeResolution::Esm(specifier)) => {
        let media_type = MediaType::from(&specifier);
        (
          specifier,
          match media_type {
            MediaType::JavaScript | MediaType::Jsx => MediaType::Mjs,
            MediaType::TypeScript | MediaType::Tsx => MediaType::Mts,
            MediaType::Dts => MediaType::Dmts,
            _ => media_type,
          },
        )
      }
      Some(resolution) => (resolution.into_url(), MediaType::Dts),
      None => (
        ModuleSpecifier::parse("deno:///missing_dependency.d.ts").unwrap(),
        MediaType::Dts,
      ),
    }
  }
}

struct NodeModulePolyfill {
  /// Name of the module like "assert" or "timers/promises"
  name: &'static str,

  /// Specifier relative to the root of `deno_std` repo, like "node/asser.ts"
  specifier: &'static str,
}

static SUPPORTED_MODULES: &[NodeModulePolyfill] = &[
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

static NODE_COMPAT_URL: Lazy<Url> = Lazy::new(|| {
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

fn find_builtin_node_module(specifier: &str) -> Option<&NodeModulePolyfill> {
  SUPPORTED_MODULES.iter().find(|m| m.name == specifier)
}

fn is_builtin_node_module(specifier: &str) -> bool {
  find_builtin_node_module(specifier).is_some()
}

pub fn resolve_builtin_node_module(specifier: &str) -> Result<Url, AnyError> {
  // NOTE(bartlomieju): `module` is special, because we don't want to use
  // `deno_std/node/module.ts`, but instead use a special shim that we
  // provide in `ext/node`.
  if specifier == "module" {
    return Ok(Url::parse("node:module").unwrap());
  }

  if let Some(module) = find_builtin_node_module(specifier) {
    let module_url = NODE_COMPAT_URL.join(module.specifier).unwrap();
    return Ok(module_url);
  }

  Err(generic_error(format!(
    "Unknown built-in Node module: {}",
    specifier
  )))
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
  uses_local_node_modules_dir: bool,
) -> Result<(), AnyError> {
  let source_code = &format!(
    r#"(async function loadBuiltinNodeModules(moduleAllUrl, nodeGlobalThisName, usesLocalNodeModulesDir) {{
      const moduleAll = await import(moduleAllUrl);
      Deno[Deno.internal].node.initialize(moduleAll.default, nodeGlobalThisName);
      if (usesLocalNodeModulesDir) {{
        Deno[Deno.internal].require.setUsesLocalNodeModulesDir();
      }}
    }})('{}', '{}', {});"#,
    MODULE_ALL_URL.as_str(),
    NODE_GLOBAL_THIS_NAME.as_str(),
    uses_local_node_modules_dir,
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
  mode: NodeResolutionMode,
  npm_resolver: &dyn RequireNpmResolver,
  permissions: &mut dyn NodePermissions,
) -> Result<Option<NodeResolution>, AnyError> {
  // Note: if we are here, then the referrer is an esm module
  // TODO(bartlomieju): skipped "policy" part as we don't plan to support it

  if is_builtin_node_module(specifier) {
    return Ok(Some(NodeResolution::BuiltIn(specifier.to_string())));
  }

  if let Ok(url) = Url::parse(specifier) {
    if url.scheme() == "data" {
      return Ok(Some(NodeResolution::Esm(url)));
    }

    let protocol = url.scheme();

    if protocol == "node" {
      let split_specifier = url.as_str().split(':');
      let specifier = split_specifier.skip(1).collect::<String>();

      if is_builtin_node_module(&specifier) {
        return Ok(Some(NodeResolution::BuiltIn(specifier)));
      }
    }

    if protocol != "file" && protocol != "data" {
      return Err(errors::err_unsupported_esm_url_scheme(&url));
    }

    // todo(dsherret): this seems wrong
    if referrer.scheme() == "data" {
      let url = referrer.join(specifier).map_err(AnyError::from)?;
      return Ok(Some(NodeResolution::Esm(url)));
    }
  }

  let url = module_resolve(
    specifier,
    referrer,
    DEFAULT_CONDITIONS,
    mode,
    npm_resolver,
    permissions,
  )?;
  let url = match url {
    Some(url) => url,
    None => return Ok(None),
  };
  let url = match mode {
    NodeResolutionMode::Execution => url,
    NodeResolutionMode::Types => {
      let path = url.to_file_path().unwrap();
      // todo(16370): the module kind is not correct here. I think we need
      // typescript to tell us if the referrer is esm or cjs
      let path = match path_to_declaration_path(path, NodeModuleKind::Esm) {
        Some(path) => path,
        None => return Ok(None),
      };
      ModuleSpecifier::from_file_path(path).unwrap()
    }
  };

  let resolve_response = url_to_node_resolution(url, npm_resolver)?;
  // TODO(bartlomieju): skipped checking errors for commonJS resolution and
  // "preserveSymlinksMain"/"preserveSymlinks" options.
  Ok(Some(resolve_response))
}

pub fn node_resolve_npm_reference(
  reference: &NpmPackageReference,
  mode: NodeResolutionMode,
  npm_resolver: &NpmPackageResolver,
  permissions: &mut dyn NodePermissions,
) -> Result<Option<NodeResolution>, AnyError> {
  let package_folder =
    npm_resolver.resolve_package_folder_from_deno_module(&reference.req)?;
  let node_module_kind = NodeModuleKind::Esm;
  let maybe_resolved_path = package_config_resolve(
    &reference
      .sub_path
      .as_ref()
      .map(|s| format!("./{}", s))
      .unwrap_or_else(|| ".".to_string()),
    &package_folder,
    node_module_kind,
    DEFAULT_CONDITIONS,
    mode,
    npm_resolver,
    permissions,
  )
  .with_context(|| {
    format!("Error resolving package config for '{}'", reference)
  })?;
  let resolved_path = match maybe_resolved_path {
    Some(resolved_path) => resolved_path,
    None => return Ok(None),
  };
  let resolved_path = match mode {
    NodeResolutionMode::Execution => resolved_path,
    NodeResolutionMode::Types => {
      match path_to_declaration_path(resolved_path, node_module_kind) {
        Some(path) => path,
        None => return Ok(None),
      }
    }
  };
  let url = ModuleSpecifier::from_file_path(resolved_path).unwrap();
  let resolve_response = url_to_node_resolution(url, npm_resolver)?;
  // TODO(bartlomieju): skipped checking errors for commonJS resolution and
  // "preserveSymlinksMain"/"preserveSymlinks" options.
  Ok(Some(resolve_response))
}

pub fn node_resolve_binary_export(
  pkg_req: &NpmPackageReq,
  bin_name: Option<&str>,
  npm_resolver: &NpmPackageResolver,
  permissions: &mut dyn NodePermissions,
) -> Result<NodeResolution, AnyError> {
  let package_folder =
    npm_resolver.resolve_package_folder_from_deno_module(pkg_req)?;
  let package_json_path = package_folder.join("package.json");
  let package_json =
    PackageJson::load(npm_resolver, permissions, package_json_path)?;
  let bin = match &package_json.bin {
    Some(bin) => bin,
    None => bail!(
      "package '{}' did not have a bin property in its package.json",
      &pkg_req.name,
    ),
  };
  let bin_entry = resolve_bin_entry_value(pkg_req, bin_name, bin)?;
  let url =
    ModuleSpecifier::from_file_path(package_folder.join(bin_entry)).unwrap();

  let resolve_response = url_to_node_resolution(url, npm_resolver)?;
  // TODO(bartlomieju): skipped checking errors for commonJS resolution and
  // "preserveSymlinksMain"/"preserveSymlinks" options.
  Ok(resolve_response)
}

fn resolve_bin_entry_value<'a>(
  pkg_req: &NpmPackageReq,
  bin_name: Option<&str>,
  bin: &'a Value,
) -> Result<&'a str, AnyError> {
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
      } else if o.len() == 1 || o.len() > 1 && o.values().all(|v| v == o.values().next().unwrap()) {
        o.values().next()
      } else {
        o.get(&pkg_req.name)
      }
    },
    _ => bail!("package '{}' did not have a bin property with a string or object value in its package.json", pkg_req.name),
  };
  let bin_entry = match bin_entry {
    Some(e) => e,
    None => {
      let keys = bin
        .as_object()
        .map(|o| {
          o.keys()
            .into_iter()
            .map(|k| format!(" * npm:{}/{}", pkg_req, k))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
      bail!(
        "package '{}' did not have a bin entry for '{}' in its package.json{}",
        pkg_req.name,
        bin_name.unwrap_or(&pkg_req.name),
        if keys.is_empty() {
          "".to_string()
        } else {
          format!("\n\nPossibilities:\n{}", keys.join("\n"))
        }
      )
    }
  };
  match bin_entry {
    Value::String(s) => Ok(s),
    _ => bail!(
      "package '{}' had a non-string sub property of bin in its package.json",
      pkg_req.name,
    ),
  }
}

pub fn load_cjs_module_from_ext_node(
  js_runtime: &mut JsRuntime,
  module: &str,
  main: bool,
  inspect_brk: bool,
) -> Result<(), AnyError> {
  fn escape_for_single_quote_string(text: &str) -> String {
    text.replace('\\', r"\\").replace('\'', r"\'")
  }

  let source_code = &format!(
    r#"(function loadCjsModule(module, inspectBrk) {{
      if (inspectBrk) {{
        Deno[Deno.internal].require.setInspectBrk();
      }}
      Deno[Deno.internal].require.Module._load(module, null, {main});
    }})('{module}', {inspect_brk});"#,
    main = main,
    module = escape_for_single_quote_string(module),
    inspect_brk = inspect_brk,
  );

  js_runtime.execute_script(&located_script_name!(), source_code)?;
  Ok(())
}

fn package_config_resolve(
  package_subpath: &str,
  package_dir: &Path,
  referrer_kind: NodeModuleKind,
  conditions: &[&str],
  mode: NodeResolutionMode,
  npm_resolver: &dyn RequireNpmResolver,
  permissions: &mut dyn NodePermissions,
) -> Result<Option<PathBuf>, AnyError> {
  let package_json_path = package_dir.join("package.json");
  let referrer = ModuleSpecifier::from_directory_path(package_dir).unwrap();
  let package_config =
    PackageJson::load(npm_resolver, permissions, package_json_path.clone())?;
  if let Some(exports) = &package_config.exports {
    let result = package_exports_resolve(
      &package_json_path,
      package_subpath.to_string(),
      exports,
      &referrer,
      referrer_kind,
      conditions,
      mode,
      npm_resolver,
      permissions,
    );
    match result {
      Ok(found) => return Ok(Some(found)),
      Err(exports_err) => {
        if mode.is_types() && package_subpath == "." {
          if let Ok(Some(path)) =
            legacy_main_resolve(&package_config, referrer_kind, mode)
          {
            return Ok(Some(path));
          } else {
            return Ok(None);
          }
        }
        return Err(exports_err);
      }
    }
  }
  if package_subpath == "." {
    return legacy_main_resolve(&package_config, referrer_kind, mode);
  }

  Ok(Some(package_dir.join(package_subpath)))
}

pub fn url_to_node_resolution(
  url: ModuleSpecifier,
  npm_resolver: &dyn RequireNpmResolver,
) -> Result<NodeResolution, AnyError> {
  let url_str = url.as_str().to_lowercase();
  if url_str.starts_with("http") {
    Ok(NodeResolution::Esm(url))
  } else if url_str.ends_with(".js") || url_str.ends_with(".d.ts") {
    let package_config = get_closest_package_json(
      &url,
      npm_resolver,
      &mut PermissionsContainer::allow_all(),
    )?;
    if package_config.typ == "module" {
      Ok(NodeResolution::Esm(url))
    } else {
      Ok(NodeResolution::CommonJs(url))
    }
  } else if url_str.ends_with(".mjs") || url_str.ends_with(".d.mts") {
    Ok(NodeResolution::Esm(url))
  } else if url_str.ends_with(".ts") {
    Err(generic_error(format!(
      "TypeScript files are not supported in npm packages: {}",
      url
    )))
  } else {
    Ok(NodeResolution::CommonJs(url))
  }
}

fn finalize_resolution(
  resolved: ModuleSpecifier,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
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

  let (is_dir, is_file) = if let Ok(stats) = std::fs::metadata(p) {
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
  mode: NodeResolutionMode,
  npm_resolver: &dyn RequireNpmResolver,
  permissions: &mut dyn NodePermissions,
) -> Result<Option<ModuleSpecifier>, AnyError> {
  // note: if we're here, the referrer is an esm module
  let url = if should_be_treated_as_relative_or_absolute_path(specifier) {
    let resolved_specifier = referrer.join(specifier)?;
    if mode.is_types() {
      let file_path = to_file_path(&resolved_specifier);
      // todo(dsherret): the node module kind is not correct and we
      // should use the value provided by typescript instead
      let declaration_path =
        path_to_declaration_path(file_path, NodeModuleKind::Esm);
      declaration_path.map(|declaration_path| {
        ModuleSpecifier::from_file_path(declaration_path).unwrap()
      })
    } else {
      Some(resolved_specifier)
    }
  } else if specifier.starts_with('#') {
    Some(
      package_imports_resolve(
        specifier,
        referrer,
        NodeModuleKind::Esm,
        conditions,
        mode,
        npm_resolver,
        permissions,
      )
      .map(|p| ModuleSpecifier::from_file_path(p).unwrap())?,
    )
  } else if let Ok(resolved) = Url::parse(specifier) {
    Some(resolved)
  } else {
    package_resolve(
      specifier,
      referrer,
      NodeModuleKind::Esm,
      conditions,
      mode,
      npm_resolver,
      permissions,
    )?
    .map(|p| ModuleSpecifier::from_file_path(p).unwrap())
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
  npm_resolver: &NpmPackageResolver,
  node_analysis_cache: &NodeAnalysisCache,
  permissions: &mut dyn NodePermissions,
) -> Result<String, AnyError> {
  fn perform_cjs_analysis(
    analysis_cache: &NodeAnalysisCache,
    specifier: &str,
    media_type: MediaType,
    code: String,
  ) -> Result<CjsAnalysis, AnyError> {
    let source_hash = NodeAnalysisCache::compute_source_hash(&code);
    if let Some(analysis) =
      analysis_cache.get_cjs_analysis(specifier, &source_hash)
    {
      return Ok(analysis);
    }

    if media_type == MediaType::Json {
      return Ok(CjsAnalysis {
        exports: vec![],
        reexports: vec![],
      });
    }

    let parsed_source = deno_ast::parse_script(deno_ast::ParseParams {
      specifier: specifier.to_string(),
      text_info: deno_ast::SourceTextInfo::new(code.into()),
      media_type,
      capture_tokens: true,
      scope_analysis: false,
      maybe_syntax: None,
    })?;
    let analysis = parsed_source.analyze_cjs();
    analysis_cache.set_cjs_analysis(specifier, &source_hash, &analysis);

    Ok(analysis)
  }

  let mut temp_var_count = 0;
  let mut handled_reexports: HashSet<String> = HashSet::default();

  let mut source = vec![
    r#"const require = Deno[Deno.internal].require.Module.createRequire(import.meta.url);"#.to_string(),
  ];

  let analysis = perform_cjs_analysis(
    node_analysis_cache,
    specifier.as_str(),
    media_type,
    code,
  )?;

  let mut all_exports = analysis
    .exports
    .iter()
    .map(|s| s.to_string())
    .collect::<HashSet<_>>();

  // (request, referrer)
  let mut reexports_to_handle = VecDeque::new();
  for reexport in analysis.reexports {
    reexports_to_handle.push_back((reexport, specifier.clone()));
  }

  while let Some((reexport, referrer)) = reexports_to_handle.pop_front() {
    if handled_reexports.contains(&reexport) {
      continue;
    }

    handled_reexports.insert(reexport.to_string());

    // First, resolve relate reexport specifier
    let resolved_reexport = resolve(
      &reexport,
      &referrer,
      // FIXME(bartlomieju): check if these conditions are okay, probably
      // should be `deno-require`, because `deno` is already used in `esm_resolver.rs`
      &["deno", "require", "default"],
      NodeResolutionMode::Execution,
      npm_resolver,
      permissions,
    )?;
    let reexport_specifier =
      ModuleSpecifier::from_file_path(resolved_reexport).unwrap();
    // Second, read the source code from disk
    let reexport_file = file_fetcher
      .get_source(&reexport_specifier)
      .ok_or_else(|| {
        anyhow!(
          "Could not find '{}' ({}) referenced from {}",
          reexport,
          reexport_specifier,
          referrer
        )
      })?;

    {
      let analysis = perform_cjs_analysis(
        node_analysis_cache,
        reexport_specifier.as_str(),
        reexport_file.media_type,
        reexport_file.source.to_string(),
      )?;

      for reexport in analysis.reexports {
        reexports_to_handle.push_back((reexport, reexport_specifier.clone()));
      }

      all_exports.extend(
        analysis
          .exports
          .into_iter()
          .filter(|e| e.as_str() != "default"),
      );
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

  for export in &all_exports {
    if export.as_str() != "default" {
      add_export(
        &mut source,
        export,
        &format!("mod[\"{}\"]", export),
        &mut temp_var_count,
      );
    }
  }

  source.push("export default mod;".to_string());

  let translated_source = source.join("\n");
  Ok(translated_source)
}

fn resolve(
  specifier: &str,
  referrer: &ModuleSpecifier,
  conditions: &[&str],
  mode: NodeResolutionMode,
  npm_resolver: &dyn RequireNpmResolver,
  permissions: &mut dyn NodePermissions,
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

  let (package_specifier, package_subpath) =
    parse_specifier(specifier).unwrap();

  // todo(dsherret): use not_found error on not found here
  let module_dir = npm_resolver.resolve_package_folder_from_package(
    package_specifier.as_str(),
    &referrer_path,
    mode,
  )?;

  let package_json_path = module_dir.join("package.json");
  if package_json_path.exists() {
    let package_json =
      PackageJson::load(npm_resolver, permissions, package_json_path.clone())?;

    if let Some(exports) = &package_json.exports {
      return package_exports_resolve(
        &package_json_path,
        package_subpath,
        exports,
        referrer,
        NodeModuleKind::Esm,
        conditions,
        mode,
        npm_resolver,
        permissions,
      );
    }

    // old school
    if package_subpath != "." {
      let d = module_dir.join(package_subpath);
      if let Ok(m) = d.metadata() {
        if m.is_dir() {
          // subdir might have a package.json that specifies the entrypoint
          let package_json_path = d.join("package.json");
          if package_json_path.exists() {
            let package_json =
              PackageJson::load(npm_resolver, permissions, package_json_path)?;
            if let Some(main) = package_json.main(NodeModuleKind::Cjs) {
              return Ok(d.join(main).clean());
            }
          }

          return Ok(d.join("index.js").clean());
        }
      }
      return file_extension_probe(d, &referrer_path);
    } else if let Some(main) = package_json.main(NodeModuleKind::Cjs) {
      return Ok(module_dir.join(main).clean());
    } else {
      return Ok(module_dir.join("index.js").clean());
    }
  }
  Err(not_found(specifier, &referrer_path))
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
      separator_index = specifier[index + 1..].find('/').map(|i| i + index + 1);
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
  p: PathBuf,
  referrer: &Path,
) -> Result<PathBuf, AnyError> {
  let p = p.clean();
  if p.exists() {
    let file_name = p.file_name().unwrap();
    let p_js = p.with_file_name(format!("{}.js", file_name.to_str().unwrap()));
    if p_js.exists() && p_js.is_file() {
      return Ok(p_js);
    } else if p.is_dir() {
      return Ok(p.join("index.js"));
    } else {
      return Ok(p);
    }
  } else if let Some(file_name) = p.file_name() {
    let p_js = p.with_file_name(format!("{}.js", file_name.to_str().unwrap()));
    if p_js.exists() && p_js.is_file() {
      return Ok(p_js);
    }
  }
  Err(not_found(&p.to_string_lossy(), referrer))
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
  use deno_core::serde_json::json;

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
  fn test_parse_specifier() {
    assert_eq!(
      parse_specifier("@some-package/core/actions"),
      Some(("@some-package/core".to_string(), "./actions".to_string()))
    );
  }

  #[test]
  fn test_resolve_bin_entry_value() {
    // should resolve the specified value
    let value = json!({
      "bin1": "./value1",
      "bin2": "./value2",
      "test": "./value3",
    });
    assert_eq!(
      resolve_bin_entry_value(
        &NpmPackageReq::from_str("test").unwrap(),
        Some("bin1"),
        &value
      )
      .unwrap(),
      "./value1"
    );

    // should resolve the value with the same name when not specified
    assert_eq!(
      resolve_bin_entry_value(
        &NpmPackageReq::from_str("test").unwrap(),
        None,
        &value
      )
      .unwrap(),
      "./value3"
    );

    // should not resolve when specified value does not exist
    assert_eq!(
      resolve_bin_entry_value(
        &NpmPackageReq::from_str("test").unwrap(),
        Some("other"),
        &value
      )
      .err()
      .unwrap()
      .to_string(),
      concat!(
        "package 'test' did not have a bin entry for 'other' in its package.json\n",
        "\n",
        "Possibilities:\n",
        " * npm:test/bin1\n",
        " * npm:test/bin2\n",
        " * npm:test/test"
      )
    );

    // should not resolve when default value can't be determined
    assert_eq!(
      resolve_bin_entry_value(
        &NpmPackageReq::from_str("asdf@1.2").unwrap(),
        None,
        &value
      )
      .err()
      .unwrap()
      .to_string(),
      concat!(
        "package 'asdf' did not have a bin entry for 'asdf' in its package.json\n",
        "\n",
        "Possibilities:\n",
        " * npm:asdf@1.2/bin1\n",
        " * npm:asdf@1.2/bin2\n",
        " * npm:asdf@1.2/test"
      )
    );

    // should resolve since all the values are the same
    let value = json!({
      "bin1": "./value",
      "bin2": "./value",
    });
    assert_eq!(
      resolve_bin_entry_value(
        &NpmPackageReq::from_str("test").unwrap(),
        None,
        &value
      )
      .unwrap(),
      "./value"
    );

    // should not resolve when specified and is a string
    let value = json!("./value");
    assert_eq!(
      resolve_bin_entry_value(
        &NpmPackageReq::from_str("test").unwrap(),
        Some("path"),
        &value
      )
      .err()
      .unwrap()
      .to_string(),
      "package 'test' did not have a bin entry for 'path' in its package.json"
    );
  }
}
