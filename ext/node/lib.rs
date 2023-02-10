// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::located_script_name;
use deno_core::Extension;
use deno_core::JsRuntime;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

pub mod errors;
mod ops;
mod package_json;
mod path;
mod polyfill;
mod resolution;

pub use package_json::PackageJson;
pub use path::PathClean;
pub use polyfill::find_builtin_node_module;
pub use polyfill::is_builtin_node_module;
pub use polyfill::NodeModulePolyfill;
pub use polyfill::NodeModulePolyfillSpecifier;
pub use polyfill::SUPPORTED_BUILTIN_NODE_MODULES;
pub use resolution::get_closest_package_json;
pub use resolution::get_package_scope_config;
pub use resolution::legacy_main_resolve;
pub use resolution::package_exports_resolve;
pub use resolution::package_imports_resolve;
pub use resolution::package_resolve;
pub use resolution::path_to_declaration_path;
pub use resolution::NodeModuleKind;
pub use resolution::NodeResolutionMode;
pub use resolution::DEFAULT_CONDITIONS;

pub trait NodePermissions {
  fn check_read(&mut self, path: &Path) -> Result<(), AnyError>;
}

pub trait RequireNpmResolver {
  fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &Path,
    mode: NodeResolutionMode,
  ) -> Result<PathBuf, AnyError>;

  fn resolve_package_folder_from_path(
    &self,
    path: &Path,
  ) -> Result<PathBuf, AnyError>;

  fn in_npm_package(&self, path: &Path) -> bool;

  fn ensure_read_permission(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &Path,
  ) -> Result<(), AnyError>;
}

pub const MODULE_ES_SHIM: &str = include_str!("./module_es_shim.js");

pub static NODE_GLOBAL_THIS_NAME: Lazy<String> = Lazy::new(|| {
  let now = std::time::SystemTime::now();
  let seconds = now
    .duration_since(std::time::SystemTime::UNIX_EPOCH)
    .unwrap()
    .as_secs();
  // use a changing variable name to make it hard to depend on this
  format!("__DENO_NODE_GLOBAL_THIS_{seconds}__")
});

pub static NODE_ENV_VAR_ALLOWLIST: Lazy<HashSet<String>> = Lazy::new(|| {
  // The full list of environment variables supported by Node.js is available
  // at https://nodejs.org/api/cli.html#environment-variables
  let mut set = HashSet::new();
  set.insert("NODE_DEBUG".to_string());
  set.insert("NODE_OPTIONS".to_string());
  set
});

pub fn init<P: NodePermissions + 'static>(
  maybe_npm_resolver: Option<Rc<dyn RequireNpmResolver>>,
) -> Extension {
  Extension::builder(env!("CARGO_PKG_NAME"))
    .esm(include_js_files!(
      "01_node.js",
      "02_require.js",
      "module_es_shim.js",
    ))
    .ops(vec![
      ops::op_require_init_paths::decl(),
      ops::op_require_node_module_paths::decl::<P>(),
      ops::op_require_proxy_path::decl(),
      ops::op_require_is_deno_dir_package::decl(),
      ops::op_require_resolve_deno_dir::decl(),
      ops::op_require_is_request_relative::decl(),
      ops::op_require_resolve_lookup_paths::decl(),
      ops::op_require_try_self_parent_path::decl::<P>(),
      ops::op_require_try_self::decl::<P>(),
      ops::op_require_real_path::decl::<P>(),
      ops::op_require_path_is_absolute::decl(),
      ops::op_require_path_dirname::decl(),
      ops::op_require_stat::decl::<P>(),
      ops::op_require_path_resolve::decl(),
      ops::op_require_path_basename::decl(),
      ops::op_require_read_file::decl::<P>(),
      ops::op_require_as_file_path::decl(),
      ops::op_require_resolve_exports::decl::<P>(),
      ops::op_require_read_closest_package_json::decl::<P>(),
      ops::op_require_read_package_scope::decl::<P>(),
      ops::op_require_package_imports_resolve::decl::<P>(),
      ops::op_require_break_on_next_statement::decl(),
    ])
    .state(move |state| {
      if let Some(npm_resolver) = maybe_npm_resolver.clone() {
        state.put(npm_resolver);
      }
      Ok(())
    })
    .build()
}

pub async fn initialize_runtime(
  js_runtime: &mut JsRuntime,
  module_all_url: &str,
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
    module_all_url,
    NODE_GLOBAL_THIS_NAME.as_str(),
    uses_local_node_modules_dir,
  );

  let value =
    js_runtime.execute_script(&located_script_name!(), source_code)?;
  js_runtime.resolve_value(value).await?;
  Ok(())
}

pub fn load_cjs_module(
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
    }})('{binary_name}');"#,
  );

  let value =
    js_runtime.execute_script(&located_script_name!(), source_code)?;
  js_runtime.resolve_value(value).await?;
  Ok(())
}
