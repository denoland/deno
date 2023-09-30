// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::futures::FutureExt;
use deno_core::ModuleSpecifier;
use deno_graph::source::NpmPackageReqResolution;
use deno_graph::source::NpmResolver;
use deno_graph::source::ResolveError;
use deno_graph::source::Resolver;
use deno_graph::source::UnknownBuiltInNodeModuleError;
use deno_graph::source::DEFAULT_JSX_IMPORT_SOURCE_MODULE;
use deno_runtime::deno_node::is_builtin_node_module;
use deno_semver::package::PackageReq;
use import_map::ImportMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::args::package_json::PackageJsonDeps;
use crate::args::JsxImportSourceConfig;
use crate::args::PackageJsonDepsProvider;
use crate::npm::CliNpmResolver;
use crate::util::sync::AtomicFlag;

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
    if let Some(Ok(value)) = self
      .maybe_import_map
      .as_ref()
      .map(|import_map| import_map.resolve(specifier, referrer))
    {
      return Ok(MappedResolution::ImportMap(value));
    };

    // then with package.json
    if let Some(deps) = self.package_json_deps_provider.deps() {
      if let Some(specifier) = resolve_package_json_dep(specifier, deps)? {
        return Ok(MappedResolution::PackageJson(specifier));
      }
    }

    Ok(MappedResolution::None)
  }
}

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug)]
pub struct CliGraphResolver {
  mapped_specifier_resolver: MappedSpecifierResolver,
  maybe_default_jsx_import_source: Option<String>,
  maybe_jsx_import_source_module: Option<String>,
  maybe_vendor_specifier: Option<ModuleSpecifier>,
  npm_resolver: Option<Arc<dyn CliNpmResolver>>,
  found_package_json_dep_flag: Arc<AtomicFlag>,
}

pub struct CliGraphResolverOptions<'a> {
  pub maybe_jsx_import_source_config: Option<JsxImportSourceConfig>,
  pub maybe_import_map: Option<Arc<ImportMap>>,
  pub maybe_vendor_dir: Option<&'a PathBuf>,
}

impl CliGraphResolver {
  pub fn new(
    npm_resolver: Option<Arc<dyn CliNpmResolver>>,
    package_json_deps_provider: Arc<PackageJsonDepsProvider>,
    options: CliGraphResolverOptions,
  ) -> Self {
    Self {
      mapped_specifier_resolver: MappedSpecifierResolver {
        maybe_import_map: options.maybe_import_map,
        package_json_deps_provider,
      },
      maybe_default_jsx_import_source: options
        .maybe_jsx_import_source_config
        .as_ref()
        .and_then(|c| c.default_specifier.clone()),
      maybe_jsx_import_source_module: options
        .maybe_jsx_import_source_config
        .map(|c| c.module),
      maybe_vendor_specifier: options
        .maybe_vendor_dir
        .and_then(|v| ModuleSpecifier::from_directory_path(v).ok()),
      npm_resolver,
      found_package_json_dep_flag: Default::default(),
    }
  }

  pub fn as_graph_resolver(&self) -> &dyn Resolver {
    self
  }

  pub fn as_graph_npm_resolver(&self) -> &dyn NpmResolver {
    self
  }

  pub fn found_package_json_dep(&self) -> bool {
    self.found_package_json_dep_flag.is_raised()
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
  ) -> Result<ModuleSpecifier, ResolveError> {
    use MappedResolution::*;
    let result = match self
      .mapped_specifier_resolver
      .resolve(specifier, referrer)?
    {
      ImportMap(specifier) => Ok(specifier),
      PackageJson(specifier) => {
        // found a specifier in the package.json, so mark that
        // we need to do an "npm install" later
        self.found_package_json_dep_flag.raise();
        Ok(specifier)
      }
      None => deno_graph::resolve_import(specifier, referrer)
        .map_err(|err| err.into()),
    };

    // When the user is vendoring, don't allow them to import directly from the vendor/ directory
    // as it might cause them confusion or duplicate dependencies. Additionally, this folder has
    // special treatment in the language server so it will definitely cause issues/confusion there
    // if they do this.
    if let Some(vendor_specifier) = &self.maybe_vendor_specifier {
      if let Ok(specifier) = &result {
        if specifier.as_str().starts_with(vendor_specifier.as_str()) {
          return Err(ResolveError::Other(anyhow!("Importing from the vendor directory is not permitted. Use a remote specifier instead or disable vendoring.")));
        }
      }
    }

    result
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

  fn on_resolve_bare_builtin_node_module(
    &self,
    module_name: &str,
    range: &deno_graph::Range,
  ) {
    let deno_graph::Range {
      start, specifier, ..
    } = range;
    let line = start.line + 1;
    let column = start.character + 1;
    log::warn!("Warning: Resolving \"{module_name}\" as \"node:{module_name}\" at {specifier}:{line}:{column}. If you want to use a built-in Node module, add a \"node:\" prefix.")
  }

  fn load_and_cache_npm_package_info(
    &self,
    package_name: &str,
  ) -> LocalBoxFuture<'static, Result<(), AnyError>> {
    match &self.npm_resolver {
      Some(npm_resolver) if npm_resolver.as_managed().is_some() => {
        let package_name = package_name.to_string();
        let npm_resolver = npm_resolver.clone();
        async move {
          if let Some(managed) = npm_resolver.as_managed() {
            managed.cache_package_info(&package_name).await?;
          }
          Ok(())
        }
        .boxed()
      }
      _ => {
        // return it succeeded and error at the import site below
        Box::pin(future::ready(Ok(())))
      }
    }
  }

  fn resolve_npm(&self, package_req: &PackageReq) -> NpmPackageReqResolution {
    match &self.npm_resolver {
      Some(npm_resolver) => {
        npm_resolver.resolve_npm_for_deno_graph(package_req)
      }
      None => NpmPackageReqResolution::Err(anyhow!(
        "npm specifiers were requested; but --no-npm is specified"
      )),
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
      deps: &BTreeMap<String, PackageReq>,
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
        PackageReq::from_str("package@1.0").unwrap(),
      ),
      (
        "package-alias".to_string(),
        PackageReq::from_str("package@^1.2").unwrap(),
      ),
      (
        "@deno/test".to_string(),
        PackageReq::from_str("@deno/test@~0.2").unwrap(),
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
