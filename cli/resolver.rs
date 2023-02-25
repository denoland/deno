// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::futures::FutureExt;
use deno_core::ModuleSpecifier;
use deno_graph::npm::NpmPackageNv;
use deno_graph::npm::NpmPackageReq;
use deno_graph::source::NpmResolver;
use deno_graph::source::Resolver;
use deno_graph::source::UnknownBuiltInNodeModuleError;
use deno_graph::source::DEFAULT_JSX_IMPORT_SOURCE_MODULE;
use deno_runtime::deno_node::is_builtin_node_module;
use import_map::ImportMap;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::args::JsxImportSourceConfig;
use crate::npm::NpmRegistryApi;
use crate::npm::NpmResolution;
use crate::npm::PackageJsonDepsInstaller;

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug, Clone)]
pub struct CliGraphResolver {
  maybe_import_map: Option<Arc<ImportMap>>,
  maybe_default_jsx_import_source: Option<String>,
  maybe_jsx_import_source_module: Option<String>,
  no_npm: bool,
  npm_registry_api: NpmRegistryApi,
  npm_resolution: NpmResolution,
  package_json_deps_installer: PackageJsonDepsInstaller,
  sync_download_semaphore: Option<Arc<tokio::sync::Semaphore>>,
}

impl Default for CliGraphResolver {
  fn default() -> Self {
    // This is not ideal, but necessary for the LSP. In the future, we should
    // refactor the LSP and force this to be initialized.
    let npm_registry_api = NpmRegistryApi::new_uninitialized();
    let npm_resolution =
      NpmResolution::new(npm_registry_api.clone(), None, None);
    Self {
      maybe_import_map: Default::default(),
      maybe_default_jsx_import_source: Default::default(),
      maybe_jsx_import_source_module: Default::default(),
      no_npm: false,
      npm_registry_api,
      npm_resolution,
      package_json_deps_installer: Default::default(),
      sync_download_semaphore: Self::create_sync_download_semaphore(),
    }
  }
}

impl CliGraphResolver {
  pub fn new(
    maybe_jsx_import_source_config: Option<JsxImportSourceConfig>,
    maybe_import_map: Option<Arc<ImportMap>>,
    no_npm: bool,
    npm_registry_api: NpmRegistryApi,
    npm_resolution: NpmResolution,
    package_json_deps_installer: PackageJsonDepsInstaller,
  ) -> Self {
    Self {
      maybe_import_map,
      maybe_default_jsx_import_source: maybe_jsx_import_source_config
        .as_ref()
        .and_then(|c| c.default_specifier.clone()),
      maybe_jsx_import_source_module: maybe_jsx_import_source_config
        .map(|c| c.module),
      no_npm,
      npm_registry_api,
      npm_resolution,
      package_json_deps_installer,
      sync_download_semaphore: Self::create_sync_download_semaphore(),
    }
  }

  fn create_sync_download_semaphore() -> Option<Arc<tokio::sync::Semaphore>> {
    if crate::npm::should_sync_download() {
      Some(Arc::new(tokio::sync::Semaphore::new(1)))
    } else {
      None
    }
  }

  pub fn as_graph_resolver(&self) -> &dyn Resolver {
    self
  }

  pub fn as_graph_npm_resolver(&self) -> &dyn NpmResolver {
    self
  }
}

impl Resolver for CliGraphResolver {
  fn default_jsx_import_source(&self) -> Option<String> {
    self.maybe_default_jsx_import_source.clone()
  }

  fn jsx_import_source_module(&self) -> &str {
    self
      .maybe_jsx_import_source_module
      .as_deref()
      .unwrap_or(DEFAULT_JSX_IMPORT_SOURCE_MODULE)
  }

  fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, AnyError> {
    // attempt to resolve with the import map first
    let maybe_import_map_err = match self
      .maybe_import_map
      .as_ref()
      .map(|import_map| import_map.resolve(specifier, referrer))
    {
      Some(Ok(value)) => return Ok(value),
      Some(Err(err)) => Some(err),
      None => None,
    };

    // then with package.json
    if let Some(deps) = self.package_json_deps_installer.package_deps().as_ref()
    {
      if let Some(specifier) = resolve_package_json_dep(specifier, deps)? {
        return Ok(specifier);
      }
    }

    // otherwise, surface the import map error or try resolving when has no import map
    if let Some(err) = maybe_import_map_err {
      Err(err.into())
    } else {
      deno_graph::resolve_import(specifier, referrer).map_err(|err| err.into())
    }
  }
}

fn resolve_package_json_dep(
  specifier: &str,
  deps: &BTreeMap<String, NpmPackageReq>,
) -> Result<Option<ModuleSpecifier>, deno_core::url::ParseError> {
  for (bare_specifier, req) in deps {
    if specifier.starts_with(bare_specifier) {
      if specifier.len() == bare_specifier.len() {
        return ModuleSpecifier::parse(&format!("npm:{req}")).map(Some);
      }
      let path = &specifier[bare_specifier.len()..];
      if path.starts_with('/') {
        return ModuleSpecifier::parse(&format!("npm:/{req}{path}")).map(Some);
      }
    }
  }

  Ok(None)
}

impl NpmResolver for CliGraphResolver {
  fn resolve_builtin_node_module(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<String>, UnknownBuiltInNodeModuleError> {
    if specifier.scheme() != "node" {
      return Ok(None);
    }

    let module_name = specifier.path().to_string();
    if is_builtin_node_module(&module_name) {
      Ok(Some(module_name))
    } else {
      Err(UnknownBuiltInNodeModuleError { module_name })
    }
  }

  fn load_and_cache_npm_package_info(
    &self,
    package_name: &str,
  ) -> LocalBoxFuture<'static, Result<(), String>> {
    if self.no_npm {
      // return it succeeded and error at the import site below
      return Box::pin(future::ready(Ok(())));
    }
    // this will internally cache the package information
    let package_name = package_name.to_string();
    let api = self.npm_registry_api.clone();
    let deps_installer = self.package_json_deps_installer.clone();
    let maybe_sync_download_semaphore = self.sync_download_semaphore.clone();
    async move {
      let permit = if let Some(semaphore) = &maybe_sync_download_semaphore {
        Some(semaphore.acquire().await.unwrap())
      } else {
        None
      };

      // trigger an npm install if the package name matches
      // a package in the package.json
      //
      // todo(dsherret): ideally this would only download if a bare
      // specifiy matched in the package.json, but deno_graph only
      // calls this once per package name and we might resolve an
      // npm specifier first which calls this, then a bare specifier
      // second and that would cause this not to occur.
      if deps_installer.has_package_name(&package_name) {
        deps_installer
          .ensure_top_level_install()
          .await
          .map_err(|err| format!("{err:#}"))?;
      }

      let result = api
        .package_info(&package_name)
        .await
        .map(|_| ())
        .map_err(|err| format!("{err:#}"));
      drop(permit);
      result
    }
    .boxed()
  }

  fn resolve_npm(
    &self,
    package_req: &NpmPackageReq,
  ) -> Result<NpmPackageNv, AnyError> {
    if self.no_npm {
      bail!("npm specifiers were requested; but --no-npm is specified")
    }
    self
      .npm_resolution
      .resolve_package_req_as_pending(package_req)
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_resolve_package_json_dep() {
    fn resolve(
      specifier: &str,
      deps: &BTreeMap<String, NpmPackageReq>,
    ) -> Result<Option<String>, String> {
      resolve_package_json_dep(specifier, deps)
        .map(|s| s.map(|s| s.to_string()))
        .map_err(|err| err.to_string())
    }

    let deps = BTreeMap::from([
      (
        "package".to_string(),
        NpmPackageReq::from_str("package@1.0").unwrap(),
      ),
      (
        "package-alias".to_string(),
        NpmPackageReq::from_str("package@^1.2").unwrap(),
      ),
      (
        "@deno/test".to_string(),
        NpmPackageReq::from_str("@deno/test@~0.2").unwrap(),
      ),
    ]);

    assert_eq!(
      resolve("package", &deps).unwrap(),
      Some("npm:package@1.0".to_string()),
    );
    assert_eq!(
      resolve("package/some_path.ts", &deps).unwrap(),
      Some("npm:/package@1.0/some_path.ts".to_string()),
    );

    assert_eq!(
      resolve("@deno/test", &deps).unwrap(),
      Some("npm:@deno/test@~0.2".to_string()),
    );
    assert_eq!(
      resolve("@deno/test/some_path.ts", &deps).unwrap(),
      Some("npm:/@deno/test@~0.2/some_path.ts".to_string()),
    );
    // matches the start, but doesn't have the same length or a path
    assert_eq!(resolve("@deno/testing", &deps).unwrap(), None,);

    // alias
    assert_eq!(
      resolve("package-alias", &deps).unwrap(),
      Some("npm:package@^1.2".to_string()),
    );

    // non-existent bare specifier
    assert_eq!(resolve("non-existent", &deps).unwrap(), None);
  }
}
