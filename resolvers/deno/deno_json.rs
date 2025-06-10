// Copyright 2018-2025 the Deno authors. MIT license.

use deno_config::deno_json::CompilerOptionsParseError;
use deno_config::deno_json::TsConfig;
use deno_config::deno_json::TsConfigType;
use deno_config::deno_json::TsConfigWithIgnoredOptions;
use deno_config::deno_json::TsTypeLib;
use deno_config::workspace::WorkspaceDirectory;
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
type TsConfigRc = crate::sync::MaybeArc<TsConfig>;
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
  deno_window_check_tsconfig: OnceCell<TsConfigRc>,
  deno_worker_check_tsconfig: OnceCell<TsConfigRc>,
  emit_tsconfig: OnceCell<TsConfigRc>,
  #[cfg(feature = "deno_ast")]
  transpile_options: OnceCell<TranspileAndEmitOptionsRc>,
}

#[derive(Debug)]
pub struct TsConfigFolderInfo<TSys: FsRead> {
  pub dir: WorkspaceDirectory,
  logged_warnings: LoggedWarningsRc,
  memoized: MemoizedValues,
  sys: TSys,
}

impl<TSys: FsRead> TsConfigFolderInfo<TSys> {
  pub fn lib_tsconfig(
    &self,
    lib: TsTypeLib,
  ) -> Result<&TsConfigRc, CompilerOptionsParseError> {
    let cell = match lib {
      TsTypeLib::DenoWindow => &self.memoized.deno_window_check_tsconfig,
      TsTypeLib::DenoWorker => &self.memoized.deno_worker_check_tsconfig,
    };

    cell.get_or_try_init(|| {
      let tsconfig_result = self
        .dir
        .to_resolved_ts_config(&self.sys, TsConfigType::Check { lib })?;
      check_warn_tsconfig(&tsconfig_result, &self.logged_warnings);
      Ok(new_rc(tsconfig_result.ts_config))
    })
  }

  pub fn emit_tsconfig(
    &self,
  ) -> Result<&TsConfigRc, CompilerOptionsParseError> {
    self.memoized.emit_tsconfig.get_or_try_init(|| {
      let tsconfig_result = self
        .dir
        .to_resolved_ts_config(&self.sys, TsConfigType::Emit)?;
      check_warn_tsconfig(&tsconfig_result, &self.logged_warnings);
      Ok(new_rc(tsconfig_result.ts_config))
    })
  }

  #[cfg(feature = "deno_ast")]
  pub fn transpile_options(
    &self,
  ) -> Result<&TranspileAndEmitOptionsRc, CompilerOptionsParseError> {
    self.memoized.transpile_options.get_or_try_init(|| {
      let ts_config = self.emit_tsconfig()?;
      ts_config_to_transpile_and_emit_options(ts_config.as_ref().clone())
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
    // separate the workspace into directories that have a tsconfig
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
  pub fn check_js_for_specifier(&self, specifier: &Url) -> bool {
    self.folder_for_specifier(specifier).dir.check_js()
  }

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

#[cfg(feature = "graph")]
impl<TSys: FsRead + std::fmt::Debug> deno_graph::CheckJsResolver
  for TsConfigResolver<TSys>
{
  fn resolve(&self, specifier: &deno_graph::ModuleSpecifier) -> bool {
    self.check_js_for_specifier(specifier)
  }
}

#[cfg(feature = "deno_ast")]
fn ts_config_to_transpile_and_emit_options(
  config: deno_config::deno_json::TsConfig,
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

fn check_warn_tsconfig(
  ts_config: &TsConfigWithIgnoredOptions,
  logged_warnings: &LoggedWarnings,
) {
  for ignored_options in &ts_config.ignored_options {
    if ignored_options
      .maybe_specifier
      .as_ref()
      .map(|s| logged_warnings.folders.insert(s.clone()))
      .unwrap_or(true)
    {
      log::warn!("{}", ignored_options);
    }
  }
  let serde_json::Value::Object(obj) = &ts_config.ts_config.0 else {
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
