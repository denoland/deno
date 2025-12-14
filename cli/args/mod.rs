// Copyright 2018-2025 the Deno authors. MIT license.

mod flags;
mod flags_net;

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
pub use deno_config::deno_json::CompilerOptions;
pub use deno_config::deno_json::ConfigFile;
use deno_config::deno_json::FmtConfig;
pub use deno_config::deno_json::FmtOptionsConfig;
pub use deno_config::deno_json::LintRulesConfig;
use deno_config::deno_json::NodeModulesDirMode;
use deno_config::deno_json::PermissionConfigValue;
use deno_config::deno_json::PermissionsObjectWithBase;
pub use deno_config::deno_json::ProseWrap;
use deno_config::deno_json::TestConfig;
pub use deno_config::glob::FilePatterns;
pub use deno_config::workspace::TsTypeLib;
use deno_config::workspace::Workspace;
use deno_config::workspace::WorkspaceDirLintConfig;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceDirectoryRc;
use deno_config::workspace::WorkspaceLintConfig;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_graph::GraphKind;
use deno_lib::args::CaData;
use deno_lib::args::has_flag_env_var;
use deno_lib::args::npm_pkg_req_ref_to_binary_command;
use deno_lib::args::npm_process_state;
use deno_lib::version::DENO_VERSION_INFO;
use deno_lib::worker::StorageKeyResolver;
use deno_npm::NpmSystemInfo;
use deno_npm_installer::LifecycleScriptsConfig;
use deno_npm_installer::graph::NpmCachingStrategy;
use deno_path_util::resolve_url_or_path;
use deno_resolver::factory::resolve_jsr_url;
use deno_runtime::deno_node::ops::ipc::ChildIpcSerialization;
use deno_runtime::deno_permissions::AllowRunDescriptor;
use deno_runtime::deno_permissions::PathDescriptor;
use deno_runtime::deno_permissions::PermissionsOptions;
use deno_runtime::inspector_server::InspectorServer;
use deno_semver::StackString;
use deno_semver::npm::NpmPackageReqReference;
use deno_telemetry::OtelConfig;
use deno_terminal::colors;
pub use flags::*;
use once_cell::sync::Lazy;
use thiserror::Error;

use crate::sys::CliSys;

pub type CliLockfile = deno_resolver::lockfile::LockfileLock<CliSys>;

pub fn jsr_url() -> &'static Url {
  static JSR_URL: Lazy<Url> = Lazy::new(|| resolve_jsr_url(&CliSys::default()));

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
      concurrent_jobs: parallelism_count(test_flags.parallel),
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

pub struct WorkspaceMainModuleResolver {
  workspace_resolver: Arc<deno_resolver::workspace::WorkspaceResolver<CliSys>>,
  node_resolver: Arc<crate::node::CliNodeResolver>,
}

impl WorkspaceMainModuleResolver {
  pub fn new(
    workspace_resolver: Arc<
      deno_resolver::workspace::WorkspaceResolver<CliSys>,
    >,
    node_resolver: Arc<crate::node::CliNodeResolver>,
  ) -> Self {
    Self {
      workspace_resolver,
      node_resolver,
    }
  }
}
impl WorkspaceMainModuleResolver {
  fn resolve_main_module(
    &self,
    specifier: &str,
    cwd: &Url,
  ) -> Result<Url, AnyError> {
    let resolution = self.workspace_resolver.resolve(
      specifier,
      cwd,
      deno_resolver::workspace::ResolutionKind::Execution,
    )?;
    let url = match resolution {
      deno_resolver::workspace::MappedResolution::Normal {
        specifier, ..
      } => specifier,
      deno_resolver::workspace::MappedResolution::WorkspaceJsrPackage {
        specifier,
        ..
      } => specifier,
      deno_resolver::workspace::MappedResolution::WorkspaceNpmPackage {
        target_pkg_json,
        sub_path,
        ..
      } => self
        .node_resolver
        .resolve_package_subpath_from_deno_module(
          target_pkg_json.clone().dir_path(),
          sub_path.as_deref(),
          Some(cwd),
          node_resolver::ResolutionMode::Import,
          node_resolver::NodeResolutionKind::Execution,
        )?
        .into_url()?,
      deno_resolver::workspace::MappedResolution::PackageJson {
        sub_path,
        dep_result,
        alias,
        ..
      } => {
        let result = dep_result
          .as_ref()
          .map_err(|e| deno_core::anyhow::anyhow!("{e}"))?;
        match result {
          deno_package_json::PackageJsonDepValue::File(file) => {
            let cwd_path = deno_path_util::url_to_file_path(cwd)?;
            deno_path_util::resolve_path(file, &cwd_path)?
          }
          deno_package_json::PackageJsonDepValue::Req(package_req) => {
            ModuleSpecifier::parse(&format!(
              "npm:{}{}",
              package_req,
              sub_path.map(|s| format!("/{}", s)).unwrap_or_default()
            ))?
          }
          deno_package_json::PackageJsonDepValue::Workspace(version_req) => {
            let pkg_folder = self
              .workspace_resolver
              .resolve_workspace_pkg_json_folder_for_pkg_json_dep(
                alias,
                version_req,
              )?;
            self
              .node_resolver
              .resolve_package_subpath_from_deno_module(
                pkg_folder,
                sub_path.as_deref(),
                Some(cwd),
                node_resolver::ResolutionMode::Import,
                node_resolver::NodeResolutionKind::Execution,
              )?
              .into_url()?
          }
          deno_package_json::PackageJsonDepValue::JsrReq(_) => {
            return Err(
              deno_resolver::DenoResolveErrorKind::UnsupportedPackageJsonJsrReq
                .into_box()
                .into(),
            );
          }
        }
      }
      deno_resolver::workspace::MappedResolution::PackageJsonImport {
        pkg_json,
      } => self
        .node_resolver
        .resolve_package_import(
          specifier,
          Some(&node_resolver::UrlOrPathRef::from_url(cwd)),
          Some(pkg_json),
          node_resolver::ResolutionMode::Import,
          node_resolver::NodeResolutionKind::Execution,
        )?
        .into_url()?,
    };
    Ok(url)
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
      DenoSubcommand::Add(_) => GraphKind::All,
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

  pub fn node_ipc_init(
    &self,
  ) -> Result<Option<(i64, ChildIpcSerialization)>, AnyError> {
    let maybe_node_channel_fd = std::env::var("NODE_CHANNEL_FD").ok();
    let maybe_node_channel_serialization = if let Ok(serialization) =
      std::env::var("NODE_CHANNEL_SERIALIZATION_MODE")
    {
      Some(serialization.parse::<ChildIpcSerialization>()?)
    } else {
      None
    };
    if let Some(node_channel_fd) = maybe_node_channel_fd {
      // Remove so that child processes don't inherit this environment variables.
      #[allow(clippy::undocumented_unsafe_blocks)]
      unsafe {
        std::env::remove_var("NODE_CHANNEL_FD");
        std::env::remove_var("NODE_CHANNEL_SERIALIZATION_MODE");
      }
      let node_channel_fd = node_channel_fd.parse::<i64>()?;
      Ok(Some((
        node_channel_fd,
        maybe_node_channel_serialization.unwrap_or(ChildIpcSerialization::Json),
      )))
    } else {
      Ok(None)
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

  pub fn node_conditions(&self) -> &[String] {
    self.flags.node_conditions.as_ref()
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

  pub fn preload_modules(&self) -> Result<Vec<ModuleSpecifier>, AnyError> {
    if self.flags.preload.is_empty() {
      return Ok(vec![]);
    }

    let mut modules = Vec::with_capacity(self.flags.preload.len());
    for preload_specifier in self.flags.preload.iter() {
      modules.push(resolve_url_or_path(preload_specifier, self.initial_cwd())?);
    }

    Ok(modules)
  }

  pub fn require_modules(&self) -> Result<Vec<ModuleSpecifier>, AnyError> {
    if self.flags.require.is_empty() {
      return Ok(vec![]);
    }

    let mut require = Vec::with_capacity(self.flags.require.len());
    for require_specifier in self.flags.require.iter() {
      require.push(resolve_url_or_path(require_specifier, self.initial_cwd())?);
    }

    Ok(require)
  }

  fn resolve_main_module_with_resolver_if_bare(
    &self,
    raw_specifier: &str,
    resolver: Option<&WorkspaceMainModuleResolver>,
    default_resolve: impl Fn() -> Result<ModuleSpecifier, AnyError>,
  ) -> Result<ModuleSpecifier, AnyError> {
    match resolver {
      Some(resolver)
        if !raw_specifier.starts_with('.')
          && !Path::new(raw_specifier).is_absolute() =>
      {
        let cwd = deno_path_util::url_from_directory_path(self.initial_cwd())?;
        resolver
          .resolve_main_module(raw_specifier, &cwd)
          .or_else(|_| default_resolve())
      }
      _ => default_resolve(),
    }
  }

  pub fn resolve_main_module_with_resolver(
    &self,
    resolver: Option<&WorkspaceMainModuleResolver>,
  ) -> Result<&ModuleSpecifier, AnyError> {
    self
      .main_module_cell
      .get_or_init(|| {
        Ok(match &self.flags.subcommand {
          DenoSubcommand::Compile(compile_flags) => {
            resolve_url_or_path(&compile_flags.source_file, self.initial_cwd())?
          }
          DenoSubcommand::Eval(_) => {
            let specifier = format!(
              "./$deno$eval.{}",
              self.flags.ext.as_deref().unwrap_or("mts")
            );
            deno_path_util::resolve_path(&specifier, self.initial_cwd())?
          }
          DenoSubcommand::Repl(_) => deno_path_util::resolve_path(
            "./$deno$repl.mts",
            self.initial_cwd(),
          )?,
          DenoSubcommand::Run(run_flags) => {
            if run_flags.is_stdin() {
              let specifier = format!(
                "./$deno$stdin.{}",
                self.flags.ext.as_deref().unwrap_or("mts")
              );
              deno_path_util::resolve_path(&specifier, self.initial_cwd())?
            } else {
              let default_resolve = || {
                let url =
                  resolve_url_or_path(&run_flags.script, self.initial_cwd())?;
                if self.is_node_main()
                  && url.scheme() == "file"
                  && MediaType::from_specifier(&url) == MediaType::Unknown
                {
                  Ok::<_, AnyError>(
                    try_resolve_node_binary_main_entrypoint(
                      &run_flags.script,
                      self.initial_cwd(),
                    )?
                    .unwrap_or(url),
                  )
                } else {
                  Ok(url)
                }
              };
              self.resolve_main_module_with_resolver_if_bare(
                &run_flags.script,
                resolver,
                default_resolve,
              )?
            }
          }
          DenoSubcommand::Serve(run_flags) => self
            .resolve_main_module_with_resolver_if_bare(
              &run_flags.script,
              resolver,
              || {
                resolve_url_or_path(&run_flags.script, self.initial_cwd())
                  .map_err(|e| e.into())
              },
            )?,
          _ => {
            bail!("No main module.")
          }
        })
      })
      .as_ref()
      .map_err(|err| deno_core::anyhow::anyhow!("{}", err))
  }

  pub fn resolve_main_module(&self) -> Result<&ModuleSpecifier, AnyError> {
    self.resolve_main_module_with_resolver(None)
  }

  pub fn resolve_file_header_overrides(
    &self,
  ) -> HashMap<ModuleSpecifier, HashMap<String, String>> {
    let maybe_main_specifier = self.resolve_main_module().ok();
    let maybe_content_type = self.flags.ext.as_ref().and_then(|ext| {
      let media_type = MediaType::from_filename(&format!("file.{}", ext));
      media_type.as_content_type()
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
    npm_process_state(&CliSys::default()).is_some()
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
  ) -> Result<Vec<(WorkspaceDirectoryRc, FmtOptions)>, AnyError> {
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
  ) -> Result<Vec<(WorkspaceDirectoryRc, LintOptions)>, AnyError> {
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
  ) -> Result<Vec<(WorkspaceDirectoryRc, TestOptions)>, AnyError> {
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
  ) -> Result<Vec<(WorkspaceDirectoryRc, BenchOptions)>, AnyError> {
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

  pub fn coverage_dir(&self) -> Option<PathBuf> {
    match &self.flags.subcommand {
      DenoSubcommand::Test(test) => test
        .coverage_dir
        .as_ref()
        .map(|dir| self.initial_cwd.join(dir))
        .or_else(|| env::var_os("DENO_COVERAGE_DIR").map(PathBuf::from)),
      DenoSubcommand::Run(flags) => flags
        .coverage_dir
        .as_ref()
        .map(|dir| self.initial_cwd.join(dir))
        .or_else(|| env::var_os("DENO_COVERAGE_DIR").map(PathBuf::from)),
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

  pub fn permissions_options(&self) -> Result<PermissionsOptions, AnyError> {
    self.permissions_options_for_dir(&self.start_dir)
  }

  pub fn permissions_options_for_dir(
    &self,
    dir: &WorkspaceDirectory,
  ) -> Result<PermissionsOptions, AnyError> {
    let config_permissions = self.resolve_config_permissions_for_dir(dir)?;
    let mut permissions_options = flags_to_permissions_options(
      &self.flags.permissions,
      config_permissions,
    )?;
    self.augment_import_permissions(&mut permissions_options);
    Ok(permissions_options)
  }

  fn resolve_config_permissions_for_dir<'a>(
    &self,
    dir: &'a WorkspaceDirectory,
  ) -> Result<Option<&'a PermissionsObjectWithBase>, AnyError> {
    let config_permissions = if let Some(name) = &self.flags.permission_set {
      if name.is_empty() {
        let maybe_subcommand_permissions = match &self.flags.subcommand {
          DenoSubcommand::Bench(_) => dir.to_bench_permissions_config()?,
          DenoSubcommand::Compile(_) => dir.to_compile_permissions_config()?,
          DenoSubcommand::Test(_) => dir.to_test_permissions_config()?,
          _ => None,
        };
        match maybe_subcommand_permissions {
          Some(permissions) => Some(permissions),
          // do not error when the default set doesn't exist in order
          // to allow providing `-P` unconditionally
          None => dir.to_permissions_config()?.sets.get("default"),
        }
      } else {
        Some(dir.to_permissions_config()?.get(name)?)
      }
    } else {
      if !self.flags.has_permission() {
        let set_config_permission_name = match &self.flags.subcommand {
          DenoSubcommand::Bench(_) => dir
            .to_bench_permissions_config()?
            .filter(|permissions| !permissions.permissions.is_empty())
            .map(|permissions| ("Bench", &permissions.base)),
          DenoSubcommand::Compile(_) => dir
            .to_compile_permissions_config()?
            .filter(|permissions| !permissions.permissions.is_empty())
            .map(|permissions| ("Compile", &permissions.base)),
          DenoSubcommand::Test(_) => dir
            .to_test_permissions_config()?
            .filter(|permissions| !permissions.permissions.is_empty())
            .map(|permissions| ("Test", &permissions.base)),
          _ => None,
        };
        if let Some((name, config_file_url)) = set_config_permission_name {
          // prevent people from wasting time wondering why benches/tests are failing
          bail!(
            "{} permissions were found in the config file. Did you mean to run with `-P` or a permission flag?\n    at {}",
            name,
            config_file_url
          );
        }
      }

      None
    };
    Ok(config_permissions)
  }

  fn augment_import_permissions(&self, options: &mut PermissionsOptions) {
    // do not add if the user specified --allow-all or --allow-import
    if options.allow_import.is_none() {
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
      if let Some(jsr_host) = allow_import_host_from_url(jsr_url())
        && jsr_host != "jsr.io:443"
      {
        imports.push(jsr_host);
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
        DenoSubcommand::Install(InstallFlags::Global(flags)) => flags
          .module_urls
          .first()
          .and_then(|url| file_to_url(url))
          .map(|url| vec![url]),
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

  pub fn trace_ops(&self) -> &Option<Vec<String>> {
    &self.flags.trace_ops
  }

  pub fn take_binary_npm_command_name(&self) -> Option<String> {
    match self.sub_command() {
      DenoSubcommand::Run(flags) => {
        const NPM_CMD_NAME_ENV_VAR_NAME: &str = "DENO_INTERNAL_NPM_CMD_NAME";
        match std::env::var(NPM_CMD_NAME_ENV_VAR_NAME) {
          Ok(var) => {
            // remove the env var so that child sub processes won't pick this up

            #[allow(clippy::undocumented_unsafe_blocks)]
            unsafe {
              std::env::remove_var(NPM_CMD_NAME_ENV_VAR_NAME)
            };
            Some(var)
          }
          Err(_) => NpmPackageReqReference::from_str(&flags.script).ok().map(
            |req_ref| npm_pkg_req_ref_to_binary_command(&req_ref).to_string(),
          ),
        }
      }
      _ => None,
    }
  }

  pub fn type_check_mode(&self) -> TypeCheckMode {
    self.flags.type_check_mode
  }

  pub fn unstable_tsgo(&self) -> bool {
    self.flags.unstable_config.tsgo || self.workspace().has_unstable("tsgo")
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

  pub fn unstable_raw_imports(&self) -> bool {
    self.flags.unstable_config.raw_imports
      || self.workspace().has_unstable("raw-imports")
  }

  pub fn unstable_lazy_dynamic_imports(&self) -> bool {
    self.flags.unstable_config.lazy_dynamic_imports
      || self.workspace().has_unstable("lazy-dynamic-imports")
  }

  pub fn unstable_sloppy_imports(&self) -> bool {
    self.flags.unstable_config.sloppy_imports
      || self.workspace().has_unstable("sloppy-imports")
  }

  pub fn unstable_features(&self) -> Vec<&str> {
    let from_config_file = self.workspace().unstable_features();
    let unstable_features = from_config_file
      .iter()
      .map(|s| s.as_str())
      .chain(
        self
          .flags
          .unstable_config
          .features
          .iter()
          .filter(|f| !from_config_file.contains(f))
          .map(|s| s.as_str()),
      )
      .collect::<Vec<_>>();

    if !unstable_features.is_empty() {
      let all_valid_unstable_flags: Vec<&str> = deno_runtime::UNSTABLE_FEATURES
        .iter()
        .map(|feature| feature.name)
        .chain(["fmt-component", "fmt-sql", "npm-lazy-caching"])
        .collect();

      // check and warn if the unstable flag of config file isn't supported, by
      // iterating through the vector holding the unstable flags
      for unstable_value_from_config_file in &unstable_features {
        if !all_valid_unstable_flags.contains(unstable_value_from_config_file) {
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

    if let Some(env_file_names) = &self.flags.env_file {
      // Only watch the exact environment files specified
      full_paths.extend(
        env_file_names
          .iter()
          .map(|name| self.initial_cwd.join(name)),
      );
    }

    if let Ok(Some(import_map_path)) = self
      .resolve_specified_import_map_specifier()
      .map(|ms| ms.and_then(|ref s| s.to_file_path().ok()))
    {
      full_paths.push(import_map_path);
    }

    for (_, folder) in self.workspace().config_folders() {
      if let Some(deno_json) = &folder.deno_json
        && deno_json.specifier.scheme() == "file"
        && let Ok(path) = deno_json.specifier.to_file_path()
      {
        full_paths.push(path);
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
      denied: Default::default(),
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
        InstallFlagsLocal::TopLevel(_)
          | InstallFlagsLocal::Add(_)
          | InstallFlagsLocal::Entrypoints(InstallEntrypointsFlags {
            lockfile_only: true,
            ..
          })
      )) | DenoSubcommand::Add(_)
        | DenoSubcommand::Outdated(_)
    ) {
      NpmCachingStrategy::Manual
    } else if self.unstable_npm_lazy_caching() {
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
  let path = initial_cwd.join(specifier);
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
    if let Some(config_file) = &maybe_config_file
      && config_file.json.import_map.is_some()
    {
      log::warn!(
        "{} the configuration file \"{}\" contains an entry for \"importMap\" that is being ignored.",
        colors::yellow("Warning"),
        config_file.specifier,
      );
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

pub fn get_default_v8_flags() -> Vec<String> {
  vec![
    "--stack-size=1024".to_string(),
    "--js-explicit-resource-management".to_string(),
    // TODO(bartlomieju): I think this can be removed as it's handled by `deno_core`
    // and its settings.
    // deno_ast removes TypeScript `assert` keywords, so this flag only affects JavaScript
    // TODO(petamoriken): Need to check TypeScript `assert` keywords in deno_ast
    "--no-harmony-import-assertions".to_string(),
  ]
}

pub fn parallelism_count(parallel: bool) -> NonZeroUsize {
  parallel
    .then(|| {
      if let Ok(value) = env::var("DENO_JOBS") {
        value.parse::<NonZeroUsize>().ok()
      } else {
        std::thread::available_parallelism().ok()
      }
    })
    .flatten()
    .unwrap_or_else(|| NonZeroUsize::new(1).unwrap())
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

// DO NOT make this public. People should use `cli_options.permissions_options/permissions_options_for_dir`
fn flags_to_permissions_options(
  flags: &PermissionFlags,
  mut config: Option<&PermissionsObjectWithBase>,
) -> Result<PermissionsOptions, AnyError> {
  fn handle_allow(
    allow_all_flag: bool,
    allow_all_config: Option<bool>,
    value: Option<&Vec<String>>,
    config: Option<&PermissionConfigValue>,
    parse_config_value: &impl Fn(&str) -> String,
  ) -> Option<Vec<String>> {
    if allow_all_flag {
      Some(vec![])
    } else if let Some(value) = value {
      Some(value.clone())
    } else if let Some(config) = config {
      match config {
        PermissionConfigValue::All => Some(vec![]),
        PermissionConfigValue::Some(items) => {
          if items.is_empty() {
            None
          } else {
            Some(
              items
                .iter()
                .map(|value| parse_config_value(value))
                .collect(),
            )
          }
        }
        PermissionConfigValue::None => None,
      }
    } else if allow_all_config == Some(true) {
      Some(vec![])
    } else {
      None
    }
  }

  fn handle_deny_or_ignore(
    value: Option<&Vec<String>>,
    config: Option<&PermissionConfigValue>,
    parse_config_value: &impl Fn(&str) -> String,
  ) -> Option<Vec<String>> {
    if let Some(value) = value {
      Some(value.clone())
    } else if let Some(config) = config {
      match config {
        PermissionConfigValue::All => Some(vec![]),
        PermissionConfigValue::Some(items) => Some(
          items
            .iter()
            .map(|value| parse_config_value(value))
            .collect(),
        ),
        PermissionConfigValue::None => None,
      }
    } else {
      None
    }
  }

  if flags.allow_all {
    config = None;
  }

  let config_dir = match &config {
    Some(config) => {
      let mut path = deno_path_util::url_to_file_path(&config.base)?;
      path.pop();
      Some(path)
    }
    None => None,
  };

  let make_fs_config_value_absolute = |value: &str| match &config_dir {
    Some(dir_path) => {
      PathDescriptor::new_known_cwd(Cow::Borrowed(Path::new(value)), dir_path)
        .into_path_buf()
        .into_os_string()
        .into_string()
        .unwrap()
    }
    None => value.to_string(),
  };
  let make_run_config_value_absolute = |value: &str| match &config_dir {
    Some(dir_path) => {
      if AllowRunDescriptor::is_path(value) {
        PathDescriptor::new_known_cwd(Cow::Borrowed(Path::new(value)), dir_path)
          .into_path_buf()
          .into_os_string()
          .into_string()
          .unwrap()
      } else {
        value.to_string()
      }
    }
    None => value.to_string(),
  };
  let identity = |value: &str| value.to_string();

  Ok(PermissionsOptions {
    allow_env: handle_allow(
      flags.allow_all,
      config.and_then(|c| c.permissions.all),
      flags.allow_env.as_ref(),
      config.and_then(|c| c.permissions.env.allow.as_ref()),
      &identity,
    ),
    deny_env: handle_deny_or_ignore(
      flags.deny_env.as_ref(),
      config.and_then(|c| c.permissions.env.deny.as_ref()),
      &identity,
    ),
    ignore_env: handle_deny_or_ignore(
      flags.ignore_env.as_ref(),
      config.and_then(|c| c.permissions.env.ignore.as_ref()),
      &identity,
    ),
    allow_net: handle_allow(
      flags.allow_all,
      config.and_then(|c| c.permissions.all),
      flags.allow_net.as_ref(),
      config.and_then(|c| c.permissions.net.allow.as_ref()),
      &identity,
    ),
    deny_net: handle_deny_or_ignore(
      flags.deny_net.as_ref(),
      config.and_then(|c| c.permissions.net.deny.as_ref()),
      &identity,
    ),
    allow_ffi: handle_allow(
      flags.allow_all,
      config.and_then(|c| c.permissions.all),
      flags.allow_ffi.as_ref(),
      config.and_then(|c| c.permissions.ffi.allow.as_ref()),
      &make_fs_config_value_absolute,
    ),
    deny_ffi: handle_deny_or_ignore(
      flags.deny_ffi.as_ref(),
      config.and_then(|c| c.permissions.ffi.deny.as_ref()),
      &make_fs_config_value_absolute,
    ),
    allow_read: handle_allow(
      flags.allow_all,
      config.and_then(|c| c.permissions.all),
      flags.allow_read.as_ref(),
      config.and_then(|c| c.permissions.read.allow.as_ref()),
      &make_fs_config_value_absolute,
    ),
    deny_read: handle_deny_or_ignore(
      flags.deny_read.as_ref(),
      config.and_then(|c| c.permissions.read.deny.as_ref()),
      &make_fs_config_value_absolute,
    ),
    ignore_read: handle_deny_or_ignore(
      flags.ignore_read.as_ref(),
      config.and_then(|c| c.permissions.read.ignore.as_ref()),
      &make_fs_config_value_absolute,
    ),
    allow_run: handle_allow(
      flags.allow_all,
      config.and_then(|c| c.permissions.all),
      flags.allow_run.as_ref(),
      config.and_then(|c| c.permissions.run.allow.as_ref()),
      &make_run_config_value_absolute,
    ),
    deny_run: handle_deny_or_ignore(
      flags.deny_run.as_ref(),
      config.and_then(|c| c.permissions.run.deny.as_ref()),
      &make_run_config_value_absolute,
    ),
    allow_sys: handle_allow(
      flags.allow_all,
      config.and_then(|c| c.permissions.all),
      flags.allow_sys.as_ref(),
      config.and_then(|c| c.permissions.sys.allow.as_ref()),
      &identity,
    ),
    deny_sys: handle_deny_or_ignore(
      flags.deny_sys.as_ref(),
      config.and_then(|c| c.permissions.sys.deny.as_ref()),
      &identity,
    ),
    allow_write: handle_allow(
      flags.allow_all,
      config.and_then(|c| c.permissions.all),
      flags.allow_write.as_ref(),
      config.and_then(|c| c.permissions.write.allow.as_ref()),
      &make_fs_config_value_absolute,
    ),
    deny_write: handle_deny_or_ignore(
      flags.deny_write.as_ref(),
      config.and_then(|c| c.permissions.write.deny.as_ref()),
      &make_fs_config_value_absolute,
    ),
    allow_import: handle_allow(
      flags.allow_all,
      config.and_then(|c| c.permissions.all),
      flags.allow_import.as_ref(),
      config.and_then(|c| c.permissions.import.allow.as_ref()),
      &identity,
    ),
    deny_import: handle_deny_or_ignore(
      flags.deny_import.as_ref(),
      config.and_then(|c| c.permissions.import.deny.as_ref()),
      &identity,
    ),
    prompt: !resolve_no_prompt(flags),
  })
}

#[cfg(test)]
mod test {
  use deno_config::deno_json::AllowDenyIgnorePermissionConfig;
  use deno_config::deno_json::AllowDenyPermissionConfig;
  use deno_config::deno_json::PermissionsObject;
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
    let actual =
      resolve_import_map_specifier(None, Some(&config_file), Path::new("/"));
    assert!(actual.is_ok());
    let actual = actual.unwrap();
    assert_eq!(actual, None);
  }

  #[test]
  fn resolve_import_map_no_config() {
    let actual = resolve_import_map_specifier(None, None, Path::new("/"));
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

  #[test]
  fn test_flags_to_permission_options() {
    let base_dir = std::env::current_dir().unwrap().join("sub");
    {
      let flags = PermissionFlags::default();
      let config = PermissionsObjectWithBase {
        base: deno_path_util::url_from_file_path(&base_dir.join("deno.json"))
          .unwrap(),
        permissions: PermissionsObject {
          all: None,
          read: AllowDenyIgnorePermissionConfig {
            allow: Some(PermissionConfigValue::Some(vec![
              ".".to_string(),
              "./read-allow".to_string(),
            ])),
            deny: Some(PermissionConfigValue::Some(vec![
              "./read-deny".to_string(),
            ])),
            ignore: Some(PermissionConfigValue::Some(vec![
              "./read-ignore".to_string(),
            ])),
          },
          write: AllowDenyPermissionConfig {
            allow: Some(PermissionConfigValue::Some(vec![
              "./write-allow".to_string(),
            ])),
            deny: Some(PermissionConfigValue::Some(vec![
              "./write-deny".to_string(),
            ])),
          },
          import: AllowDenyPermissionConfig {
            allow: Some(PermissionConfigValue::Some(vec![
              "jsr.io".to_string(),
            ])),
            deny: Some(PermissionConfigValue::Some(vec![
              "example.com".to_string(),
            ])),
          },
          env: AllowDenyIgnorePermissionConfig {
            allow: Some(PermissionConfigValue::Some(vec![
              "env-allow".to_string(),
            ])),
            deny: Some(PermissionConfigValue::Some(vec![
              "env-deny".to_string(),
            ])),
            ignore: Some(PermissionConfigValue::Some(vec![
              "env-ignore".to_string(),
            ])),
          },
          net: AllowDenyPermissionConfig {
            allow: Some(PermissionConfigValue::Some(vec![
              "net-allow".to_string(),
            ])),
            deny: Some(PermissionConfigValue::Some(vec![
              "net-deny".to_string(),
            ])),
          },
          run: AllowDenyPermissionConfig {
            allow: Some(PermissionConfigValue::Some(vec![
              "run-allow".to_string(),
              "./relative-run-allow".to_string(),
            ])),
            deny: Some(PermissionConfigValue::Some(vec![
              "run-deny".to_string(),
              "./relative-run-deny".to_string(),
            ])),
          },
          ffi: AllowDenyPermissionConfig {
            allow: Some(PermissionConfigValue::Some(vec![
              "./ffi-allow".to_string(),
            ])),
            deny: Some(PermissionConfigValue::Some(vec![
              "./ffi-deny".to_string(),
            ])),
          },
          sys: AllowDenyPermissionConfig {
            allow: Some(PermissionConfigValue::Some(vec![
              "sys-allow".to_string(),
            ])),
            deny: Some(PermissionConfigValue::Some(vec![
              "sys-deny".to_string(),
            ])),
          },
        },
      };
      let permissions_options =
        flags_to_permissions_options(&flags, Some(&config)).unwrap();
      assert_eq!(
        permissions_options,
        PermissionsOptions {
          allow_env: Some(vec!["env-allow".to_string()]),
          deny_env: Some(vec!["env-deny".to_string()]),
          ignore_env: Some(vec!["env-ignore".to_string()]),
          allow_net: Some(vec!["net-allow".to_string()]),
          deny_net: Some(vec!["net-deny".to_string()]),
          allow_ffi: Some(vec![
            base_dir
              .join("ffi-allow")
              .into_os_string()
              .into_string()
              .unwrap()
          ]),
          deny_ffi: Some(vec![
            base_dir
              .join("ffi-deny")
              .into_os_string()
              .into_string()
              .unwrap()
          ]),
          allow_read: Some(vec![
            base_dir.clone().into_os_string().into_string().unwrap(),
            base_dir
              .join("read-allow")
              .into_os_string()
              .into_string()
              .unwrap()
          ]),
          deny_read: Some(vec![
            base_dir
              .join("read-deny")
              .into_os_string()
              .into_string()
              .unwrap()
          ]),
          ignore_read: Some(vec![
            base_dir
              .join("read-ignore")
              .into_os_string()
              .into_string()
              .unwrap()
          ]),
          allow_run: Some(vec![
            "run-allow".to_string(),
            base_dir
              .join("relative-run-allow")
              .into_os_string()
              .into_string()
              .unwrap()
          ]),
          deny_run: Some(vec![
            "run-deny".to_string(),
            base_dir
              .join("relative-run-deny")
              .into_os_string()
              .into_string()
              .unwrap()
          ]),
          allow_sys: Some(vec!["sys-allow".to_string()]),
          deny_sys: Some(vec!["sys-deny".to_string()]),
          allow_write: Some(vec![
            base_dir
              .join("write-allow")
              .into_os_string()
              .into_string()
              .unwrap()
          ]),
          deny_write: Some(vec![
            base_dir
              .join("write-deny")
              .into_os_string()
              .into_string()
              .unwrap()
          ]),
          allow_import: Some(vec!["jsr.io".to_string()]),
          deny_import: Some(vec!["example.com".to_string()]),
          prompt: true
        }
      );
    }
    {
      let flags = PermissionFlags {
        allow_read: Some(vec!["./folder".to_string()]),
        ..Default::default()
      };
      let config = PermissionsObjectWithBase {
        base: deno_path_util::url_from_file_path(&base_dir.join("deno.json"))
          .unwrap(),
        permissions: PermissionsObject {
          // will use all permissions except for the explicitly specified permissions
          // and the explicit flag will replace
          all: Some(true),
          write: AllowDenyPermissionConfig {
            allow: Some(PermissionConfigValue::Some(vec![
              "./write-allow".to_string(),
            ])),
            deny: Some(PermissionConfigValue::Some(vec![
              "./write-deny".to_string(),
            ])),
          },
          ..Default::default()
        },
      };
      let permissions_options =
        flags_to_permissions_options(&flags, Some(&config)).unwrap();
      assert_eq!(
        permissions_options,
        PermissionsOptions {
          allow_env: Some(vec![]),
          deny_env: None,
          ignore_env: None,
          allow_net: Some(vec![]),
          deny_net: None,
          allow_ffi: Some(vec![]),
          deny_ffi: None,
          allow_read: Some(vec!["./folder".to_string()]),
          deny_read: None,
          ignore_read: None,
          allow_run: Some(vec![]),
          deny_run: None,
          allow_sys: Some(vec![]),
          deny_sys: None,
          allow_write: Some(vec![
            base_dir
              .join("write-allow")
              .into_os_string()
              .into_string()
              .unwrap()
          ]),
          deny_write: Some(vec![
            base_dir
              .join("write-deny")
              .into_os_string()
              .into_string()
              .unwrap()
          ]),
          allow_import: Some(vec![]),
          deny_import: None,
          prompt: true
        }
      );
    }
  }
}
