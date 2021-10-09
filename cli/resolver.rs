// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
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
