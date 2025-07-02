// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_config::deno_json::parse_compiler_options;
use deno_config::deno_json::CompilerOptions;
use deno_config::deno_json::CompilerOptionsParseError;
use deno_config::deno_json::CompilerOptionsWithIgnoredOptions;
use deno_config::glob::PathOrPatternSet;
use deno_config::workspace::get_base_compiler_options_for_emit;
use deno_config::workspace::CompilerOptionsSource;
use deno_config::workspace::CompilerOptionsSourceKind;
use deno_config::workspace::CompilerOptionsType;
use deno_config::workspace::JsxImportSourceConfig;
use deno_config::workspace::JsxImportSourceSpecifierConfig;
use deno_config::workspace::ToMaybeJsxImportSourceConfigError;
use deno_config::workspace::TsTypeLib;
use deno_config::workspace::WorkspaceDirectory;
use deno_path_util::normalize_path;
use deno_path_util::url_from_file_path;
use deno_path_util::url_to_file_path;
use deno_terminal::colors;
use deno_unsync::sync::AtomicFlag;
use indexmap::IndexMap;
use indexmap::IndexSet;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::NodeResolutionKind;
use node_resolver::NodeResolver;
use node_resolver::ResolutionMode;
#[cfg(feature = "sync")]
use once_cell::sync::OnceCell;
#[cfg(not(feature = "sync"))]
use once_cell::unsync::OnceCell;
use sys_traits::FsRead;
use url::Url;

use crate::collections::FolderScopedMap;
use crate::factory::ConfigDiscoveryOption;
use crate::factory::WorkspaceDirectoryProvider;
use crate::npm::DenoInNpmPackageChecker;
use crate::npm::NpmResolver;
use crate::npm::NpmResolverSys;
use crate::sync::new_rc;

#[allow(clippy::disallowed_types)]
type CompilerOptionsRc = crate::sync::MaybeArc<CompilerOptions>;
#[allow(clippy::disallowed_types)]
pub type CompilerOptionsTypesRc =
  crate::sync::MaybeArc<Vec<(Url, Vec<String>)>>;

#[cfg(feature = "deno_ast")]
#[derive(Debug)]
pub struct TranspileAndEmitOptions {
  pub transpile: deno_ast::TranspileOptions,
  pub emit: deno_ast::EmitOptions,
  // stored ahead of time so we don't have to recompute this a lot
  pub pre_computed_hash: u64,
}

#[cfg(feature = "deno_ast")]
#[allow(clippy::disallowed_types)]
pub type TranspileAndEmitOptionsRc =
  crate::sync::MaybeArc<TranspileAndEmitOptions>;

#[derive(Debug, Default)]
struct LoggedWarnings {
  experimental_decorators: AtomicFlag,
  folders: crate::sync::MaybeDashSet<Url>,
}

#[allow(clippy::disallowed_types)]
type LoggedWarningsRc = crate::sync::MaybeArc<LoggedWarnings>;

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
pub struct CompilerOptionsData {
  pub sources: Vec<CompilerOptionsSource>,
  source_kind: CompilerOptionsSourceKind,
  memoized: MemoizedValues,
  logged_warnings: LoggedWarningsRc,
}

impl CompilerOptionsData {
  fn new(
    sources: Vec<CompilerOptionsSource>,
    source_kind: CompilerOptionsSourceKind,
    logged_warnings: LoggedWarningsRc,
  ) -> Self {
    Self {
      sources,
      source_kind,
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
        compiler_options: get_base_compiler_options_for_emit(
          typ,
          self.source_kind,
        ),
        ignored_options: Vec::new(),
      };
      for source in &self.sources {
        let Some(compiler_options) = source.compiler_options.as_ref() else {
          continue;
        };
        let object = serde_json::from_value(compiler_options.0.clone())
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
      if self.source_kind != CompilerOptionsSourceKind::TsConfig {
        check_warn_compiler_options(&result, &self.logged_warnings);
      }
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
            .as_ref()?
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
      let jsx = self.sources.iter().rev().find_map(|s| Some((s.compiler_options.as_ref()?.0.as_object()?.get("jsx")?.as_str()?, &s.specifier)));
      let is_jsx_automatic = matches!(
        jsx,
        Some(("react-jsx" | "preserve" | "react-jsxdev" | "precompile", _)),
      );
      let import_source = self.sources.iter().rev().find_map(|s| {
        Some(JsxImportSourceSpecifierConfig {
          specifier: s.compiler_options.as_ref()?.0.as_object()?.get("jsxImportSource")?.as_str()?.to_string(),
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
          specifier: s.compiler_options.as_ref()?.0.as_object()?.get("jsxImportSourceTypes")?.as_str()?.to_string(),
          base: s.specifier.clone()
        })
      }).or_else(|| import_source.clone());
      let module = match jsx {
        Some(("react-jsx" | "preserve", _)) => "jsx-runtime".to_string(),
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
          s.compiler_options
            .as_ref()?
            .0
            .as_object()?
            .get("checkJs")?
            .as_bool()
        })
        .unwrap_or(false)
    })
  }
}

// A resolved element of the `files` array in a tsconfig.
#[derive(Debug, Clone)]
pub struct TsConfigFile {
  pub relative_specifier: String,
  pub absolute_path: PathBuf,
}

impl TsConfigFile {
  fn from_raw(raw: &str, dir_path: impl AsRef<Path>) -> Self {
    let relative_specifier = if raw.starts_with("./")
      || raw.starts_with("../")
      || raw.starts_with('/')
    {
      raw.to_string()
    } else {
      format!("./{raw}")
    };
    let path = Path::new(raw);
    let absolute_path = if path.is_absolute() {
      normalize_path(path)
    } else {
      normalize_path(dir_path.as_ref().join(path))
    };
    Self {
      relative_specifier,
      absolute_path,
    }
  }
}

#[derive(Debug)]
struct TsConfigFileFilter {
  // Note that `files`, `include` and `exclude` are overwritten, not merged,
  // when using `extends`. So we only need to store one referrer for `files`.
  // See: https://www.typescriptlang.org/tsconfig/#extends.
  files: Option<(Url, Vec<TsConfigFile>)>,
  include: Option<PathOrPatternSet>,
  exclude: Option<PathOrPatternSet>,
  dir_path: PathBuf,
}

impl TsConfigFileFilter {
  fn includes_path(&self, path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    if let Some((_, files)) = &self.files {
      if files.iter().any(|f| f.absolute_path == path) {
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
pub struct TsConfigData {
  compiler_options: CompilerOptionsData,
  filter: TsConfigFileFilterRc,
  references: Vec<String>,
}

impl TsConfigData {
  pub fn files(&self) -> Option<(&Url, &Vec<TsConfigFile>)> {
    let (referrer, files) = self.filter.files.as_ref()?;
    Some((referrer, files))
  }

  fn specifier(&self) -> &Url {
    &self
      .compiler_options
      .sources
      .last()
      .expect("Tsconfigs should always have at least one source.")
      .specifier
  }
}

fn is_maybe_directory_error(err: &std::io::Error) -> bool {
  let kind = err.kind();
  kind == ErrorKind::IsADirectory
    // This happens on Windows for some reason.
    || cfg!(windows) && kind == ErrorKind::PermissionDenied
}

type TsConfigNodeResolver<TSys> = NodeResolver<
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  NpmResolver<TSys>,
  TSys,
>;

type GetNodeResolverFn<'a, NSys> =
  Box<dyn Fn(&Url) -> Option<&'a TsConfigNodeResolver<NSys>> + 'a>;

struct TsConfigCollector<'a, 'b, TSys: FsRead, NSys: NpmResolverSys> {
  roots: BTreeSet<PathBuf>,
  collected: IndexMap<Url, Rc<TsConfigData>>,
  read_cache: HashMap<PathBuf, Result<Rc<TsConfigData>, Rc<std::io::Error>>>,
  currently_reading: IndexSet<PathBuf>,
  sys: &'a TSys,
  get_node_resolver: GetNodeResolverFn<'b, NSys>,
  logged_warnings: &'a LoggedWarningsRc,
}

impl<TSys: FsRead + std::fmt::Debug, NSys: NpmResolverSys> std::fmt::Debug
  for TsConfigCollector<'_, '_, TSys, NSys>
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TsConfigCollector")
      .field("roots", &self.roots)
      .field("collected", &self.collected)
      .field("read_cache", &self.read_cache)
      .field("currently_reading", &self.currently_reading)
      .field("sys", &self.sys)
      .field("logged_warnings", &self.logged_warnings)
      .finish()
  }
}

impl<'a, 'b, TSys: FsRead, NSys: NpmResolverSys>
  TsConfigCollector<'a, 'b, TSys, NSys>
{
  fn new(
    sys: &'a TSys,
    get_node_resolver: GetNodeResolverFn<'b, NSys>,
    logged_warnings: &'a LoggedWarningsRc,
  ) -> Self {
    Self {
      roots: Default::default(),
      collected: Default::default(),
      read_cache: Default::default(),
      currently_reading: Default::default(),
      sys,
      get_node_resolver,
      logged_warnings,
    }
  }

  fn add_root(&mut self, path: PathBuf) {
    self.roots.insert(path);
  }

  fn collect(mut self) -> Vec<TsConfigData> {
    for root in std::mem::take(&mut self.roots) {
      let Ok(ts_config) = self.read_ts_config_with_cache(root) else {
        continue;
      };
      self.visit_reference(ts_config);
    }
    let Self { collected, .. } = { self };
    collected
      .into_values()
      .map(|t| {
        Rc::try_unwrap(t).expect(
          "No other references should be held since the read cache is dropped.",
        )
      })
      .collect()
  }

  fn visit_reference(&mut self, ts_config: Rc<TsConfigData>) {
    let specifier = ts_config.specifier();
    if self.collected.contains_key(specifier) {
      return;
    }
    let Some(dir_path) = url_to_file_path(specifier)
      .ok()
      .and_then(|p| Some(p.parent()?.to_path_buf()))
    else {
      return;
    };
    for reference in &ts_config.references {
      let reference_path = Path::new(reference);
      let reference_path = if reference_path.is_absolute() {
        Cow::Borrowed(reference_path)
      } else {
        Cow::Owned(dir_path.join(reference_path))
      };
      match self.read_ts_config_with_cache(&reference_path) {
        Ok(ts_config) => self.visit_reference(ts_config),
        Err(err) if is_maybe_directory_error(&err) => {
          if let Ok(ts_config) =
            self.read_ts_config_with_cache(reference_path.join("tsconfig.json"))
          {
            self.visit_reference(ts_config)
          }
        }
        _ => {}
      }
    }
    self.collected.insert(specifier.clone(), ts_config);
  }

  fn read_ts_config_with_cache(
    &mut self,
    path: impl AsRef<Path>,
  ) -> Result<Rc<TsConfigData>, Rc<std::io::Error>> {
    let path = normalize_path(path.as_ref());
    self.read_cache.get(&path).cloned().unwrap_or_else(|| {
      if !self.currently_reading.insert(path.clone()) {
        return Err(Rc::new(std::io::Error::new(
          ErrorKind::Other,
          "Cycle detected while following `extends`.",
        )));
      }
      let result = self.read_ts_config(&path).map(Rc::new).map_err(Rc::new);
      self.currently_reading.pop();
      self.read_cache.insert(path, result.clone());
      result
    })
  }

  fn read_ts_config(
    &mut self,
    path: impl AsRef<Path>,
  ) -> Result<TsConfigData, std::io::Error> {
    let path = path.as_ref();
    let warn = |err: &dyn std::fmt::Display| {
      log::warn!("Failed reading {}: {}", path.display(), err);
    };
    let specifier = url_from_file_path(path)
      .inspect_err(|e| warn(e))
      .map_err(|err| std::io::Error::new(ErrorKind::InvalidInput, err))?;
    let text = self.sys.fs_read_to_string(path).inspect_err(|e| {
      if e.kind() != ErrorKind::NotFound && !is_maybe_directory_error(e) {
        warn(e)
      }
    })?;
    let value = jsonc_parser::parse_to_serde_value(&text, &Default::default())
      .inspect_err(|e| warn(e))
      .ok()
      .flatten();
    let object = value.as_ref().and_then(|v| v.as_object());
    let extends_targets = object
      .and_then(|o| o.get("extends"))
      .into_iter()
      .flat_map(|v| {
        if let Some(s) = v.as_str() {
          vec![s]
        } else if let Some(a) = v.as_array() {
          a.iter().filter_map(|v| v.as_str()).collect()
        } else {
          Vec::new()
        }
      })
      .filter_map(|s| {
        let node_resolver = (self.get_node_resolver)(&specifier)?;
        let node_resolution = node_resolver
          .resolve(
            s,
            &specifier,
            ResolutionMode::Require,
            NodeResolutionKind::Execution,
          )
          .ok()?;
        let url = node_resolution.into_url().ok()?;
        let path = url_to_file_path(&url).ok()?;
        self.read_ts_config_with_cache(&path).ok()
      })
      .collect::<Vec<_>>();
    let sources = extends_targets
      .iter()
      .flat_map(|t| &t.compiler_options.sources)
      .cloned()
      .chain([CompilerOptionsSource {
        specifier: specifier.clone(),
        compiler_options: object
          .and_then(|o| o.get("compilerOptions"))
          .filter(|v| !v.is_null())
          .cloned()
          .map(CompilerOptions),
      }])
      .collect();
    let dir_path = path.parent().expect("file path should have a parent");
    let files = object
      .and_then(|o| {
        let files = o
          .get("files")?
          .as_array()?
          .iter()
          .filter_map(|v| Some(TsConfigFile::from_raw(v.as_str()?, dir_path)))
          .collect();
        Some((specifier, files))
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
    let references = object
      .and_then(|o| o.get("references")?.as_array())
      .into_iter()
      .flatten()
      .filter_map(|v| Some(v.as_object()?.get("path")?.as_str()?.to_string()))
      .collect();
    Ok(TsConfigData {
      compiler_options: CompilerOptionsData::new(
        sources,
        CompilerOptionsSourceKind::TsConfig,
        self.logged_warnings.clone(),
      ),
      filter: new_rc(TsConfigFileFilter {
        files,
        include,
        exclude,
        dir_path: dir_path.to_path_buf(),
      }),
      references,
    })
  }
}

#[derive(Debug)]
pub struct CompilerOptionsResolver {
  workspace_configs: FolderScopedMap<CompilerOptionsData>,
  ts_configs: Vec<TsConfigData>,
}

impl Default for CompilerOptionsResolver {
  fn default() -> Self {
    Self {
      workspace_configs: FolderScopedMap::new(CompilerOptionsData::new(
        Vec::new(),
        CompilerOptionsSourceKind::DenoJson,
        Default::default(),
      )),
      ts_configs: Vec::new(),
    }
  }
}

impl CompilerOptionsResolver {
  pub fn new<TSys: FsRead, NSys: NpmResolverSys>(
    sys: &TSys,
    workspace_directory_provider: &WorkspaceDirectoryProvider,
    node_resolver: &TsConfigNodeResolver<NSys>,
    config_discover: &ConfigDiscoveryOption,
  ) -> Self {
    if matches!(config_discover, ConfigDiscoveryOption::Disabled) {
      return Self::default();
    }
    let logged_warnings = new_rc(LoggedWarnings::default());
    let root_dir = workspace_directory_provider.root();
    let mut workspace_configs = FolderScopedMap::new(CompilerOptionsData::new(
      root_dir.to_configured_compiler_options_sources(),
      CompilerOptionsSourceKind::DenoJson,
      logged_warnings.clone(),
    ));
    let mut ts_config_collector = TsConfigCollector::new(
      sys,
      Box::new(|_| Some(node_resolver)),
      &logged_warnings,
    );
    for (dir_url, dir) in workspace_directory_provider.entries() {
      if dir.has_deno_or_pkg_json() {
        ts_config_collector.add_root(dir.dir_path().join("tsconfig.json"));
      }
      if let Some(dir_url) = dir_url {
        workspace_configs.insert(
          dir_url.clone(),
          CompilerOptionsData::new(
            dir.to_configured_compiler_options_sources(),
            CompilerOptionsSourceKind::DenoJson,
            logged_warnings.clone(),
          ),
        );
      }
    }
    Self {
      workspace_configs,
      ts_configs: ts_config_collector.collect(),
    }
  }

  pub fn unscoped(&self) -> &CompilerOptionsData {
    &self.workspace_configs.unscoped
  }

  pub fn for_specifier(&self, specifier: &Url) -> &CompilerOptionsData {
    if let Ok(path) = url_to_file_path(specifier) {
      for ts_config in &self.ts_configs {
        if ts_config.filter.includes_path(&path) {
          return &ts_config.compiler_options;
        }
      }
    }
    self.workspace_configs.get_for_specifier(specifier)
  }

  pub fn all(&self) -> impl Iterator<Item = &CompilerOptionsData> {
    self
      .workspace_configs
      .entries()
      .map(|(_, r)| r)
      .chain(self.ts_configs.iter().map(|t| &t.compiler_options))
  }

  pub fn size(&self) -> usize {
    self.workspace_configs.count() + self.ts_configs.len()
  }

  pub fn ts_configs(&self) -> &Vec<TsConfigData> {
    &self.ts_configs
  }

  pub fn new_for_lsp<TSys: FsRead, NSys: NpmResolverSys>(
    sys: &TSys,
    dirs: BTreeMap<&Url, &WorkspaceDirectory>,
    get_node_resolver: GetNodeResolverFn<'_, NSys>,
  ) -> Self {
    let logged_warnings = new_rc(LoggedWarnings::default());
    let mut workspace_configs = FolderScopedMap::new(CompilerOptionsData::new(
      Vec::new(),
      CompilerOptionsSourceKind::DenoJson,
      logged_warnings.clone(),
    ));
    let mut ts_config_collector =
      TsConfigCollector::new(sys, get_node_resolver, &logged_warnings);
    for dir in dirs.values() {
      if dir.has_deno_or_pkg_json() {
        ts_config_collector.add_root(dir.dir_path().join("tsconfig.json"));
      }
      workspace_configs.insert(
        dir.dir_url().clone(),
        CompilerOptionsData::new(
          dir.to_configured_compiler_options_sources(),
          CompilerOptionsSourceKind::DenoJson,
          logged_warnings.clone(),
        ),
      );
    }
    Self {
      workspace_configs,
      ts_configs: ts_config_collector.collect(),
    }
  }
}

#[cfg(feature = "graph")]
impl deno_graph::CheckJsResolver for CompilerOptionsResolver {
  fn resolve(&self, specifier: &Url) -> bool {
    self.for_specifier(specifier).check_js()
  }
}

#[allow(clippy::disallowed_types)]
pub type CompilerOptionsResolverRc =
  crate::sync::MaybeArc<CompilerOptionsResolver>;

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
        .try_map(|d| Ok(d.jsx_import_source_config()?.cloned()))?,
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

#[allow(clippy::disallowed_types)]
pub type JsxImportSourceConfigRc = crate::sync::MaybeArc<JsxImportSourceConfig>;

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
