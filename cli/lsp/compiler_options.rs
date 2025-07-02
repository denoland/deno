// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::sync::Arc;

use deno_config::deno_json::CompilerOptions;
use deno_config::workspace::TsTypeLib;
use deno_core::url::Url;
use deno_resolver::deno_json::CompilerOptionsResolver;

use crate::lsp::config::Config;
use crate::lsp::logging::lsp_warn;
use crate::lsp::resolver::LspResolver;
use crate::sys::CliSys;

#[derive(Debug, Default)]
pub struct LspCompilerOptionsResolver {
  inner: CompilerOptionsResolver,
}

impl LspCompilerOptionsResolver {
  pub fn new(config: &Config, resolver: &LspResolver) -> Self {
    Self {
      inner: CompilerOptionsResolver::new_for_lsp(
        &CliSys::default(),
        config
          .tree
          .data_by_scope()
          .iter()
          .map(|(s, d)| (s.as_ref(), d.member_dir.as_ref()))
          .collect(),
        Box::new(|s| {
          resolver
            .get_scoped_resolver(Some(s))
            .as_node_resolver()
            .map(|r| r.as_ref())
        }),
      ),
    }
  }

  pub fn key_for_specifier(&self, specifier: &Url) -> &str {
    self
      .inner
      .for_specifier(specifier)
      .sources
      .last()
      .map(|s| s.specifier.as_str())
      .unwrap_or(".")
  }

  pub fn compiler_options_by_key(
    &self,
  ) -> BTreeMap<&str, &Arc<CompilerOptions>> {
    self
      .inner
      .all()
      .map(|d| {
        let source = d
          .sources
          .last()
          .map(|s| s.specifier.as_str())
          .unwrap_or(".");
        let compiler_options = d
          .compiler_options_for_lib(TsTypeLib::DenoWindow)
          .inspect_err(|err| {
            lsp_warn!("{err:#}");
          })
          .ok()
          .unwrap_or_else(|| {
            self
              .inner
              .unscoped()
              .compiler_options_for_lib(TsTypeLib::DenoWindow)
              .expect("Unscoped compiler options should not error.")
          });
        (source, compiler_options)
      })
      .collect()
  }
}
