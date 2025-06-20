// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::ErrorKind;
use std::path::Path;

use deno_config::deno_json::get_base_compiler_options_for_emit;
use deno_config::deno_json::parse_compiler_options;
use deno_config::deno_json::CompilerOptions;
use deno_config::deno_json::CompilerOptionsParseError;
use deno_config::deno_json::CompilerOptionsType;
use deno_config::deno_json::CompilerOptionsWithIgnoredOptions;
use deno_config::deno_json::TsTypeLib;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathKind;
use deno_config::workspace::CompilerOptionsSource;
use deno_config::workspace::JsxImportSourceConfig;
use deno_config::workspace::JsxImportSourceSpecifierConfig;
use deno_config::workspace::ToMaybeJsxImportSourceConfigError;
use deno_config::workspace::WorkspaceDirectory;
use deno_path_util::url_from_file_path;
use deno_terminal::colors;
use deno_unsync::sync::AtomicFlag;
#[cfg(feature = "sync")]
use once_cell::sync::OnceCell;
#[cfg(not(feature = "sync"))]
use once_cell::unsync::OnceCell;
use sys_traits::FsRead;
use url::Url;

use crate::collections::FolderScopedMap;
use crate::factory::WorkspaceRc;
use crate::sync::new_rc;

#[allow(clippy::disallowed_types)]
pub type CompilerOptionsResolverRc =
  crate::sync::MaybeArc<CompilerOptionsResolver>;

#[allow(clippy::disallowed_types)]
pub type TsConfigResolverRc<TSys> =
  crate::sync::MaybeArc<TsConfigResolver<TSys>>;

#[allow(clippy::disallowed_types)]
type CompilerOptionsRc = crate::sync::MaybeArc<CompilerOptions>;
#[allow(clippy::disallowed_types)]
type LoggedWarningsRc = crate::sync::MaybeArc<LoggedWarnings>;
#[cfg(feature = "deno_ast")]
#[allow(clippy::disallowed_types)]
pub type TranspileAndEmitOptionsRc =
  crate::sync::MaybeArc<TranspileAndEmitOptions>;

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
  jsx_import_source_config: OnceCell<Option<JsxImportSourceConfig>>,
  check_js: OnceCell<bool>,
}

#[derive(Debug)]
pub struct CompilerOptionsReference {
  sources: Vec<CompilerOptionsSource>,
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

  pub fn jsx_import_source_config(
    &self,
  ) -> Result<Option<&JsxImportSourceConfig>, ToMaybeJsxImportSourceConfigError>
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
      Ok(Some(JsxImportSourceConfig {
        module,
        import_source,
        import_source_types,
      }))
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

#[derive(Debug)]
struct TsConfigReference {
  compiler_options: CompilerOptionsReference,
  files: FilePatterns,
}

impl TsConfigReference {
  fn maybe_read_from_dir<TSys: FsRead>(
    sys: &TSys,
    dir_path: impl AsRef<Path>,
    logged_warnings: &LoggedWarningsRc,
  ) -> Option<Self> {
    let path = dir_path.as_ref().join("tsconfig.json");
    let warn = |err: &dyn std::fmt::Display| {
      log::warn!("Failed reading {}: {}", path.display(), err);
    };
    let text = sys
      .fs_read_to_string(&path)
      .inspect_err(|e| {
        if !matches!(e.kind(), ErrorKind::NotFound | ErrorKind::IsADirectory) {
          warn(e)
        }
      })
      .ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let object = value.as_object();
    let sources = if let Some(c) = object.and_then(|o| o.get("compilerOptions"))
    {
      vec![CompilerOptionsSource {
        specifier: url_from_file_path(&path).inspect_err(|e| warn(e)).ok()?,
        compiler_options: CompilerOptions(c.clone()),
      }]
    } else {
      Vec::new()
    };

    // TODO(nayeemrmn): To implement "extends", traverse it and recursively
    // prepend the targets to `sources`.

    let mut files = FilePatterns::new_with_base(path.parent()?.to_path_buf());
    if let Some(object) = object {
      if let Some(files_field) = object.get("files") {
        // TODO(This PR): Implement.
        // files.include = ...;
      }
      if let Some(include) = object.get("include") {
        // TODO(This PR): Implement.
        // files.include = ...;
      }
      if let Some(exclude) = object.get("exclude") {
        // TODO(This PR): Implement.
        // files.exclude = ...;
      }
    }
    Some(Self {
      compiler_options: CompilerOptionsReference::new(
        sources,
        logged_warnings.clone(),
      ),
      files,
    })
  }
}

#[derive(Debug)]
pub struct CompilerOptionsResolver {
  workspace_configs: FolderScopedMap<CompilerOptionsReference>,
  ts_configs: Vec<TsConfigReference>,
}

impl CompilerOptionsResolver {
  pub fn from_workspace<TSys: FsRead>(
    sys: &TSys,
    workspace: &WorkspaceRc,
  ) -> Self {
    let logged_warnings = new_rc(LoggedWarnings::default());
    let mut ts_configs = Vec::new();
    let mut workspace_configs = FolderScopedMap::new(
      CompilerOptionsReference::new(Vec::new(), logged_warnings.clone()),
    );
    for dir_url in workspace.config_folders().keys() {
      let dir = workspace.resolve_member_dir(dir_url);
      workspace_configs.insert(
        dir_url.clone(),
        CompilerOptionsReference::new(
          dir.to_configured_compiler_options_sources(),
          logged_warnings.clone(),
        ),
      );
      if let Some(ts_config) = TsConfigReference::maybe_read_from_dir(
        sys,
        dir.dir_path(),
        &logged_warnings,
      ) {
        ts_configs.push(ts_config);
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
    if let Ok(path) = specifier.to_file_path() {
      for ts_config in &self.ts_configs {
        if ts_config.files.matches_path(&path, PathKind::File) {
          return &ts_config.compiler_options;
        }
      }
    }
    self.workspace_configs.get_for_specifier(specifier)
  }

  pub fn reference_count(&self) -> usize {
    self.workspace_configs.count() + self.ts_configs.len()
  }
}

#[cfg(feature = "graph")]
impl deno_graph::CheckJsResolver for CompilerOptionsResolver {
  fn resolve(&self, specifier: &Url) -> bool {
    self.reference_for_specifier(specifier).check_js()
  }
}

#[derive(Debug)]
pub struct TsConfigFolderInfo<TSys: FsRead> {
  pub dir: WorkspaceDirectory,
  logged_warnings: LoggedWarningsRc,
  memoized: MemoizedValues,
  sys: TSys,
}

impl<TSys: FsRead> TsConfigFolderInfo<TSys> {
  pub fn lib_compiler_options(
    &self,
    lib: TsTypeLib,
  ) -> Result<&CompilerOptionsRc, CompilerOptionsParseError> {
    let cell = match lib {
      TsTypeLib::DenoWindow => {
        &self.memoized.deno_window_check_compiler_options
      }
      TsTypeLib::DenoWorker => {
        &self.memoized.deno_worker_check_compiler_options
      }
    };

    cell.get_or_try_init(|| {
      let compiler_options_result = self.dir.to_resolved_compiler_options(
        &self.sys,
        CompilerOptionsType::Check { lib },
      )?;
      check_warn_compiler_options(
        &compiler_options_result,
        &self.logged_warnings,
      );
      Ok(new_rc(compiler_options_result.compiler_options))
    })
  }

  pub fn emit_compiler_options(
    &self,
  ) -> Result<&CompilerOptionsRc, CompilerOptionsParseError> {
    self.memoized.emit_compiler_options.get_or_try_init(|| {
      let compiler_options_result = self
        .dir
        .to_resolved_compiler_options(&self.sys, CompilerOptionsType::Emit)?;
      check_warn_compiler_options(
        &compiler_options_result,
        &self.logged_warnings,
      );
      Ok(new_rc(compiler_options_result.compiler_options))
    })
  }

  #[cfg(feature = "deno_ast")]
  pub fn transpile_options(
    &self,
  ) -> Result<&TranspileAndEmitOptionsRc, CompilerOptionsParseError> {
    self.memoized.transpile_options.get_or_try_init(|| {
      let compiler_options = self.emit_compiler_options()?;
      compiler_options_to_transpile_and_emit_options(
        compiler_options.as_ref().clone(),
      )
      .map(new_rc)
      .map_err(|source| CompilerOptionsParseError {
        specifier: self
          .dir
          .maybe_deno_json()
          .map(|d| d.specifier.clone())
          .unwrap_or_else(|| {
            // will never happen because each dir should have a
            // deno.json if we got here
            debug_assert!(false);
            self.dir.dir_url().as_ref().clone()
          }),
        source,
      })
    })
  }
}

#[derive(Debug)]
pub struct TsConfigResolver<TSys: FsRead> {
  map: FolderScopedMap<TsConfigFolderInfo<TSys>>,
}

impl<TSys: FsRead + Clone> TsConfigResolver<TSys> {
  pub fn from_workspace(sys: &TSys, workspace: &WorkspaceRc) -> Self {
    // separate the workspace into directories that have compiler options
    let root_dir = workspace.resolve_member_dir(workspace.root_dir());
    let logged_warnings = new_rc(LoggedWarnings::default());
    let mut map = FolderScopedMap::new(TsConfigFolderInfo {
      dir: root_dir,
      logged_warnings: logged_warnings.clone(),
      memoized: Default::default(),
      sys: sys.clone(),
    });
    for (url, folder) in workspace.config_folders() {
      let folder_has_compiler_options = folder
        .deno_json
        .as_ref()
        .map(|d| d.json.compiler_options.is_some())
        .unwrap_or(false);
      if url != workspace.root_dir() && folder_has_compiler_options {
        let dir = workspace.resolve_member_dir(url);
        map.insert(
          url.clone(),
          TsConfigFolderInfo {
            dir,
            logged_warnings: logged_warnings.clone(),
            memoized: Default::default(),
            sys: sys.clone(),
          },
        );
      }
    }
    Self { map }
  }
}

impl<TSys: FsRead> TsConfigResolver<TSys> {
  #[cfg(feature = "deno_ast")]
  pub fn transpile_and_emit_options(
    &self,
    specifier: &Url,
  ) -> Result<&TranspileAndEmitOptionsRc, CompilerOptionsParseError> {
    let value = self.map.get_for_specifier(specifier);
    value.transpile_options()
  }

  pub fn folder_for_specifier(
    &self,
    specifier: &Url,
  ) -> &TsConfigFolderInfo<TSys> {
    self.folder_for_specifier_str(specifier.as_str())
  }

  pub fn folder_for_specifier_str(
    &self,
    specifier: &str,
  ) -> &TsConfigFolderInfo<TSys> {
    self.map.get_for_specifier_str(specifier)
  }

  pub fn folder_count(&self) -> usize {
    self.map.count()
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
