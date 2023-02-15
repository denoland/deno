// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_graph::source::Resolver;
use deno_graph::source::DEFAULT_JSX_IMPORT_SOURCE_MODULE;
use import_map::ImportMap;
use std::sync::Arc;

use crate::args::JsxImportSourceConfig;

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug, Clone, Default)]
pub struct CliGraphResolver {
  maybe_import_map: Option<Arc<ImportMap>>,
  maybe_default_jsx_import_source: Option<String>,
  maybe_jsx_import_source_module: Option<String>,
}

impl CliGraphResolver {
  pub fn new(
    maybe_jsx_import_source_config: Option<JsxImportSourceConfig>,
    maybe_import_map: Option<Arc<ImportMap>>,
  ) -> Self {
    Self {
      maybe_import_map,
      maybe_default_jsx_import_source: maybe_jsx_import_source_config
        .as_ref()
        .and_then(|c| c.default_specifier.clone()),
      maybe_jsx_import_source_module: maybe_jsx_import_source_config
        .map(|c| c.module),
    }
  }

  pub fn as_graph_resolver(&self) -> &dyn Resolver {
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
