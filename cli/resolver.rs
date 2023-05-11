// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::anyhow;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::futures::FutureExt;
use deno_core::ModuleSpecifier;
use deno_core::TaskQueue;
use deno_graph::source::NpmPackageReqResolution;
use deno_graph::source::NpmResolver;
use deno_graph::source::Resolver;
use deno_graph::source::UnknownBuiltInNodeModuleError;
use deno_graph::source::DEFAULT_JSX_IMPORT_SOURCE_MODULE;
use deno_npm::registry::NpmRegistryApi;
use deno_runtime::deno_node::is_builtin_node_module;
use deno_semver::npm::NpmPackageReq;
use import_map::ImportMap;
use std::sync::Arc;

use crate::args::package_json::PackageJsonDeps;
use crate::args::JsxImportSourceConfig;
use crate::args::PackageJsonDepsProvider;
use crate::npm::CliNpmRegistryApi;
use crate::npm::NpmResolution;
use crate::npm::PackageJsonDepsInstaller;
use crate::util::sync::AtomicFlag;

/// Check that a resolved specifier isn't an `ext:` URL. Normally scheme checks
/// are done on load rather than resolve. This is needed because `ext:` modules
/// are preloaded in deno_core and won't be requested from the embedder, so we
/// need to catch them on resolution instead.
///
/// TODO(nayeemrmn): Maybe use a separate module map for `ext:` modules to avoid
/// this problem. As of writing this is blocked by the way `node:` specifiers
/// are implemented.
pub fn validate_scheme_for_resolution(
  specifier: &ModuleSpecifier,
) -> Result<(), AnyError> {
  if specifier.scheme() == "ext" {
    return Err(type_error(
      "Cannot load extension module from external code",
    ));
  }
  Ok(())
}

/// Result of checking if a specifier is mapped via
/// an import map or package.json.
pub enum MappedResolution {
  None,
  PackageJson(ModuleSpecifier),
  ImportMap(ModuleSpecifier),
}

impl MappedResolution {
  pub fn into_specifier(self) -> Option<ModuleSpecifier> {
    match self {
      MappedResolution::None => Option::None,
      MappedResolution::PackageJson(specifier) => Some(specifier),
      MappedResolution::ImportMap(specifier) => Some(specifier),
    }
  }
}

/// Resolver for specifiers that could be mapped via an
/// import map or package.json.
#[derive(Debug)]
pub struct MappedSpecifierResolver {
  maybe_import_map: Option<Arc<ImportMap>>,
  package_json_deps_provider: Arc<PackageJsonDepsProvider>,
}

impl MappedSpecifierResolver {
  pub fn new(
    maybe_import_map: Option<Arc<ImportMap>>,
    package_json_deps_provider: Arc<PackageJsonDepsProvider>,
  ) -> Self {
    Self {
      maybe_import_map,
      package_json_deps_provider,
    }
  }

  pub fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<MappedResolution, AnyError> {
    // attempt to resolve with the import map first
    let maybe_import_map_err = match self
      .maybe_import_map
      .as_ref()
      .map(|import_map| import_map.resolve(specifier, referrer))
    {
      Some(Ok(value)) => return Ok(MappedResolution::ImportMap(value)),
      Some(Err(err)) => Some(err),
      None => None,
    };

    // then with package.json
    if let Some(deps) = self.package_json_deps_provider.deps() {
      if let Some(specifier) = resolve_package_json_dep(specifier, deps)? {
        return Ok(MappedResolution::PackageJson(specifier));
      }
    }

    // otherwise, surface the import map error or try resolving when has no import map
    if let Some(err) = maybe_import_map_err {
      Err(err.into())
    } else {
      Ok(MappedResolution::None)
    }
  }
}

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug)]
pub struct CliGraphResolver {
  mapped_specifier_resolver: MappedSpecifierResolver,
  maybe_default_jsx_import_source: Option<String>,
  maybe_jsx_import_source_module: Option<String>,
  no_npm: bool,
  npm_registry_api: Arc<CliNpmRegistryApi>,
  npm_resolution: Arc<NpmResolution>,
  package_json_deps_installer: Arc<PackageJsonDepsInstaller>,
  found_package_json_dep_flag: Arc<AtomicFlag>,
  sync_download_queue: Option<Arc<TaskQueue>>,
}

impl Default for CliGraphResolver {
  fn default() -> Self {
    // This is not ideal, but necessary for the LSP. In the future, we should
    // refactor the LSP and force this to be initialized.
    let npm_registry_api = Arc::new(CliNpmRegistryApi::new_uninitialized());
    let npm_resolution = Arc::new(NpmResolution::from_serialized(
      npm_registry_api.clone(),
      None,
      None,
    ));
    Self {
      mapped_specifier_resolver: MappedSpecifierResolver {
        maybe_import_map: Default::default(),
        package_json_deps_provider: Default::default(),
      },
      maybe_default_jsx_import_source: Default::default(),
      maybe_jsx_import_source_module: Default::default(),
      no_npm: false,
      npm_registry_api,
      npm_resolution,
      package_json_deps_installer: Default::default(),
      found_package_json_dep_flag: Default::default(),
      sync_download_queue: Self::create_sync_download_queue(),
    }
  }
}

impl CliGraphResolver {
  pub fn new(
    maybe_jsx_import_source_config: Option<JsxImportSourceConfig>,
    maybe_import_map: Option<Arc<ImportMap>>,
    no_npm: bool,
    npm_registry_api: Arc<CliNpmRegistryApi>,
    npm_resolution: Arc<NpmResolution>,
    package_json_deps_provider: Arc<PackageJsonDepsProvider>,
    package_json_deps_installer: Arc<PackageJsonDepsInstaller>,
  ) -> Self {
    Self {
      mapped_specifier_resolver: MappedSpecifierResolver {
        maybe_import_map,
        package_json_deps_provider,
      },
      maybe_default_jsx_import_source: maybe_jsx_import_source_config
        .as_ref()
        .and_then(|c| c.default_specifier.clone()),
      maybe_jsx_import_source_module: maybe_jsx_import_source_config
        .map(|c| c.module),
      no_npm,
      npm_registry_api,
      npm_resolution,
      package_json_deps_installer,
      found_package_json_dep_flag: Default::default(),
      sync_download_queue: Self::create_sync_download_queue(),
    }
  }

  fn create_sync_download_queue() -> Option<Arc<TaskQueue>> {
    if crate::npm::should_sync_download() {
      Some(Default::default())
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

  pub async fn top_level_package_json_install_if_necessary(
    &self,
  ) -> Result<(), AnyError> {
    if self.found_package_json_dep_flag.is_raised() {
      self
        .package_json_deps_installer
        .ensure_top_level_install()
        .await?;
    }
    Ok(())
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
    use MappedResolution::*;
    match self
      .mapped_specifier_resolver
      .resolve(specifier, referrer)?
    {
      ImportMap(specifier) => {
        validate_scheme_for_resolution(&specifier)?;
        Ok(specifier)
      }
      PackageJson(specifier) => {
        // found a specifier in the package.json, so mark that
        // we need to do an "npm install" later
        self.found_package_json_dep_flag.raise();
        Ok(specifier)
      }
      None => match deno_graph::resolve_import(specifier, referrer) {
        Ok(specifier) => {
          validate_scheme_for_resolution(&specifier)?;
          Ok(specifier)
        }
        Err(err) => Err(err.into()),
      },
    }
  }
}

fn resolve_package_json_dep(
  specifier: &str,
  deps: &PackageJsonDeps,
) -> Result<Option<ModuleSpecifier>, AnyError> {
  for (bare_specifier, req_result) in deps {
    if specifier.starts_with(bare_specifier) {
      let path = &specifier[bare_specifier.len()..];
      if path.is_empty() || path.starts_with('/') {
        let req = req_result.as_ref().map_err(|err| {
          anyhow!(
            "Parsing version constraints in the application-level package.json is more strict at the moment.\n\n{:#}",
            err.clone()
          )
        })?;
        return Ok(Some(ModuleSpecifier::parse(&format!("npm:{req}{path}"))?));
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
  ) -> LocalBoxFuture<'static, Result<(), AnyError>> {
    if self.no_npm {
      // return it succeeded and error at the import site below
      return Box::pin(future::ready(Ok(())));
    }
    // this will internally cache the package information
    let package_name = package_name.to_string();
    let api = self.npm_registry_api.clone();
    let maybe_sync_download_queue = self.sync_download_queue.clone();
    async move {
      let permit = if let Some(task_queue) = &maybe_sync_download_queue {
        Some(task_queue.acquire().await)
      } else {
        None
      };

      let result = api
        .package_info(&package_name)
        .await
        .map(|_| ())
        .map_err(|err| err.into());
      drop(permit);
      result
    }
    .boxed()
  }

  fn resolve_npm(
    &self,
    package_req: &NpmPackageReq,
  ) -> NpmPackageReqResolution {
    if self.no_npm {
      return NpmPackageReqResolution::Err(anyhow!(
        "npm specifiers were requested; but --no-npm is specified"
      ));
    }

    let result = self
      .npm_resolution
      .resolve_package_req_as_pending(package_req);
    match result {
      Ok(nv) => NpmPackageReqResolution::Ok(nv),
      Err(err) => {
        if self.npm_registry_api.mark_force_reload() {
          log::debug!("Restarting npm specifier resolution to check for new registry information. Error: {:#}", err);
          NpmPackageReqResolution::ReloadRegistryInfo(err.into())
        } else {
          NpmPackageReqResolution::Err(err.into())
        }
      }
    }
  }
}

#[cfg(test)]
mod test {
  use std::collections::BTreeMap;

  use super::*;

  #[test]
  fn test_resolve_package_json_dep() {
    fn resolve(
      specifier: &str,
      deps: &BTreeMap<String, NpmPackageReq>,
    ) -> Result<Option<String>, String> {
      let deps = deps
        .iter()
        .map(|(key, value)| (key.to_string(), Ok(value.clone())))
        .collect();
      resolve_package_json_dep(specifier, &deps)
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
      Some("npm:package@1.0/some_path.ts".to_string()),
    );

    assert_eq!(
      resolve("@deno/test", &deps).unwrap(),
      Some("npm:@deno/test@~0.2".to_string()),
    );
    assert_eq!(
      resolve("@deno/test/some_path.ts", &deps).unwrap(),
      Some("npm:@deno/test@~0.2/some_path.ts".to_string()),
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
