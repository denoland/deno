// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_config::deno_json::CompilerOptions;
use deno_config::glob::FilePatterns;
use deno_config::glob::WalkEntry;
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
use crate::util::fs::CollectSpecifiersOptions;
use crate::util::fs::canonicalize_path_maybe_not_exists;
use crate::util::fs::collect_specifiers;

#[derive(Debug, Clone)]
pub struct LspCompilerOptionsData {
  pub workspace_dir_or_source_url: Option<Arc<Url>>,
  pub compiler_options: Arc<CompilerOptions>,
  pub compiler_options_types: Arc<Vec<(Url, Vec<String>)>>,
  pub skip_lib_check: bool,
  pub jsx_import_source_config: Option<Arc<JsxImportSourceConfig>>,
  pub ts_config_files: Option<(Arc<Url>, Vec<TsConfigFile>)>,
  pub ts_config_roots: Arc<Vec<Url>>,
  watched_files: HashSet<Arc<Url>>,
}

#[derive(Debug)]
pub struct LspCompilerOptionsResolver {
  pub inner: Arc<CompilerOptionsResolver>,
  data: BTreeMap<CompilerOptionsKey, LspCompilerOptionsData>,
  // Tsconfig root collection walks the filesystem, so cache the result keyed by
  // the file patterns. Resolver rebuilds triggered by config changes that don't
  // affect a tsconfig's `include`/`files`/`exclude` reuse the prior walk.
  ts_config_roots_cache: HashMap<FilePatterns, Arc<Vec<Url>>>,
}

impl Default for LspCompilerOptionsResolver {
  fn default() -> Self {
    Self::from_inner(Default::default(), &Default::default())
  }
}

impl LspCompilerOptionsResolver {
  pub fn new(
    config: &Config,
    resolver: &LspResolver,
    previous: Option<&LspCompilerOptionsResolver>,
  ) -> Self {
    let empty_cache = HashMap::new();
    let previous_roots_cache = previous
      .map(|p| &p.ts_config_roots_cache)
      .unwrap_or(&empty_cache);
    Self::from_inner(
      CompilerOptionsResolver::new_for_dirs_by_scope(
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
      ),
      previous_roots_cache,
    )
  }

  fn from_inner(
    inner: CompilerOptionsResolver,
    previous_roots_cache: &HashMap<FilePatterns, Arc<Vec<Url>>>,
  ) -> Self {
    let mut ts_config_roots_cache = HashMap::new();
    let ts_config_roots = inner
      .ts_config_file_patterns()
      .filter_map(|(key, file_patterns)| {
        if let Some(roots) = previous_roots_cache.get(&file_patterns) {
          ts_config_roots_cache.insert(file_patterns, roots.clone());
          return Some((key, roots.clone()));
        }
        let roots = collect_specifiers(
          CollectSpecifiersOptions {
            file_patterns: file_patterns.clone(),
            vendor_folder: None,
            include_ignored_specified: true,
          },
          is_lsp_root_file,
        )
        .inspect_err(|err| {
          lsp_warn!("Failed to collect tsconfig roots: {err:#}");
        })
        .ok()?;
        let roots = Arc::new(roots);
        ts_config_roots_cache.insert(file_patterns, roots.clone());
        Some((key, roots))
      })
      .collect::<BTreeMap<_, _>>();
    let data = inner
      .entries()
      .map(|(k, d, f)| {
        let ts_config_roots =
          ts_config_roots.get(&k).cloned().unwrap_or_default();
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
            skip_lib_check: d.skip_lib_check(),
            jsx_import_source_config: d
              .jsx_import_source_config()
              .inspect_err(|err| {
                lsp_warn!("{err:#}");
              })
              .ok()
              .flatten()
              .cloned(),
            ts_config_files: f.map(|(r, f)| (r.clone(), f.clone())),
            ts_config_roots,
            watched_files: d
              .sources
              .iter()
              .flat_map(|s| {
                std::iter::once(s.specifier.clone()).chain(
                  s.specifier
                    .to_file_path()
                    .ok()
                    .and_then(|p| canonicalize_path_maybe_not_exists(&p).ok())
                    .and_then(|p| Url::from_file_path(p).ok().map(Arc::new)),
                )
              })
              .collect(),
          },
        )
      })
      .collect();
    Self {
      inner: Arc::new(inner),
      data,
      ts_config_roots_cache,
    }
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

  pub fn entries(
    &self,
  ) -> impl Iterator<Item = (&CompilerOptionsKey, &LspCompilerOptionsData)> {
    self.data.iter()
  }

  pub fn is_watched_file(&self, specifier: &Url) -> bool {
    self
      .data
      .values()
      .any(|d| d.watched_files.contains(specifier))
  }
}

fn is_lsp_root_file(entry: WalkEntry) -> bool {
  matches!(
    MediaType::from_path(entry.path),
    MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::Mjs
      | MediaType::Cjs
      | MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Json
      | MediaType::Jsonc
      | MediaType::Tsx
  )
}
