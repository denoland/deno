// Copyright 2018-2026 the Deno authors. MIT license.

//! Support for the DOM test environment (`deno test --dom`).
//!
//! When enabled, every test worker evaluates a generated preload module that
//! imports a DOM library from npm (happy-dom or jsdom), creates a window and
//! populates `globalThis` with the DOM globals (`document`, `window`,
//! `HTMLElement`, ...). The DOM library version is built into Deno, but a
//! version declared in the workspace import map or package.json takes
//! precedence.

use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use deno_config::deno_json::TestDomLibrary;
use deno_config::workspace::Workspace;
use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_semver::package::PackageReq;

/// Default version of the happy-dom npm package used when the workspace
/// doesn't declare one.
pub const DEFAULT_HAPPY_DOM_VERSION: &str = "20.10.2";
/// Default version of the jsdom npm package used when the workspace doesn't
/// declare one.
pub const DEFAULT_JSDOM_VERSION: &str = "29.1.1";

const DOM_SETUP_TEMPLATE: &str = include_str!("dom_setup.js");

pub struct DomSetupModule {
  /// Data URL of the generated setup module, evaluated as a preload module
  /// in every test worker.
  pub module_url: Url,
  /// The baked-in default package req that needs to be installed before
  /// running tests. `None` when the workspace declares the package itself.
  pub default_package_req: Option<PackageReq>,
}

pub fn resolve_dom_setup_module(
  workspace: &Workspace,
  lib: TestDomLibrary,
) -> Result<DomSetupModule, AnyError> {
  let package_name = lib.package_name();
  let (import_specifier, default_package_req) =
    match find_user_dom_specifier(workspace, package_name) {
      Some(specifier) => (specifier, None),
      None => {
        let default_version = match lib {
          TestDomLibrary::HappyDom => DEFAULT_HAPPY_DOM_VERSION,
          TestDomLibrary::Jsdom => DEFAULT_JSDOM_VERSION,
        };
        let req_str = format!("{}@{}", package_name, default_version);
        let req = PackageReq::from_str(&req_str)
          .with_context(|| format!("Invalid package req '{}'", req_str))?;
        (format!("npm:{}", req_str), Some(req))
      }
    };
  if import_specifier.contains('"')
    || import_specifier.contains('\\')
    || import_specifier.contains('\n')
  {
    bail!(
      "Cannot use '{}' as the {} specifier for the DOM test environment",
      import_specifier,
      package_name
    );
  }
  let code = DOM_SETUP_TEMPLATE
    .replace("__DENO_TEST_DOM_PACKAGE__", &import_specifier)
    .replace("__DENO_TEST_DOM_LIB__", package_name);
  let module_url = Url::parse(&format!(
    "data:application/javascript;base64,{}",
    BASE64_STANDARD.encode(code)
  ))?;
  Ok(DomSetupModule {
    module_url,
    default_package_req,
  })
}

/// Returns the import specifier to use for the DOM library when the
/// workspace declares it itself, either in a deno.json import map or as a
/// package.json dependency.
fn find_user_dom_specifier(
  workspace: &Workspace,
  package_name: &str,
) -> Option<String> {
  // import map entries take precedence over package.json dependencies,
  // matching the workspace resolution order for bare specifiers
  for deno_json in workspace.deno_jsons() {
    if let Some(serde_json::Value::Object(imports)) = &deno_json.json.imports
      && let Some(serde_json::Value::String(value)) = imports.get(package_name)
    {
      // resolve relative entries against the config file like an import map
      if Url::parse(value).is_ok() {
        return Some(value.clone());
      } else if let Ok(url) = deno_json.specifier.join(value) {
        return Some(url.to_string());
      }
    }
  }
  for pkg_json in workspace.package_jsons() {
    for deps in [&pkg_json.dependencies, &pkg_json.dev_dependencies] {
      if let Some(version) = deps.as_ref().and_then(|d| d.get(package_name)) {
        return Some(if version.starts_with("npm:") {
          // aliased dependency, e.g. "happy-dom": "npm:happy-dom@20"
          version.clone()
        } else if version_req_looks_like_npm_version(version) {
          format!("npm:{}@{}", package_name, version)
        } else {
          // file:, link:, workspace:, git: etc. - let the package itself
          // resolve to whatever is installed
          format!("npm:{}", package_name)
        });
      }
    }
  }
  None
}

fn version_req_looks_like_npm_version(version: &str) -> bool {
  PackageReq::from_str(&format!("package@{}", version)).is_ok()
}
