// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::resolve_import;
use deno_core::ModuleSpecifier;
use deno_graph::source::Resolver;
use import_map::ImportMap;

/// Wraps an import map to be used when building a deno_graph module graph.
/// This is done to avoid having `import_map` be a direct dependency of
/// `deno_graph`.
#[derive(Debug)]
pub(crate) struct ImportMapResolver<'a>(&'a ImportMap);

impl<'a> ImportMapResolver<'a> {
  pub fn new(import_map: &'a ImportMap) -> Self {
    Self(import_map)
  }

  pub fn as_resolver(&'a self) -> &'a dyn Resolver {
    self
  }
}

impl Resolver for ImportMapResolver<'_> {
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

#[derive(Debug)]
pub(crate) struct JsxResolver<'a> {
  jsx_import_source_module: String,
  maybe_import_map_resolver: Option<&'a ImportMapResolver<'a>>,
}

impl<'a> JsxResolver<'a> {
  pub fn new(
    jsx_import_source_module: String,
    maybe_import_map_resolver: Option<&'a ImportMapResolver<'a>>,
  ) -> Self {
    Self {
      jsx_import_source_module,
      maybe_import_map_resolver,
    }
  }

  pub fn as_resolver(&'a self) -> &'a dyn Resolver {
    self
  }
}

impl Resolver for JsxResolver<'_> {
  fn jsx_import_source_module(&self) -> &str {
    self.jsx_import_source_module.as_str()
  }

  fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, AnyError> {
    self.maybe_import_map_resolver.map_or_else(
      || resolve_import(specifier, referrer.as_str()).map_err(|err| err.into()),
      |r| r.resolve(specifier, referrer),
    )
  }
}
