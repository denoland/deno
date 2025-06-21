// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use deno_config::deno_json::get_base_compiler_options_for_emit;
use deno_config::deno_json::parse_compiler_options;
use deno_config::deno_json::CompilerOptions;
use deno_config::deno_json::CompilerOptionsParseError;
use deno_config::deno_json::CompilerOptionsType;
use deno_config::deno_json::CompilerOptionsWithIgnoredOptions;
use deno_config::deno_json::TsTypeLib;
use deno_config::glob::PathOrPatternSet;
use deno_config::workspace::CompilerOptionsSource;
use deno_config::workspace::JsxImportSourceConfig;
use deno_config::workspace::JsxImportSourceSpecifierConfig;
use deno_config::workspace::ToMaybeJsxImportSourceConfigError;
use deno_path_util::url_from_file_path;
use deno_path_util::url_to_file_path;
use deno_terminal::colors;
use deno_unsync::sync::AtomicFlag;
#[cfg(feature = "sync")]
use once_cell::sync::OnceCell;
#[cfg(not(feature = "sync"))]
use once_cell::unsync::OnceCell;
use sys_traits::FsRead;
use url::Url;

use crate::collections::FolderScopedMap;
use crate::factory::WorkspaceDirectoryProvider;
use crate::sync::new_rc;

#[allow(clippy::disallowed_types)]
pub type CompilerOptionsResolverRc =
  crate::sync::MaybeArc<CompilerOptionsResolver>;

#[allow(clippy::disallowed_types)]
type CompilerOptionsRc = crate::sync::MaybeArc<CompilerOptions>;
#[allow(clippy::disallowed_types)]
type LoggedWarningsRc = crate::sync::MaybeArc<LoggedWarnings>;
#[cfg(feature = "deno_ast")]
#[allow(clippy::disallowed_types)]
pub type TranspileAndEmitOptionsRc =
  crate::sync::MaybeArc<TranspileAndEmitOptions>;
#[allow(clippy::disallowed_types)]
pub type CompilerOptionsTypesRc =
  crate::sync::MaybeArc<Vec<(Url, Vec<String>)>>;
#[allow(clippy::disallowed_types)]
pub type JsxImportSourceConfigRc = crate::sync::MaybeArc<JsxImportSourceConfig>;

#[cfg(feature = "deno_ast")]
#[derive(Debug)]
pub struct TranspileAndEmitOptions {
  pub transpile: deno_ast::TranspileOptions,
  pub emit: deno_ast::EmitOptions,
  // stored ahead of time so we don't have to recompute this a lot
  pub pre_computed_hash: u64,
}

#[derive(Debug, Default)]
struct LoggedWarnings {
  experimental_decorators: AtomicFlag,
  folders: crate::sync::MaybeDashSet<Url>,
}

#[derive(Default, Debug)]
struct MemoizedValues {
  deno_window_check_compiler_options: OnceCell<CompilerOptionsRc>,
  deno_worker_check_compiler_options: OnceCell<CompilerOptionsRc>,
  emit_compiler_options: OnceCell<CompilerOptionsRc>,
  #[cfg(feature = "deno_ast")]
  transpile_options: OnceCell<TranspileAndEmitOptionsRc>,
  compiler_options_types: OnceCell<CompilerOptionsTypesRc>,
  jsx_import_source_config: OnceCell<Option<JsxImportSourceConfigRc>>,
  check_js: OnceCell<bool>,
}

#[derive(Debug)]
pub struct CompilerOptionsReference {
  pub sources: Vec<CompilerOptionsSource>,
  memoized: MemoizedValues,
  logged_warnings: LoggedWarningsRc,
}

impl CompilerOptionsReference {
  fn new(
    sources: Vec<CompilerOptionsSource>,
    logged_warnings: LoggedWarningsRc,
  ) -> Self {
    Self {
      sources,
      memoized: Default::default(),
      logged_warnings,
    }
  }

  pub fn compiler_options_for_lib(
    &self,
    lib: TsTypeLib,
  ) -> Result<&CompilerOptionsRc, CompilerOptionsParseError> {
    self.compiler_options_inner(CompilerOptionsType::Check { lib })
  }

  pub fn compiler_options_for_emit(
    &self,
  ) -> Result<&CompilerOptionsRc, CompilerOptionsParseError> {
    self.compiler_options_inner(CompilerOptionsType::Emit)
  }

  fn compiler_options_inner(
    &self,
    typ: CompilerOptionsType,
  ) -> Result<&CompilerOptionsRc, CompilerOptionsParseError> {
    let cell = match typ {
      CompilerOptionsType::Bundle => unreachable!(),
      CompilerOptionsType::Check {
        lib: TsTypeLib::DenoWindow,
      } => &self.memoized.deno_window_check_compiler_options,
      CompilerOptionsType::Check {
        lib: TsTypeLib::DenoWorker,
      } => &self.memoized.deno_worker_check_compiler_options,
      CompilerOptionsType::Emit => &self.memoized.emit_compiler_options,
    };
    cell.get_or_try_init(|| {
      let mut result = CompilerOptionsWithIgnoredOptions {
        compiler_options: get_base_compiler_options_for_emit(typ),
        ignored_options: Vec::new(),
      };
      for source in &self.sources {
        let object = serde_json::from_value(source.compiler_options.0.clone())
          .map_err(|err| CompilerOptionsParseError {
            specifier: source.specifier.clone(),
            source: err,
          })?;
        let parsed = parse_compiler_options(object, Some(&source.specifier));
        result.compiler_options.merge_object_mut(parsed.options);
        if let Some(ignored) = parsed.maybe_ignored {
          result.ignored_options.push(ignored);
        }
      }
      check_warn_compiler_options(&result, &self.logged_warnings);
      Ok(new_rc(result.compiler_options))
    })
  }

  #[cfg(feature = "deno_ast")]
  pub fn transpile_options(
    &self,
  ) -> Result<&TranspileAndEmitOptionsRc, CompilerOptionsParseError> {
    self.memoized.transpile_options.get_or_try_init(|| {
      let compiler_options = self.compiler_options_for_emit()?;
      compiler_options_to_transpile_and_emit_options(
        compiler_options.as_ref().clone(),
      )
      .map(new_rc)
      .map_err(|source| CompilerOptionsParseError {
        specifier: self.sources.last().map(|s| s.specifier.clone()).expect(
          "Compiler options parse errors must come from a user source.",
        ),
        source,
      })
    })
  }

  pub fn compiler_options_types(&self) -> &CompilerOptionsTypesRc {
    self.memoized.compiler_options_types.get_or_init(|| {
      let types = self
        .sources
        .iter()
        .filter_map(|s| {
          let types = s
            .compiler_options
            .0
            .as_object()?
            .get("types")?
            .as_array()?
            .iter()
            .filter_map(|v| Some(v.as_str()?.to_string()))
            .collect();
          Some((s.specifier.clone(), types))
        })
        .collect();
      new_rc(types)
    })
  }

  pub fn jsx_import_source_config(
    &self,
  ) -> Result<Option<&JsxImportSourceConfigRc>, ToMaybeJsxImportSourceConfigError>
  {
    self.memoized.jsx_import_source_config.get_or_try_init(|| {
      let jsx = self.sources.iter().rev().find_map(|s| Some((s.compiler_options.0.as_object()?.get("jsx")?.as_str()?, &s.specifier)));
      let is_jsx_automatic = matches!(
        jsx,
        Some(("react-jsx" | "react-jsxdev" | "precompile", _)),
      );
      let import_source = self.sources.iter().rev().find_map(|s| {
        Some(JsxImportSourceSpecifierConfig {
          specifier: s.compiler_options.0.as_object()?.get("jsxImportSource")?.as_str()?.to_string(),
          base: s.specifier.clone()
        })
      }).or_else(|| {
        if !is_jsx_automatic {
          return None;
        }
        Some(JsxImportSourceSpecifierConfig {
          base: self.sources.last()?.specifier.clone(),
          specifier: "react".to_string()
        })
      });
      let import_source_types = self.sources.iter().rev().find_map(|s| {
        Some(JsxImportSourceSpecifierConfig {
          specifier: s.compiler_options.0.as_object()?.get("jsxImportSourceTypes")?.as_str()?.to_string(),
          base: s.specifier.clone()
        })
      }).or_else(|| import_source.clone());
      let module = match jsx {
        Some(("react-jsx", _)) => "jsx-runtime".to_string(),
        Some(("react-jsxdev", _)) => "jsx-dev-runtime".to_string(),
        Some(("react", _)) | None => {
          if let Some(import_source) = &import_source {
            return Err(
              ToMaybeJsxImportSourceConfigError::InvalidJsxImportSourceValue(
                import_source.base.clone(),
              ),
            );
          }
          if let Some(import_source_types) = &import_source_types {
            return Err(
              ToMaybeJsxImportSourceConfigError::InvalidJsxImportSourceTypesValue(
                import_source_types.base.clone(),
              ),
            );
          }
          return Ok(None);
        }
        Some(("precompile", _)) => "jsx-runtime".to_string(),
        Some((setting, setting_source)) => {
          return Err(
            ToMaybeJsxImportSourceConfigError::InvalidJsxCompilerOption {
              value: setting.to_string(),
              specifier: setting_source.clone(),
            },
          )
        }
      };
      Ok(Some(new_rc(JsxImportSourceConfig {
        module,
        import_source,
        import_source_types,
      })))
    }).map(|c| c.as_ref())
  }

  pub fn check_js(&self) -> bool {
    *self.memoized.check_js.get_or_init(|| {
      self
        .sources
        .iter()
        .rev()
        .find_map(|s| {
          s.compiler_options.0.as_object()?.get("checkJs")?.as_bool()
        })
        .unwrap_or(false)
    })
  }
}

#[derive(Debug, Clone)]
struct TsConfigFileFilter {
  // Note that `files`, `include` and `exclude` are overwritten, not merged,
  // when using `extends`. So we only need to store one referrer for `files`.
  // See: https://www.typescriptlang.org/tsconfig/#extends.
  files: Option<(Url, Vec<PathBuf>)>,
  include: Option<PathOrPatternSet>,
  exclude: Option<PathOrPatternSet>,
  dir_path: PathBuf,
}

impl TsConfigFileFilter {
  fn includes_path(&self, path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    if let Some((_, files)) = &self.files {
      if files.iter().any(|p| p == path) {
        return true;
      }
    }
    if let Some(exclude) = &self.exclude {
      if exclude.matches_path(path) {
        return false;
      }
    }
    if let Some(include) = &self.include {
      if include.matches_path(path) {
        return true;
      }
    } else if path.starts_with(&self.dir_path) {
      return true;
    }
    false
  }
}

#[allow(clippy::disallowed_types)]
type TsConfigFileFilterRc = crate::sync::MaybeArc<TsConfigFileFilter>;

#[derive(Debug)]
struct TsConfigReference {
  compiler_options: CompilerOptionsReference,
  filter: TsConfigFileFilterRc,
}

impl TsConfigReference {
  fn maybe_read_from_dir<TSys: FsRead>(
    sys: &TSys,
    dir_path: impl AsRef<Path>,
    logged_warnings: &LoggedWarningsRc,
  ) -> Option<Self> {
    let dir_path = dir_path.as_ref();
    let path = dir_path.join("tsconfig.json");
    let warn = |err: &dyn std::fmt::Display| {
      log::warn!("Failed reading {}: {}", path.display(), err);
    };
    let url = url_from_file_path(&path).inspect_err(|e| warn(e)).ok()?;
    let text = sys
      .fs_read_to_string(&path)
      .inspect_err(|e| {
        if !matches!(e.kind(), ErrorKind::NotFound | ErrorKind::IsADirectory) {
          warn(e)
        }
      })
      .ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&text)
      .inspect_err(|e| warn(e))
      .ok();
    let object = value.as_ref().and_then(|v| v.as_object());

    // TODO(nayeemrmn): Implement `extends`.
    let extends_targets = Vec::<&TsConfigReference>::new();

    let compiler_options_value = object
      .and_then(|o| o.get("compilerOptions"))
      .filter(|v| !v.is_null());
    let sources = extends_targets
      .iter()
      .flat_map(|t| &t.compiler_options.sources)
      .cloned()
      .chain(compiler_options_value.map(|v| CompilerOptionsSource {
        specifier: url.clone(),
        compiler_options: CompilerOptions(v.clone()),
      }))
      .collect();
    let files = object
      .and_then(|o| {
        let files = o
          .get("files")?
          .as_array()?
          .iter()
          .filter_map(|v| {
            let path = Path::new(v.as_str()?);
            if path.is_absolute() {
              Some(path.to_path_buf())
            } else {
              Some(dir_path.join(path))
            }
          })
          .collect();
        Some((url, files))
      })
      .or_else(|| {
        extends_targets
          .iter()
          .rev()
          .find_map(|t| t.filter.files.clone())
      });
    let include = object
      .and_then(|o| {
        PathOrPatternSet::from_include_relative_path_or_patterns(
          dir_path,
          &o.get("include")?
            .as_array()?
            .iter()
            .filter_map(|v| Some(v.as_str()?.to_string()))
            .collect::<Vec<_>>(),
        )
        .ok()
      })
      .or_else(|| {
        extends_targets
          .iter()
          .rev()
          .find_map(|t| t.filter.include.clone())
      })
      .or_else(|| files.is_some().then(Default::default));
    let exclude = object
      .and_then(|o| {
        PathOrPatternSet::from_exclude_relative_path_or_patterns(
          dir_path,
          &o.get("exclude")?
            .as_array()?
            .iter()
            .filter_map(|v| Some(v.as_str()?.to_string()))
            .collect::<Vec<_>>(),
        )
        .ok()
      })
      .or_else(|| {
        extends_targets
          .iter()
          .rev()
          .find_map(|t| t.filter.exclude.clone())
      });
    Some(Self {
      compiler_options: CompilerOptionsReference::new(
        sources,
        logged_warnings.clone(),
      ),
      filter: new_rc(TsConfigFileFilter {
        files,
        include,
        exclude,
        dir_path: dir_path.to_path_buf(),
      }),
    })
  }
}

#[derive(Debug)]
pub struct CompilerOptionsResolver {
  workspace_configs: FolderScopedMap<CompilerOptionsReference>,
  ts_configs: Vec<TsConfigReference>,
}

impl CompilerOptionsResolver {
  pub fn new<TSys: FsRead>(
    sys: &TSys,
    workspace_directory_provider: &WorkspaceDirectoryProvider,
  ) -> Self {
    let logged_warnings = new_rc(LoggedWarnings::default());
    let mut ts_configs = Vec::new();
    let root_dir = workspace_directory_provider.root();
    let mut workspace_configs =
      FolderScopedMap::new(CompilerOptionsReference::new(
        root_dir.to_configured_compiler_options_sources(),
        logged_warnings.clone(),
      ));
    for (dir_url, dir) in workspace_directory_provider.entries() {
      if let Some(ts_config) = TsConfigReference::maybe_read_from_dir(
        sys,
        dir.dir_path(),
        &logged_warnings,
      ) {
        ts_configs.push(ts_config);
      }
      if let Some(dir_url) = dir_url {
        workspace_configs.insert(
          dir_url.clone(),
          CompilerOptionsReference::new(
            dir.to_configured_compiler_options_sources(),
            logged_warnings.clone(),
          ),
        );
      }
    }
    ts_configs.reverse();
    Self {
      workspace_configs,
      ts_configs,
    }
  }

  pub fn reference_for_specifier(
    &self,
    specifier: &Url,
  ) -> &CompilerOptionsReference {
    if let Ok(path) = url_to_file_path(specifier) {
      for ts_config in &self.ts_configs {
        if ts_config.filter.includes_path(&path) {
          return &ts_config.compiler_options;
        }
      }
    }
    self.workspace_configs.get_for_specifier(specifier)
  }

  pub fn references(&self) -> impl Iterator<Item = &CompilerOptionsReference> {
    self
      .workspace_configs
      .entries()
      .map(|(_, r)| r)
      .chain(self.ts_configs.iter().map(|t| &t.compiler_options))
  }

  pub fn reference_count(&self) -> usize {
    self.workspace_configs.count() + self.ts_configs.len()
  }

  // pub fn ts_config_files(&self) -> impl Iterator<Item = (&Url, )> {}
}

#[cfg(feature = "graph")]
impl deno_graph::CheckJsResolver for CompilerOptionsResolver {
  fn resolve(&self, specifier: &Url) -> bool {
    self.reference_for_specifier(specifier).check_js()
  }
}

/// JSX config stored in `CompilerOptionsResolver`, but fallibly resolved
/// ahead of time as needed for the graph resolver.
#[derive(Debug)]
pub struct JsxImportSourceConfigResolver {
  workspace_configs: FolderScopedMap<Option<JsxImportSourceConfigRc>>,
  ts_configs: Vec<(Option<JsxImportSourceConfigRc>, TsConfigFileFilterRc)>,
}

impl JsxImportSourceConfigResolver {
  pub fn from_compiler_options_resolver(
    compiler_options_resolver: &CompilerOptionsResolver,
  ) -> Result<Self, ToMaybeJsxImportSourceConfigError> {
    Ok(Self {
      workspace_configs: compiler_options_resolver
        .workspace_configs
        .try_map(|r| Ok(r.jsx_import_source_config()?.cloned()))?,
      ts_configs: compiler_options_resolver
        .ts_configs
        .iter()
        .map(|t| {
          Ok((
            t.compiler_options.jsx_import_source_config()?.cloned(),
            t.filter.clone(),
          ))
        })
        .collect::<Result<_, _>>()?,
    })
  }

  pub fn for_specifier(
    &self,
    specifier: &Url,
  ) -> Option<&JsxImportSourceConfigRc> {
    if let Ok(path) = url_to_file_path(specifier) {
      for (config, filter) in &self.ts_configs {
        if filter.includes_path(&path) {
          return config.as_ref();
        }
      }
    }
    self.workspace_configs.get_for_specifier(specifier).as_ref()
  }
}

#[cfg(feature = "deno_ast")]
fn compiler_options_to_transpile_and_emit_options(
  config: deno_config::deno_json::CompilerOptions,
) -> Result<TranspileAndEmitOptions, serde_json::Error> {
  let options: deno_config::deno_json::EmitConfigOptions =
    serde_json::from_value(config.0)?;
  let imports_not_used_as_values =
    match options.imports_not_used_as_values.as_str() {
      "preserve" => deno_ast::ImportsNotUsedAsValues::Preserve,
      "error" => deno_ast::ImportsNotUsedAsValues::Error,
      _ => deno_ast::ImportsNotUsedAsValues::Remove,
    };
  let (transform_jsx, jsx_automatic, jsx_development, precompile_jsx) =
    match options.jsx.as_str() {
      "react" => (true, false, false, false),
      "react-jsx" => (true, true, false, false),
      "react-jsxdev" => (true, true, true, false),
      "precompile" => (false, false, false, true),
      _ => (false, false, false, false),
    };
  let source_map = if options.inline_source_map {
    deno_ast::SourceMapOption::Inline
  } else if options.source_map {
    deno_ast::SourceMapOption::Separate
  } else {
    deno_ast::SourceMapOption::None
  };
  let transpile = deno_ast::TranspileOptions {
    use_ts_decorators: options.experimental_decorators,
    use_decorators_proposal: !options.experimental_decorators,
    emit_metadata: options.emit_decorator_metadata,
    imports_not_used_as_values,
    jsx_automatic,
    jsx_development,
    jsx_factory: options.jsx_factory,
    jsx_fragment_factory: options.jsx_fragment_factory,
    jsx_import_source: options.jsx_import_source,
    precompile_jsx,
    precompile_jsx_skip_elements: options.jsx_precompile_skip_elements,
    precompile_jsx_dynamic_props: None,
    transform_jsx,
    var_decl_imports: false,
    // todo(dsherret): support verbatim_module_syntax here properly
    verbatim_module_syntax: false,
  };
  let emit = deno_ast::EmitOptions {
    inline_sources: options.inline_sources,
    remove_comments: false,
    source_map,
    source_map_base: None,
    source_map_file: None,
  };
  let transpile_and_emit_options_hash = {
    use std::hash::Hash;
    use std::hash::Hasher;
    let mut hasher = twox_hash::XxHash64::default();
    transpile.hash(&mut hasher);
    emit.hash(&mut hasher);
    hasher.finish()
  };
  Ok(TranspileAndEmitOptions {
    transpile,
    emit,
    pre_computed_hash: transpile_and_emit_options_hash,
  })
}

fn check_warn_compiler_options(
  compiler_options: &CompilerOptionsWithIgnoredOptions,
  logged_warnings: &LoggedWarnings,
) {
  for ignored_options in &compiler_options.ignored_options {
    if ignored_options
      .maybe_specifier
      .as_ref()
      .map(|s| logged_warnings.folders.insert(s.clone()))
      .unwrap_or(true)
    {
      log::warn!("{}", ignored_options);
    }
  }
  let serde_json::Value::Object(obj) = &compiler_options.compiler_options.0
  else {
    return;
  };
  if obj.get("experimentalDecorators") == Some(&serde_json::Value::Bool(true))
    && logged_warnings.experimental_decorators.raise()
  {
    log::warn!(
      "{} experimentalDecorators compiler option is deprecated and may be removed at any time",
      colors::yellow("Warning"),
    );
  }
}
