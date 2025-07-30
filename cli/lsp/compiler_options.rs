// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::sync::Arc;

use deno_config::deno_json::CompilerOptions;
use deno_config::workspace::TsTypeLib;
use deno_core::url::Url;
use deno_resolver::deno_json::CompilerOptionsKey;
use deno_resolver::deno_json::CompilerOptionsResolver;
use deno_resolver::deno_json::CompilerOptionsType;
use deno_resolver::deno_json::JsxImportSourceConfig;
use deno_resolver::deno_json::TsConfigFile;
use deno_resolver::deno_json::get_base_compiler_options_for_emit;

use crate::lsp::config::Config;
use crate::lsp::logging::lsp_warn;
use crate::lsp::resolver::LspResolver;
use crate::sys::CliSys;

#[derive(Debug, Clone)]
pub struct LspCompilerOptionsData {
  pub workspace_dir_or_source_url: Option<Arc<Url>>,
  pub compiler_options: Arc<CompilerOptions>,
  pub compiler_options_types: Arc<Vec<(Url, Vec<String>)>>,
  pub jsx_import_source_config: Option<Arc<JsxImportSourceConfig>>,
  pub ts_config_files: Option<(Arc<Url>, Vec<TsConfigFile>)>,
}

#[derive(Debug)]
pub struct LspCompilerOptionsResolver {
  inner: CompilerOptionsResolver,
  data: BTreeMap<CompilerOptionsKey, LspCompilerOptionsData>,
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
    let data = inner
      .entries()
      .map(|(k, d, f)| {
        (
          k,
          LspCompilerOptionsData {
            workspace_dir_or_source_url: d
              .workspace_dir_or_source_url()
              .cloned(),
            compiler_options: d
              .compiler_options_for_lib(TsTypeLib::DenoWindow)
              .inspect_err(|err| {
                lsp_warn!("{err:#}");
              })
              .ok()
              .cloned()
              .unwrap_or_else(|| {
                Arc::new(get_base_compiler_options_for_emit(
                  CompilerOptionsType::Check {
                    lib: TsTypeLib::DenoWindow,
                  },
                  d.source_kind,
                ))
              }),
            compiler_options_types: d.compiler_options_types().clone(),
            jsx_import_source_config: d
              .jsx_import_source_config()
              .inspect_err(|err| {
                lsp_warn!("{err:#}");
              })
              .ok()
              .flatten()
              .cloned(),
            ts_config_files: f.map(|(r, f)| (r.clone(), f.clone())),
          },
        )
      })
      .collect();
    Self { inner, data }
  }

  pub fn for_specifier(&self, specifier: &Url) -> &LspCompilerOptionsData {
    self
      .data
      .get(&self.inner.entry_for_specifier(specifier).0)
      .expect("Stored key should be mapped.")
  }

  pub fn entry_for_specifier(
    &self,
    specifier: &Url,
  ) -> (&CompilerOptionsKey, &LspCompilerOptionsData) {
    self
      .data
      .get_key_value(&self.inner.entry_for_specifier(specifier).0)
      .expect("Stored key should be mapped.")
  }

  pub fn for_key(
    &self,
    key: &CompilerOptionsKey,
  ) -> Option<&LspCompilerOptionsData> {
    self.data.get(key)
  }

  #[allow(clippy::type_complexity)]
  pub fn entries(
    &self,
  ) -> impl Iterator<Item = (&CompilerOptionsKey, &LspCompilerOptionsData)> {
    self.data.iter()
  }
}
