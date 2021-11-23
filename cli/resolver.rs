// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::resolve_import;
use deno_core::ModuleSpecifier;
use deno_graph::source::Resolver;
use import_map::ImportMap;
use std::sync::Arc;

/// Wraps an import map to be used when building a deno_graph module graph.
/// This is done to avoid having `import_map` be a direct dependency of
/// `deno_graph`.
#[derive(Debug, Clone)]
pub(crate) struct ImportMapResolver(Arc<ImportMap>);

impl ImportMapResolver {
  pub fn new(import_map: Arc<ImportMap>) -> Self {
    Self(import_map)
  }

  pub fn as_resolver(&self) -> &dyn Resolver {
    self
  }
}

impl Resolver for ImportMapResolver {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, AnyError> {
    self
      .0
      .resolve(specifier, referrer.as_str())
      .map_err(|err| err.into())
  }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct JsxResolver {
  jsx_import_source_module: String,
  maybe_import_map_resolver: Option<ImportMapResolver>,
}

impl JsxResolver {
  pub fn new(
    jsx_import_source_module: String,
    maybe_import_map_resolver: Option<ImportMapResolver>,
  ) -> Self {
    Self {
      jsx_import_source_module,
      maybe_import_map_resolver,
    }
  }

  pub fn as_resolver(&self) -> &dyn Resolver {
    self
  }
}

impl Resolver for JsxResolver {
  fn jsx_import_source_module(&self) -> &str {
    self.jsx_import_source_module.as_str()
  }

  fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, AnyError> {
    self.maybe_import_map_resolver.as_ref().map_or_else(
      || resolve_import(specifier, referrer.as_str()).map_err(|err| err.into()),
      |r| r.resolve(specifier, referrer),
    )
  }
}
