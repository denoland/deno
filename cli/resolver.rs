// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::generic_error;
use deno_core::resolve_import;
use deno_core::ModuleSpecifier;
use deno_graph::source::ResolveResponse;
use deno_graph::source::Resolver;
use deno_graph::source::DEFAULT_JSX_IMPORT_SOURCE_MODULE;
use import_map::ImportMap;
use std::sync::Arc;

use crate::args::config_file::JsxImportSourceConfig;

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug, Clone, Default)]
pub struct CliResolver {
  maybe_import_map: Option<Arc<ImportMap>>,
  maybe_default_jsx_import_source: Option<String>,
  maybe_jsx_import_source_module: Option<String>,
}

impl CliResolver {
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

  pub fn with_import_map(import_map: Arc<ImportMap>) -> Self {
    Self::new(None, Some(import_map))
  }

  pub fn as_graph_resolver(&self) -> &dyn Resolver {
    self
  }
}

impl Resolver for CliResolver {
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
  ) -> ResolveResponse {
    let r = if let Some(import_map) = &self.maybe_import_map {
      match import_map.resolve(specifier, referrer) {
        Ok(resolved_specifier) => {
          ResolveResponse::Specifier(resolved_specifier)
        }
        Err(err) => ResolveResponse::Err(err.into()),
      }
    } else {
      match resolve_import(specifier, referrer.as_str()) {
        Ok(specifier) => ResolveResponse::Specifier(specifier),
        Err(err) => ResolveResponse::Err(err.into()),
      }
    };

    match r {
      ResolveResponse::Specifier(specifier) => {
        let referrer_scheme = referrer.scheme();
        let specifier_scheme = specifier.scheme();
        if referrer_scheme == "https" && specifier_scheme == "http" {
          let msg = format!("Modules imported via https are not allowed to import http modules.\n  Importing: {}", specifier.as_str());
          ResolveResponse::Err(generic_error(msg))
        } else if matches!(referrer_scheme, "https" | "http")
          && !matches!(specifier_scheme, "https" | "http" | "npm")
        {
          let msg = format!("Remote modules are not allowed to import local modules. Consider using a dynamic import instead.\n  Importing: {}", specifier.as_str());
          ResolveResponse::Err(generic_error(msg))
        } else {
          ResolveResponse::Specifier(specifier)
        }
      }
      r => r,
    }
  }
}
