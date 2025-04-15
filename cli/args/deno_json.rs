// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::sync::Arc;

use deno_ast::SourceMapOption;
use deno_config::deno_json::CompilerOptionsParseError;
use deno_config::deno_json::TsConfig;
use deno_config::deno_json::TsConfigType;
use deno_config::deno_json::TsConfigWithIgnoredOptions;
use deno_config::deno_json::TsTypeLib;
use deno_config::workspace::Workspace;
use deno_config::workspace::WorkspaceDirectory;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::unsync::sync::AtomicFlag;
use deno_core::url::Url;
use deno_lib::util::hash::FastInsecureHasher;
use deno_lint::linter::LintConfig as DenoLintConfig;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use once_cell::sync::OnceCell;

use crate::util::collections::FolderScopedMap;

pub fn import_map_deps(
  import_map: &serde_json::Value,
) -> HashSet<JsrDepPackageReq> {
  let values = imports_values(import_map.get("imports"))
    .into_iter()
    .chain(scope_values(import_map.get("scopes")));
  values_to_set(values)
}

pub fn deno_json_deps(
  config: &deno_config::deno_json::ConfigFile,
) -> HashSet<JsrDepPackageReq> {
  let values = imports_values(config.json.imports.as_ref())
    .into_iter()
    .chain(scope_values(config.json.scopes.as_ref()));
  let mut set = values_to_set(values);

  if let Some(serde_json::Value::Object(compiler_options)) =
    &config.json.compiler_options
  {
    // add jsxImportSource
    if let Some(serde_json::Value::String(value)) =
      compiler_options.get("jsxImportSource")
    {
      if let Some(dep_req) = value_to_dep_req(value) {
        set.insert(dep_req);
      }
    }
    // add jsxImportSourceTypes
    if let Some(serde_json::Value::String(value)) =
      compiler_options.get("jsxImportSourceTypes")
    {
      if let Some(dep_req) = value_to_dep_req(value) {
        set.insert(dep_req);
      }
    }
    // add the dependencies in the types array
    if let Some(serde_json::Value::Array(types)) = compiler_options.get("types")
    {
      for value in types {
        if let serde_json::Value::String(value) = value {
          if let Some(dep_req) = value_to_dep_req(value) {
            set.insert(dep_req);
          }
        }
      }
    }
  }

  set
}

fn imports_values(value: Option<&serde_json::Value>) -> Vec<&String> {
  let Some(obj) = value.and_then(|v| v.as_object()) else {
    return Vec::new();
  };
  let mut items = Vec::with_capacity(obj.len());
  for value in obj.values() {
    if let serde_json::Value::String(value) = value {
      items.push(value);
    }
  }
  items
}

fn scope_values(value: Option<&serde_json::Value>) -> Vec<&String> {
  let Some(obj) = value.and_then(|v| v.as_object()) else {
    return Vec::new();
  };
  obj.values().flat_map(|v| imports_values(Some(v))).collect()
}

fn values_to_set<'a>(
  values: impl Iterator<Item = &'a String>,
) -> HashSet<JsrDepPackageReq> {
  let mut entries = HashSet::new();
  for value in values {
    if let Some(dep_req) = value_to_dep_req(value) {
      entries.insert(dep_req);
    }
  }
  entries
}

fn value_to_dep_req(value: &str) -> Option<JsrDepPackageReq> {
  if let Ok(req_ref) = JsrPackageReqReference::from_str(value) {
    Some(JsrDepPackageReq::jsr(req_ref.into_inner().req))
  } else if let Ok(req_ref) = NpmPackageReqReference::from_str(value) {
    Some(JsrDepPackageReq::npm(req_ref.into_inner().req))
  } else {
    None
  }
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
        deno_runtime::colors::yellow("Warning"),
      );
  }
}

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
  folders: dashmap::DashSet<Url>,
}

#[derive(Default, Debug)]
struct MemoizedValues {
  deno_window_check_tsconfig: OnceCell<Arc<TsConfig>>,
  deno_worker_check_tsconfig: OnceCell<Arc<TsConfig>>,
  emit_tsconfig: OnceCell<Arc<TsConfig>>,
  transpile_options: OnceCell<Arc<TranspileAndEmitOptions>>,
}

#[derive(Debug)]
pub struct TsConfigFolderInfo {
  pub dir: WorkspaceDirectory,
  logged_warnings: Arc<LoggedWarnings>,
  memoized: MemoizedValues,
}

impl TsConfigFolderInfo {
  pub fn lib_tsconfig(
    &self,
    lib: TsTypeLib,
  ) -> Result<&Arc<TsConfig>, CompilerOptionsParseError> {
    let cell = match lib {
      TsTypeLib::DenoWindow => &self.memoized.deno_window_check_tsconfig,
      TsTypeLib::DenoWorker => &self.memoized.deno_worker_check_tsconfig,
    };

    cell.get_or_try_init(|| {
      let tsconfig_result = self
        .dir
        .to_resolved_ts_config(TsConfigType::Check { lib })?;
      check_warn_tsconfig(&tsconfig_result, &self.logged_warnings);
      Ok(Arc::new(tsconfig_result.ts_config))
    })
  }

  pub fn emit_tsconfig(
    &self,
  ) -> Result<&Arc<TsConfig>, CompilerOptionsParseError> {
    self.memoized.emit_tsconfig.get_or_try_init(|| {
      let tsconfig_result =
        self.dir.to_resolved_ts_config(TsConfigType::Emit)?;
      check_warn_tsconfig(&tsconfig_result, &self.logged_warnings);
      Ok(Arc::new(tsconfig_result.ts_config))
    })
  }

  pub fn transpile_options(
    &self,
  ) -> Result<&Arc<TranspileAndEmitOptions>, CompilerOptionsParseError> {
    self.memoized.transpile_options.get_or_try_init(|| {
      let ts_config = self.emit_tsconfig()?;
      ts_config_to_transpile_and_emit_options(ts_config.as_ref().clone())
        .map(Arc::new)
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
pub struct TsConfigResolver {
  map: FolderScopedMap<TsConfigFolderInfo>,
}

impl TsConfigResolver {
  pub fn from_workspace(workspace: &Arc<Workspace>) -> Self {
    // separate the workspace into directories that have a tsconfig
    let root_dir = workspace.resolve_member_dir(workspace.root_dir());
    let logged_warnings = Arc::new(LoggedWarnings::default());
    let mut map = FolderScopedMap::new(TsConfigFolderInfo {
      dir: root_dir,
      logged_warnings: logged_warnings.clone(),
      memoized: Default::default(),
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
          },
        );
      }
    }
    Self { map }
  }

  pub fn check_js_for_specifier(&self, specifier: &Url) -> bool {
    self.folder_for_specifier(specifier).dir.check_js()
  }

  pub fn deno_lint_config(
    &self,
    specifier: &Url,
  ) -> Result<DenoLintConfig, AnyError> {
    let transpile_options =
      &self.transpile_and_emit_options(specifier)?.transpile;
    // don't bother storing this in a cell because deno_lint requires an owned value
    Ok(DenoLintConfig {
      default_jsx_factory: (!transpile_options.jsx_automatic)
        .then(|| transpile_options.jsx_factory.clone()),
      default_jsx_fragment_factory: (!transpile_options.jsx_automatic)
        .then(|| transpile_options.jsx_fragment_factory.clone()),
    })
  }

  pub fn transpile_and_emit_options(
    &self,
    specifier: &Url,
  ) -> Result<&Arc<TranspileAndEmitOptions>, CompilerOptionsParseError> {
    let value = self.map.get_for_specifier(specifier);
    value.transpile_options()
  }

  pub fn folder_for_specifier(&self, specifier: &Url) -> &TsConfigFolderInfo {
    self.folder_for_specifier_str(specifier.as_str())
  }

  pub fn folder_for_specifier_str(
    &self,
    specifier: &str,
  ) -> &TsConfigFolderInfo {
    self.map.get_for_specifier_str(specifier)
  }

  pub fn folder_count(&self) -> usize {
    self.map.count()
  }
}

impl deno_graph::CheckJsResolver for TsConfigResolver {
  fn resolve(&self, specifier: &deno_graph::ModuleSpecifier) -> bool {
    self.check_js_for_specifier(specifier)
  }
}

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
    SourceMapOption::Inline
  } else if options.source_map {
    SourceMapOption::Separate
  } else {
    SourceMapOption::None
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
    let mut hasher = FastInsecureHasher::new_without_deno_version();
    hasher.write_hashable(&transpile);
    hasher.write_hashable(&emit);
    hasher.finish()
  };
  Ok(TranspileAndEmitOptions {
    transpile,
    emit,
    pre_computed_hash: transpile_and_emit_options_hash,
  })
}
