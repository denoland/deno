// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_graph::source::Resolver;
use import_map::ImportMap;
use std::sync::Arc;

#[derive(Debug)]
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
