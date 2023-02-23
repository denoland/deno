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
use std::collections::HashMap;
use std::sync::Arc;

use crate::args::JsxImportSourceConfig;
use crate::npm::NpmRegistryApi;
use crate::npm::NpmResolution;

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug, Clone)]
pub struct CliGraphResolver {
  maybe_import_map: Option<Arc<ImportMap>>,
  maybe_package_json_deps: Option<HashMap<String, NpmPackageReq>>,
  maybe_default_jsx_import_source: Option<String>,
  maybe_jsx_import_source_module: Option<String>,
  no_npm: bool,
  npm_registry_api: NpmRegistryApi,
  npm_resolution: NpmResolution,
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
      maybe_package_json_deps: Default::default(),
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
    maybe_package_json_deps: Option<HashMap<String, NpmPackageReq>>,
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
      maybe_package_json_deps,
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
    if let Some(import_map) = &self.maybe_import_map {
      return import_map
        .resolve(specifier, referrer)
        .map_err(|err| err.into());
    }

    if let Some(deps) = self.maybe_package_json_deps.as_ref() {
      if let Some(req) = deps.get(specifier) {
        return Ok(ModuleSpecifier::parse(&format!("npm:{req}")).unwrap());
      }
    }

    deno_graph::resolve_import(specifier, referrer).map_err(|err| err.into())
  }
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
    let mut maybe_sync_download_semaphore =
      self.sync_download_semaphore.clone();
    async move {
      let result = if let Some(semaphore) = maybe_sync_download_semaphore.take()
      {
        let _permit = semaphore.acquire().await.unwrap();
        api.package_info(&package_name).await
      } else {
        api.package_info(&package_name).await
      };
      result.map(|_| ()).map_err(|err| format!("{err:#}"))
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
      .resolve_package_req_for_deno_graph(package_req)
  }
}
