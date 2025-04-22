// Copyright 2018-2025 the Deno authors. MIT license.

pub mod deno_json;
mod flags;
mod flags_net;
mod lockfile;
mod package_json;

use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_cache_dir::file_fetcher::CacheSetting;
pub use deno_config::deno_json::BenchConfig;
pub use deno_config::deno_json::ConfigFile;
use deno_config::deno_json::FmtConfig;
pub use deno_config::deno_json::FmtOptionsConfig;
pub use deno_config::deno_json::LintRulesConfig;
use deno_config::deno_json::NodeModulesDirMode;
pub use deno_config::deno_json::ProseWrap;
use deno_config::deno_json::TestConfig;
pub use deno_config::deno_json::TsConfig;
pub use deno_config::deno_json::TsTypeLib;
pub use deno_config::glob::FilePatterns;
use deno_config::workspace::Workspace;
use deno_config::workspace::WorkspaceDirLintConfig;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceLintConfig;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_graph::GraphKind;
use deno_lib::args::has_flag_env_var;
use deno_lib::args::npm_pkg_req_ref_to_binary_command;
use deno_lib::args::CaData;
use deno_lib::args::NPM_PROCESS_STATE;
use deno_lib::version::DENO_VERSION_INFO;
use deno_lib::worker::StorageKeyResolver;
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_permissions::PermissionsOptions;
use deno_runtime::inspector_server::InspectorServer;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::StackString;
use deno_telemetry::OtelConfig;
use deno_terminal::colors;
use dotenvy::from_filename;
pub use flags::*;
pub use lockfile::AtomicWriteFileWithRetriesError;
pub use lockfile::CliLockfile;
pub use lockfile::CliLockfileReadFromPathOptions;
use once_cell::sync::Lazy;
pub use package_json::NpmInstallDepsProvider;
pub use package_json::PackageJsonDepValueParseWithLocationError;
use sys_traits::FsRead;
use thiserror::Error;

use crate::sys::CliSys;

pub static DENO_DISABLE_PEDANTIC_NODE_WARNINGS: Lazy<bool> = Lazy::new(|| {
  std::env::var("DENO_DISABLE_PEDANTIC_NODE_WARNINGS")
    .ok()
    .is_some()
});

pub fn jsr_url() -> &'static Url {
  static JSR_URL: Lazy<Url> = Lazy::new(|| {
    let env_var_name = "JSR_URL";
    if let Ok(registry_url) = std::env::var(env_var_name) {
      // ensure there is a trailing slash for the directory
      let registry_url = format!("{}/", registry_url.trim_end_matches('/'));
      match Url::parse(&registry_url) {
        Ok(url) => {
          return url;
        }
        Err(err) => {
          log::debug!(
            "Invalid {} environment variable: {:#}",
            env_var_name,
            err,
          );
        }
      }
    }

    Url::parse("https://jsr.io/").unwrap()
  });

  &JSR_URL
}

pub fn jsr_api_url() -> &'static Url {
  static JSR_API_URL: Lazy<Url> = Lazy::new(|| {
    let mut jsr_api_url = jsr_url().clone();
    jsr_api_url.set_path("api/");
    jsr_api_url
  });

  &JSR_API_URL
}

#[derive(Debug, Clone)]
pub struct ExternalImportMap {
  pub path: PathBuf,
  pub value: serde_json::Value,
}

#[derive(Debug)]
pub struct WorkspaceExternalImportMapLoader {
  sys: CliSys,
  workspace: Arc<Workspace>,
  maybe_external_import_map:
    once_cell::sync::OnceCell<Option<ExternalImportMap>>,
}

impl WorkspaceExternalImportMapLoader {
  pub fn new(sys: CliSys, workspace: Arc<Workspace>) -> Self {
    Self {
      sys,
      workspace,
      maybe_external_import_map: Default::default(),
    }
  }

  pub fn get_or_load(&self) -> Result<Option<&ExternalImportMap>, AnyError> {
    self
      .maybe_external_import_map
      .get_or_try_init(|| {
        let Some(deno_json) = self.workspace.root_deno_json() else {
          return Ok(None);
        };
        if deno_json.is_an_import_map() {
          return Ok(None);
        }
        let Some(path) = deno_json.to_import_map_path()? else {
          return Ok(None);
        };
        let contents =
          self.sys.fs_read_to_string(&path).with_context(|| {
            format!("Unable to read import map at '{}'", path.display())
          })?;
        let value = serde_json::from_str(&contents)?;
        Ok(Some(ExternalImportMap { path, value }))
      })
      .map(|v| v.as_ref())
  }
}

pub struct WorkspaceBenchOptions {
  pub filter: Option<String>,
  pub json: bool,
  pub no_run: bool,
  pub permit_no_files: bool,
}

impl WorkspaceBenchOptions {
  pub fn resolve(bench_flags: &BenchFlags) -> Self {
    Self {
      filter: bench_flags.filter.clone(),
      json: bench_flags.json,
      no_run: bench_flags.no_run,
      permit_no_files: bench_flags.permit_no_files,
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchOptions {
  pub files: FilePatterns,
}

impl BenchOptions {
  pub fn resolve(bench_config: BenchConfig, _bench_flags: &BenchFlags) -> Self {
    // this is the same, but keeping the same pattern as everywhere else for the future
    Self {
      files: bench_config.files,
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct UnstableFmtOptions {
  pub component: bool,
  pub sql: bool,
}

#[derive(Clone, Debug)]
pub struct FmtOptions {
  pub options: FmtOptionsConfig,
  pub unstable: UnstableFmtOptions,
  pub files: FilePatterns,
}

impl Default for FmtOptions {
  fn default() -> Self {
    Self::new_with_base(PathBuf::from("/"))
  }
}

impl FmtOptions {
  pub fn new_with_base(base: PathBuf) -> Self {
    Self {
      options: FmtOptionsConfig::default(),
      unstable: Default::default(),
      files: FilePatterns::new_with_base(base),
    }
  }

  pub fn resolve(
    fmt_config: FmtConfig,
    unstable: UnstableFmtOptions,
    fmt_flags: &FmtFlags,
  ) -> Self {
    Self {
      options: resolve_fmt_options(fmt_flags, fmt_config.options),
      unstable: UnstableFmtOptions {
        component: unstable.component || fmt_flags.unstable_component,
        sql: unstable.sql || fmt_flags.unstable_sql,
      },
      files: fmt_config.files,
    }
  }
}

fn resolve_fmt_options(
  fmt_flags: &FmtFlags,
  mut options: FmtOptionsConfig,
) -> FmtOptionsConfig {
  if let Some(use_tabs) = fmt_flags.use_tabs {
    options.use_tabs = Some(use_tabs);
  }

  if let Some(line_width) = fmt_flags.line_width {
    options.line_width = Some(line_width.get());
  }

  if let Some(indent_width) = fmt_flags.indent_width {
    options.indent_width = Some(indent_width.get());
  }

  if let Some(single_quote) = fmt_flags.single_quote {
    options.single_quote = Some(single_quote);
  }

  if let Some(prose_wrap) = &fmt_flags.prose_wrap {
    options.prose_wrap = Some(match prose_wrap.as_str() {
      "always" => ProseWrap::Always,
      "never" => ProseWrap::Never,
      "preserve" => ProseWrap::Preserve,
      // validators in `flags.rs` makes other values unreachable
      _ => unreachable!(),
    });
  }

  if let Some(no_semis) = &fmt_flags.no_semicolons {
    options.semi_colons = Some(!no_semis);
  }

  options
}

#[derive(Clone, Debug)]
pub struct WorkspaceTestOptions {
  pub doc: bool,
  pub no_run: bool,
  pub fail_fast: Option<NonZeroUsize>,
  pub permit_no_files: bool,
  pub filter: Option<String>,
  pub shuffle: Option<u64>,
  pub concurrent_jobs: NonZeroUsize,
  pub trace_leaks: bool,
  pub reporter: TestReporterConfig,
  pub junit_path: Option<String>,
  pub hide_stacktraces: bool,
}

impl WorkspaceTestOptions {
  pub fn resolve(test_flags: &TestFlags) -> Self {
    Self {
      permit_no_files: test_flags.permit_no_files,
      concurrent_jobs: test_flags
        .concurrent_jobs
        .unwrap_or_else(|| NonZeroUsize::new(1).unwrap()),
      doc: test_flags.doc,
      fail_fast: test_flags.fail_fast,
      filter: test_flags.filter.clone(),
      no_run: test_flags.no_run,
      shuffle: test_flags.shuffle,
      trace_leaks: test_flags.trace_leaks,
      reporter: test_flags.reporter,
      junit_path: test_flags.junit_path.clone(),
      hide_stacktraces: test_flags.hide_stacktraces,
    }
  }
}

#[derive(Debug, Clone)]
pub struct TestOptions {
  pub files: FilePatterns,
}

impl TestOptions {
  pub fn resolve(test_config: TestConfig, _test_flags: &TestFlags) -> Self {
    // this is the same, but keeping the same pattern as everywhere else for the future
    Self {
      files: test_config.files,
    }
  }
}

#[derive(Clone, Copy, Default, Debug)]
pub enum LintReporterKind {
  #[default]
  Pretty,
  Json,
  Compact,
}

#[derive(Clone, Debug)]
pub struct WorkspaceLintOptions {
  pub reporter_kind: LintReporterKind,
}

impl WorkspaceLintOptions {
  pub fn resolve(
    lint_config: &WorkspaceLintConfig,
    lint_flags: &LintFlags,
  ) -> Result<Self, AnyError> {
    let mut maybe_reporter_kind = if lint_flags.json {
      Some(LintReporterKind::Json)
    } else if lint_flags.compact {
      Some(LintReporterKind::Compact)
    } else {
      None
    };

    if maybe_reporter_kind.is_none() {
      // Flag not set, so try to get lint reporter from the config file.
      maybe_reporter_kind = match lint_config.report.as_deref() {
        Some("json") => Some(LintReporterKind::Json),
        Some("compact") => Some(LintReporterKind::Compact),
        Some("pretty") => Some(LintReporterKind::Pretty),
        Some(_) => {
          bail!("Invalid lint report type in config file")
        }
        None => None,
      }
    }
    Ok(Self {
      reporter_kind: maybe_reporter_kind.unwrap_or_default(),
    })
  }
}

#[derive(Clone, Debug)]
pub struct LintOptions {
  pub rules: LintRulesConfig,
  pub files: FilePatterns,
  pub fix: bool,
  pub plugins: Vec<Url>,
}

impl Default for LintOptions {
  fn default() -> Self {
    Self::new_with_base(PathBuf::from("/"))
  }
}

impl LintOptions {
  pub fn new_with_base(base: PathBuf) -> Self {
    Self {
      rules: Default::default(),
      files: FilePatterns::new_with_base(base),
      fix: false,
      plugins: vec![],
    }
  }

  pub fn resolve(
    lint_config: WorkspaceDirLintConfig,
    lint_flags: &LintFlags,
  ) -> Result<Self, AnyError> {
    let rules = resolve_lint_rules_options(
      lint_config.rules,
      lint_flags.maybe_rules_tags.clone(),
      lint_flags.maybe_rules_include.clone(),
      lint_flags.maybe_rules_exclude.clone(),
    );

    let mut plugins = lint_config.plugins;
    plugins.sort_unstable();

    Ok(Self {
      files: lint_config.files,
      rules,
      fix: lint_flags.fix,
      plugins,
    })
  }
}

fn resolve_lint_rules_options(
  config_rules: LintRulesConfig,
  mut maybe_rules_tags: Option<Vec<String>>,
  mut maybe_rules_include: Option<Vec<String>>,
  mut maybe_rules_exclude: Option<Vec<String>>,
) -> LintRulesConfig {
  // Try to get configured rules. CLI flags take precedence
  // over config file, i.e. if there's `rules.include` in config file
  // and `--rules-include` CLI flag, only the flag value is taken into account.
  if maybe_rules_include.is_none() {
    maybe_rules_include = config_rules.include;
  }
  if maybe_rules_exclude.is_none() {
    maybe_rules_exclude = config_rules.exclude;
  }
  if maybe_rules_tags.is_none() {
    maybe_rules_tags = config_rules.tags;
  }

  LintRulesConfig {
    exclude: maybe_rules_exclude,
    include: maybe_rules_include,
    tags: maybe_rules_tags,
  }
}

/// Holds the resolved options of many sources used by subcommands
/// and provides some helper function for creating common objects.
#[derive(Debug)]
pub struct CliOptions {
  // the source of the options is a detail the rest of the
  // application need not concern itself with, so keep these private
  flags: Arc<Flags>,
  initial_cwd: PathBuf,
  main_module_cell: std::sync::OnceLock<Result<ModuleSpecifier, AnyError>>,
  pub start_dir: Arc<WorkspaceDirectory>,
}

impl CliOptions {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    flags: Arc<Flags>,
    initial_cwd: PathBuf,
    start_dir: Arc<WorkspaceDirectory>,
  ) -> Result<Self, AnyError> {
    if let Some(insecure_allowlist) =
      flags.unsafely_ignore_certificate_errors.as_ref()
    {
      let domains = if insecure_allowlist.is_empty() {
        "for all hostnames".to_string()
      } else {
        format!("for: {}", insecure_allowlist.join(", "))
      };
      let msg =
        format!("DANGER: TLS certificate validation is disabled {}", domains);
      {
        log::error!("{}", colors::yellow(msg));
      }
    }

    load_env_variables_from_env_file(flags.env_file.as_ref());

    Ok(Self {
      flags,
      initial_cwd,
      main_module_cell: std::sync::OnceLock::new(),
      start_dir,
    })
  }

  pub fn from_flags(
    flags: Arc<Flags>,
    initial_cwd: PathBuf,
    start_dir: Arc<WorkspaceDirectory>,
  ) -> Result<Self, AnyError> {
    for diagnostic in start_dir.workspace.diagnostics() {
      log::warn!("{} {}", colors::yellow("Warning"), diagnostic);
    }

    log::debug!("Finished config loading.");

    Self::new(flags, initial_cwd, start_dir)
  }

  #[inline(always)]
  pub fn initial_cwd(&self) -> &Path {
    &self.initial_cwd
  }

  #[inline(always)]
  pub fn workspace(&self) -> &Arc<Workspace> {
    &self.start_dir.workspace
  }

  pub fn graph_kind(&self) -> GraphKind {
    match self.sub_command() {
      DenoSubcommand::Cache(_) => GraphKind::All,
      DenoSubcommand::Check(_) => GraphKind::TypesOnly,
      DenoSubcommand::Install(InstallFlags::Local(_)) => GraphKind::All,
      _ => self.type_check_mode().as_graph_kind(),
    }
  }

  pub fn ts_type_lib_window(&self) -> TsTypeLib {
    TsTypeLib::DenoWindow
  }

  pub fn ts_type_lib_worker(&self) -> TsTypeLib {
    TsTypeLib::DenoWorker
  }

  pub fn cache_setting(&self) -> CacheSetting {
    if self.flags.cached_only {
      CacheSetting::Only
    } else if !self.flags.cache_blocklist.is_empty() {
      CacheSetting::ReloadSome(self.flags.cache_blocklist.clone())
    } else if self.flags.reload {
      CacheSetting::ReloadAll
    } else {
      CacheSetting::Use
    }
  }

  pub fn npm_system_info(&self) -> NpmSystemInfo {
    self.sub_command().npm_system_info()
  }

  /// Resolve the specifier for a specified import map.
  ///
  /// This will NOT include the config file if it
  /// happens to be an import map.
  pub fn resolve_specified_import_map_specifier(
    &self,
  ) -> Result<Option<ModuleSpecifier>, ImportMapSpecifierResolveError> {
    resolve_import_map_specifier(
      self.flags.import_map_path.as_deref(),
      self.workspace().root_deno_json().map(|c| c.as_ref()),
      &self.initial_cwd,
    )
  }

  pub fn node_ipc_fd(&self) -> Option<i64> {
    let maybe_node_channel_fd = std::env::var("NODE_CHANNEL_FD").ok();
    if let Some(node_channel_fd) = maybe_node_channel_fd {
      // Remove so that child processes don't inherit this environment variable.
      std::env::remove_var("NODE_CHANNEL_FD");
      node_channel_fd.parse::<i64>().ok()
    } else {
      None
    }
  }

  pub fn serve_port(&self) -> Option<u16> {
    if let DenoSubcommand::Serve(flags) = self.sub_command() {
      Some(flags.port)
    } else {
      None
    }
  }

  pub fn serve_host(&self) -> Option<String> {
    if let DenoSubcommand::Serve(flags) = self.sub_command() {
      Some(flags.host.clone())
    } else {
      None
    }
  }

  pub fn eszip(&self) -> bool {
    self.flags.eszip
  }

  pub fn otel_config(&self) -> OtelConfig {
    self.flags.otel_config()
  }

  pub fn no_legacy_abort(&self) -> bool {
    self.flags.no_legacy_abort()
  }

  pub fn env_file_name(&self) -> Option<&Vec<String>> {
    self.flags.env_file.as_ref()
  }

  pub fn resolve_main_module(&self) -> Result<&ModuleSpecifier, AnyError> {
    self
      .main_module_cell
      .get_or_init(|| {
        Ok(match &self.flags.subcommand {
          DenoSubcommand::Compile(compile_flags) => {
            resolve_url_or_path(&compile_flags.source_file, self.initial_cwd())?
          }
          DenoSubcommand::Eval(_) => {
            resolve_url_or_path("./$deno$eval.mts", self.initial_cwd())?
          }
          DenoSubcommand::Repl(_) => {
            resolve_url_or_path("./$deno$repl.mts", self.initial_cwd())?
          }
          DenoSubcommand::Run(run_flags) => {
            if run_flags.is_stdin() {
              resolve_url_or_path("./$deno$stdin.mts", self.initial_cwd())?
            } else {
              let url =
                resolve_url_or_path(&run_flags.script, self.initial_cwd())?;
              if self.is_node_main()
                && url.scheme() == "file"
                && MediaType::from_specifier(&url) == MediaType::Unknown
              {
                try_resolve_node_binary_main_entrypoint(
                  &run_flags.script,
                  self.initial_cwd(),
                )?
                .unwrap_or(url)
              } else {
                url
              }
            }
          }
          DenoSubcommand::Serve(run_flags) => {
            resolve_url_or_path(&run_flags.script, self.initial_cwd())?
          }
          _ => {
            bail!("No main module.")
          }
        })
      })
      .as_ref()
      .map_err(|err| deno_core::anyhow::anyhow!("{}", err))
  }

  pub fn resolve_file_header_overrides(
    &self,
  ) -> HashMap<ModuleSpecifier, HashMap<String, String>> {
    let maybe_main_specifier = self.resolve_main_module().ok();
    // TODO(Cre3per): This mapping moved to deno_ast with https://github.com/denoland/deno_ast/issues/133 and should be available in deno_ast >= 0.25.0 via `MediaType::from_path(...).as_media_type()`
    let maybe_content_type =
      self.flags.ext.as_ref().and_then(|el| match el.as_str() {
        "ts" => Some("text/typescript"),
        "tsx" => Some("text/tsx"),
        "js" => Some("text/javascript"),
        "jsx" => Some("text/jsx"),
        _ => None,
      });

    if let (Some(main_specifier), Some(content_type)) =
      (maybe_main_specifier, maybe_content_type)
    {
      HashMap::from([(
        main_specifier.clone(),
        HashMap::from([("content-type".to_string(), content_type.to_string())]),
      )])
    } else {
      HashMap::default()
    }
  }

  pub fn resolve_storage_key_resolver(&self) -> StorageKeyResolver {
    if let Some(location) = &self.flags.location {
      StorageKeyResolver::from_flag(location)
    } else if let Some(deno_json) = self.start_dir.maybe_deno_json() {
      StorageKeyResolver::from_config_file_url(&deno_json.specifier)
    } else {
      StorageKeyResolver::new_use_main_module()
    }
  }

  // If the main module should be treated as being in an npm package.
  // This is triggered via a secret environment variable which is used
  // for functionality like child_process.fork. Users should NOT depend
  // on this functionality.
  pub fn is_node_main(&self) -> bool {
    NPM_PROCESS_STATE.is_some()
  }

  /// Gets the explicitly specified NodeModulesDir setting.
  ///
  /// Use `WorkspaceFactory.node_modules_dir_mode()` to get the resolved value.
  pub fn specified_node_modules_dir(
    &self,
  ) -> Result<
    Option<NodeModulesDirMode>,
    deno_config::deno_json::NodeModulesDirParseError,
  > {
    if let Some(flag) = self.flags.node_modules_dir {
      return Ok(Some(flag));
    }
    self.workspace().node_modules_dir()
  }

  pub fn vendor_dir_path(&self) -> Option<&PathBuf> {
    self.workspace().vendor_dir_path()
  }

  pub fn resolve_inspector_server(
    &self,
  ) -> Result<Option<InspectorServer>, AnyError> {
    let maybe_inspect_host = self
      .flags
      .inspect
      .or(self.flags.inspect_brk)
      .or(self.flags.inspect_wait);

    let Some(host) = maybe_inspect_host else {
      return Ok(None);
    };

    Ok(Some(InspectorServer::new(
      host,
      DENO_VERSION_INFO.user_agent,
    )?))
  }

  pub fn resolve_fmt_options_for_members(
    &self,
    fmt_flags: &FmtFlags,
  ) -> Result<Vec<(WorkspaceDirectory, FmtOptions)>, AnyError> {
    let cli_arg_patterns =
      fmt_flags.files.as_file_patterns(self.initial_cwd())?;
    let member_configs = self
      .workspace()
      .resolve_fmt_config_for_members(&cli_arg_patterns)?;
    let unstable = self.resolve_config_unstable_fmt_options();
    let mut result = Vec::with_capacity(member_configs.len());
    for (ctx, config) in member_configs {
      let options = FmtOptions::resolve(config, unstable.clone(), fmt_flags);
      result.push((ctx, options));
    }
    Ok(result)
  }

  pub fn resolve_config_unstable_fmt_options(&self) -> UnstableFmtOptions {
    let workspace = self.workspace();
    UnstableFmtOptions {
      component: workspace.has_unstable("fmt-component"),
      sql: workspace.has_unstable("fmt-sql"),
    }
  }

  pub fn resolve_workspace_lint_options(
    &self,
    lint_flags: &LintFlags,
  ) -> Result<WorkspaceLintOptions, AnyError> {
    let lint_config = self.workspace().to_lint_config()?;
    WorkspaceLintOptions::resolve(&lint_config, lint_flags)
  }

  pub fn resolve_lint_options_for_members(
    &self,
    lint_flags: &LintFlags,
  ) -> Result<Vec<(WorkspaceDirectory, LintOptions)>, AnyError> {
    let cli_arg_patterns =
      lint_flags.files.as_file_patterns(self.initial_cwd())?;
    let member_configs = self
      .workspace()
      .resolve_lint_config_for_members(&cli_arg_patterns)?;
    let mut result = Vec::with_capacity(member_configs.len());
    for (ctx, config) in member_configs {
      let options = LintOptions::resolve(config, lint_flags)?;
      result.push((ctx, options));
    }
    Ok(result)
  }

  pub fn resolve_workspace_test_options(
    &self,
    test_flags: &TestFlags,
  ) -> WorkspaceTestOptions {
    WorkspaceTestOptions::resolve(test_flags)
  }

  pub fn resolve_test_options_for_members(
    &self,
    test_flags: &TestFlags,
  ) -> Result<Vec<(WorkspaceDirectory, TestOptions)>, AnyError> {
    let cli_arg_patterns =
      test_flags.files.as_file_patterns(self.initial_cwd())?;
    let workspace_dir_configs = self
      .workspace()
      .resolve_test_config_for_members(&cli_arg_patterns)?;
    let mut result = Vec::with_capacity(workspace_dir_configs.len());
    for (member_dir, config) in workspace_dir_configs {
      let options = TestOptions::resolve(config, test_flags);
      result.push((member_dir, options));
    }
    Ok(result)
  }

  pub fn resolve_workspace_bench_options(
    &self,
    bench_flags: &BenchFlags,
  ) -> WorkspaceBenchOptions {
    WorkspaceBenchOptions::resolve(bench_flags)
  }

  pub fn resolve_bench_options_for_members(
    &self,
    bench_flags: &BenchFlags,
  ) -> Result<Vec<(WorkspaceDirectory, BenchOptions)>, AnyError> {
    let cli_arg_patterns =
      bench_flags.files.as_file_patterns(self.initial_cwd())?;
    let workspace_dir_configs = self
      .workspace()
      .resolve_bench_config_for_members(&cli_arg_patterns)?;
    let mut result = Vec::with_capacity(workspace_dir_configs.len());
    for (member_dir, config) in workspace_dir_configs {
      let options = BenchOptions::resolve(config, bench_flags);
      result.push((member_dir, options));
    }
    Ok(result)
  }

  /// Vector of user script CLI arguments.
  pub fn argv(&self) -> &Vec<String> {
    &self.flags.argv
  }

  pub fn ca_data(&self) -> &Option<CaData> {
    &self.flags.ca_data
  }

  pub fn ca_stores(&self) -> &Option<Vec<String>> {
    &self.flags.ca_stores
  }

  pub fn coverage_dir(&self) -> Option<String> {
    match &self.flags.subcommand {
      DenoSubcommand::Test(test) => test
        .coverage_dir
        .as_ref()
        .map(ToOwned::to_owned)
        .or_else(|| env::var("DENO_COVERAGE_DIR").ok()),
      _ => None,
    }
  }

  pub fn enable_op_summary_metrics(&self) -> bool {
    self.flags.enable_op_summary_metrics
      || matches!(
        self.flags.subcommand,
        DenoSubcommand::Test(_)
          | DenoSubcommand::Repl(_)
          | DenoSubcommand::Jupyter(_)
      )
  }

  pub fn enable_testing_features(&self) -> bool {
    self.flags.enable_testing_features
  }

  pub fn ext_flag(&self) -> &Option<String> {
    &self.flags.ext
  }

  pub fn has_hmr(&self) -> bool {
    if let DenoSubcommand::Run(RunFlags {
      watch: Some(WatchFlagsWithPaths { hmr, .. }),
      ..
    }) = &self.flags.subcommand
    {
      *hmr
    } else if let DenoSubcommand::Serve(ServeFlags {
      watch: Some(WatchFlagsWithPaths { hmr, .. }),
      ..
    }) = &self.flags.subcommand
    {
      *hmr
    } else {
      false
    }
  }

  /// If the --inspect or --inspect-brk flags are used.
  pub fn is_inspecting(&self) -> bool {
    self.flags.inspect.is_some()
      || self.flags.inspect_brk.is_some()
      || self.flags.inspect_wait.is_some()
  }

  pub fn inspect_brk(&self) -> Option<SocketAddr> {
    self.flags.inspect_brk
  }

  pub fn inspect_wait(&self) -> Option<SocketAddr> {
    self.flags.inspect_wait
  }

  pub fn log_level(&self) -> Option<log::Level> {
    self.flags.log_level
  }

  pub fn is_quiet(&self) -> bool {
    self
      .log_level()
      .map(|l| l == log::Level::Error)
      .unwrap_or(false)
  }

  pub fn location_flag(&self) -> &Option<Url> {
    &self.flags.location
  }

  pub fn no_remote(&self) -> bool {
    self.flags.no_remote
  }

  pub fn no_npm(&self) -> bool {
    self.flags.no_npm
  }

  pub fn permissions_options(&self) -> PermissionsOptions {
    // bury this in here to ensure people use cli_options.permissions_options()
    fn flags_to_options(flags: &PermissionFlags) -> PermissionsOptions {
      fn handle_allow<T: Default>(
        allow_all: bool,
        value: Option<T>,
      ) -> Option<T> {
        if allow_all {
          assert!(value.is_none());
          Some(T::default())
        } else {
          value
        }
      }

      PermissionsOptions {
        allow_all: flags.allow_all,
        allow_env: handle_allow(flags.allow_all, flags.allow_env.clone()),
        deny_env: flags.deny_env.clone(),
        allow_net: handle_allow(flags.allow_all, flags.allow_net.clone()),
        deny_net: flags.deny_net.clone(),
        allow_ffi: handle_allow(flags.allow_all, flags.allow_ffi.clone()),
        deny_ffi: flags.deny_ffi.clone(),
        allow_read: handle_allow(flags.allow_all, flags.allow_read.clone()),
        deny_read: flags.deny_read.clone(),
        allow_run: handle_allow(flags.allow_all, flags.allow_run.clone()),
        deny_run: flags.deny_run.clone(),
        allow_sys: handle_allow(flags.allow_all, flags.allow_sys.clone()),
        deny_sys: flags.deny_sys.clone(),
        allow_write: handle_allow(flags.allow_all, flags.allow_write.clone()),
        deny_write: flags.deny_write.clone(),
        allow_import: handle_allow(flags.allow_all, flags.allow_import.clone()),
        prompt: !resolve_no_prompt(flags),
      }
    }

    let mut permissions_options = flags_to_options(&self.flags.permissions);
    self.augment_import_permissions(&mut permissions_options);
    permissions_options
  }

  fn augment_import_permissions(&self, options: &mut PermissionsOptions) {
    // do not add if the user specified --allow-all or --allow-import
    if !options.allow_all && options.allow_import.is_none() {
      options.allow_import = Some(self.implicit_allow_import());
    }
  }

  fn implicit_allow_import(&self) -> Vec<String> {
    // allow importing from anywhere when using cached only
    if self.cache_setting() == CacheSetting::Only {
      vec![] // allow all imports
    } else {
      // implicitly allow some trusted hosts and the CLI arg urls
      let cli_arg_urls = self.get_cli_arg_urls();
      let builtin_allowed_import_hosts = [
        "jsr.io:443",
        "deno.land:443",
        "esm.sh:443",
        "cdn.jsdelivr.net:443",
        "raw.githubusercontent.com:443",
        "gist.githubusercontent.com:443",
      ];
      let mut imports = Vec::with_capacity(
        builtin_allowed_import_hosts.len() + cli_arg_urls.len() + 1,
      );
      imports
        .extend(builtin_allowed_import_hosts.iter().map(|s| s.to_string()));
      // also add the JSR_URL env var
      if let Some(jsr_host) = allow_import_host_from_url(jsr_url()) {
        if jsr_host != "jsr.io:443" {
          imports.push(jsr_host);
        }
      }
      // include the cli arg urls
      for url in cli_arg_urls {
        if let Some(host) = allow_import_host_from_url(&url) {
          imports.push(host);
        }
      }
      imports
    }
  }

  fn get_cli_arg_urls(&self) -> Vec<Cow<'_, Url>> {
    fn files_to_urls(files: &[String]) -> Vec<Cow<'_, Url>> {
      files.iter().filter_map(|f| file_to_url(f)).collect()
    }

    fn file_to_url(file: &str) -> Option<Cow<'_, Url>> {
      Url::parse(file).ok().map(Cow::Owned)
    }

    self
      .resolve_main_module()
      .ok()
      .map(|url| vec![Cow::Borrowed(url)])
      .or_else(|| match &self.flags.subcommand {
        DenoSubcommand::Cache(cache_flags) => {
          Some(files_to_urls(&cache_flags.files))
        }
        DenoSubcommand::Check(check_flags) => {
          Some(files_to_urls(&check_flags.files))
        }
        DenoSubcommand::Install(InstallFlags::Global(flags)) => {
          file_to_url(&flags.module_url).map(|url| vec![url])
        }
        DenoSubcommand::Doc(DocFlags {
          source_files: DocSourceFileFlag::Paths(paths),
          ..
        }) => Some(files_to_urls(paths)),
        DenoSubcommand::Info(InfoFlags {
          file: Some(file), ..
        }) => file_to_url(file).map(|url| vec![url]),
        _ => None,
      })
      .unwrap_or_default()
  }

  pub fn reload_flag(&self) -> bool {
    self.flags.reload
  }

  pub fn seed(&self) -> Option<u64> {
    self.flags.seed
  }

  pub fn sub_command(&self) -> &DenoSubcommand {
    &self.flags.subcommand
  }

  pub fn strace_ops(&self) -> &Option<Vec<String>> {
    &self.flags.strace_ops
  }

  pub fn take_binary_npm_command_name(&self) -> Option<String> {
    match self.sub_command() {
      DenoSubcommand::Run(flags) => {
        const NPM_CMD_NAME_ENV_VAR_NAME: &str = "DENO_INTERNAL_NPM_CMD_NAME";
        match std::env::var(NPM_CMD_NAME_ENV_VAR_NAME) {
          Ok(var) => {
            // remove the env var so that child sub processes won't pick this up
            std::env::remove_var(NPM_CMD_NAME_ENV_VAR_NAME);
            Some(var)
          }
          Err(_) => NpmPackageReqReference::from_str(&flags.script)
            .ok()
            .map(|req_ref| npm_pkg_req_ref_to_binary_command(&req_ref)),
        }
      }
      _ => None,
    }
  }

  pub fn type_check_mode(&self) -> TypeCheckMode {
    self.flags.type_check_mode
  }

  pub fn unsafely_ignore_certificate_errors(&self) -> &Option<Vec<String>> {
    &self.flags.unsafely_ignore_certificate_errors
  }

  pub fn unstable_bare_node_builtins(&self) -> bool {
    self.flags.unstable_config.bare_node_builtins
      || self.workspace().has_unstable("bare-node-builtins")
  }

  pub fn unstable_detect_cjs(&self) -> bool {
    self.flags.unstable_config.detect_cjs
      || self.workspace().has_unstable("detect-cjs")
  }

  pub fn detect_cjs(&self) -> bool {
    // only enabled when there's a package.json in order to not have a
    // perf penalty for non-npm Deno projects of searching for the closest
    // package.json beside each module
    self.workspace().package_jsons().next().is_some() || self.is_node_main()
  }

  pub fn unstable_lazy_dynamic_imports(&self) -> bool {
    self.flags.unstable_config.lazy_dynamic_imports
      || self.workspace().has_unstable("lazy-dynamic-imports")
  }

  pub fn unstable_sloppy_imports(&self) -> bool {
    self.flags.unstable_config.sloppy_imports
      || self.workspace().has_unstable("sloppy-imports")
  }

  pub fn unstable_features(&self) -> Vec<String> {
    let from_config_file = self.workspace().unstable_features();
    let unstable_features = from_config_file
      .iter()
      .chain(
        self
          .flags
          .unstable_config
          .features
          .iter()
          .filter(|f| !from_config_file.contains(f)),
      )
      .map(|f| f.to_owned())
      .collect::<Vec<_>>();

    if !unstable_features.is_empty() {
      let all_valid_unstable_flags: Vec<&str> = crate::UNSTABLE_GRANULAR_FLAGS
        .iter()
        .map(|granular_flag| granular_flag.name)
        .chain([
          "byonm",
          "bare-node-builtins",
          "detect-cjs",
          "fmt-component",
          "fmt-sql",
          "lazy-dynamic-imports",
          "npm-lazy-caching",
          "npm-patch",
          "sloppy-imports",
          "lockfile-v5",
        ])
        .collect();

      // check and warn if the unstable flag of config file isn't supported, by
      // iterating through the vector holding the unstable flags
      for unstable_value_from_config_file in &unstable_features {
        if !all_valid_unstable_flags
          .contains(&unstable_value_from_config_file.as_str())
        {
          log::warn!(
            "{} '{}' isn't a valid unstable feature",
            colors::yellow("Warning"),
            unstable_value_from_config_file
          );
        }
      }
    }

    unstable_features
  }

  pub fn v8_flags(&self) -> &Vec<String> {
    &self.flags.v8_flags
  }

  pub fn code_cache_enabled(&self) -> bool {
    self.flags.code_cache_enabled
  }

  pub fn watch_paths(&self) -> Vec<PathBuf> {
    let mut full_paths = Vec::new();
    if let DenoSubcommand::Run(RunFlags {
      watch: Some(WatchFlagsWithPaths { paths, .. }),
      ..
    })
    | DenoSubcommand::Serve(ServeFlags {
      watch: Some(WatchFlagsWithPaths { paths, .. }),
      ..
    }) = &self.flags.subcommand
    {
      full_paths.extend(paths.iter().map(|path| self.initial_cwd.join(path)));
    }

    if let Ok(Some(import_map_path)) = self
      .resolve_specified_import_map_specifier()
      .map(|ms| ms.and_then(|ref s| s.to_file_path().ok()))
    {
      full_paths.push(import_map_path);
    }

    for (_, folder) in self.workspace().config_folders() {
      if let Some(deno_json) = &folder.deno_json {
        if deno_json.specifier.scheme() == "file" {
          if let Ok(path) = deno_json.specifier.to_file_path() {
            full_paths.push(path);
          }
        }
      }
      if let Some(pkg_json) = &folder.pkg_json {
        full_paths.push(pkg_json.path.clone());
      }
    }
    full_paths
  }

  pub fn lifecycle_scripts_config(&self) -> LifecycleScriptsConfig {
    LifecycleScriptsConfig {
      allowed: self.flags.allow_scripts.clone(),
      initial_cwd: self.initial_cwd.clone(),
      root_dir: self.workspace().root_dir_path(),
      explicit_install: matches!(
        self.sub_command(),
        DenoSubcommand::Install(_)
          | DenoSubcommand::Cache(_)
          | DenoSubcommand::Add(_)
      ),
    }
  }

  pub fn unstable_npm_lazy_caching(&self) -> bool {
    self.flags.unstable_config.npm_lazy_caching
      || self.workspace().has_unstable("npm-lazy-caching")
  }

  pub fn default_npm_caching_strategy(&self) -> NpmCachingStrategy {
    if matches!(
      self.sub_command(),
      DenoSubcommand::Install(InstallFlags::Local(
        InstallFlagsLocal::TopLevel | InstallFlagsLocal::Add(_)
      )) | DenoSubcommand::Add(_)
        | DenoSubcommand::Outdated(_)
    ) {
      NpmCachingStrategy::Manual
    } else if self.flags.unstable_config.npm_lazy_caching {
      NpmCachingStrategy::Lazy
    } else {
      NpmCachingStrategy::Eager
    }
  }
}

fn try_resolve_node_binary_main_entrypoint(
  specifier: &str,
  initial_cwd: &Path,
) -> Result<Option<Url>, AnyError> {
  // node allows running files at paths without a `.js` extension
  // or at directories with an index.js file
  let path = deno_core::normalize_path(initial_cwd.join(specifier));
  if path.is_dir() {
    let index_file = path.join("index.js");
    Ok(if index_file.is_file() {
      Some(deno_path_util::url_from_file_path(&index_file)?)
    } else {
      None
    })
  } else {
    let path = path.with_extension(
      path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!("{}.js", s))
        .unwrap_or("js".to_string()),
    );
    if path.is_file() {
      Ok(Some(deno_path_util::url_from_file_path(&path)?))
    } else {
      Ok(None)
    }
  }
}

#[derive(Debug, Error)]
#[error("Bad URL for import map.")]
pub struct ImportMapSpecifierResolveError {
  #[source]
  source: deno_path_util::ResolveUrlOrPathError,
}

fn resolve_import_map_specifier(
  maybe_import_map_path: Option<&str>,
  maybe_config_file: Option<&ConfigFile>,
  current_dir: &Path,
) -> Result<Option<Url>, ImportMapSpecifierResolveError> {
  if let Some(import_map_path) = maybe_import_map_path {
    if let Some(config_file) = &maybe_config_file {
      if config_file.json.import_map.is_some() {
        log::warn!(
          "{} the configuration file \"{}\" contains an entry for \"importMap\" that is being ignored.",
          colors::yellow("Warning"),
          config_file.specifier,
        );
      }
    }
    let specifier =
      deno_path_util::resolve_url_or_path(import_map_path, current_dir)
        .map_err(|source| ImportMapSpecifierResolveError { source })?;
    Ok(Some(specifier))
  } else {
    Ok(None)
  }
}

/// Resolves the no_prompt value based on the cli flags and environment.
pub fn resolve_no_prompt(flags: &PermissionFlags) -> bool {
  flags.no_prompt || has_flag_env_var("DENO_NO_PROMPT")
}

pub fn config_to_deno_graph_workspace_member(
  config: &ConfigFile,
) -> Result<deno_graph::WorkspaceMember, AnyError> {
  let name: StackString = match &config.json.name {
    Some(name) => name.as_str().into(),
    None => bail!("Missing 'name' field in config file."),
  };
  let version = match &config.json.version {
    Some(name) => Some(deno_semver::Version::parse_standard(name)?),
    None => None,
  };
  Ok(deno_graph::WorkspaceMember {
    base: config.specifier.join("./").unwrap(),
    name,
    version,
    exports: config.to_exports_config()?.into_map(),
  })
}

fn load_env_variables_from_env_file(filename: Option<&Vec<String>>) {
  let Some(env_file_names) = filename else {
    return;
  };

  for env_file_name in env_file_names.iter().rev() {
    match from_filename(env_file_name) {
      Ok(_) => (),
      Err(error) => {
        match error {
          dotenvy::Error::LineParse(line, index)=> log::info!("{} Parsing failed within the specified environment file: {} at index: {} of the value: {}", colors::yellow("Warning"), env_file_name, index, line),
          dotenvy::Error::Io(_)=> log::info!("{} The `--env-file` flag was used, but the environment file specified '{}' was not found.", colors::yellow("Warning"), env_file_name),
          dotenvy::Error::EnvVar(_)=> log::info!("{} One or more of the environment variables isn't present or not unicode within the specified environment file: {}", colors::yellow("Warning"), env_file_name),
          _ => log::info!("{} Unknown failure occurred with the specified environment file: {}", colors::yellow("Warning"), env_file_name),
        }
      }
    }
  }
}

/// Gets the --allow-import host from the provided url
fn allow_import_host_from_url(url: &Url) -> Option<String> {
  let host = url.host()?;
  if let Some(port) = url.port() {
    Some(format!("{}:{}", host, port))
  } else {
    match url.scheme() {
      "https" => Some(format!("{}:443", host)),
      "http" => Some(format!("{}:80", host)),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, Copy)]
pub enum NpmCachingStrategy {
  Eager,
  Lazy,
  Manual,
}

#[cfg(test)]
mod test {
  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn resolve_import_map_flags_take_precedence() {
    let config_text = r#"{
      "importMap": "import_map.json"
    }"#;
    let cwd = &std::env::current_dir().unwrap();
    let config_specifier = Url::parse("file:///deno/deno.jsonc").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
    let actual = resolve_import_map_specifier(
      Some("import-map.json"),
      Some(&config_file),
      cwd,
    );
    let import_map_path = cwd.join("import-map.json");
    let expected_specifier =
      deno_path_util::url_from_file_path(&import_map_path).unwrap();
    assert!(actual.is_ok());
    let actual = actual.unwrap();
    assert_eq!(actual, Some(expected_specifier));
  }

  #[test]
  fn resolve_import_map_none() {
    let config_text = r#"{}"#;
    let config_specifier = Url::parse("file:///deno/deno.jsonc").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
    let actual = resolve_import_map_specifier(
      None,
      Some(&config_file),
      &PathBuf::from("/"),
    );
    assert!(actual.is_ok());
    let actual = actual.unwrap();
    assert_eq!(actual, None);
  }

  #[test]
  fn resolve_import_map_no_config() {
    let actual = resolve_import_map_specifier(None, None, &PathBuf::from("/"));
    assert!(actual.is_ok());
    let actual = actual.unwrap();
    assert_eq!(actual, None);
  }

  #[test]
  fn jsr_urls() {
    let reg_url = jsr_url();
    assert!(reg_url.as_str().ends_with('/'));
    let reg_api_url = jsr_api_url();
    assert!(reg_api_url.as_str().ends_with('/'));
  }

  #[test]
  fn test_allow_import_host_from_url() {
    fn parse(text: &str) -> Option<String> {
      allow_import_host_from_url(&Url::parse(text).unwrap())
    }

    assert_eq!(
      parse("http://127.0.0.1:4250"),
      Some("127.0.0.1:4250".to_string())
    );
    assert_eq!(parse("http://jsr.io"), Some("jsr.io:80".to_string()));
    assert_eq!(
      parse("https://example.com"),
      Some("example.com:443".to_string())
    );
    assert_eq!(
      parse("http://example.com"),
      Some("example.com:80".to_string())
    );
    assert_eq!(parse("file:///example.com"), None);
  }
}
