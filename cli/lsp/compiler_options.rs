// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_config::deno_json::CompilerOptions;
use deno_config::workspace::TsTypeLib;
use deno_core::url::Url;
use deno_resolver::deno_json::CompilerOptionsData;
use deno_resolver::deno_json::CompilerOptionsKey;
use deno_resolver::deno_json::CompilerOptionsResolver;
use deno_resolver::deno_json::CompilerOptionsType;
use deno_resolver::deno_json::JsxImportSourceConfig;
use deno_resolver::deno_json::TsConfigFile;
use deno_resolver::deno_json::get_base_compiler_options_for_emit;

use crate::lsp::config::Config;
use crate::lsp::resolver::LspResolver;
use crate::sys::CliSys;

#[derive(Debug, Copy, Clone)]
pub struct LspCompilerOptionsData<'a> {
  inner: &'a CompilerOptionsData,
}

impl<'a> LspCompilerOptionsData<'a> {
  pub fn workspace_dir_or_source_url(&self) -> Option<&'a Arc<Url>> {
    self.inner.workspace_dir_or_source_url()
  }

  pub fn compiler_options(&self) -> Arc<CompilerOptions> {
    self
      .inner
      .compiler_options_for_lib(TsTypeLib::DenoWindow)
      // TODO(nayeemrmn): Only print this once.
      // .inspect_err(|err| {
      //   lsp_warn!("{err:#}");
      // })
      .ok()
      .cloned()
      .unwrap_or_else(|| {
        Arc::new(get_base_compiler_options_for_emit(
          CompilerOptionsType::Check {
            lib: TsTypeLib::DenoWindow,
          },
          self.inner.defaults,
        ))
      })
  }

  pub fn compiler_options_types(&self) -> &'a Arc<Vec<(Url, Vec<String>)>> {
    self.inner.compiler_options_types()
  }

  pub fn jsx_import_source_config(
    &self,
  ) -> Option<&'a Arc<JsxImportSourceConfig>> {
    self
      .inner
      .jsx_import_source_config()
      // TODO(nayeemrmn): Only print this once.
      // .inspect_err(|err| {
      //   lsp_warn!("{err:#}");
      // })
      .ok()
      .flatten()
  }
}

#[derive(Debug)]
pub struct LspCompilerOptionsResolver {
  inner: CompilerOptionsResolver,
}

impl Default for LspCompilerOptionsResolver {
  fn default() -> Self {
    Self::from_inner(Default::default())
  }
}

impl LspCompilerOptionsResolver {
  pub fn new(config: &Config, resolver: &LspResolver) -> Self {
    Self::from_inner(CompilerOptionsResolver::new_for_dirs_by_scope(
      &CliSys::default(),
      config
        .tree
        .data_by_scope()
        .iter()
        .map(|(s, d)| (s, d.member_dir.as_ref()))
        .collect(),
      Box::new(|s| {
        resolver
          .get_scoped_resolver(Some(s))
          .as_node_resolver()
          .map(|r| r.as_ref())
      }),
    ))
  }

  fn from_inner(inner: CompilerOptionsResolver) -> Self {
    Self { inner }
  }

  pub fn for_specifier(&self, specifier: &Url) -> LspCompilerOptionsData<'_> {
    LspCompilerOptionsData {
      inner: self.inner.for_specifier(specifier),
    }
  }

  pub fn entry_for_specifier(
    &self,
    specifier: &Url,
  ) -> (CompilerOptionsKey, LspCompilerOptionsData<'_>) {
    let (key, data) = self.inner.entry_for_specifier(specifier);
    let data = LspCompilerOptionsData { inner: data };
    (key, data)
  }

  pub fn for_key(
    &self,
    key: &CompilerOptionsKey,
  ) -> Option<LspCompilerOptionsData<'_>> {
    Some(LspCompilerOptionsData {
      inner: self.inner.for_key(key)?,
    })
  }

  #[allow(clippy::type_complexity)]
  pub fn entries(
    &self,
  ) -> impl Iterator<
    Item = (
      CompilerOptionsKey,
      LspCompilerOptionsData<'_>,
      Option<(&Arc<Url>, &Vec<TsConfigFile>)>,
    ),
  > {
    self
      .inner
      .entries()
      .map(|(k, d, f)| (k, LspCompilerOptionsData { inner: d }, f))
  }
}
