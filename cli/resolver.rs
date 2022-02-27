// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::resolve_import;
use deno_core::ModuleSpecifier;
use deno_graph::source::ResolveResponse;
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
  ) -> ResolveResponse {
    match self.0.resolve(specifier, referrer) {
      Ok(specifier) => ResolveResponse::Specifier(specifier),
      Err(err) => ResolveResponse::Err(err.into()),
    }
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
  ) -> ResolveResponse {
    self.maybe_import_map_resolver.as_ref().map_or_else(
      || match resolve_import(specifier, referrer.as_str()) {
        Ok(specifier) => ResolveResponse::Specifier(specifier),
        Err(err) => ResolveResponse::Err(err.into()),
      },
      |r| r.resolve(specifier, referrer),
    )
  }
}
