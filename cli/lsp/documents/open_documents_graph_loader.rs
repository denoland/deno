// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::Document;
use deno_core::futures::future;
use deno_core::futures::FutureExt;
use deno_core::ModuleSpecifier;
use lsp::Url;
use std::collections::HashMap;
use tower_lsp::lsp_types as lsp;

/// Loader that will look at the open documents.
pub struct OpenDocumentsGraphLoader<'a> {
  pub inner_loader: &'a mut dyn deno_graph::source::Loader,
  pub open_docs: &'a HashMap<ModuleSpecifier, Document>,
}

impl<'a> OpenDocumentsGraphLoader<'a> {
  fn load_from_docs(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<deno_graph::source::LoadFuture> {
    if specifier.scheme() == "file" {
      if let Some(doc) = self.open_docs.get(specifier) {
        return Some(
          future::ready(Ok(Some(deno_graph::source::LoadResponse::Module {
            content: doc.text(),
            specifier: doc.specifier().clone(),
            maybe_headers: None,
          })))
          .boxed_local(),
        );
      }
    }
    None
  }
}

impl<'a> deno_graph::source::Loader for OpenDocumentsGraphLoader<'a> {
  fn registry_url(&self) -> &Url {
    self.inner_loader.registry_url()
  }

  fn load(
    &mut self,
    specifier: &ModuleSpecifier,
    is_dynamic: bool,
    cache_setting: deno_graph::source::CacheSetting,
  ) -> deno_graph::source::LoadFuture {
    match self.load_from_docs(specifier) {
      Some(fut) => fut,
      None => self.inner_loader.load(specifier, is_dynamic, cache_setting),
    }
  }

  fn cache_module_info(
    &mut self,
    specifier: &deno_ast::ModuleSpecifier,
    source: &str,
    module_info: &deno_graph::ModuleInfo,
  ) {
    self
      .inner_loader
      .cache_module_info(specifier, source, module_info)
  }
}
