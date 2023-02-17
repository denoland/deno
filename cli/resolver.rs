// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;
use deno_core::ModuleSpecifier;
use deno_graph::npm::NpmPackageId;
use deno_graph::npm::NpmPackageReq;
use deno_graph::source::NpmResolver;
use deno_graph::source::Resolver;
use deno_graph::source::DEFAULT_JSX_IMPORT_SOURCE_MODULE;
use deno_runtime::deno_node::is_builtin_node_module;
use import_map::ImportMap;
use std::sync::Arc;

use crate::args::JsxImportSourceConfig;
use crate::npm::NpmRegistryApi;
use crate::npm::NpmResolution;

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug, Clone)]
pub struct CliGraphResolver {
  maybe_import_map: Option<Arc<ImportMap>>,
  maybe_default_jsx_import_source: Option<String>,
  maybe_jsx_import_source_module: Option<String>,
  npm_registry_api: NpmRegistryApi,
  npm_resolution: NpmResolution,
}

impl Default for CliGraphResolver {
  fn default() -> Self {
    // This is not ideal, but necessary for the LSP. In the future, we should
    // refactor the LSP and force this to be initialized.
    let npm_registry_api = NpmRegistryApi::new_uninitialized();
    let npm_resolution = NpmResolution::new(npm_registry_api.clone(), None);
    Self {
      maybe_import_map: Default::default(),
      maybe_default_jsx_import_source: Default::default(),
      maybe_jsx_import_source_module: Default::default(),
      npm_registry_api,
      npm_resolution,
    }
  }
}

impl CliGraphResolver {
  pub fn new(
    maybe_jsx_import_source_config: Option<JsxImportSourceConfig>,
    maybe_import_map: Option<Arc<ImportMap>>,
    npm_registry_api: NpmRegistryApi,
    npm_resolution: NpmResolution,
  ) -> Self {
    Self {
      maybe_import_map,
      maybe_default_jsx_import_source: maybe_jsx_import_source_config
        .as_ref()
        .and_then(|c| c.default_specifier.clone()),
      maybe_jsx_import_source_module: maybe_jsx_import_source_config
        .map(|c| c.module),
      npm_registry_api,
      npm_resolution,
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
      import_map
        .resolve(specifier, referrer)
        .map_err(|err| err.into())
    } else {
      deno_graph::resolve_import(specifier, referrer).map_err(|err| err.into())
    }
  }
}

impl NpmResolver for CliGraphResolver {
  fn supports_node_specifiers(&self) -> bool {
    true
  }

  fn is_builtin_node_module(&self, module_name: &str) -> bool {
    is_builtin_node_module(module_name)
  }

  fn load_and_cache_npm_package_info(
    &self,
    package_name: String,
  ) -> BoxFuture<'static, Result<(), String>> {
    // this will internally cache the package information
    let api = self.npm_registry_api.clone();
    async move {
      api
        .package_info(&package_name)
        .await
        .map(|_| ())
        .map_err(|err| format!("{err:#}"))
    }
    .boxed()
  }

  fn resolve_npm(
    &self,
    package_req: &NpmPackageReq,
  ) -> Result<NpmPackageId, AnyError> {
    self
      .npm_resolution
      .resolve_deno_graph_package_req(package_req)
  }
}
