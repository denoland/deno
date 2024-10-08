// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::num::NonZeroU8;
use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use clap::builder::styling::AnsiColor;
use clap::builder::FalseyValueParser;
use clap::error::ErrorKind;
use clap::value_parser;
use clap::Arg;
use clap::ArgAction;
use clap::ArgMatches;
use clap::ColorChoice;
use clap::Command;
use clap::ValueHint;
use color_print::cstr;
use deno_config::deno_json::NodeModulesDirMode;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPatternSet;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::url::Url;
use deno_graph::GraphKind;
use deno_path_util::normalize_path;
use deno_path_util::url_to_file_path;
use deno_runtime::deno_permissions::PermissionsOptions;
use deno_runtime::deno_permissions::SysDescriptor;
use log::debug;
use log::Level;
use serde::Deserialize;
use serde::Serialize;

use crate::args::resolve_no_prompt;
use crate::util::fs::canonicalize_path;

use super::flags_net;
use super::jsr_url;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ConfigFlag {
  #[default]
  Discover,
  Path(String),
  Disabled,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileFlags {
  pub ignore: Vec<String>,
  pub include: Vec<String>,
}

impl FileFlags {
  pub fn as_file_patterns(
    &self,
    base: &Path,
  ) -> Result<FilePatterns, AnyError> {
    Ok(FilePatterns {
      include: if self.include.is_empty() {
        None
      } else {
        Some(PathOrPatternSet::from_include_relative_path_or_patterns(
          base,
          &self.include,
        )?)
      },
      exclude: PathOrPatternSet::from_exclude_relative_path_or_patterns(
        base,
        &self.ignore,
      )?,
      base: base.to_path_buf(),
    })
  }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AddFlags {
  pub packages: Vec<String>,
  pub dev: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RemoveFlags {
  pub packages: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BenchFlags {
  pub files: FileFlags,
  pub filter: Option<String>,
  pub json: bool,
  pub no_run: bool,
  pub watch: Option<WatchFlags>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheFlags {
  pub files: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckFlags {
  pub files: Vec<String>,
  pub doc: bool,
  pub doc_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileFlags {
  pub source_file: String,
  pub output: Option<String>,
  pub args: Vec<String>,
  pub target: Option<String>,
  pub no_terminal: bool,
  pub icon: Option<String>,
  pub include: Vec<String>,
}

impl CompileFlags {
  pub fn resolve_target(&self) -> String {
    self
      .target
      .clone()
      .unwrap_or_else(|| env!("TARGET").to_string())
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionsFlags {
  pub buf: Box<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum CoverageType {
  #[default]
  Summary,
  Detailed,
  Lcov,
  Html,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct CoverageFlags {
  pub files: FileFlags,
  pub output: Option<String>,
  pub include: Vec<String>,
  pub exclude: Vec<String>,
  pub r#type: CoverageType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocSourceFileFlag {
  Builtin,
  Paths(Vec<String>),
}

impl Default for DocSourceFileFlag {
  fn default() -> Self {
    Self::Builtin
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocHtmlFlag {
  pub name: Option<String>,
  pub category_docs_path: Option<String>,
  pub symbol_redirect_map_path: Option<String>,
  pub default_symbol_map_path: Option<String>,
  pub strip_trailing_html: bool,
  pub output: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocFlags {
  pub private: bool,
  pub json: bool,
  pub lint: bool,
  pub html: Option<DocHtmlFlag>,
  pub source_files: DocSourceFileFlag,
  pub filter: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalFlags {
  pub print: bool,
  pub code: String,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct FmtFlags {
  pub check: bool,
  pub files: FileFlags,
  pub use_tabs: Option<bool>,
  pub line_width: Option<NonZeroU32>,
  pub indent_width: Option<NonZeroU8>,
  pub single_quote: Option<bool>,
  pub prose_wrap: Option<String>,
  pub no_semicolons: Option<bool>,
  pub watch: Option<WatchFlags>,
  pub unstable_component: bool,
}

impl FmtFlags {
  pub fn is_stdin(&self) -> bool {
    let args = &self.files.include;
    args.len() == 1 && args[0] == "-"
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitFlags {
  pub dir: Option<String>,
  pub lib: bool,
  pub serve: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InfoFlags {
  pub json: bool,
  pub file: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallFlagsGlobal {
  pub module_url: String,
  pub args: Vec<String>,
  pub name: Option<String>,
  pub root: Option<String>,
  pub force: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallKind {
  Local(InstallFlagsLocal),
  Global(InstallFlagsGlobal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallFlagsLocal {
  Add(AddFlags),
  TopLevel,
  Entrypoints(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallFlags {
  pub kind: InstallKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JSONReferenceFlags {
  pub json: deno_core::serde_json::Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JupyterFlags {
  pub install: bool,
  pub kernel: bool,
  pub conn_file: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallFlagsGlobal {
  pub name: String,
  pub root: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UninstallKind {
  Local(RemoveFlags),
  Global(UninstallFlagsGlobal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallFlags {
  pub kind: UninstallKind,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LintFlags {
  pub files: FileFlags,
  pub rules: bool,
  pub fix: bool,
  pub maybe_rules_tags: Option<Vec<String>>,
  pub maybe_rules_include: Option<Vec<String>>,
  pub maybe_rules_exclude: Option<Vec<String>>,
  pub json: bool,
  pub compact: bool,
  pub watch: Option<WatchFlags>,
}

impl LintFlags {
  pub fn is_stdin(&self) -> bool {
    let args = &self.files.include;
    args.len() == 1 && args[0] == "-"
  }
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct ReplFlags {
  pub eval_files: Option<Vec<String>>,
  pub eval: Option<String>,
  pub is_default_command: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct RunFlags {
  pub script: String,
  pub watch: Option<WatchFlagsWithPaths>,
  pub bare: bool,
}

impl RunFlags {
  #[cfg(test)]
  pub fn new_default(script: String) -> Self {
    Self {
      script,
      watch: None,
      bare: false,
    }
  }

  pub fn is_stdin(&self) -> bool {
    self.script == "-"
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServeFlags {
  pub script: String,
  pub watch: Option<WatchFlagsWithPaths>,
  pub port: u16,
  pub host: String,
  pub worker_count: Option<usize>,
}

impl ServeFlags {
  #[cfg(test)]
  pub fn new_default(script: String, port: u16, host: &str) -> Self {
    Self {
      script,
      watch: None,
      port,
      host: host.to_owned(),
      worker_count: None,
    }
  }
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct WatchFlags {
  pub hmr: bool,
  pub no_clear_screen: bool,
  pub exclude: Vec<String>,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct WatchFlagsWithPaths {
  pub hmr: bool,
  pub paths: Vec<String>,
  pub no_clear_screen: bool,
  pub exclude: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskFlags {
  pub cwd: Option<String>,
  pub task: Option<String>,
  pub is_run: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TestReporterConfig {
  #[default]
  Pretty,
  Dot,
  Junit,
  Tap,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TestFlags {
  pub doc: bool,
  pub no_run: bool,
  pub coverage_dir: Option<String>,
  pub clean: bool,
  pub fail_fast: Option<NonZeroUsize>,
  pub files: FileFlags,
  pub permit_no_files: bool,
  pub filter: Option<String>,
  pub shuffle: Option<u64>,
  pub concurrent_jobs: Option<NonZeroUsize>,
  pub trace_leaks: bool,
  pub watch: Option<WatchFlagsWithPaths>,
  pub reporter: TestReporterConfig,
  pub junit_path: Option<String>,
  pub hide_stacktraces: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeFlags {
  pub dry_run: bool,
  pub force: bool,
  pub release_candidate: bool,
  pub canary: bool,
  pub version: Option<String>,
  pub output: Option<String>,
  pub version_or_hash_or_channel: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishFlags {
  pub token: Option<String>,
  pub dry_run: bool,
  pub allow_slow_types: bool,
  pub allow_dirty: bool,
  pub no_provenance: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelpFlags {
  pub help: clap::builder::StyledStr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DenoSubcommand {
  Add(AddFlags),
  Remove(RemoveFlags),
  Bench(BenchFlags),
  Bundle,
  Cache(CacheFlags),
  Check(CheckFlags),
  Clean,
  Compile(CompileFlags),
  Completions(CompletionsFlags),
  Coverage(CoverageFlags),
  Doc(DocFlags),
  Eval(EvalFlags),
  Fmt(FmtFlags),
  Init(InitFlags),
  Info(InfoFlags),
  Install(InstallFlags),
  JSONReference(JSONReferenceFlags),
  Jupyter(JupyterFlags),
  Uninstall(UninstallFlags),
  Lsp,
  Lint(LintFlags),
  Repl(ReplFlags),
  Run(RunFlags),
  Serve(ServeFlags),
  Task(TaskFlags),
  Test(TestFlags),
  Types,
  Upgrade(UpgradeFlags),
  Vendor,
  Publish(PublishFlags),
  Help(HelpFlags),
}

impl DenoSubcommand {
  pub fn is_run(&self) -> bool {
    matches!(self, Self::Run(_))
  }

  // Returns `true` if the subcommand depends on testing infrastructure.
  pub fn needs_test(&self) -> bool {
    matches!(
      self,
      Self::Test(_)
        | Self::Jupyter(_)
        | Self::Repl(_)
        | Self::Bench(_)
        | Self::Lsp
    )
  }
}

impl Default for DenoSubcommand {
  fn default() -> DenoSubcommand {
    DenoSubcommand::Repl(ReplFlags {
      eval_files: None,
      eval: None,
      is_default_command: true,
    })
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TypeCheckMode {
  /// Type-check all modules.
  All,
  /// Skip type-checking of all modules. The default value for "deno run" and
  /// several other subcommands.
  None,
  /// Only type-check local modules. The default value for "deno test" and
  /// several other subcommands.
  Local,
}

impl TypeCheckMode {
  /// Gets if type checking will occur under this mode.
  pub fn is_true(&self) -> bool {
    match self {
      Self::None => false,
      Self::Local | Self::All => true,
    }
  }

  /// Gets the corresponding module `GraphKind` that should be created
  /// for the current `TypeCheckMode`.
  pub fn as_graph_kind(&self) -> GraphKind {
    match self.is_true() {
      true => GraphKind::All,
      false => GraphKind::CodeOnly,
    }
  }
}

impl Default for TypeCheckMode {
  fn default() -> Self {
    Self::None
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaData {
  /// The string is a file path
  File(String),
  /// This variant is not exposed as an option in the CLI, it is used internally
  /// for standalone binaries.
  Bytes(Vec<u8>),
}

// Info needed to run NPM lifecycle scripts
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct LifecycleScriptsConfig {
  pub allowed: PackagesAllowedScripts,
  pub initial_cwd: PathBuf,
  pub root_dir: PathBuf,
  /// Part of an explicit `deno install`
  pub explicit_install: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
/// The set of npm packages that are allowed to run lifecycle scripts.
pub enum PackagesAllowedScripts {
  All,
  Some(Vec<String>),
  #[default]
  None,
}

fn parse_packages_allowed_scripts(s: &str) -> Result<String, AnyError> {
  if !s.starts_with("npm:") {
    bail!("Invalid package for --allow-scripts: '{}'. An 'npm:' specifier is required", s);
  } else {
    Ok(s.into())
  }
}

#[derive(
  Clone, Default, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize,
)]
pub struct UnstableConfig {
  // TODO(bartlomieju): remove in Deno 2.5
  pub legacy_flag_enabled: bool, // --unstable
  pub bare_node_builtins: bool,  // --unstable-bare-node-builts
  pub sloppy_imports: bool,
  pub features: Vec<String>, // --unstabe-kv --unstable-cron
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct InternalFlags {
  /// Used when the language server is configured with an
  /// explicit cache option.
  pub cache_path: Option<PathBuf>,
  /// Only reads to the lockfile instead of writing to it.
  pub lockfile_skip_write: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct Flags {
  /// Vector of CLI arguments - these are user script arguments, all Deno
  /// specific flags are removed.
  pub argv: Vec<String>,
  pub subcommand: DenoSubcommand,

  pub frozen_lockfile: Option<bool>,
  pub ca_stores: Option<Vec<String>>,
  pub ca_data: Option<CaData>,
  pub cache_blocklist: Vec<String>,
  pub cached_only: bool,
  pub type_check_mode: TypeCheckMode,
  pub config_flag: ConfigFlag,
  pub node_modules_dir: Option<NodeModulesDirMode>,
  pub vendor: Option<bool>,
  pub enable_op_summary_metrics: bool,
  pub enable_testing_features: bool,
  pub ext: Option<String>,
  /// Flags that aren't exposed in the CLI, but are used internally.
  pub internal: InternalFlags,
  pub ignore: Vec<String>,
  pub import_map_path: Option<String>,
  pub env_file: Option<String>,
  pub inspect_brk: Option<SocketAddr>,
  pub inspect_wait: Option<SocketAddr>,
  pub inspect: Option<SocketAddr>,
  pub location: Option<Url>,
  pub lock: Option<String>,
  pub log_level: Option<Level>,
  pub no_remote: bool,
  pub no_lock: bool,
  pub no_npm: bool,
  pub reload: bool,
  pub seed: Option<u64>,
  pub strace_ops: Option<Vec<String>>,
  pub unstable_config: UnstableConfig,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub v8_flags: Vec<String>,
  pub code_cache_enabled: bool,
  pub permissions: PermissionFlags,
  pub allow_scripts: PackagesAllowedScripts,
}

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct PermissionFlags {
  pub allow_all: bool,
  pub allow_env: Option<Vec<String>>,
  pub deny_env: Option<Vec<String>>,
  pub allow_ffi: Option<Vec<String>>,
  pub deny_ffi: Option<Vec<String>>,
  pub allow_net: Option<Vec<String>>,
  pub deny_net: Option<Vec<String>>,
  pub allow_read: Option<Vec<String>>,
  pub deny_read: Option<Vec<String>>,
  pub allow_run: Option<Vec<String>>,
  pub deny_run: Option<Vec<String>>,
  pub allow_sys: Option<Vec<String>>,
  pub deny_sys: Option<Vec<String>>,
  pub allow_write: Option<Vec<String>>,
  pub deny_write: Option<Vec<String>>,
  pub no_prompt: bool,
  pub allow_import: Option<Vec<String>>,
}

impl PermissionFlags {
  pub fn has_permission(&self) -> bool {
    self.allow_all
      || self.allow_env.is_some()
      || self.deny_env.is_some()
      || self.allow_ffi.is_some()
      || self.deny_ffi.is_some()
      || self.allow_net.is_some()
      || self.deny_net.is_some()
      || self.allow_read.is_some()
      || self.deny_read.is_some()
      || self.allow_run.is_some()
      || self.deny_run.is_some()
      || self.allow_sys.is_some()
      || self.deny_sys.is_some()
      || self.allow_write.is_some()
      || self.deny_write.is_some()
      || self.allow_import.is_some()
  }

  pub fn to_options(&self, cli_arg_urls: &[Cow<Url>]) -> PermissionsOptions {
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

    fn handle_imports(
      cli_arg_urls: &[Cow<Url>],
      imports: Option<Vec<String>>,
    ) -> Option<Vec<String>> {
      if imports.is_some() {
        return imports;
      }

      let builtin_allowed_import_hosts = [
        "jsr.io:443",
        "deno.land:443",
        "esm.sh:443",
        "cdn.jsdelivr.net:443",
        "raw.githubusercontent.com:443",
        "gist.githubusercontent.com:443",
      ];

      let mut imports =
        Vec::with_capacity(builtin_allowed_import_hosts.len() + 1);
      imports
        .extend(builtin_allowed_import_hosts.iter().map(|s| s.to_string()));

      // also add the JSR_URL env var
      if let Some(jsr_host) = allow_import_host_from_url(jsr_url()) {
        imports.push(jsr_host);
      }
      // include the cli arg urls
      for url in cli_arg_urls {
        if let Some(host) = allow_import_host_from_url(url) {
          imports.push(host);
        }
      }

      Some(imports)
    }

    PermissionsOptions {
      allow_all: self.allow_all,
      allow_env: handle_allow(self.allow_all, self.allow_env.clone()),
      deny_env: self.deny_env.clone(),
      allow_net: handle_allow(self.allow_all, self.allow_net.clone()),
      deny_net: self.deny_net.clone(),
      allow_ffi: handle_allow(self.allow_all, self.allow_ffi.clone()),
      deny_ffi: self.deny_ffi.clone(),
      allow_read: handle_allow(self.allow_all, self.allow_read.clone()),
      deny_read: self.deny_read.clone(),
      allow_run: handle_allow(self.allow_all, self.allow_run.clone()),
      deny_run: self.deny_run.clone(),
      allow_sys: handle_allow(self.allow_all, self.allow_sys.clone()),
      deny_sys: self.deny_sys.clone(),
      allow_write: handle_allow(self.allow_all, self.allow_write.clone()),
      deny_write: self.deny_write.clone(),
      allow_import: handle_imports(
        cli_arg_urls,
        handle_allow(self.allow_all, self.allow_import.clone()),
      ),
      prompt: !resolve_no_prompt(self),
    }
  }
}

/// Gets the --allow-import host from the provided url
fn allow_import_host_from_url(url: &Url) -> Option<String> {
  let host = url.host()?;
  if let Some(port) = url.port() {
    Some(format!("{}:{}", host, port))
  } else {
    use deno_core::url::Host::*;
    match host {
      Domain(domain) if domain == "jsr.io" && url.scheme() == "https" => None,
      _ => match url.scheme() {
        "https" => Some(format!("{}:443", host)),
        "http" => Some(format!("{}:80", host)),
        _ => None,
      },
    }
  }
}

fn join_paths(allowlist: &[String], d: &str) -> String {
  allowlist
    .iter()
    .map(|path| path.to_string())
    .collect::<Vec<String>>()
    .join(d)
}

impl Flags {
  /// Return list of permission arguments that are equivalent
  /// to the ones used to create `self`.
  pub fn to_permission_args(&self) -> Vec<String> {
    let mut args = vec![];

    if self.permissions.allow_all {
      args.push("--allow-all".to_string());
      return args;
    }

    match &self.permissions.allow_read {
      Some(read_allowlist) if read_allowlist.is_empty() => {
        args.push("--allow-read".to_string());
      }
      Some(read_allowlist) => {
        let s = format!("--allow-read={}", join_paths(read_allowlist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_read {
      Some(read_denylist) if read_denylist.is_empty() => {
        args.push("--deny-read".to_string());
      }
      Some(read_denylist) => {
        let s = format!("--deny-read={}", join_paths(read_denylist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_write {
      Some(write_allowlist) if write_allowlist.is_empty() => {
        args.push("--allow-write".to_string());
      }
      Some(write_allowlist) => {
        let s = format!("--allow-write={}", join_paths(write_allowlist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_write {
      Some(write_denylist) if write_denylist.is_empty() => {
        args.push("--deny-write".to_string());
      }
      Some(write_denylist) => {
        let s = format!("--deny-write={}", join_paths(write_denylist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_net {
      Some(net_allowlist) if net_allowlist.is_empty() => {
        args.push("--allow-net".to_string());
      }
      Some(net_allowlist) => {
        let s = format!("--allow-net={}", net_allowlist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_net {
      Some(net_denylist) if net_denylist.is_empty() => {
        args.push("--deny-net".to_string());
      }
      Some(net_denylist) => {
        let s = format!("--deny-net={}", net_denylist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.unsafely_ignore_certificate_errors {
      Some(ic_allowlist) if ic_allowlist.is_empty() => {
        args.push("--unsafely-ignore-certificate-errors".to_string());
      }
      Some(ic_allowlist) => {
        let s = format!(
          "--unsafely-ignore-certificate-errors={}",
          ic_allowlist.join(",")
        );
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_env {
      Some(env_allowlist) if env_allowlist.is_empty() => {
        args.push("--allow-env".to_string());
      }
      Some(env_allowlist) => {
        let s = format!("--allow-env={}", env_allowlist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_env {
      Some(env_denylist) if env_denylist.is_empty() => {
        args.push("--deny-env".to_string());
      }
      Some(env_denylist) => {
        let s = format!("--deny-env={}", env_denylist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_run {
      Some(run_allowlist) if run_allowlist.is_empty() => {
        args.push("--allow-run".to_string());
      }
      Some(run_allowlist) => {
        let s = format!("--allow-run={}", run_allowlist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_run {
      Some(run_denylist) if run_denylist.is_empty() => {
        args.push("--deny-run".to_string());
      }
      Some(run_denylist) => {
        let s = format!("--deny-run={}", run_denylist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_sys {
      Some(sys_allowlist) if sys_allowlist.is_empty() => {
        args.push("--allow-sys".to_string());
      }
      Some(sys_allowlist) => {
        let s = format!("--allow-sys={}", sys_allowlist.join(","));
        args.push(s)
      }
      _ => {}
    }

    match &self.permissions.deny_sys {
      Some(sys_denylist) if sys_denylist.is_empty() => {
        args.push("--deny-sys".to_string());
      }
      Some(sys_denylist) => {
        let s = format!("--deny-sys={}", sys_denylist.join(","));
        args.push(s)
      }
      _ => {}
    }

    match &self.permissions.allow_ffi {
      Some(ffi_allowlist) if ffi_allowlist.is_empty() => {
        args.push("--allow-ffi".to_string());
      }
      Some(ffi_allowlist) => {
        let s = format!("--allow-ffi={}", join_paths(ffi_allowlist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_ffi {
      Some(ffi_denylist) if ffi_denylist.is_empty() => {
        args.push("--deny-ffi".to_string());
      }
      Some(ffi_denylist) => {
        let s = format!("--deny-ffi={}", join_paths(ffi_denylist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_import {
      Some(allowlist) if allowlist.is_empty() => {
        args.push("--allow-import".to_string());
      }
      Some(allowlist) => {
        let s = format!("--allow-import={}", allowlist.join(","));
        args.push(s);
      }
      _ => {}
    }

    args
  }

  /// Extract the paths the config file should be discovered from.
  ///
  /// Returns `None` if the config file should not be auto-discovered.
  pub fn config_path_args(&self, current_dir: &Path) -> Option<Vec<PathBuf>> {
    fn resolve_multiple_files(
      files_or_dirs: &[String],
      current_dir: &Path,
    ) -> Vec<PathBuf> {
      let mut seen = HashSet::with_capacity(files_or_dirs.len());
      let result = files_or_dirs
        .iter()
        .filter_map(|p| {
          let path = normalize_path(current_dir.join(p));
          if seen.insert(path.clone()) {
            Some(path)
          } else {
            None
          }
        })
        .collect::<Vec<_>>();
      if result.is_empty() {
        vec![current_dir.to_path_buf()]
      } else {
        result
      }
    }

    use DenoSubcommand::*;
    match &self.subcommand {
      Fmt(FmtFlags { files, .. }) => {
        Some(resolve_multiple_files(&files.include, current_dir))
      }
      Lint(LintFlags { files, .. }) => {
        Some(resolve_multiple_files(&files.include, current_dir))
      }
      Run(RunFlags { script, .. })
      | Compile(CompileFlags {
        source_file: script,
        ..
      }) => {
        if let Ok(module_specifier) = resolve_url_or_path(script, current_dir) {
          if module_specifier.scheme() == "file"
            || module_specifier.scheme() == "npm"
          {
            if let Ok(p) = url_to_file_path(&module_specifier) {
              Some(vec![p.parent().unwrap().to_path_buf()])
            } else {
              Some(vec![current_dir.to_path_buf()])
            }
          } else {
            // When the entrypoint doesn't have file: scheme (it's the remote
            // script), then we don't auto discover config file.
            None
          }
        } else {
          Some(vec![current_dir.to_path_buf()])
        }
      }
      Task(TaskFlags {
        cwd: Some(path), ..
      }) => {
        // todo(dsherret): Why is this canonicalized? Document why.
        // attempt to resolve the config file from the task subcommand's
        // `--cwd` when specified
        match canonicalize_path(&PathBuf::from(path)) {
          Ok(path) => Some(vec![path]),
          Err(_) => Some(vec![current_dir.to_path_buf()]),
        }
      }
      _ => Some(vec![current_dir.to_path_buf()]),
    }
  }

  pub fn has_permission(&self) -> bool {
    self.permissions.has_permission()
  }

  pub fn has_permission_in_argv(&self) -> bool {
    self.argv.iter().any(|arg| {
      arg == "--allow-all"
        || arg.starts_with("--allow-env")
        || arg.starts_with("--deny-env")
        || arg.starts_with("--allow-ffi")
        || arg.starts_with("--deny-ffi")
        || arg.starts_with("--allow-net")
        || arg.starts_with("--deny-net")
        || arg.starts_with("--allow-read")
        || arg.starts_with("--deny-read")
        || arg.starts_with("--allow-run")
        || arg.starts_with("--deny-run")
        || arg.starts_with("--allow-sys")
        || arg.starts_with("--deny-sys")
        || arg.starts_with("--allow-write")
        || arg.starts_with("--deny-write")
    })
  }

  #[inline(always)]
  fn allow_all(&mut self) {
    self.permissions.allow_all = true;
    self.permissions.allow_read = None;
    self.permissions.allow_env = None;
    self.permissions.allow_net = None;
    self.permissions.allow_run = None;
    self.permissions.allow_write = None;
    self.permissions.allow_sys = None;
    self.permissions.allow_ffi = None;
    self.permissions.allow_import = None;
  }

  pub fn resolve_watch_exclude_set(
    &self,
  ) -> Result<PathOrPatternSet, AnyError> {
    if let DenoSubcommand::Run(RunFlags {
      watch:
        Some(WatchFlagsWithPaths {
          exclude: excluded_paths,
          ..
        }),
      ..
    })
    | DenoSubcommand::Bench(BenchFlags {
      watch:
        Some(WatchFlags {
          exclude: excluded_paths,
          ..
        }),
      ..
    })
    | DenoSubcommand::Test(TestFlags {
      watch:
        Some(WatchFlagsWithPaths {
          exclude: excluded_paths,
          ..
        }),
      ..
    })
    | DenoSubcommand::Lint(LintFlags {
      watch:
        Some(WatchFlags {
          exclude: excluded_paths,
          ..
        }),
      ..
    })
    | DenoSubcommand::Fmt(FmtFlags {
      watch:
        Some(WatchFlags {
          exclude: excluded_paths,
          ..
        }),
      ..
    }) = &self.subcommand
    {
      let cwd = std::env::current_dir()?;
      PathOrPatternSet::from_exclude_relative_path_or_patterns(
        &cwd,
        excluded_paths,
      )
      .context("Failed resolving watch exclude patterns.")
    } else {
      Ok(PathOrPatternSet::default())
    }
  }
}

static ENV_VARIABLES_HELP: &str = cstr!(
  r#"<y>Environment variables:</>
<y>Docs:</> <c>https://docs.deno.com/go/env-vars</>

  <g>DENO_AUTH_TOKENS</>      A semi-colon separated list of bearer tokens and hostnames
                        to use when fetching remote modules from private repositories
                         <p(245)>(e.g. "abcde12345@deno.land;54321edcba@github.com")</>
  <g>DENO_CERT</>             Load certificate authorities from PEM encoded file
  <g>DENO_DIR</>              Set the cache directory
  <g>DENO_INSTALL_ROOT</>     Set deno install's output directory
                         <p(245)>(defaults to $HOME/.deno/bin)</>
  <g>DENO_NO_PACKAGE_JSON</>  Disables auto-resolution of package.json
  <g>DENO_NO_UPDATE_CHECK</>  Set to disable checking if a newer Deno version is available
  <g>DENO_TLS_CA_STORE</>     Comma-separated list of order dependent certificate stores.
                        Possible values: "system", "mozilla".
                         <p(245)>(defaults to "mozilla")</>
  <g>HTTP_PROXY</>            Proxy address for HTTP requests
                         <p(245)>(module downloads, fetch)</>
  <g>HTTPS_PROXY</>           Proxy address for HTTPS requests
                         <p(245)>(module downloads, fetch)</>
  <g>NO_COLOR</>              Set to disable color
  <g>NO_PROXY</>              Comma-separated list of hosts which do not use a proxy
                         <p(245)>(module downloads, fetch)</>
  <g>NPM_CONFIG_REGISTRY</>   URL to use for the npm registry."#
);

static DENO_HELP: &str = cstr!(
  "Deno: <g>A modern JavaScript and TypeScript runtime</>

<p(245)>Usage:</> <g>{usage}</>

<y>Commands:</>
  <y>Execution:</>
    <g>run</>          Run a JavaScript or TypeScript program, or a task
                  <p(245)>deno run main.ts  |  deno run --allow-net=google.com main.ts  |  deno main.ts</>
    <g>serve</>        Run a server
                  <p(245)>deno serve main.ts</>
    <g>task</>         Run a task defined in the configuration file
                  <p(245)>deno task dev</>
    <g>repl</>         Start an interactive Read-Eval-Print Loop (REPL) for Deno
    <g>eval</>         Evaluate a script from the command line

  <y>Dependency management:</>
    <g>add</>          Add dependencies
                  <p(245)>deno add @std/assert  |  deno add npm:express</>
    <g>install</>      Install script as an executable
    <g>uninstall</>    Uninstall a script previously installed with deno install
    <g>remove</>       Remove dependencies from the configuration file

  <y>Tooling:</>
    <g>bench</>        Run benchmarks
                  <p(245)>deno bench bench.ts</>
    <g>check</>        Type-check the dependencies
    <g>clean</>        Remove the cache directory
    <g>compile</>      Compile the script into a self contained executable
                  <p(245)>deno compile main.ts  |  deno compile --target=x86_64-unknown-linux-gnu</>
    <g>coverage</>     Print coverage reports
    <g>doc</>          Genereate and show documentation for a module or built-ins
                  <p(245)>deno doc  |  deno doc --json  |  deno doc --html mod.ts</>
    <g>fmt</>          Format source files
                  <p(245)>deno fmt  |  deno fmt main.ts</>
    <g>info</>         Show info about cache or info related to source file
    <g>jupyter</>      Deno kernel for Jupyter notebooks
    <g>lint</>         Lint source files
    <g>init</>         Initialize a new project
    <g>test</>         Run tests
                  <p(245)>deno test  |  deno test test.ts</>
    <g>publish</>      Publish the current working directory's package or workspace
    <g>upgrade</>      Upgrade deno executable to given version
                  <p(245)>deno upgrade  |  deno upgrade 1.45.0  |  deno upgrade canary</>
{after-help}

<y>Docs:</> https://docs.deno.com
<y>Standard Library:</> https://jsr.io/@std
<y>Bugs:</> https://github.com/denoland/deno/issues
<y>Discord:</> https://discord.gg/deno
");

/// Main entry point for parsing deno's command line flags.
pub fn flags_from_vec(args: Vec<OsString>) -> clap::error::Result<Flags> {
  let mut app = clap_root();
  let mut matches =
    app
      .try_get_matches_from_mut(&args)
      .map_err(|mut e| match e.kind() {
        ErrorKind::MissingRequiredArgument => {
          if let Some(clap::error::ContextValue::Strings(s)) =
            e.get(clap::error::ContextKind::InvalidArg)
          {
            if s.len() == 1
              && s[0] == "--global"
              && args.iter().any(|arg| arg == "install")
            {
              e.insert(
                clap::error::ContextKind::Usage,
                clap::error::ContextValue::StyledStr(
                  "Note: Permission flags can only be used in a global setting"
                    .into(),
                ),
              );
            }
          }

          e
        }
        _ => e,
      })?;

  let mut flags = Flags::default();

  if matches.get_flag("quiet") {
    flags.log_level = Some(Level::Error);
  } else if let Some(log_level) = matches.get_one::<String>("log-level") {
    flags.log_level = match log_level.as_str() {
      "trace" => Some(Level::Trace),
      "debug" => Some(Level::Debug),
      "info" => Some(Level::Info),
      _ => unreachable!(),
    };
  }

  if let Some(help_expansion) = matches.get_one::<String>("help").cloned() {
    let mut subcommand = if let Some((sub, _)) = matches.remove_subcommand() {
      app.find_subcommand(sub).unwrap().clone()
    } else {
      app
    };

    if help_expansion == "unstable"
      && subcommand
        .get_arguments()
        .any(|arg| arg.get_id().as_str() == "unstable")
    {
      subcommand = enable_unstable(subcommand);
    }

    help_parse(&mut flags, subcommand);
    return Ok(flags);
  } else if matches.contains_id("help") {
    let subcommand = if let Some((sub, _)) = matches.remove_subcommand() {
      app.find_subcommand(sub).unwrap().clone()
    } else {
      app
    };

    help_parse(&mut flags, subcommand);
    return Ok(flags);
  } else if let Some(help_subcommand_matches) =
    matches.subcommand_matches("help")
  {
    app.build();
    let subcommand =
      if let Some(sub) = help_subcommand_matches.subcommand_name() {
        app.find_subcommand(sub).unwrap().clone()
      } else {
        app
      };

    help_parse(&mut flags, subcommand);
    return Ok(flags);
  }

  if let Some((subcommand, mut m)) = matches.remove_subcommand() {
    let pre_subcommand_arg = app
      .get_arguments()
      .filter(|arg| !arg.is_global_set())
      .find(|arg| {
        matches
          .value_source(arg.get_id().as_str())
          .is_some_and(|value| value == clap::parser::ValueSource::CommandLine)
      })
      .map(|arg| {
        format!(
          "--{}",
          arg.get_long().unwrap_or_else(|| arg.get_id().as_str())
        )
      });

    if let Some(arg) = pre_subcommand_arg {
      let usage = app.find_subcommand_mut(&subcommand).unwrap().render_usage();

      let mut err =
        clap::error::Error::new(ErrorKind::UnknownArgument).with_cmd(&app);
      err.insert(
        clap::error::ContextKind::InvalidArg,
        clap::error::ContextValue::String(arg.clone()),
      );

      let valid = app.get_styles().get_valid();

      let styled_suggestion = clap::builder::StyledStr::from(format!(
        "'{}{subcommand} {arg}{}' exists",
        valid.render(),
        valid.render_reset()
      ));

      err.insert(
        clap::error::ContextKind::Suggested,
        clap::error::ContextValue::StyledStrs(vec![styled_suggestion]),
      );
      err.insert(
        clap::error::ContextKind::Usage,
        clap::error::ContextValue::StyledStr(usage),
      );

      return Err(err);
    }

    match subcommand.as_str() {
      "add" => add_parse(&mut flags, &mut m),
      "remove" => remove_parse(&mut flags, &mut m),
      "bench" => bench_parse(&mut flags, &mut m)?,
      "bundle" => bundle_parse(&mut flags, &mut m),
      "cache" => cache_parse(&mut flags, &mut m)?,
      "check" => check_parse(&mut flags, &mut m)?,
      "clean" => clean_parse(&mut flags, &mut m),
      "compile" => compile_parse(&mut flags, &mut m)?,
      "completions" => completions_parse(&mut flags, &mut m, app),
      "coverage" => coverage_parse(&mut flags, &mut m)?,
      "doc" => doc_parse(&mut flags, &mut m)?,
      "eval" => eval_parse(&mut flags, &mut m)?,
      "fmt" => fmt_parse(&mut flags, &mut m)?,
      "init" => init_parse(&mut flags, &mut m),
      "info" => info_parse(&mut flags, &mut m)?,
      "install" => install_parse(&mut flags, &mut m)?,
      "json_reference" => json_reference_parse(&mut flags, &mut m, app),
      "jupyter" => jupyter_parse(&mut flags, &mut m),
      "lint" => lint_parse(&mut flags, &mut m)?,
      "lsp" => lsp_parse(&mut flags, &mut m),
      "repl" => repl_parse(&mut flags, &mut m)?,
      "run" => run_parse(&mut flags, &mut m, app, false)?,
      "serve" => serve_parse(&mut flags, &mut m, app)?,
      "task" => task_parse(&mut flags, &mut m),
      "test" => test_parse(&mut flags, &mut m)?,
      "types" => types_parse(&mut flags, &mut m),
      "uninstall" => uninstall_parse(&mut flags, &mut m),
      "upgrade" => upgrade_parse(&mut flags, &mut m),
      "vendor" => vendor_parse(&mut flags, &mut m),
      "publish" => publish_parse(&mut flags, &mut m),
      _ => unreachable!(),
    }
  } else {
    let has_non_globals = app
      .get_arguments()
      .filter(|arg| !arg.is_global_set())
      .any(|arg| {
        matches
          .value_source(arg.get_id().as_str())
          .is_some_and(|value| value != clap::parser::ValueSource::DefaultValue)
      });

    if has_non_globals || matches.contains_id("script_arg") {
      run_parse(&mut flags, &mut matches, app, true)?;
    } else {
      handle_repl_flags(
        &mut flags,
        ReplFlags {
          eval_files: None,
          eval: None,
          is_default_command: true,
        },
      )
    }
  }

  Ok(flags)
}

fn enable_unstable(command: Command) -> Command {
  command
    .mut_arg("unstable", |arg| {
      let new_help = arg
        .get_help()
        .unwrap()
        .to_string()
        .split_once("\n")
        .unwrap()
        .0
        .to_string();
      arg.help_heading(UNSTABLE_HEADING).help(new_help)
    })
    .mut_args(|arg| {
      // long_help here is being used as a metadata, see unstable args definition
      if arg.get_help_heading() == Some(UNSTABLE_HEADING)
        && arg.get_long_help().is_some()
      {
        arg.hide(false)
      } else {
        arg
      }
    })
}

macro_rules! heading {
    ($($name:ident = $title:expr),+; $total:literal) => {
      $(const $name: &str = $title;)+
      const HEADING_ORDER: [&str; $total] = [$($name),+];
    };
}

heading! {
  // subcommand flags headings
  DOC_HEADING = "Documentation options",
  FMT_HEADING = "Formatting options",
  COMPILE_HEADING = "Compile options",
  LINT_HEADING = "Linting options",
  TEST_HEADING = "Testing options",
  UPGRADE_HEADING = "Upgrade options",
  PUBLISH_HEADING = "Publishing options",

  // categorized flags headings
  TYPE_CHECKING_HEADING = "Type checking options",
  FILE_WATCHING_HEADING = "File watching options",
  DEBUGGING_HEADING = "Debugging options",
  DEPENDENCY_MANAGEMENT_HEADING = "Dependency management options",

  UNSTABLE_HEADING = "Unstable options";
  12
}

fn help_parse(flags: &mut Flags, mut subcommand: Command) {
  let mut args = subcommand
    .get_arguments()
    .map(|arg| {
      (
        arg.get_id().as_str().to_string(),
        arg.get_help_heading().map(|h| h.to_string()),
      )
    })
    .collect::<Vec<_>>();
  args.sort_by(|a, b| {
    a.1
      .as_ref()
      .map(|heading| HEADING_ORDER.iter().position(|h| h == heading))
      .cmp(
        &b.1
          .as_ref()
          .map(|heading| HEADING_ORDER.iter().position(|h| h == heading)),
      )
      .then(a.0.cmp(&b.0))
  });

  for (mut i, (arg, heading)) in args.into_iter().enumerate() {
    if let Some(heading) = heading {
      let heading_i = HEADING_ORDER.iter().position(|h| h == &heading).unwrap();
      i += heading_i * 100;
    }

    subcommand = subcommand.mut_arg(arg, |arg| arg.display_order(i));
  }

  flags.subcommand = DenoSubcommand::Help(HelpFlags {
    help: subcommand.render_help(),
  });
}

// copied from clap, https://github.com/clap-rs/clap/blob/4e1a565b8adb4f2ad74a9631565574767fdc37ae/clap_builder/src/parser/features/suggestions.rs#L11-L26
pub fn did_you_mean<T, I>(v: &str, possible_values: I) -> Vec<String>
where
  T: AsRef<str>,
  I: IntoIterator<Item = T>,
{
  let mut candidates: Vec<(f64, String)> = possible_values
    .into_iter()
    // GH #4660: using `jaro` because `jaro_winkler` implementation in `strsim-rs` is wrong
    // causing strings with common prefix >=10 to be considered perfectly similar
    .map(|pv| (strsim::jaro(v, pv.as_ref()), pv.as_ref().to_owned()))
    // Confidence of 0.7 so that bar -> baz is suggested
    .filter(|(confidence, _)| *confidence > 0.7)
    .collect();
  candidates
    .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
  candidates.into_iter().map(|(_, pv)| pv).collect()
}

fn handle_repl_flags(flags: &mut Flags, repl_flags: ReplFlags) {
  // If user runs just `deno` binary we enter REPL and allow all permissions.
  if repl_flags.is_default_command {
    flags.allow_all();
  }
  flags.subcommand = DenoSubcommand::Repl(repl_flags);
}

pub fn clap_root() -> Command {
  let long_version = format!(
    "{} ({}, {}, {})\nv8 {}\ntypescript {}",
    crate::version::DENO_VERSION_INFO.deno,
    crate::version::DENO_VERSION_INFO.release_channel.name(),
    env!("PROFILE"),
    env!("TARGET"),
    deno_core::v8::VERSION_STRING,
    crate::version::DENO_VERSION_INFO.typescript
  );

  run_args(Command::new("deno"), true)
    .args(unstable_args(UnstableArgsConfig::ResolutionAndRuntime))
    .next_line_help(false)
    .bin_name("deno")
    .styles(
      clap::builder::Styles::styled()
        .header(AnsiColor::Yellow.on_default())
        .usage(AnsiColor::White.on_default())
        .literal(AnsiColor::Green.on_default())
        .placeholder(AnsiColor::Green.on_default()),
    )
    .color(ColorChoice::Auto)
    .term_width(800)
    .version(crate::version::DENO_VERSION_INFO.deno)
    .long_version(long_version)
    .disable_version_flag(true)
    .disable_help_flag(true)
    .disable_help_subcommand(true)
    .arg(
      Arg::new("help")
        .short('h')
        .long("help")
        .hide(true)
        .action(ArgAction::Append)
        .num_args(0..=1)
        .require_equals(true)
        .value_parser(["unstable"])
        .global(true),
    )
    .arg(
      Arg::new("version")
        .short('V')
        .short_alias('v')
        .long("version")
        .action(ArgAction::Version)
        .help("Print version"),
    )
    .arg(
      Arg::new("log-level")
        .short('L')
        .long("log-level")
        .help("Set log level")
        .hide(true)
        .value_parser(["trace", "debug", "info"])
        .global(true),
    )
    .arg(
      Arg::new("quiet")
        .short('q')
        .long("quiet")
        .help("Suppress diagnostic output")
        .action(ArgAction::SetTrue)
        .global(true),
    )
    .subcommand(run_subcommand())
    .subcommand(serve_subcommand())
    .defer(|cmd| {
      let cmd = cmd
        .subcommand(add_subcommand())
        .subcommand(remove_subcommand())
        .subcommand(bench_subcommand())
        .subcommand(bundle_subcommand())
        .subcommand(cache_subcommand())
        .subcommand(check_subcommand())
        .subcommand(clean_subcommand())
        .subcommand(compile_subcommand())
        .subcommand(completions_subcommand())
        .subcommand(coverage_subcommand())
        .subcommand(doc_subcommand())
        .subcommand(eval_subcommand())
        .subcommand(fmt_subcommand())
        .subcommand(init_subcommand())
        .subcommand(info_subcommand())
        .subcommand(install_subcommand())
        .subcommand(json_reference_subcommand())
        .subcommand(jupyter_subcommand())
        .subcommand(uninstall_subcommand())
        .subcommand(lsp_subcommand())
        .subcommand(lint_subcommand())
        .subcommand(publish_subcommand())
        .subcommand(repl_subcommand())
        .subcommand(task_subcommand())
        .subcommand(test_subcommand())
        .subcommand(types_subcommand())
        .subcommand(upgrade_subcommand())
        .subcommand(vendor_subcommand());

      let help = help_subcommand(&cmd);
      cmd.subcommand(help)
    })
    .help_template(DENO_HELP)
    .after_help(ENV_VARIABLES_HELP)
    .next_line_help(false)
}

#[inline(always)]
fn command(
  name: &'static str,
  about: impl clap::builder::IntoResettable<clap::builder::StyledStr>,
  unstable_args_config: UnstableArgsConfig,
) -> Command {
  Command::new(name)
    .about(about)
    .args(unstable_args(unstable_args_config))
}

fn help_subcommand(app: &Command) -> Command {
  command("help", None, UnstableArgsConfig::None)
    .disable_version_flag(true)
    .disable_help_subcommand(true)
    .subcommands(app.get_subcommands().map(|command| {
      Command::new(command.get_name().to_owned())
        .disable_help_flag(true)
        .disable_version_flag(true)
    }))
}

fn add_dev_arg() -> Arg {
  Arg::new("dev")
    .long("dev")
    .short('D')
    .help("Add as a dev dependency")
    .long_help("Add the package as a dev dependency. Note: This only applies when adding to a `package.json` file.")
    .action(ArgAction::SetTrue)
}

fn add_subcommand() -> Command {
  command(
    "add",
    cstr!(
      "Add dependencies to your configuration file.
  <p(245)>deno add @std/path</>

You can add multiple dependencies at once:
  <p(245)>deno add @std/path @std/assert</>"
    ),
    UnstableArgsConfig::None,
  )
  .defer(|cmd| {
    cmd
      .arg(
        Arg::new("packages")
          .help("List of packages to add")
          .required_unless_present("help")
          .num_args(1..)
          .action(ArgAction::Append),
      )
      .arg(add_dev_arg())
  })
}

fn remove_subcommand() -> Command {
  command(
    "remove",
    cstr!(
      "Remove dependencies from the configuration file.
  <p(245)>deno remove @std/path</>

You can remove multiple dependencies at once:
  <p(245)>deno remove @std/path @std/assert</>
"
    ),
    UnstableArgsConfig::None,
  )
  .defer(|cmd| {
    cmd.arg(
      Arg::new("packages")
        .help("List of packages to remove")
        .required_unless_present("help")
        .num_args(1..)
        .action(ArgAction::Append),
    )
  })
}

fn bench_subcommand() -> Command {
  command(
    "bench",
    cstr!("Run benchmarks using Deno's built-in bench tool.

Evaluate the given files, run all benches declared with 'Deno.bench()' and report results to standard output:
  <p(245)>deno bench src/fetch_bench.ts src/signal_bench.ts</>

If you specify a directory instead of a file, the path is expanded to all contained files matching the glob <c>{*_,*.,}bench.{js,mjs,ts,mts,jsx,tsx}</>:
  <p(245)>deno bench src/</>

<y>Read more:</> <c>https://docs.deno.com/go/bench</>"),
    UnstableArgsConfig::ResolutionAndRuntime,
  )
  .defer(|cmd| {
    runtime_args(cmd, true, false)
      .arg(check_arg(true))
      .arg(
        Arg::new("json")
          .long("json")
          .action(ArgAction::SetTrue)
          .help("UNSTABLE: Output benchmark result in JSON format"),
      )
      .arg(
        Arg::new("ignore")
          .long("ignore")
          .num_args(1..)
          .action(ArgAction::Append)
          .require_equals(true)
          .help("Ignore files"),
      )
      .arg(
        Arg::new("filter")
          .long("filter")
          .allow_hyphen_values(true)
          .help(
          "Run benchmarks with this string or regexp pattern in the bench name",
        ),
      )
      .arg(
        Arg::new("files")
          .help("List of file names to run")
          .num_args(..)
          .action(ArgAction::Append),
      )
      .arg(
        Arg::new("no-run")
          .long("no-run")
          .help("Cache bench modules, but don't run benchmarks")
          .action(ArgAction::SetTrue),
      )
      .arg(watch_arg(false))
      .arg(watch_exclude_arg())
      .arg(no_clear_screen_arg())
      .arg(script_arg().last(true))
      .arg(env_file_arg())
      .arg(executable_ext_arg())
  })
}

fn bundle_subcommand() -> Command {
  command("bundle",  "⚠️ `deno bundle` was removed in Deno 2.

See the Deno 1.x to 2.x Migration Guide for migration instructions: https://docs.deno.com/runtime/manual/advanced/migrate_deprecations", UnstableArgsConfig::ResolutionOnly)
    .hide(true)
}

fn cache_subcommand() -> Command {
  command(
    "cache",
    cstr!("Cache and compile remote dependencies.

Download and compile a module with all of its static dependencies and save them in the local cache, without running any code:
  <p(245)>deno cache jsr:@std/http/file-server</>

Future runs of this module will trigger no downloads or compilation unless --reload is specified

<y>Read more:</> <c>https://docs.deno.com/go/cache</>"),
    UnstableArgsConfig::ResolutionOnly,
)
  .hide(true)
  .defer(|cmd| {
    compile_args(cmd)
      .arg(check_arg(false))
      .arg(
        Arg::new("file")
          .num_args(1..)
          .required_unless_present("help")
          .value_hint(ValueHint::FilePath),
      )
      .arg(frozen_lockfile_arg())
      .arg(allow_scripts_arg())
      .arg(allow_import_arg())
  })
}

fn clean_subcommand() -> Command {
  command(
    "clean",
    cstr!("Remove the cache directory (<c>$DENO_DIR</>)"),
    UnstableArgsConfig::None,
  )
}

fn check_subcommand() -> Command {
  command("check",
      cstr!("Download and type-check without execution.

  <p(245)>deno check jsr:@std/http/file-server</>

Unless --reload is specified, this command will not re-download already cached dependencies

<y>Read more:</> <c>https://docs.deno.com/go/check</>"),
          UnstableArgsConfig::ResolutionAndRuntime
    )
    .defer(|cmd| {
      compile_args_without_check_args(cmd)
        .arg(
          Arg::new("all")
            .long("all")
            .help("Type-check all code, including remote modules and npm packages")
            .action(ArgAction::SetTrue)
            .conflicts_with("no-remote")
        )
        .arg(
          // past alias for --all
          Arg::new("remote")
            .long("remote")
            .help("Type-check all modules, including remote ones")
            .action(ArgAction::SetTrue)
            .conflicts_with("no-remote")
            .hide(true)
        )
        .arg(
          Arg::new("doc")
            .long("doc")
            .help("Type-check code blocks in JSDoc as well as actual code")
            .action(ArgAction::SetTrue)
        )
        .arg(
          Arg::new("doc-only")
          .long("doc-only")
          .help("Type-check code blocks in JSDoc and Markdown only")
            .action(ArgAction::SetTrue)
            .conflicts_with("doc")
        )
        .arg(
          Arg::new("file")
            .num_args(1..)
            .required_unless_present("help")
            .value_hint(ValueHint::FilePath),
        )
        .arg(allow_import_arg())
      }
    )
}

fn compile_subcommand() -> Command {
  command(
    "compile",
    cstr!("Compiles the given script into a self contained executable.

  <p(245)>deno compile --allow-read --allow-net jsr:@std/http/file-server</>
  <p(245)>deno compile --output file_server jsr:@std/http/file-server</>

Any flags specified which affect runtime behavior will be applied to the resulting binary.

This allows distribution of a Deno application to systems that do not have Deno installed.
Under the hood, it bundles a slimmed down version of the Deno runtime along with your
JavaScript or TypeScript code.

Cross-compiling to different target architectures is supported using the <c>--target</> flag.
On the first invocation with deno will download the proper binary and cache it in <c>$DENO_DIR</>.

<y>Read more:</> <c>https://docs.deno.com/go/compile</>
"),
    UnstableArgsConfig::ResolutionAndRuntime,
  )
  .defer(|cmd| {
    runtime_args(cmd, true, false)
      .arg(check_arg(true))
      .arg(
        Arg::new("include")
          .long("include")
          .help(
            cstr!("Includes an additional module in the compiled executable's module graph.
  <p(245)>Use this flag if a dynamically imported module or a web worker main module
  fails to load in the executable. This flag can be passed multiple times,
  to include multiple additional modules.</>",
          ))
          .action(ArgAction::Append)
          .value_hint(ValueHint::FilePath)
          .help_heading(COMPILE_HEADING),
      )
      .arg(
        Arg::new("output")
          .long("output")
          .short('o')
          .value_parser(value_parser!(String))
          .help(cstr!("Output file <p(245)>(defaults to $PWD/<<inferred-name>>)</>"))
          .value_hint(ValueHint::FilePath)
          .help_heading(COMPILE_HEADING),
      )
      .arg(
        Arg::new("target")
          .long("target")
          .help("Target OS architecture")
          .value_parser([
            "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-gnu",
            "x86_64-pc-windows-msvc",
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
          ])
          .help_heading(COMPILE_HEADING),
      )
      .arg(
        Arg::new("no-terminal")
          .long("no-terminal")
          .help("Hide terminal on Windows")
          .action(ArgAction::SetTrue)
          .help_heading(COMPILE_HEADING),
      )
      .arg(
        Arg::new("icon")
          .long("icon")
          .help("Set the icon of the executable on Windows (.ico)")
          .value_parser(value_parser!(String))
          .help_heading(COMPILE_HEADING),
      )
      .arg(executable_ext_arg())
      .arg(env_file_arg())
      .arg(
        script_arg()
          .required_unless_present("help")
          .trailing_var_arg(true),
      )
  })
}

fn completions_subcommand() -> Command {
  command(
    "completions",
    cstr!(
      "Output shell completion script to standard output.

  <p(245)>deno completions bash > /usr/local/etc/bash_completion.d/deno.bash</>
  <p(245)>source /usr/local/etc/bash_completion.d/deno.bash</>"
    ),
    UnstableArgsConfig::None,
  )
  .defer(|cmd| {
    cmd.disable_help_subcommand(true).arg(
      Arg::new("shell")
        .value_parser(["bash", "fish", "powershell", "zsh", "fig"])
        .required_unless_present("help"),
    )
  })
}

fn coverage_subcommand() -> Command {
  command(
    "coverage",
    cstr!("Print coverage reports from coverage profiles.

Collect a coverage profile with deno test:
  <p(245)>deno test --coverage=cov_profile</>

Print a report to stdout:
  <p(245)>deno coverage cov_profile</>

Include urls that start with the file schema and exclude files ending with <c>test.ts</> and <c>test.js</>,
for an url to match it must match the include pattern and not match the exclude pattern:
  <p(245)>deno coverage --include=\"^file:\" --exclude=\"test\\.(ts|js)\" cov_profile</>

Write a report using the lcov format:
  <p(245)>deno coverage --lcov --output=cov.lcov cov_profile/</>

Generate html reports from lcov:
  <p(245)>genhtml -o html_cov cov.lcov</>

<y>Read more:</> <c>https://docs.deno.com/go/coverage</>"),
    UnstableArgsConfig::None,
  )
  .defer(|cmd| {
    cmd
      .arg(
        Arg::new("ignore")
          .long("ignore")
          .num_args(1..)
          .action(ArgAction::Append)
          .require_equals(true)
          .help("Ignore coverage files")
          .value_hint(ValueHint::AnyPath),
      )
      .arg(
        Arg::new("include")
          .long("include")
          .num_args(1..)
          .action(ArgAction::Append)
          .value_name("regex")
          .require_equals(true)
          .default_value(r"^file:")
          .help("Include source files in the report"),
      )
      .arg(
        Arg::new("exclude")
          .long("exclude")
          .num_args(1..)
          .action(ArgAction::Append)
          .value_name("regex")
          .require_equals(true)
          .default_value(r"test\.(js|mjs|ts|jsx|tsx)$")
          .help("Exclude source files from the report"),
      )
      .arg(
        Arg::new("lcov")
          .long("lcov")
          .help("Output coverage report in lcov format")
          .action(ArgAction::SetTrue),
      )
      .arg(
        Arg::new("output")
          .requires("lcov")
          .long("output")
          .value_parser(value_parser!(String))
          .help(
            cstr!("Exports the coverage report in lcov format to the given file.
  <p(245)>If no --output arg is specified then the report is written to stdout.</>",
          ))
          .require_equals(true)
          .value_hint(ValueHint::FilePath),
      )
      .arg(
        Arg::new("html")
          .long("html")
          .help("Output coverage report in HTML format in the given directory")
          .action(ArgAction::SetTrue),
      )
      .arg(
        Arg::new("detailed")
          .long("detailed")
          .help("Output coverage report in detailed format in the terminal")
          .action(ArgAction::SetTrue),
      )
      .arg(
        Arg::new("files")
          .num_args(0..)
          .action(ArgAction::Append)
          .value_hint(ValueHint::AnyPath),
      )
  })
}

fn doc_subcommand() -> Command {
  command("doc",
      cstr!("Show documentation for a module.

Output documentation to standard output:
    <p(245)>deno doc ./path/to/module.ts</>

Output documentation in HTML format:
    <p(245)>deno doc --html --name=\"My library\" ./path/to/module.ts</>

Lint a module for documentation diagnostics:
    <p(245)>deno doc --lint ./path/to/module.ts</>

Target a specific symbol:
    <p(245)>deno doc ./path/to/module.ts MyClass.someField</>

Show documentation for runtime built-ins:
    <p(245)>deno doc</>
    <p(245)>deno doc --filter Deno.Listener</>

<y>Read more:</> <c>https://docs.deno.com/go/doc</>"),
          UnstableArgsConfig::ResolutionOnly
    )
    .defer(|cmd| {
      cmd
        .arg(import_map_arg())
        .arg(reload_arg())
        .arg(lock_arg())
        .arg(no_lock_arg())
        .arg(no_npm_arg())
        .arg(no_remote_arg())
        .arg(allow_import_arg())
        .arg(
          Arg::new("json")
            .long("json")
            .help("Output documentation in JSON format")
            .action(ArgAction::SetTrue)
            .help_heading(DOC_HEADING),
        )
        .arg(
          Arg::new("html")
            .long("html")
            .help("Output documentation in HTML format")
            .action(ArgAction::SetTrue)
            .display_order(1000)
            .conflicts_with("json").help_heading(DOC_HEADING)
        )
        .arg(
          Arg::new("name")
            .long("name")
            .help("The name that will be used in the docs (ie for breadcrumbs)")
            .action(ArgAction::Set)
            .require_equals(true).help_heading(DOC_HEADING)
        )
        .arg(
          Arg::new("category-docs")
            .long("category-docs")
            .help("Path to a JSON file keyed by category and an optional value of a markdown doc")
            .requires("html")
            .action(ArgAction::Set)
            .require_equals(true).help_heading(DOC_HEADING)
        )
        .arg(
          Arg::new("symbol-redirect-map")
            .long("symbol-redirect-map")
            .help("Path to a JSON file keyed by file, with an inner map of symbol to an external link")
            .requires("html")
            .action(ArgAction::Set)
            .require_equals(true).help_heading(DOC_HEADING)
        )
        .arg(
          Arg::new("strip-trailing-html")
            .long("strip-trailing-html")
            .help("Remove trailing .html from various links. Will still generate files with a .html extension")
            .requires("html")
            .action(ArgAction::SetTrue).help_heading(DOC_HEADING)
        )
        .arg(
          Arg::new("default-symbol-map")
            .long("default-symbol-map")
            .help("Uses the provided mapping of default name to wanted name for usage blocks")
            .requires("html")
            .action(ArgAction::Set)
            .require_equals(true).help_heading(DOC_HEADING)
        )
        .arg(
          Arg::new("output")
            .long("output")
            .help("Directory for HTML documentation output")
            .action(ArgAction::Set)
            .require_equals(true)
            .value_hint(ValueHint::DirPath)
            .value_parser(value_parser!(String)).help_heading(DOC_HEADING)
        )
        .arg(
          Arg::new("private")
            .long("private")
            .help("Output private documentation")
            .action(ArgAction::SetTrue).help_heading(DOC_HEADING),
        )
        .arg(
          Arg::new("filter")
            .long("filter")
            .help("Dot separated path to symbol")
            .conflicts_with("json")
            .conflicts_with("lint")
            .conflicts_with("html").help_heading(DOC_HEADING),
        )
        .arg(
          Arg::new("lint")
            .long("lint")
            .help("Output documentation diagnostics.")
            .action(ArgAction::SetTrue).help_heading(DOC_HEADING),
        )
        // TODO(nayeemrmn): Make `--builtin` a proper option. Blocked by
        // https://github.com/clap-rs/clap/issues/1794. Currently `--builtin` is
        // just a possible value of `source_file` so leading hyphens must be
        // enabled.
        .allow_hyphen_values(true)
        .arg(
          Arg::new("source_file")
            .num_args(1..)
            .action(ArgAction::Append)
            .value_hint(ValueHint::FilePath)
            .required_if_eq_any([("html", "true"), ("lint", "true")]),
        )
    })
}

fn eval_subcommand() -> Command {
  command(
    "eval",
    cstr!(
      "Evaluate JavaScript from the command line.
  <p(245)>deno eval \"console.log('hello world')\"</>

To evaluate as TypeScript:
  <p(245)>deno eval --ext=ts \"const v: string = 'hello'; console.log(v)\"</>

This command has implicit access to all permissions.

<y>Read more:</> <c>https://docs.deno.com/go/eval</>"
    ),
    UnstableArgsConfig::ResolutionAndRuntime,
  )
  .defer(|cmd| {
    runtime_args(cmd, false, true)
      .arg(check_arg(false))
      .arg(executable_ext_arg())
      .arg(
        Arg::new("print")
          .long("print")
          .short('p')
          .help("print result to stdout")
          .action(ArgAction::SetTrue),
      )
      .arg(
        Arg::new("code_arg")
          .num_args(1..)
          .action(ArgAction::Append)
          .help("Code to evaluate")
          .value_name("CODE_ARG")
          .required_unless_present("help"),
      )
      .arg(env_file_arg())
  })
}

fn fmt_subcommand() -> Command {
  command(
    "fmt",
    cstr!("Auto-format various file types.
  <p(245)>deno fmt myfile1.ts myfile2.ts</>

Supported file types are:
  <p(245)>JavaScript, TypeScript, Markdown, JSON(C) and Jupyter Notebooks</>

Supported file types which are behind corresponding unstable flags (see formatting options):
  <p(245)>HTML, CSS, SCSS, SASS, LESS, YAML, Svelte, Vue, Astro and Angular</>

Format stdin and write to stdout:
  <p(245)>cat file.ts | deno fmt -</>

Check if the files are formatted:
  <p(245)>deno fmt --check</>

Ignore formatting code by preceding it with an ignore comment:
  <p(245)>// deno-fmt-ignore</>

Ignore formatting a file by adding an ignore comment at the top of the file:
  <p(245)>// deno-fmt-ignore-file</>

<y>Read more:</> <c>https://docs.deno.com/go/fmt</>"),
    UnstableArgsConfig::None,
  )
  .defer(|cmd| {
    cmd
      .arg(config_arg())
      .arg(no_config_arg())
      .arg(
         Arg::new("check")
          .long("check")
          .help("Check if the source files are formatted")
          .num_args(0)
          .help_heading(FMT_HEADING),
      )
      .arg(
        Arg::new("ext")
          .long("ext")
          .help("Set content type of the supplied file")
          .value_parser([
            "ts", "tsx", "js", "jsx", "md", "json", "jsonc", "css", "scss",
            "sass", "less", "html", "svelte", "vue", "astro", "yml", "yaml",
            "ipynb",
          ])
          .help_heading(FMT_HEADING),
      )
      .arg(
        Arg::new("ignore")
          .long("ignore")
          .num_args(1..)
          .action(ArgAction::Append)
          .require_equals(true)
          .help("Ignore formatting particular source files")
          .value_hint(ValueHint::AnyPath)
          .help_heading(FMT_HEADING),
      )
      .arg(
        Arg::new("files")
          .num_args(1..)
          .action(ArgAction::Append)
          .value_hint(ValueHint::AnyPath),
      )
      .arg(watch_arg(false))
      .arg(watch_exclude_arg())
      .arg(no_clear_screen_arg())
      .arg(
        Arg::new("use-tabs")
          .long("use-tabs")
          .alias("options-use-tabs")
          .num_args(0..=1)
          .value_parser(value_parser!(bool))
          .default_missing_value("true")
          .require_equals(true)
          .help(
          cstr!(  "Use tabs instead of spaces for indentation <p(245)>[default: false]</>"),
          )
          .help_heading(FMT_HEADING),
      )
      .arg(
        Arg::new("line-width")
          .long("line-width")
          .alias("options-line-width")
          .help(cstr!("Define maximum line width <p(245)>[default: 80]</>"))
          .value_parser(value_parser!(NonZeroU32))
          .help_heading(FMT_HEADING),
      )
      .arg(
        Arg::new("indent-width")
          .long("indent-width")
          .alias("options-indent-width")
          .help(cstr!("Define indentation width <p(245)>[default: 2]</>"))
          .value_parser(value_parser!(NonZeroU8))
          .help_heading(FMT_HEADING),
      )
      .arg(
        Arg::new("single-quote")
          .long("single-quote")
          .alias("options-single-quote")
          .num_args(0..=1)
          .value_parser(value_parser!(bool))
          .default_missing_value("true")
          .require_equals(true)
          .help(cstr!("Use single quotes <p(245)>[default: false]</>"))
          .help_heading(FMT_HEADING),
      )
      .arg(
        Arg::new("prose-wrap")
          .long("prose-wrap")
          .alias("options-prose-wrap")
          .value_parser(["always", "never", "preserve"])
          .help(cstr!("Define how prose should be wrapped <p(245)>[default: always]</>"))
          .help_heading(FMT_HEADING),
      )
      .arg(
        Arg::new("no-semicolons")
          .long("no-semicolons")
          .alias("options-no-semicolons")
          .num_args(0..=1)
          .value_parser(value_parser!(bool))
          .default_missing_value("true")
          .require_equals(true)
          .help(
           cstr!("Don't use semicolons except where necessary <p(245)>[default: false]</>"),
          )
          .help_heading(FMT_HEADING),
      )
      .arg(
        Arg::new("unstable-css")
          .long("unstable-css")
          .help("Enable formatting CSS, SCSS, Sass and Less files")
          .value_parser(FalseyValueParser::new())
          .action(ArgAction::SetTrue)
          .help_heading(FMT_HEADING)
          .hide(true),
      )
      .arg(
        Arg::new("unstable-html")
          .long("unstable-html")
          .help("Enable formatting HTML files")
          .value_parser(FalseyValueParser::new())
          .action(ArgAction::SetTrue)
          .help_heading(FMT_HEADING)
          .hide(true),
      )
      .arg(
        Arg::new("unstable-component")
          .long("unstable-component")
          .help("Enable formatting Svelte, Vue, Astro and Angular files")
          .value_parser(FalseyValueParser::new())
          .action(ArgAction::SetTrue)
          .help_heading(FMT_HEADING),
      )
      .arg(
        Arg::new("unstable-yaml")
          .long("unstable-yaml")
          .help("Enable formatting YAML files")
          .value_parser(FalseyValueParser::new())
          .action(ArgAction::SetTrue)
          .help_heading(FMT_HEADING)
          .hide(true),
      )
  })
}

fn init_subcommand() -> Command {
  command("init", "scaffolds a basic Deno project with a script, test, and configuration file", UnstableArgsConfig::None).defer(
    |cmd| {
      cmd
        .arg(Arg::new("dir").value_hint(ValueHint::DirPath))
        .arg(
          Arg::new("lib")
            .long("lib")
            .help("Generate an example library project")
            .action(ArgAction::SetTrue),
        )
        .arg(
          Arg::new("serve")
            .long("serve")
            .help("Generate an example project for `deno serve`")
            .conflicts_with("lib")
            .action(ArgAction::SetTrue),
        )
    },
  )
}

fn info_subcommand() -> Command {
  command("info",
      cstr!("Show information about a module or the cache directories.

Get information about a module:
  <p(245)>deno info jsr:@std/http/file-server</>

The following information is shown:
  local: Local path of the file
  type: JavaScript, TypeScript, or JSON
  emit: Local path of compiled source code (TypeScript only)
  dependencies: Dependency tree of the source file

<y>Read more:</> <c>https://docs.deno.com/go/info</>"),
          UnstableArgsConfig::ResolutionOnly
    )
    .defer(|cmd| cmd
      .arg(Arg::new("file").value_hint(ValueHint::FilePath))
      .arg(reload_arg().requires("file"))
      .arg(ca_file_arg())
      .arg(unsafely_ignore_certificate_errors_arg())
      .arg(
        location_arg()
          .conflicts_with("file")
          .help(cstr!("Show files used for origin bound APIs like the Web Storage API when running a script with <c>--location=<<HREF>></>"))
      )
      .arg(no_check_arg().hide(true)) // TODO(lucacasonato): remove for 2.0
      .arg(no_config_arg())
      .arg(no_remote_arg())
      .arg(no_npm_arg())
      .arg(lock_arg())
      .arg(no_lock_arg())
      .arg(config_arg())
      .arg(import_map_arg())
      .arg(node_modules_dir_arg())
      .arg(vendor_arg())
      .arg(
        Arg::new("json")
          .long("json")
          .help("UNSTABLE: Outputs the information in JSON format")
          .action(ArgAction::SetTrue),
      ))
      .arg(allow_import_arg())
}

fn install_subcommand() -> Command {
  command("install", cstr!("Installs dependencies either in the local project or globally to a bin directory.

<g>Local installation</>

Add dependencies to the local project's configuration (<p(245)>deno.json / package.json</>) and installs them
in the package cache. If no dependency is specified, installs all dependencies listed in the config file.
If the <p(245)>--entrypoint</> flag is passed, installs the dependencies of the specified entrypoint(s).

  <p(245)>deno install</>
  <p(245)>deno install @std/bytes</>
  <p(245)>deno install npm:chalk</>
  <p(245)>deno install --entrypoint entry1.ts entry2.ts</>

<g>Global installation</>

If the <bold>--global</> flag is set, installs a script as an executable in the installation root's bin directory.

  <p(245)>deno install --global --allow-net --allow-read jsr:@std/http/file-server</>
  <p(245)>deno install -g https://examples.deno.land/color-logging.ts</>

To change the executable name, use <c>-n</>/<c>--name</>:
  <p(245)>deno install -g --allow-net --allow-read -n serve jsr:@std/http/file-server</>

The executable name is inferred by default:
  - Attempt to take the file stem of the URL path. The above example would
    become <p(245)>file_server</>.
  - If the file stem is something generic like <p(245)>main</>, <p(245)>mod</>, <p(245)>index</> or <p(245)>cli</>,
    and the path has no parent, take the file name of the parent path. Otherwise
    settle with the generic name.
  - If the resulting name has an <p(245)>@...</> suffix, strip it.

To change the installation root, use <c>--root</>:
  <p(245)>deno install -g --allow-net --allow-read --root /usr/local jsr:@std/http/file-server</>

The installation root is determined, in order of precedence:
  - <p(245)>--root</> option
  - <p(245)>DENO_INSTALL_ROOT</> environment variable
  - <p(245)>$HOME/.deno</>

These must be added to the path manually if required."), UnstableArgsConfig::ResolutionAndRuntime)
    .visible_alias("i")
    .defer(|cmd| {
      permission_args(runtime_args(cmd, false, true), Some("global"))
        .arg(check_arg(true))
        .arg(allow_scripts_arg())
        .arg(
          Arg::new("cmd")
            .required_if_eq("global", "true")
            .required_if_eq("entrypoint", "true")
            .num_args(1..)
            .value_hint(ValueHint::FilePath),
        )
        .arg(
          Arg::new("name")
            .long("name")
            .short('n')
            .requires("global")
            .help("Executable file name"),
        )
        .arg(
          Arg::new("root")
            .long("root")
            .requires("global")
            .help("Installation root")
            .value_hint(ValueHint::DirPath),
        )
        .arg(
          Arg::new("force")
            .long("force")
            .requires("global")
            .short('f')
            .help("Forcefully overwrite existing installation")
            .action(ArgAction::SetTrue),
        )
        .arg(
          Arg::new("global")
            .long("global")
            .short('g')
            .help("Install a package or script as a globally available executable")
            .action(ArgAction::SetTrue),
        )
        .arg(
          Arg::new("entrypoint")
            .long("entrypoint")
            .short('e')
            .conflicts_with("global")
            .action(ArgAction::SetTrue)
            .help("Install dependents of the specified entrypoint(s)"),
        )
        .arg(env_file_arg())
        .arg(add_dev_arg().conflicts_with("entrypoint").conflicts_with("global"))
    })
}

fn json_reference_subcommand() -> Command {
  Command::new("json_reference").hide(true)
}

fn jupyter_subcommand() -> Command {
  command("jupyter", "Deno kernel for Jupyter notebooks", UnstableArgsConfig::ResolutionAndRuntime)
    .arg(
      Arg::new("install")
        .long("install")
        .help("Installs kernelspec, requires 'jupyter' command to be available.")
        .conflicts_with("kernel")
        .action(ArgAction::SetTrue)
    )
    .arg(
      Arg::new("kernel")
        .long("kernel")
        .help("Start the kernel")
        .conflicts_with("install")
        .requires("conn")
        .action(ArgAction::SetTrue)
    )
    .arg(
      Arg::new("conn")
        .long("conn")
        .help("Path to JSON file describing connection parameters, provided by Jupyter")
        .value_parser(value_parser!(String))
        .value_hint(ValueHint::FilePath)
        .conflicts_with("install"))
}

fn uninstall_subcommand() -> Command {
  command(
    "uninstall",
    cstr!("Uninstalls a dependency or an executable script in the installation root's bin directory.
  <p(245)>deno uninstall @std/dotenv chalk</>
  <p(245)>deno uninstall --global file_server</>

To change the installation root, use <c>--root</> flag:
  <p(245)>deno uninstall --global --root /usr/local serve</>

The installation root is determined, in order of precedence:
  - <p(245)>--root</> option
  - <p(245)>DENO_INSTALL_ROOT</> environment variable
  - <p(245)>$HOME/.deno</>"),
    UnstableArgsConfig::None,
  )
  .defer(|cmd| {
    cmd
      .arg(Arg::new("name-or-package").required_unless_present("help"))
      .arg(
        Arg::new("root")
          .long("root")
          .help("Installation root")
          .requires("global")
          .value_hint(ValueHint::DirPath),
      )
      .arg(
        Arg::new("global")
          .long("global")
          .short('g')
          .help("Remove globally installed package or module")
          .action(ArgAction::SetTrue),
      )
      .arg(
        Arg::new("additional-packages")
          .help("List of additional packages to remove")
          .conflicts_with("global")
          .num_args(1..)
          .action(ArgAction::Append)
      )
  })
}

fn lsp_subcommand() -> Command {
  Command::new("lsp").about(
    "The 'deno lsp' subcommand provides a way for code editors and IDEs to interact with Deno
using the Language Server Protocol. Usually humans do not use this subcommand directly.
For example, 'deno lsp' can provide IDEs with go-to-definition support and automatic code formatting.

How to connect various editors and IDEs to 'deno lsp': https://docs.deno.com/go/lsp",
  )
}

fn lint_subcommand() -> Command {
  command(
    "lint",
    cstr!("Lint JavaScript/TypeScript source code.

  <p(245)>deno lint</>
  <p(245)>deno lint myfile1.ts myfile2.js</>

Print result as JSON:
  <p(245)>deno lint --json</>

Read from stdin:
  <p(245)>cat file.ts | deno lint -</>
  <p(245)>cat file.ts | deno lint --json -</>

List available rules:
  <p(245)>deno lint --rules</>

To ignore specific diagnostics, you can write an ignore comment on the preceding line with a rule name (or multiple):
  <p(245)>// deno-lint-ignore no-explicit-any</>
  <p(245)>// deno-lint-ignore require-await no-empty</>

To ignore linting on an entire file, you can add an ignore comment at the top of the file:
  <p(245)>// deno-lint-ignore-file</>

<y>Read more:</> <c>https://docs.deno.com/go/lint</>
"),
    UnstableArgsConfig::ResolutionOnly,
  )
  .defer(|cmd| {
    cmd
      .arg(
        Arg::new("fix")
          .long("fix")
          .help("Fix any linting errors for rules that support it")
          .action(ArgAction::SetTrue)
          .help_heading(LINT_HEADING),
      )
      .arg(
            Arg::new("ext")
                .long("ext")
                .require_equals(true)
                .value_name("EXT")
                .help("Specify the file extension to lint when reading from stdin.\
  For example, use `jsx` to lint JSX files or `tsx` for TSX files.\
  This argument is necessary because stdin input does not automatically infer the file type.\
  Example usage: `cat file.jsx | deno lint - --ext=jsx`."),
        )
        .arg(
        Arg::new("rules")
          .long("rules")
          .help("List available rules")
          .action(ArgAction::SetTrue)
          .help_heading(LINT_HEADING),
      )
      .arg(
        Arg::new("rules-tags")
          .long("rules-tags")
          .require_equals(true)
          .num_args(1..)
          .action(ArgAction::Append)
          .use_value_delimiter(true)
          .help("Use set of rules with a tag")
          .help_heading(LINT_HEADING),
      )
      .arg(
        Arg::new("rules-include")
          .long("rules-include")
          .require_equals(true)
          .num_args(1..)
          .use_value_delimiter(true)
          .conflicts_with("rules")
          .help("Include lint rules")
          .help_heading(LINT_HEADING),
      )
      .arg(
        Arg::new("rules-exclude")
          .long("rules-exclude")
          .require_equals(true)
          .num_args(1..)
          .use_value_delimiter(true)
          .conflicts_with("rules")
          .help("Exclude lint rules")
          .help_heading(LINT_HEADING),
      )
      .arg(no_config_arg())
      .arg(config_arg())
      .arg(
        Arg::new("ignore")
          .long("ignore")
          .num_args(1..)
          .action(ArgAction::Append)
          .require_equals(true)
          .help("Ignore linting particular source files")
          .value_hint(ValueHint::AnyPath)
          .help_heading(LINT_HEADING),
      )
      .arg(
        Arg::new("json")
          .long("json")
          .help("Output lint result in JSON format")
          .action(ArgAction::SetTrue)
          .help_heading(LINT_HEADING),
      )
      .arg(
        Arg::new("compact")
          .long("compact")
          .help("Output lint result in compact format")
          .action(ArgAction::SetTrue)
          .conflicts_with("json")
          .help_heading(LINT_HEADING),
      )
      .arg(
        Arg::new("files")
          .num_args(1..)
          .action(ArgAction::Append)
          .value_hint(ValueHint::AnyPath),
      )
      .arg(watch_arg(false))
      .arg(watch_exclude_arg())
      .arg(no_clear_screen_arg())
  })
}

fn repl_subcommand() -> Command {
  command("repl", cstr!(
    "Starts a read-eval-print-loop, which lets you interactively build up program state in the global context.
It is especially useful for quick prototyping and checking snippets of code.

TypeScript is supported, however it is not type-checked, only transpiled."
  ), UnstableArgsConfig::ResolutionAndRuntime)
    .defer(|cmd| runtime_args(cmd, true, true)
      .arg(check_arg(false))
      .arg(
        Arg::new("eval-file")
          .long("eval-file")
          .num_args(1..)
          .action(ArgAction::Append)
          .require_equals(true)
          .help("Evaluates the provided file(s) as scripts when the REPL starts. Accepts file paths and URLs")
          .value_hint(ValueHint::AnyPath),
      )
      .arg(
        Arg::new("eval")
          .long("eval")
          .help("Evaluates the provided code when the REPL starts")
          .value_name("code"),
      )
      .after_help(cstr!("<y>Environment variables:</>
  <g>DENO_REPL_HISTORY</>  Set REPL history file path. History file is disabled when the value is empty.
                       <p(245)>[default: $DENO_DIR/deno_history.txt]</>"))
    )
    .arg(env_file_arg())
    .arg(
      Arg::new("args")
        .num_args(0..)
        .action(ArgAction::Append)
        .value_name("ARGS")
        .last(true)
    )
}

fn run_args(command: Command, top_level: bool) -> Command {
  runtime_args(command, true, true)
    .arg(check_arg(false))
    .arg(watch_arg(true))
    .arg(hmr_arg(true))
    .arg(watch_exclude_arg())
    .arg(no_clear_screen_arg())
    .arg(executable_ext_arg())
    .arg(if top_level {
      script_arg().trailing_var_arg(true).hide(true)
    } else {
      script_arg().trailing_var_arg(true)
    })
    .arg(env_file_arg())
    .arg(no_code_cache_arg())
}

fn run_subcommand() -> Command {
  run_args(command("run", cstr!("Run a JavaScript or TypeScript program, or a task or script.

By default all programs are run in sandbox without access to disk, network or ability to spawn subprocesses.
  <p(245)>deno run https://examples.deno.land/hello-world.ts</>

Grant permission to read from disk and listen to network:
  <p(245)>deno run --allow-read --allow-net jsr:@std/http/file-server</>

Grant permission to read allow-listed files from disk:
  <p(245)>deno run --allow-read=/etc jsr:@std/http/file-server</>

Grant all permissions:
  <p(245)>deno run -A jsr:@std/http/file-server</>

Specifying the filename '-' to read the file from stdin.
  <p(245)>curl https://examples.deno.land/hello-world.ts | deno run -</>

<y>Read more:</> <c>https://docs.deno.com/go/run</>"), UnstableArgsConfig::ResolutionAndRuntime), false)
}

fn serve_host_validator(host: &str) -> Result<String, String> {
  if Url::parse(&format!("internal://{host}:9999")).is_ok() {
    Ok(host.to_owned())
  } else {
    Err(format!("Bad serve host: {host}"))
  }
}

fn serve_subcommand() -> Command {
  runtime_args(command("serve", cstr!("Run a server defined in a main module

The serve command uses the default exports of the main module to determine which servers to start.

Start a server defined in server.ts:
  <p(245)>deno serve server.ts</>

Start a server defined in server.ts, watching for changes and running on port 5050:
  <p(245)>deno serve --watch --port 5050 server.ts</>

<y>Read more:</> <c>https://docs.deno.com/go/serve</>"), UnstableArgsConfig::ResolutionAndRuntime), true, true)
    .arg(
      Arg::new("port")
        .long("port")
        .help(cstr!("The TCP port to serve on. Pass 0 to pick a random free port <p(245)>[default: 8000]</>"))
        .value_parser(value_parser!(u16)),
    )
    .arg(
      Arg::new("host")
        .long("host")
        .help("The TCP address to serve on, defaulting to 0.0.0.0 (all interfaces)")
        .value_parser(serve_host_validator),
    )
    .arg(
      parallel_arg("multiple server workers")
    )
    .arg(check_arg(false))
    .arg(watch_arg(true))
    .arg(hmr_arg(true))
    .arg(watch_exclude_arg())
    .arg(no_clear_screen_arg())
    .arg(executable_ext_arg())
    .arg(
      script_arg()
        .required_unless_present_any(["help", "v8-flags"])
        .trailing_var_arg(true),
    )
    .arg(env_file_arg())
    .arg(no_code_cache_arg())
}

fn task_subcommand() -> Command {
  command(
    "task",
    cstr!(
      "Run a task defined in the configuration file.
  <p(245)>deno task build</>

List all available tasks:
  <p(245)>deno task</>"
    ),
    UnstableArgsConfig::ResolutionAndRuntime,
  )
  .defer(|cmd| {
    cmd
      .allow_external_subcommands(true)
      .subcommand_value_name("TASK")
      .arg(config_arg())
      .arg(
        Arg::new("cwd")
          .long("cwd")
          .value_name("DIR")
          .help("Specify the directory to run the task in")
          .value_hint(ValueHint::DirPath),
      )
      .arg(node_modules_dir_arg())
  })
}

fn test_subcommand() -> Command {
  command("test",
      cstr!("Run tests using Deno's built-in test runner.

Evaluate the given modules, run all tests declared with <bold>Deno.</><y>test()</> and report results to standard output:
  <p(245)>deno test src/fetch_test.ts src/signal_test.ts</>

Directory arguments are expanded to all contained files matching the glob <c>{*_,*.,}test.{js,mjs,ts,mts,jsx,tsx}</>
or <c>**/__tests__/**</>:
 <p(245)>deno test src/</>

<y>Read more:</> <c>https://docs.deno.com/go/test</>"),
          UnstableArgsConfig::ResolutionAndRuntime
    )
    .defer(|cmd|
      runtime_args(cmd, true, true)
      .arg(check_arg(true))
      .arg(
        Arg::new("ignore")
          .long("ignore")
          .num_args(1..)
          .action(ArgAction::Append)
          .require_equals(true)
          .help("Ignore files")
          .value_hint(ValueHint::AnyPath),
      )
      .arg(
        Arg::new("no-run")
          .long("no-run")
          .help("Cache test modules, but don't run tests")
          .action(ArgAction::SetTrue)
          .help_heading(TEST_HEADING),
      )
      .arg(
        Arg::new("trace-leaks")
          .long("trace-leaks")
          .help("Enable tracing of leaks. Useful when debugging leaking ops in test, but impacts test execution time")
          .action(ArgAction::SetTrue)
          .help_heading(TEST_HEADING),
      )
      .arg(
        Arg::new("doc")
          .long("doc")
          .help("Evaluate code blocks in JSDoc and Markdown")
          .action(ArgAction::SetTrue)
          .help_heading(TEST_HEADING),
      )
      .arg(
        Arg::new("fail-fast")
          .long("fail-fast")
          .alias("failfast")
          .help("Stop after N errors. Defaults to stopping after first failure")
          .num_args(0..=1)
          .require_equals(true)
          .value_name("N")
          .value_parser(value_parser!(NonZeroUsize))
          .help_heading(TEST_HEADING))
      .arg(
        Arg::new("permit-no-files")
          .long("permit-no-files")
          .help("Don't return an error code if no test files were found")
          .action(ArgAction::SetTrue)
          .help_heading(TEST_HEADING),
      )
      .arg(
        Arg::new("filter")
          .allow_hyphen_values(true)
          .long("filter")
          .help("Run tests with this string or regexp pattern in the test name")
          .help_heading(TEST_HEADING),
      )
      .arg(
        Arg::new("shuffle")
          .long("shuffle")
          .value_name("NUMBER")
          .help("Shuffle the order in which the tests are run")
          .num_args(0..=1)
          .require_equals(true)
          .value_parser(value_parser!(u64))
          .help_heading(TEST_HEADING),
      )
      .arg(
        Arg::new("coverage")
          .long("coverage")
          .value_name("DIR")
          .num_args(0..=1)
          .require_equals(true)
          .default_missing_value("coverage")
          .conflicts_with("inspect")
          .conflicts_with("inspect-wait")
          .conflicts_with("inspect-brk")
          .help("Collect coverage profile data into DIR. If DIR is not specified, it uses 'coverage/'")
          .help_heading(TEST_HEADING),
      )
      .arg(
        Arg::new("clean")
          .long("clean")
          .help(cstr!("Empty the temporary coverage profile data directory before running tests.
  <p(245)>Note: running multiple `deno test --clean` calls in series or parallel for the same coverage directory may cause race conditions.</>"))
          .action(ArgAction::SetTrue)
          .help_heading(TEST_HEADING),
      )
      .arg(
        parallel_arg("test modules")
      )
      .arg(
        Arg::new("files")
          .help("List of file names to run")
          .num_args(0..)
          .action(ArgAction::Append)
          .value_hint(ValueHint::AnyPath),
      )
      .arg(
        watch_arg(true)
          .conflicts_with("no-run")
          .conflicts_with("coverage"),
      )
      .arg(watch_exclude_arg())
      .arg(no_clear_screen_arg())
      .arg(script_arg().last(true))
      .arg(
        Arg::new("junit-path")
          .long("junit-path")
          .value_name("PATH")
          .value_hint(ValueHint::FilePath)
          .help("Write a JUnit XML test report to PATH. Use '-' to write to stdout which is the default when PATH is not provided")
          .help_heading(TEST_HEADING)
      )
      .arg(
        Arg::new("reporter")
          .long("reporter")
          .help("Select reporter to use. Default to 'pretty'")
          .value_parser(["pretty", "dot", "junit", "tap"])
          .help_heading(TEST_HEADING)
      )
      .arg(
        Arg::new("hide-stacktraces")
          .long("hide-stacktraces")
          .help("Hide stack traces for errors in failure test results.")
          .action(ArgAction::SetTrue)
      )
      .arg(env_file_arg())
      .arg(executable_ext_arg())
    )
}

fn parallel_arg(descr: &str) -> Arg {
  Arg::new("parallel")
    .long("parallel")
    .help(format!("Run {descr} in parallel. Parallelism defaults to the number of available CPUs or the value of the DENO_JOBS environment variable"))
    .action(ArgAction::SetTrue)
}

fn types_subcommand() -> Command {
  command(
    "types",
    cstr!(
      "Print runtime TypeScript declarations.

  <p(245)>deno types > lib.deno.d.ts</>

The declaration file could be saved and used for typing information."
    ),
    UnstableArgsConfig::None,
  )
}

pub static UPGRADE_USAGE: &str = cstr!(
  "<g>Latest</>
  <bold>deno upgrade</>

<g>Specific version</>
  <bold>deno upgrade</> <p(245)>1.45.0</>
  <bold>deno upgrade</> <p(245)>1.46.0-rc.1</>
  <bold>deno upgrade</> <p(245)>9bc2dd29ad6ba334fd57a20114e367d3c04763d4</>

<g>Channel</>
  <bold>deno upgrade</> <p(245)>stable</>
  <bold>deno upgrade</> <p(245)>rc</>
  <bold>deno upgrade</> <p(245)>canary</>"
);

fn upgrade_subcommand() -> Command {
  command(
    "upgrade",
    color_print::cformat!("Upgrade deno executable to the given version.

{}

The version is downloaded from <p(245)>https://dl.deno.land</> and is used to replace the current executable.

If you want to not replace the current Deno executable but instead download an update to a
different location, use the <c>--output</> flag:
  <p(245)>deno upgrade --output $HOME/my_deno</>

<y>Read more:</> <c>https://docs.deno.com/go/upgrade</>", UPGRADE_USAGE),
    UnstableArgsConfig::None,
  )
  .hide(cfg!(not(feature = "upgrade")))
  .defer(|cmd| {
    cmd
      .arg(
        Arg::new("version")
          .long("version")
          .help("The version to upgrade to")
          .help_heading(UPGRADE_HEADING)// NOTE(bartlomieju): pre-v1.46 compat
          .hide(true),
      )
      .arg(
        Arg::new("output")
          .long("output")
          .help("The path to output the updated version to")
          .value_parser(value_parser!(String))
          .value_hint(ValueHint::FilePath)
          .help_heading(UPGRADE_HEADING),
      )
      .arg(
        Arg::new("dry-run")
          .long("dry-run")
          .help("Perform all checks without replacing old exe")
          .action(ArgAction::SetTrue)
          .help_heading(UPGRADE_HEADING),
      )
      .arg(
        Arg::new("force")
          .long("force")
          .short('f')
          .help("Replace current exe even if not out-of-date")
          .action(ArgAction::SetTrue)
          .help_heading(UPGRADE_HEADING),
      )
      .arg(
        Arg::new("canary")
          .long("canary")
          .help("Upgrade to canary builds")
          .action(ArgAction::SetTrue)
          .help_heading(UPGRADE_HEADING)// NOTE(bartlomieju): pre-v1.46 compat
          .hide(true),
      )
      .arg(
        Arg::new("release-candidate")
          .long("rc")
          .help("Upgrade to a release candidate")
          .conflicts_with_all(["canary", "version"])
          .action(ArgAction::SetTrue)
          .help_heading(UPGRADE_HEADING)
          // NOTE(bartlomieju): pre-v1.46 compat
          .hide(true),
      )
      .arg(
        Arg::new("version-or-hash-or-channel")
          .help(cstr!("Version <p(245)>(v1.46.0)</>, channel <p(245)>(rc, canary)</> or commit hash <p(245)>(9bc2dd29ad6ba334fd57a20114e367d3c04763d4)</>"))
          .value_name("VERSION")
          .action(ArgAction::Append)
          .trailing_var_arg(true),
      )
      .arg(ca_file_arg())
      .arg(unsafely_ignore_certificate_errors_arg())
  })
}

fn vendor_subcommand() -> Command {
  command("vendor",
      "⚠️ `deno vendor` was removed in Deno 2.

See the Deno 1.x to 2.x Migration Guide for migration instructions: https://docs.deno.com/runtime/manual/advanced/migrate_deprecations",
      UnstableArgsConfig::ResolutionOnly
    )
    .hide(true)
}

fn publish_subcommand() -> Command {
  command("publish", "Publish the current working directory's package or workspace to JSR", UnstableArgsConfig::ResolutionOnly)
    .defer(|cmd| {
      cmd
      .arg(
        Arg::new("token")
          .long("token")
          .help("The API token to use when publishing. If unset, interactive authentication is be used")
          .help_heading(PUBLISH_HEADING)
      )
        .arg(config_arg())
        .arg(no_config_arg())
        .arg(
          Arg::new("dry-run")
            .long("dry-run")
            .help("Prepare the package for publishing performing all checks and validations without uploading")
            .action(ArgAction::SetTrue)
          .help_heading(PUBLISH_HEADING),
        )
        .arg(
          Arg::new("allow-slow-types")
            .long("allow-slow-types")
            .help("Allow publishing with slow types")
            .action(ArgAction::SetTrue)
          .help_heading(PUBLISH_HEADING),
        )
        .arg(
          Arg::new("allow-dirty")
            .long("allow-dirty")
            .help("Allow publishing if the repository has uncommitted changed")
            .action(ArgAction::SetTrue)
          .help_heading(PUBLISH_HEADING),
        ).arg(
        Arg::new("no-provenance")
          .long("no-provenance")
          .help(cstr!("Disable provenance attestation.
  <p(245)>Enabled by default on Github actions, publicly links the package to where it was built and published from.</>"))
          .action(ArgAction::SetTrue)
        .help_heading(PUBLISH_HEADING)
      )
        .arg(check_arg(/* type checks by default */ true))
        .arg(no_check_arg())
    })
}

fn compile_args(app: Command) -> Command {
  compile_args_without_check_args(app.arg(no_check_arg()))
}

fn compile_args_without_check_args(app: Command) -> Command {
  app
    .arg(import_map_arg())
    .arg(no_remote_arg())
    .arg(no_npm_arg())
    .arg(node_modules_dir_arg())
    .arg(vendor_arg())
    .arg(config_arg())
    .arg(no_config_arg())
    .arg(reload_arg())
    .arg(lock_arg())
    .arg(no_lock_arg())
    .arg(ca_file_arg())
    .arg(unsafely_ignore_certificate_errors_arg())
}

fn permission_args(app: Command, requires: Option<&'static str>) -> Command {
  app
    .after_help(cstr!(r#"<y>Permission options:</>
<y>Docs</>: <c>https://docs.deno.com/go/permissions</>

  <g>-A, --allow-all</>                          Allow all permissions.
  <g>--no-prompt</>                              Always throw if required permission wasn't passed.
                                             <p(245)>Can also be set via the DENO_NO_PROMPT environment variable.</>
  <g>-R, --allow-read[=<<PATH>...]</>             Allow file system read access. Optionally specify allowed paths.
                                             <p(245)>--allow-read  |  --allow-read="/etc,/var/log.txt"</>
  <g>-W, --allow-write[=<<PATH>...]</>            Allow file system write access. Optionally specify allowed paths.
                                             <p(245)>--allow-write  |  --allow-write="/etc,/var/log.txt"</>
  <g>-I, --allow-import[=<<IP_OR_HOSTNAME>...]</> Allow importing from remote hosts. Optionally specify allowed IP addresses and host names, with ports as necessary.
                                            Default value: <p(245)>deno.land:443,jsr.io:443,esm.sh:443,cdn.jsdelivr.net:443,raw.githubusercontent.com:443,user.githubusercontent.com:443</>
                                             <p(245)>--allow-import  |  --allow-import="example.com,github.com"</>
  <g>-N, --allow-net[=<<IP_OR_HOSTNAME>...]</>    Allow network access. Optionally specify allowed IP addresses and host names, with ports as necessary.
                                             <p(245)>--allow-net  |  --allow-net="localhost:8080,deno.land"</>
  <g>-E, --allow-env[=<<VARIABLE_NAME>...]</>     Allow access to environment variables. Optionally specify accessible environment variables.
                                             <p(245)>--allow-env  |  --allow-env="PORT,HOME,PATH"</>
  <g>-S, --allow-sys[=<<API_NAME>...]</>          Allow access to OS information. Optionally allow specific APIs by function name.
                                             <p(245)>--allow-sys  |  --allow-sys="systemMemoryInfo,osRelease"</>
      <g>--allow-run[=<<PROGRAM_NAME>...]</>      Allow running subprocesses. Optionally specify allowed runnable program names.
                                             <p(245)>--allow-run  |  --allow-run="whoami,ps"</>
      <g>--allow-ffi[=<<PATH>...]</>              (Unstable) Allow loading dynamic libraries. Optionally specify allowed directories or files.
                                             <p(245)>--allow-ffi  |  --allow-ffi="./libfoo.so"</>
  <g>    --deny-read[=<<PATH>...]</>              Deny file system read access. Optionally specify denied paths.
                                             <p(245)>--deny-read  |  --deny-read="/etc,/var/log.txt"</>
  <g>    --deny-write[=<<PATH>...]</>             Deny file system write access. Optionally specify denied paths.
                                             <p(245)>--deny-write  |  --deny-write="/etc,/var/log.txt"</>
  <g>    --deny-net[=<<IP_OR_HOSTNAME>...]</>     Deny network access. Optionally specify defined IP addresses and host names, with ports as necessary.
                                             <p(245)>--deny-net  |  --deny-net="localhost:8080,deno.land"</>
  <g>    --deny-env[=<<VARIABLE_NAME>...]</>      Deny access to environment variables. Optionally specify inacessible environment variables.
                                             <p(245)>--deny-env  |  --deny-env="PORT,HOME,PATH"</>
  <g>    --deny-sys[=<<API_NAME>...]</>           Deny access to OS information. Optionally deny specific APIs by function name.
                                             <p(245)>--deny-sys  |  --deny-sys="systemMemoryInfo,osRelease"</>
      <g>--deny-run[=<<PROGRAM_NAME>...]</>       Deny running subprocesses. Optionally specify denied runnable program names.
                                             <p(245)>--deny-run  |  --deny-run="whoami,ps"</>
      <g>--deny-ffi[=<<PATH>...]</>               (Unstable) Deny loading dynamic libraries. Optionally specify denied directories or files.
                                             <p(245)>--deny-ffi  |  --deny-ffi="./libfoo.so"</>
"#))
    .arg(
      {
        let mut arg = allow_all_arg().hide(true);
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("allow-read")
          .long("allow-read")
          .short('R')
          .num_args(0..)
          .action(ArgAction::Append)
          .require_equals(true)
          .value_name("PATH")
          .help("Allow file system read access. Optionally specify allowed paths")
          .value_hint(ValueHint::AnyPath)
          .hide(true);
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("deny-read")
          .long("deny-read")
          .num_args(0..)
          .action(ArgAction::Append)
          .require_equals(true)
          .value_name("PATH")
          .help("Deny file system read access. Optionally specify denied paths")
          .value_hint(ValueHint::AnyPath)
          .hide(true);
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("allow-write")
          .long("allow-write")
          .short('W')
          .num_args(0..)
          .action(ArgAction::Append)
          .require_equals(true)
          .value_name("PATH")
          .help("Allow file system write access. Optionally specify allowed paths")
          .value_hint(ValueHint::AnyPath)
          .hide(true);
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("deny-write")
          .long("deny-write")
          .num_args(0..)
          .action(ArgAction::Append)
          .require_equals(true)
          .value_name("PATH")
          .help("Deny file system write access. Optionally specify denied paths")
          .value_hint(ValueHint::AnyPath)
          .hide(true);
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("allow-net")
          .long("allow-net")
          .short('N')
          .num_args(0..)
          .use_value_delimiter(true)
          .require_equals(true)
          .value_name("IP_OR_HOSTNAME")
          .help("Allow network access. Optionally specify allowed IP addresses and host names, with ports as necessary")
          .value_parser(flags_net::validator)
          .hide(true)
          ;
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("deny-net")
          .long("deny-net")
          .num_args(0..)
          .use_value_delimiter(true)
          .require_equals(true)
          .value_name("IP_OR_HOSTNAME")
          .help("Deny network access. Optionally specify denied IP addresses and host names, with ports as necessary")
          .value_parser(flags_net::validator)
          .hide(true)
          ;
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("allow-env")
          .long("allow-env")
          .short('E')
          .num_args(0..)
          .use_value_delimiter(true)
          .require_equals(true)
          .value_name("VARIABLE_NAME")
          .help("Allow access to system environment information. Optionally specify accessible environment variables")
          .value_parser(|key: &str| {
            if key.is_empty() || key.contains(&['=', '\0'] as &[char]) {
              return Err(format!("invalid key \"{key}\""));
            }

            Ok(if cfg!(windows) {
              key.to_uppercase()
            } else {
              key.to_string()
            })
          })
          .hide(true)
          ;
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("deny-env")
          .long("deny-env")
          .num_args(0..)
          .use_value_delimiter(true)
          .require_equals(true)
          .value_name("VARIABLE_NAME")
          .help("Deny access to system environment information. Optionally specify accessible environment variables")
          .value_parser(|key: &str| {
            if key.is_empty() || key.contains(&['=', '\0'] as &[char]) {
              return Err(format!("invalid key \"{key}\""));
            }

            Ok(if cfg!(windows) {
              key.to_uppercase()
            } else {
              key.to_string()
            })
          })
          .hide(true)
          ;
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("allow-sys")
          .long("allow-sys")
          .short('S')
          .num_args(0..)
          .use_value_delimiter(true)
          .require_equals(true)
          .value_name("API_NAME")
          .help("Allow access to OS information. Optionally allow specific APIs by function name")
          .value_parser(|key: &str| SysDescriptor::parse(key.to_string()).map(|s| s.into_string()))
          .hide(true)
          ;
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("deny-sys")
          .long("deny-sys")
          .num_args(0..)
          .use_value_delimiter(true)
          .require_equals(true)
          .value_name("API_NAME")
          .help("Deny access to OS information. Optionally deny specific APIs by function name")
          .value_parser(|key: &str| SysDescriptor::parse(key.to_string()).map(|s| s.into_string()))
          .hide(true)
          ;
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("allow-run")
          .long("allow-run")
          .num_args(0..)
          .use_value_delimiter(true)
          .require_equals(true)
          .value_name("PROGRAM_NAME")
          .help("Allow running subprocesses. Optionally specify allowed runnable program names")
          .hide(true)
          ;
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("deny-run")
          .long("deny-run")
          .num_args(0..)
          .use_value_delimiter(true)
          .require_equals(true)
          .value_name("PROGRAM_NAME")
          .help("Deny running subprocesses. Optionally specify denied runnable program names")
          .hide(true)
          ;
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg

      }
    )
    .arg(
      {
        let mut arg = Arg::new("allow-ffi")
          .long("allow-ffi")
          .num_args(0..)
          .action(ArgAction::Append)
          .require_equals(true)
          .value_name("PATH")
          .help("(Unstable) Allow loading dynamic libraries. Optionally specify allowed directories or files")
          .value_hint(ValueHint::AnyPath)
          .hide(true);
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("deny-ffi")
          .long("deny-ffi")
          .num_args(0..)
          .action(ArgAction::Append)
          .require_equals(true)
          .value_name("PATH")
          .help("(Unstable) Deny loading dynamic libraries. Optionally specify denied directories or files")
          .value_hint(ValueHint::AnyPath)
          .hide(true);
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("allow-hrtime")
          .long("allow-hrtime")
          .action(ArgAction::SetTrue)
          .help("REMOVED in Deno 2.0")
          .hide(true);
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("deny-hrtime")
          .long("deny-hrtime")
          .action(ArgAction::SetTrue)
          .help("REMOVED in Deno 2.0")
          .hide(true);
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = Arg::new("no-prompt")
          .long("no-prompt")
          .action(ArgAction::SetTrue)
          .hide(true)
          .help("Always throw if required permission wasn't passed");
        if let Some(requires) = requires {
          arg = arg.requires(requires)
        }
        arg
      }
    )
    .arg(
      {
        let mut arg = allow_import_arg().hide(true);
        if let Some(requires) = requires {
          // allow this for install --global
          if requires != "global" {
            arg = arg.requires(requires)
          }
        }
        arg
      }
    )
}

fn allow_all_arg() -> Arg {
  Arg::new("allow-all")
    .short('A')
    .long("allow-all")
    .conflicts_with("allow-read")
    .conflicts_with("allow-write")
    .conflicts_with("allow-net")
    .conflicts_with("allow-env")
    .conflicts_with("allow-run")
    .conflicts_with("allow-sys")
    .conflicts_with("allow-ffi")
    .conflicts_with("allow-import")
    .action(ArgAction::SetTrue)
    .help("Allow all permissions")
}

fn runtime_args(
  app: Command,
  include_perms: bool,
  include_inspector: bool,
) -> Command {
  let app = compile_args(app);
  let app = if include_perms {
    permission_args(app, None)
  } else {
    app
  };
  let app = if include_inspector {
    inspect_args(app)
  } else {
    app
  };
  app
    .arg(frozen_lockfile_arg())
    .arg(cached_only_arg())
    .arg(location_arg())
    .arg(v8_flags_arg())
    .arg(seed_arg())
    .arg(enable_testing_features_arg())
    .arg(strace_ops_arg())
}

fn allow_import_arg() -> Arg {
  Arg::new("allow-import")
    .long("allow-import")
    .short('I')
    .num_args(0..)
    .use_value_delimiter(true)
    .require_equals(true)
    .value_name("IP_OR_HOSTNAME")
    .help(cstr!(
      "Allow importing from remote hosts. Optionally specify allowed IP addresses and host names, with ports as necessary. Default value: <p(245)>deno.land:443,jsr.io:443,esm.sh:443,cdn.jsdelivr.net:443,raw.githubusercontent.com:443,user.githubusercontent.com:443</>"
    ))
    .value_parser(flags_net::validator)
}

fn inspect_args(app: Command) -> Command {
  app
    .arg(
      Arg::new("inspect")
        .long("inspect")
        .value_name("HOST_AND_PORT")
        .default_missing_value("127.0.0.1:9229")
        .help(cstr!("Activate inspector on host:port <p(245)>[default: 127.0.0.1:9229]</>"))
        .num_args(0..=1)
        .require_equals(true)
        .value_parser(value_parser!(SocketAddr))
        .help_heading(DEBUGGING_HEADING),
    )
    .arg(
      Arg::new("inspect-brk")
        .long("inspect-brk")
        .value_name("HOST_AND_PORT")
        .default_missing_value("127.0.0.1:9229")
        .help(
          "Activate inspector on host:port, wait for debugger to connect and break at the start of user script",
        )
        .num_args(0..=1)
        .require_equals(true)
        .value_parser(value_parser!(SocketAddr))
        .help_heading(DEBUGGING_HEADING),
    )
    .arg(
      Arg::new("inspect-wait")
        .long("inspect-wait")
        .value_name("HOST_AND_PORT")
        .default_missing_value("127.0.0.1:9229")
        .help(
          "Activate inspector on host:port and wait for debugger to connect before running user code",
        )
        .num_args(0..=1)
        .require_equals(true)
        .value_parser(value_parser!(SocketAddr))
        .help_heading(DEBUGGING_HEADING),
    )
}

fn import_map_arg() -> Arg {
  Arg::new("import-map")
    .long("import-map")
    .alias("importmap")
    .value_name("FILE")
    .help(cstr!(
      "Load import map file from local file or remote URL
  <p(245)>Docs: https://docs.deno.com/runtime/manual/basics/import_maps</>",
    ))
    .value_hint(ValueHint::FilePath)
    .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn env_file_arg() -> Arg {
  Arg::new("env-file")
    .long("env-file")
    .alias("env")
    .value_name("FILE")
    .help(cstr!(
      "Load environment variables from local file
  <p(245)>Only the first environment variable with a given key is used.
  Existing process environment variables are not overwritten.</>"
    ))
    .value_hint(ValueHint::FilePath)
    .default_missing_value(".env")
    .require_equals(true)
    .num_args(0..=1)
}

fn reload_arg() -> Arg {
  Arg::new("reload")
    .short('r')
    .num_args(0..)
    .action(ArgAction::Append)
    .require_equals(true)
    .long("reload")
    .value_name("CACHE_BLOCKLIST")
    .help(
      cstr!("Reload source code cache (recompile TypeScript)
  <p(245)>no value                                                 Reload everything
  jsr:@std/http/file-server,jsr:@std/assert/assert-equals  Reloads specific modules
  npm:                                                     Reload all npm modules
  npm:chalk                                                Reload specific npm module</>",
    ))
    .value_hint(ValueHint::FilePath)
    .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn ca_file_arg() -> Arg {
  Arg::new("cert")
    .long("cert")
    .value_name("FILE")
    .help("Load certificate authority from PEM encoded file")
    .value_hint(ValueHint::FilePath)
}

fn cached_only_arg() -> Arg {
  Arg::new("cached-only")
    .long("cached-only")
    .action(ArgAction::SetTrue)
    .help("Require that remote dependencies are already cached")
    .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn frozen_lockfile_arg() -> Arg {
  Arg::new("frozen")
    .long("frozen")
    .alias("frozen-lockfile")
    .value_parser(value_parser!(bool))
    .value_name("BOOLEAN")
    .num_args(0..=1)
    .require_equals(true)
    .default_missing_value("true")
    .help("Error out if lockfile is out of date")
    .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

/// Used for subcommands that operate on executable scripts only.
/// `deno fmt` has its own `--ext` arg because its possible values differ.
/// If --ext is not provided and the script doesn't have a file extension,
/// deno_graph::parse_module() defaults to js.
fn executable_ext_arg() -> Arg {
  Arg::new("ext")
    .long("ext")
    .help("Set content type of the supplied file")
    .value_parser(["ts", "tsx", "js", "jsx"])
}

fn location_arg() -> Arg {
  Arg::new("location")
    .long("location")
    .value_name("HREF")
    .value_parser(|href: &str| -> Result<Url, String> {
      let url = Url::parse(href);
      if url.is_err() {
        return Err("Failed to parse URL".to_string());
      }
      let mut url = url.unwrap();
      if !["http", "https"].contains(&url.scheme()) {
        return Err("Expected protocol \"http\" or \"https\"".to_string());
      }
      url.set_username("").unwrap();
      url.set_password(None).unwrap();
      Ok(url)
    })
    .help(cstr!(
      "Value of <p(245)>globalThis.location</> used by some web APIs"
    ))
    .value_hint(ValueHint::Url)
}

fn enable_testing_features_arg() -> Arg {
  Arg::new("enable-testing-features-do-not-use")
    .long("enable-testing-features-do-not-use")
    .help("INTERNAL: Enable internal features used during integration testing")
    .action(ArgAction::SetTrue)
    .hide(true)
}

fn strace_ops_arg() -> Arg {
  Arg::new("strace-ops")
    .long("strace-ops")
    .num_args(0..)
    .use_value_delimiter(true)
    .require_equals(true)
    .value_name("OPS")
    .help("Trace low-level op calls")
    .hide(true)
}

fn v8_flags_arg() -> Arg {
  Arg::new("v8-flags")
    .long("v8-flags")
    .num_args(..)
    .use_value_delimiter(true)
    .require_equals(true)
    .value_name("V8_FLAGS")
    .help( cstr!("To see a list of all available flags use --v8-flags=--help
  <p(245)>Flags can also be set via the DENO_V8_FLAGS environment variable.
  Any flags set with this flag are appended after the DENO_V8_FLAGS environment variable</>"))
}

fn seed_arg() -> Arg {
  Arg::new("seed")
    .long("seed")
    .value_name("NUMBER")
    .help("Set the random number generator seed")
    .value_parser(value_parser!(u64))
}

fn hmr_arg(takes_files: bool) -> Arg {
  let arg = Arg::new("hmr")
    .long("watch-hmr")
    // NOTE(bartlomieju): compatibility with Deno pre-1.46
    .alias("unstable-hmr")
    .help("Watch for file changes and hot replace modules")
    .conflicts_with("watch")
    .help_heading(FILE_WATCHING_HEADING);

  if takes_files {
    arg
      .value_name("FILES")
      .num_args(0..)
      .action(ArgAction::Append)
      .require_equals(true)
      .help(
        cstr!(
        "Watch for file changes and restart process automatically.
  <p(245)>Local files from entry point module graph are watched by default.
  Additional paths might be watched by passing them as arguments to this flag.</>"),
      )
      .value_hint(ValueHint::AnyPath)
  } else {
    arg.action(ArgAction::SetTrue).help(cstr!(
      "Watch for file changes and restart process automatically.
  <p(245)>Only local files from entry point module graph are watched.</>"
    ))
  }
}

fn watch_arg(takes_files: bool) -> Arg {
  let arg = Arg::new("watch")
    .long("watch")
    .help_heading(FILE_WATCHING_HEADING);

  if takes_files {
    arg
      .value_name("FILES")
      .num_args(0..)
      .action(ArgAction::Append)
      .require_equals(true)
      .help(
        cstr!(
        "Watch for file changes and restart process automatically.
  <p(245)>Local files from entry point module graph are watched by default.
  Additional paths might be watched by passing them as arguments to this flag.</>"),
      )
      .value_hint(ValueHint::AnyPath)
  } else {
    arg.action(ArgAction::SetTrue).help(cstr!(
      "Watch for file changes and restart process automatically.
  <p(245)>Only local files from entry point module graph are watched.</>"
    ))
  }
}

fn no_clear_screen_arg() -> Arg {
  Arg::new("no-clear-screen")
    .requires("watch")
    .long("no-clear-screen")
    .action(ArgAction::SetTrue)
    .help("Do not clear terminal screen when under watch mode")
    .help_heading(FILE_WATCHING_HEADING)
}

fn no_code_cache_arg() -> Arg {
  Arg::new("no-code-cache")
    .long("no-code-cache")
    .help("Disable V8 code cache feature")
    .action(ArgAction::SetTrue)
}

fn watch_exclude_arg() -> Arg {
  Arg::new("watch-exclude")
    .long("watch-exclude")
    .help("Exclude provided files/patterns from watch mode")
    .value_name("FILES")
    .num_args(0..)
    .action(ArgAction::Append)
    .require_equals(true)
    .value_hint(ValueHint::AnyPath)
    .help_heading(FILE_WATCHING_HEADING)
}

fn no_check_arg() -> Arg {
  Arg::new("no-check")
    .num_args(0..=1)
    .require_equals(true)
    .value_name("NO_CHECK_TYPE")
    .long("no-check")
    .help("Skip type-checking. If the value of \"remote\" is supplied, diagnostic errors from remote modules will be ignored")
    .help_heading(TYPE_CHECKING_HEADING)
}

fn check_arg(checks_local_by_default: bool) -> Arg {
  let arg = Arg::new("check")
    .conflicts_with("no-check")
    .long("check")
    .num_args(0..=1)
    .require_equals(true)
    .value_name("CHECK_TYPE")
    .help_heading(TYPE_CHECKING_HEADING);

  if checks_local_by_default {
    arg.help(
      cstr!("Set type-checking behavior. This subcommand type-checks local modules by default, so adding --check is redundant
  <p(245)>If the value of \"all\" is supplied, remote modules will be included.
  Alternatively, the 'deno check' subcommand can be used</>",
    ))
  } else {
    arg.help(cstr!(
      "Enable type-checking. This subcommand does not type-check by default
  <p(245)>If the value of \"all\" is supplied, remote modules will be included.
  Alternatively, the 'deno check' subcommand can be used</>"
    ))
  }
}

fn script_arg() -> Arg {
  Arg::new("script_arg")
    .num_args(0..)
    .action(ArgAction::Append)
    // NOTE: these defaults are provided
    // so `deno run --v8-flags=--help` works
    // without specifying file to run.
    .default_value_ifs([
      ("v8-flags", "--help", Some("_")),
      ("v8-flags", "-help", Some("_")),
    ])
    .help("Script arg")
    .value_name("SCRIPT_ARG")
    .value_hint(ValueHint::FilePath)
}

fn lock_arg() -> Arg {
  Arg::new("lock")
    .long("lock")
    .value_name("FILE")
    .default_missing_value("./deno.lock")
    .help("Check the specified lock file. (If value is not provided, defaults to \"./deno.lock\")")
    .num_args(0..=1)
    .value_parser(value_parser!(String))
    .value_hint(ValueHint::FilePath)
    .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn no_lock_arg() -> Arg {
  Arg::new("no-lock")
    .long("no-lock")
    .action(ArgAction::SetTrue)
    .help("Disable auto discovery of the lock file")
    .conflicts_with("lock")
    .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn config_arg() -> Arg {
  Arg::new("config")
    .short('c')
    .long("config")
    .value_name("FILE")
    .help(cstr!("Configure different aspects of deno including TypeScript, linting, and code formatting
  <p(245)>Typically the configuration file will be called `deno.json` or `deno.jsonc` and
  automatically detected; in that case this flag is not necessary.
  Docs: https://docs.deno.com/go/config</>"))
    .value_hint(ValueHint::FilePath)
}

fn no_config_arg() -> Arg {
  Arg::new("no-config")
    .long("no-config")
    .action(ArgAction::SetTrue)
    .help("Disable automatic loading of the configuration file")
    .conflicts_with("config")
}

fn no_remote_arg() -> Arg {
  Arg::new("no-remote")
    .long("no-remote")
    .action(ArgAction::SetTrue)
    .help("Do not resolve remote modules")
    .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn no_npm_arg() -> Arg {
  Arg::new("no-npm")
    .long("no-npm")
    .action(ArgAction::SetTrue)
    .help("Do not resolve npm modules")
    .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn node_modules_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  let value = matches.remove_one::<NodeModulesDirMode>("node-modules-dir");
  if let Some(mode) = value {
    flags.node_modules_dir = Some(mode);
  }
}

fn node_modules_dir_arg() -> Arg {
  fn parse_node_modules_dir_mode(
    s: &str,
  ) -> Result<NodeModulesDirMode, String> {
    match s {
      "auto" | "true" => Ok(NodeModulesDirMode::Auto),
      "manual" => Ok(NodeModulesDirMode::Manual),
      "none" | "false" => Ok(NodeModulesDirMode::None),
      _ => Err(format!(
        "Invalid value '{}': expected \"auto\", \"manual\" or \"none\"",
        s
      )),
    }
  }

  Arg::new("node-modules-dir")
    .long("node-modules-dir")
    .num_args(0..=1)
    .default_missing_value("auto")
    .value_parser(clap::builder::ValueParser::new(parse_node_modules_dir_mode))
    .value_name("MODE")
    .require_equals(true)
    .help("Sets the node modules management mode for npm packages")
    .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn vendor_arg() -> Arg {
  Arg::new("vendor")
    .long("vendor")
    .num_args(0..=1)
    .value_parser(value_parser!(bool))
    .default_missing_value("true")
    .require_equals(true)
    .help("Toggles local vendor folder usage for remote modules and a node_modules folder for npm packages")
    .help_heading(DEPENDENCY_MANAGEMENT_HEADING)
}

fn unsafely_ignore_certificate_errors_arg() -> Arg {
  Arg::new("unsafely-ignore-certificate-errors")
    .hide(true)
    .long("unsafely-ignore-certificate-errors")
    .num_args(0..)
    .use_value_delimiter(true)
    .require_equals(true)
    .value_name("HOSTNAMES")
    .help("DANGER: Disables verification of TLS certificates")
    .value_parser(flags_net::validator)
}

fn allow_scripts_arg() -> Arg {
  Arg::new("allow-scripts")
    .long("allow-scripts")
    .num_args(0..)
    .action(ArgAction::Append)
    .require_equals(true)
    .value_name("PACKAGE")
    .value_parser(parse_packages_allowed_scripts)
    .help(cstr!("Allow running npm lifecycle scripts for the given packages
  <p(245)>Note: Scripts will only be executed when using a node_modules directory (`--node-modules-dir`)</>"))
}

enum UnstableArgsConfig {
  // for backwards-compatability
  None,
  ResolutionOnly,
  ResolutionAndRuntime,
}

struct UnstableArgsIter {
  idx: usize,
  cfg: UnstableArgsConfig,
}

impl Iterator for UnstableArgsIter {
  type Item = Arg;

  fn next(&mut self) -> Option<Self::Item> {
    let arg = if self.idx == 0 {
      Arg::new("unstable")
        .long("unstable")
        .help(cstr!("Enable all unstable features and APIs. Instead of using this flag, consider enabling individual unstable features
  <p(245)>To view the list of individual unstable feature flags, run this command again with --help=unstable</>"))
        .action(ArgAction::SetTrue)
        .hide(matches!(self.cfg, UnstableArgsConfig::None))
    } else if self.idx == 1 {
      Arg::new("unstable-bare-node-builtins")
        .long("unstable-bare-node-builtins")
        .help("Enable unstable bare node builtins feature")
        .env("DENO_UNSTABLE_BARE_NODE_BUILTINS")
        .value_parser(FalseyValueParser::new())
        .action(ArgAction::SetTrue)
        .hide(true)
        .long_help(match self.cfg {
          UnstableArgsConfig::None => None,
          UnstableArgsConfig::ResolutionOnly
          | UnstableArgsConfig::ResolutionAndRuntime => Some("true"),
        })
        .help_heading(UNSTABLE_HEADING)
    } else if self.idx == 2 {
      Arg::new("unstable-byonm")
        .long("unstable-byonm")
        .value_parser(FalseyValueParser::new())
        .action(ArgAction::SetTrue)
        .hide(true)
        .help_heading(UNSTABLE_HEADING)
    } else if self.idx == 3 {
      Arg::new("unstable-sloppy-imports")
      .long("unstable-sloppy-imports")
      .help("Enable unstable resolving of specifiers by extension probing, .js to .ts, and directory probing")
      .env("DENO_UNSTABLE_SLOPPY_IMPORTS")
      .value_parser(FalseyValueParser::new())
      .action(ArgAction::SetTrue)
      .hide(true)
      .long_help(match self.cfg {
        UnstableArgsConfig::None => None,
        UnstableArgsConfig::ResolutionOnly | UnstableArgsConfig::ResolutionAndRuntime => Some("true")
      })
      .help_heading(UNSTABLE_HEADING)
    } else if self.idx > 3 {
      let granular_flag = crate::UNSTABLE_GRANULAR_FLAGS.get(self.idx - 4)?;
      Arg::new(format!("unstable-{}", granular_flag.name))
        .long(format!("unstable-{}", granular_flag.name))
        .help(granular_flag.help_text)
        .action(ArgAction::SetTrue)
        .hide(true)
        .help_heading(UNSTABLE_HEADING)
        // we don't render long help, so using it here as a sort of metadata
        .long_help(if granular_flag.show_in_help {
          match self.cfg {
            UnstableArgsConfig::None | UnstableArgsConfig::ResolutionOnly => {
              None
            }
            UnstableArgsConfig::ResolutionAndRuntime => Some("true"),
          }
        } else {
          None
        })
    } else {
      return None;
    };
    self.idx += 1;
    Some(arg.display_order(self.idx + 1000))
  }
}

fn unstable_args(cfg: UnstableArgsConfig) -> impl IntoIterator<Item = Arg> {
  UnstableArgsIter { idx: 0, cfg }
}

fn allow_scripts_arg_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  let Some(parts) = matches.remove_many::<String>("allow-scripts") else {
    return Ok(());
  };
  if parts.len() == 0 {
    flags.allow_scripts = PackagesAllowedScripts::All;
  } else {
    flags.allow_scripts = PackagesAllowedScripts::Some(
      parts
        .flat_map(flat_escape_split_commas)
        .collect::<Result<_, _>>()?,
    );
  }
  Ok(())
}

fn add_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.subcommand = DenoSubcommand::Add(add_parse_inner(matches, None));
}

fn add_parse_inner(
  matches: &mut ArgMatches,
  packages: Option<clap::parser::Values<String>>,
) -> AddFlags {
  let packages = packages
    .unwrap_or_else(|| matches.remove_many::<String>("packages").unwrap())
    .collect();
  let dev = matches.get_flag("dev");
  AddFlags { packages, dev }
}

fn remove_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.subcommand = DenoSubcommand::Remove(RemoveFlags {
    packages: matches.remove_many::<String>("packages").unwrap().collect(),
  });
}

fn bench_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  flags.type_check_mode = TypeCheckMode::Local;

  runtime_args_parse(flags, matches, true, false)?;
  ext_arg_parse(flags, matches);

  // NOTE: `deno bench` always uses `--no-prompt`, tests shouldn't ever do
  // interactive prompts, unless done by user code
  flags.permissions.no_prompt = true;

  let json = matches.get_flag("json");

  let ignore = match matches.remove_many::<String>("ignore") {
    Some(f) => f
      .flat_map(flat_escape_split_commas)
      .collect::<Result<_, _>>()?,
    None => vec![],
  };

  let filter = matches.remove_one::<String>("filter");

  if matches.contains_id("script_arg") {
    flags
      .argv
      .extend(matches.remove_many::<String>("script_arg").unwrap());
  }

  let include = if let Some(files) = matches.remove_many::<String>("files") {
    files.collect()
  } else {
    Vec::new()
  };

  let no_run = matches.get_flag("no-run");

  flags.subcommand = DenoSubcommand::Bench(BenchFlags {
    files: FileFlags { include, ignore },
    filter,
    json,
    no_run,
    watch: watch_arg_parse(matches)?,
  });

  Ok(())
}

fn bundle_parse(flags: &mut Flags, _matches: &mut ArgMatches) {
  flags.subcommand = DenoSubcommand::Bundle;
}

fn cache_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  compile_args_parse(flags, matches)?;
  unstable_args_parse(flags, matches, UnstableArgsConfig::ResolutionOnly);
  frozen_lockfile_arg_parse(flags, matches);
  allow_scripts_arg_parse(flags, matches)?;
  allow_import_parse(flags, matches);
  let files = matches.remove_many::<String>("file").unwrap().collect();
  flags.subcommand = DenoSubcommand::Cache(CacheFlags { files });
  Ok(())
}

fn check_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  flags.type_check_mode = TypeCheckMode::Local;
  compile_args_without_check_parse(flags, matches)?;
  unstable_args_parse(flags, matches, UnstableArgsConfig::ResolutionAndRuntime);
  let files = matches.remove_many::<String>("file").unwrap().collect();
  if matches.get_flag("all") || matches.get_flag("remote") {
    flags.type_check_mode = TypeCheckMode::All;
  }
  flags.subcommand = DenoSubcommand::Check(CheckFlags {
    files,
    doc: matches.get_flag("doc"),
    doc_only: matches.get_flag("doc-only"),
  });
  allow_import_parse(flags, matches);
  Ok(())
}

fn clean_parse(flags: &mut Flags, _matches: &mut ArgMatches) {
  flags.subcommand = DenoSubcommand::Clean;
}

fn compile_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  flags.type_check_mode = TypeCheckMode::Local;
  runtime_args_parse(flags, matches, true, false)?;

  let mut script = matches.remove_many::<String>("script_arg").unwrap();
  let source_file = script.next().unwrap();
  let args = script.collect();
  let output = matches.remove_one::<String>("output");
  let target = matches.remove_one::<String>("target");
  let icon = matches.remove_one::<String>("icon");
  let no_terminal = matches.get_flag("no-terminal");
  let include = match matches.remove_many::<String>("include") {
    Some(f) => f.collect(),
    None => vec![],
  };
  ext_arg_parse(flags, matches);

  flags.subcommand = DenoSubcommand::Compile(CompileFlags {
    source_file,
    output,
    args,
    target,
    no_terminal,
    icon,
    include,
  });

  Ok(())
}

fn completions_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
  mut app: Command,
) {
  use clap_complete::generate;
  use clap_complete::shells::Bash;
  use clap_complete::shells::Fish;
  use clap_complete::shells::PowerShell;
  use clap_complete::shells::Zsh;
  use clap_complete_fig::Fig;

  let mut buf: Vec<u8> = vec![];
  let name = "deno";

  match matches.get_one::<String>("shell").unwrap().as_str() {
    "bash" => generate(Bash, &mut app, name, &mut buf),
    "fish" => generate(Fish, &mut app, name, &mut buf),
    "powershell" => generate(PowerShell, &mut app, name, &mut buf),
    "zsh" => generate(Zsh, &mut app, name, &mut buf),
    "fig" => generate(Fig, &mut app, name, &mut buf),
    _ => unreachable!(),
  }

  flags.subcommand = DenoSubcommand::Completions(CompletionsFlags {
    buf: buf.into_boxed_slice(),
  });
}

fn coverage_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  let files = match matches.remove_many::<String>("files") {
    Some(f) => f.collect(),
    None => vec!["coverage".to_string()], // default
  };
  let ignore = match matches.remove_many::<String>("ignore") {
    Some(f) => f
      .flat_map(flat_escape_split_commas)
      .collect::<Result<Vec<_>, _>>()?,
    None => vec![],
  };
  let include = match matches.remove_many::<String>("include") {
    Some(f) => f.collect(),
    None => vec![],
  };
  let exclude = match matches.remove_many::<String>("exclude") {
    Some(f) => f.collect(),
    None => vec![],
  };
  let r#type = if matches.get_flag("lcov") {
    CoverageType::Lcov
  } else if matches.get_flag("html") {
    CoverageType::Html
  } else if matches.get_flag("detailed") {
    CoverageType::Detailed
  } else {
    CoverageType::Summary
  };
  let output = matches.remove_one::<String>("output");
  flags.subcommand = DenoSubcommand::Coverage(CoverageFlags {
    files: FileFlags {
      include: files,
      ignore,
    },
    output,
    include,
    exclude,
    r#type,
  });
  Ok(())
}

fn doc_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  unstable_args_parse(flags, matches, UnstableArgsConfig::ResolutionOnly);
  import_map_arg_parse(flags, matches);
  reload_arg_parse(flags, matches)?;
  lock_arg_parse(flags, matches);
  no_lock_arg_parse(flags, matches);
  no_npm_arg_parse(flags, matches);
  no_remote_arg_parse(flags, matches);
  allow_import_parse(flags, matches);

  let source_files_val = matches.remove_many::<String>("source_file");
  let source_files = if let Some(val) = source_files_val {
    let vals: Vec<String> = val.collect();

    if vals.len() == 1 {
      if vals[0] == "--builtin" {
        DocSourceFileFlag::Builtin
      } else {
        DocSourceFileFlag::Paths(vec![vals[0].to_string()])
      }
    } else {
      DocSourceFileFlag::Paths(
        vals.into_iter().filter(|v| v != "--builtin").collect(),
      )
    }
  } else {
    DocSourceFileFlag::Builtin
  };
  let private = matches.get_flag("private");
  let lint = matches.get_flag("lint");
  let json = matches.get_flag("json");
  let filter = matches.remove_one::<String>("filter");
  let html = if matches.get_flag("html") {
    let name = matches.remove_one::<String>("name");
    let category_docs_path = matches.remove_one::<String>("category-docs");
    let symbol_redirect_map_path =
      matches.remove_one::<String>("symbol-redirect-map");
    let strip_trailing_html = matches.get_flag("strip-trailing-html");
    let default_symbol_map_path =
      matches.remove_one::<String>("default-symbol-map");
    let output = matches
      .remove_one::<String>("output")
      .unwrap_or(String::from("./docs/"));
    Some(DocHtmlFlag {
      name,
      category_docs_path,
      symbol_redirect_map_path,
      default_symbol_map_path,
      strip_trailing_html,
      output,
    })
  } else {
    None
  };

  flags.subcommand = DenoSubcommand::Doc(DocFlags {
    source_files,
    json,
    lint,
    html,
    filter,
    private,
  });
  Ok(())
}

fn eval_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  runtime_args_parse(flags, matches, false, true)?;
  unstable_args_parse(flags, matches, UnstableArgsConfig::ResolutionAndRuntime);
  flags.allow_all();

  ext_arg_parse(flags, matches);

  let print = matches.get_flag("print");
  let mut code_args = matches.remove_many::<String>("code_arg").unwrap();
  let code = code_args.next().unwrap();
  flags.argv.extend(code_args);

  flags.subcommand = DenoSubcommand::Eval(EvalFlags { print, code });
  Ok(())
}

fn fmt_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  config_args_parse(flags, matches);
  ext_arg_parse(flags, matches);

  let include = match matches.remove_many::<String>("files") {
    Some(f) => f.collect(),
    None => vec![],
  };
  let ignore = match matches.remove_many::<String>("ignore") {
    Some(f) => f
      .flat_map(flat_escape_split_commas)
      .collect::<Result<Vec<_>, _>>()?,
    None => vec![],
  };

  let use_tabs = matches.remove_one::<bool>("use-tabs");
  let line_width = matches.remove_one::<NonZeroU32>("line-width");
  let indent_width = matches.remove_one::<NonZeroU8>("indent-width");
  let single_quote = matches.remove_one::<bool>("single-quote");
  let prose_wrap = matches.remove_one::<String>("prose-wrap");
  let no_semicolons = matches.remove_one::<bool>("no-semicolons");
  let unstable_component = matches.get_flag("unstable-component");

  flags.subcommand = DenoSubcommand::Fmt(FmtFlags {
    check: matches.get_flag("check"),
    files: FileFlags { include, ignore },
    use_tabs,
    line_width,
    indent_width,
    single_quote,
    prose_wrap,
    no_semicolons,
    watch: watch_arg_parse(matches)?,
    unstable_component,
  });
  Ok(())
}

fn init_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.subcommand = DenoSubcommand::Init(InitFlags {
    dir: matches.remove_one::<String>("dir"),
    lib: matches.get_flag("lib"),
    serve: matches.get_flag("serve"),
  });
}

fn info_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  unstable_args_parse(flags, matches, UnstableArgsConfig::ResolutionOnly);
  reload_arg_parse(flags, matches)?;
  config_args_parse(flags, matches);
  import_map_arg_parse(flags, matches);
  location_arg_parse(flags, matches);
  ca_file_arg_parse(flags, matches);
  unsafely_ignore_certificate_errors_parse(flags, matches);
  node_modules_and_vendor_dir_arg_parse(flags, matches);
  lock_args_parse(flags, matches);
  no_remote_arg_parse(flags, matches);
  no_npm_arg_parse(flags, matches);
  allow_import_parse(flags, matches);
  let json = matches.get_flag("json");
  flags.subcommand = DenoSubcommand::Info(InfoFlags {
    file: matches.remove_one::<String>("file"),
    json,
  });

  Ok(())
}

fn install_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  runtime_args_parse(flags, matches, true, true)?;

  let global = matches.get_flag("global");
  if global {
    let root = matches.remove_one::<String>("root");
    let force = matches.get_flag("force");
    let name = matches.remove_one::<String>("name");
    let mut cmd_values =
      matches.remove_many::<String>("cmd").unwrap_or_default();

    let module_url = cmd_values.next().unwrap();
    let args = cmd_values.collect();

    flags.subcommand = DenoSubcommand::Install(InstallFlags {
      kind: InstallKind::Global(InstallFlagsGlobal {
        name,
        module_url,
        args,
        root,
        force,
      }),
    });

    return Ok(());
  }

  // allow scripts only applies to local install
  allow_scripts_arg_parse(flags, matches)?;
  if matches.get_flag("entrypoint") {
    let entrypoints = matches.remove_many::<String>("cmd").unwrap_or_default();
    flags.subcommand = DenoSubcommand::Install(InstallFlags {
      kind: InstallKind::Local(InstallFlagsLocal::Entrypoints(
        entrypoints.collect(),
      )),
    });
  } else if let Some(add_files) = matches
    .remove_many("cmd")
    .map(|packages| add_parse_inner(matches, Some(packages)))
  {
    flags.subcommand = DenoSubcommand::Install(InstallFlags {
      kind: InstallKind::Local(InstallFlagsLocal::Add(add_files)),
    })
  } else {
    flags.subcommand = DenoSubcommand::Install(InstallFlags {
      kind: InstallKind::Local(InstallFlagsLocal::TopLevel),
    });
  }
  Ok(())
}

fn json_reference_parse(
  flags: &mut Flags,
  _matches: &mut ArgMatches,
  mut app: Command,
) {
  use deno_core::serde_json::json;

  app.build();

  fn serialize_command(
    mut command: Command,
    top_level: bool,
  ) -> deno_core::serde_json::Value {
    let args = command
      .get_arguments()
      .filter(|arg| {
        !arg.is_hide_set()
          && if top_level {
            arg.is_global_set()
          } else {
            !arg.is_global_set()
          }
      })
      .map(|arg| {
        let name = arg.get_id().as_str();
        let short = arg.get_short();
        let long = arg.get_long();
        let required = arg.is_required_set();
        let help = arg.get_help().map(|help| help.ansi().to_string());
        let help_heading = arg
          .get_help_heading()
          .map(|help_heading| help_heading.to_string());
        let usage = arg.to_string();

        json!({
          "name": name,
          "short": short,
          "long": long,
          "required": required,
          "help": help,
          "help_heading": help_heading,
          "usage": usage,
        })
      })
      .collect::<Vec<_>>();

    let name = command.get_name().to_string();
    let about = command.get_about().map(|about| about.ansi().to_string());
    let usage = command.render_usage().ansi().to_string();

    let subcommands = command
      .get_subcommands()
      .map(|command| {
        serialize_command(
          if command
            .get_arguments()
            .any(|arg| arg.get_id().as_str() == "unstable")
          {
            enable_unstable(command.clone())
          } else {
            command.clone()
          },
          false,
        )
      })
      .collect::<Vec<_>>();

    json!({
      "name": name,
      "about": about,
      "args": args,
      "subcommands": subcommands,
      "usage": usage,
    })
  }

  flags.subcommand = DenoSubcommand::JSONReference(JSONReferenceFlags {
    json: serialize_command(app, true),
  })
}

fn jupyter_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  unstable_args_parse(flags, matches, UnstableArgsConfig::ResolutionAndRuntime);

  let conn_file = matches.remove_one::<String>("conn");
  let kernel = matches.get_flag("kernel");
  let install = matches.get_flag("install");

  flags.subcommand = DenoSubcommand::Jupyter(JupyterFlags {
    install,
    kernel,
    conn_file,
  });
}

fn uninstall_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  let name = matches.remove_one::<String>("name-or-package").unwrap();

  let kind = if matches.get_flag("global") {
    let root = matches.remove_one::<String>("root");
    UninstallKind::Global(UninstallFlagsGlobal { name, root })
  } else {
    let packages: Vec<_> = vec![name]
      .into_iter()
      .chain(
        matches
          .remove_many::<String>("additional-packages")
          .unwrap_or_default(),
      )
      .collect();
    UninstallKind::Local(RemoveFlags { packages })
  };

  flags.subcommand = DenoSubcommand::Uninstall(UninstallFlags { kind });
}

fn lsp_parse(flags: &mut Flags, _matches: &mut ArgMatches) {
  flags.subcommand = DenoSubcommand::Lsp;
}

fn lint_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  unstable_args_parse(flags, matches, UnstableArgsConfig::ResolutionOnly);
  ext_arg_parse(flags, matches);
  config_args_parse(flags, matches);

  let files = match matches.remove_many::<String>("files") {
    Some(f) => f.collect(),
    None => vec![],
  };
  let ignore = match matches.remove_many::<String>("ignore") {
    Some(f) => f
      .flat_map(flat_escape_split_commas)
      .collect::<Result<Vec<_>, _>>()?,
    None => vec![],
  };
  let fix = matches.get_flag("fix");
  let rules = matches.get_flag("rules");
  let maybe_rules_tags = matches
    .remove_many::<String>("rules-tags")
    .map(|f| f.collect());

  let maybe_rules_include = matches
    .remove_many::<String>("rules-include")
    .map(|f| f.collect());

  let maybe_rules_exclude = matches
    .remove_many::<String>("rules-exclude")
    .map(|f| f.collect());

  let json = matches.get_flag("json");
  let compact = matches.get_flag("compact");

  flags.subcommand = DenoSubcommand::Lint(LintFlags {
    files: FileFlags {
      include: files,
      ignore,
    },
    fix,
    rules,
    maybe_rules_tags,
    maybe_rules_include,
    maybe_rules_exclude,
    json,
    compact,
    watch: watch_arg_parse(matches)?,
  });
  Ok(())
}

fn repl_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  runtime_args_parse(flags, matches, true, true)?;
  unsafely_ignore_certificate_errors_parse(flags, matches);

  let eval_files = matches
    .remove_many::<String>("eval-file")
    .map(|values| {
      values
        .flat_map(flat_escape_split_commas)
        .collect::<Result<Vec<_>, _>>()
    })
    .transpose()?;

  if let Some(args) = matches.remove_many::<String>("args") {
    flags.argv.extend(args);
  }

  handle_repl_flags(
    flags,
    ReplFlags {
      eval_files,
      eval: matches.remove_one::<String>("eval"),
      is_default_command: false,
    },
  );
  Ok(())
}

fn run_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
  mut app: Command,
  bare: bool,
) -> clap::error::Result<()> {
  runtime_args_parse(flags, matches, true, true)?;
  ext_arg_parse(flags, matches);

  flags.code_cache_enabled = !matches.get_flag("no-code-cache");

  if let Some(mut script_arg) = matches.remove_many::<String>("script_arg") {
    let script = script_arg.next().unwrap();
    flags.argv.extend(script_arg);
    flags.subcommand = DenoSubcommand::Run(RunFlags {
      script,
      watch: watch_arg_parse_with_paths(matches)?,
      bare,
    });
  } else if bare {
    return Err(app.override_usage("deno [OPTIONS] [COMMAND] [SCRIPT_ARG]...").error(
      clap::error::ErrorKind::MissingRequiredArgument,
      "[SCRIPT_ARG] may only be omitted with --v8-flags=--help, else to use the repl with arguments, please use the `deno repl` subcommand",
    ));
  } else {
    return Err(app.find_subcommand_mut("run").unwrap().error(
      clap::error::ErrorKind::MissingRequiredArgument,
      "[SCRIPT_ARG] may only be omitted with --v8-flags=--help",
    ));
  }

  Ok(())
}

fn serve_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
  app: Command,
) -> clap::error::Result<()> {
  // deno serve implies --allow-net=host:port
  let port = matches.remove_one::<u16>("port").unwrap_or(8000);
  let host = matches
    .remove_one::<String>("host")
    .unwrap_or_else(|| "0.0.0.0".to_owned());

  let worker_count = parallel_arg_parse(matches).map(|v| v.get());

  runtime_args_parse(flags, matches, true, true)?;
  // If the user didn't pass --allow-net, add this port to the network
  // allowlist. If the host is 0.0.0.0, we add :{port} and allow the same network perms
  // as if it was passed to --allow-net directly.
  let allowed = flags_net::parse(vec![if host == "0.0.0.0" {
    format!(":{port}")
  } else {
    format!("{host}:{port}")
  }])?;
  match &mut flags.permissions.allow_net {
    None if !flags.permissions.allow_all => {
      flags.permissions.allow_net = Some(allowed)
    }
    None => {}
    Some(v) => {
      if !v.is_empty() {
        v.extend(allowed);
      }
    }
  }
  flags.code_cache_enabled = !matches.get_flag("no-code-cache");

  let mut script_arg =
    matches.remove_many::<String>("script_arg").ok_or_else(|| {
      let mut app = app;
      let subcommand = &mut app.find_subcommand_mut("serve").unwrap();
      subcommand.error(
        clap::error::ErrorKind::MissingRequiredArgument,
        "[SCRIPT_ARG] may only be omitted with --v8-flags=--help",
      )
    })?;

  let script = script_arg.next().unwrap();
  flags.argv.extend(script_arg);

  ext_arg_parse(flags, matches);

  flags.subcommand = DenoSubcommand::Serve(ServeFlags {
    script,
    watch: watch_arg_parse_with_paths(matches)?,
    port,
    host,
    worker_count,
  });

  Ok(())
}

fn task_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.config_flag = matches
    .remove_one::<String>("config")
    .map(ConfigFlag::Path)
    .unwrap_or(ConfigFlag::Discover);

  unstable_args_parse(flags, matches, UnstableArgsConfig::ResolutionAndRuntime);
  node_modules_arg_parse(flags, matches);

  let mut task_flags = TaskFlags {
    cwd: matches.remove_one::<String>("cwd"),
    task: None,
    is_run: false,
  };

  if let Some((task, mut matches)) = matches.remove_subcommand() {
    task_flags.task = Some(task);

    flags.argv.extend(
      matches
        .remove_many::<std::ffi::OsString>("")
        .into_iter()
        .flatten()
        .filter_map(|arg| arg.into_string().ok()),
    );
  }

  flags.subcommand = DenoSubcommand::Task(task_flags);
}

fn parallel_arg_parse(matches: &mut ArgMatches) -> Option<NonZeroUsize> {
  if matches.get_flag("parallel") {
    if let Ok(value) = env::var("DENO_JOBS") {
      value.parse::<NonZeroUsize>().ok()
    } else {
      std::thread::available_parallelism().ok()
    }
  } else {
    None
  }
}

fn test_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  flags.type_check_mode = TypeCheckMode::Local;
  runtime_args_parse(flags, matches, true, true)?;
  ext_arg_parse(flags, matches);

  // NOTE: `deno test` always uses `--no-prompt`, tests shouldn't ever do
  // interactive prompts, unless done by user code
  flags.permissions.no_prompt = true;

  let ignore = match matches.remove_many::<String>("ignore") {
    Some(f) => f
      .flat_map(flat_escape_split_commas)
      .collect::<Result<_, _>>()?,
    None => vec![],
  };

  let no_run = matches.get_flag("no-run");
  let trace_leaks = matches.get_flag("trace-leaks");
  let doc = matches.get_flag("doc");
  #[allow(clippy::print_stderr)]
  let permit_no_files = matches.get_flag("permit-no-files");
  let filter = matches.remove_one::<String>("filter");
  let clean = matches.get_flag("clean");

  let fail_fast = if matches.contains_id("fail-fast") {
    Some(
      matches
        .remove_one::<NonZeroUsize>("fail-fast")
        .unwrap_or_else(|| NonZeroUsize::new(1).unwrap()),
    )
  } else {
    None
  };

  let shuffle = if matches.contains_id("shuffle") {
    Some(
      matches
        .remove_one::<u64>("shuffle")
        .unwrap_or_else(rand::random),
    )
  } else {
    None
  };

  if let Some(script_arg) = matches.remove_many::<String>("script_arg") {
    flags.argv.extend(script_arg);
  }

  let concurrent_jobs = parallel_arg_parse(matches);

  let include = if let Some(files) = matches.remove_many::<String>("files") {
    files.collect()
  } else {
    Vec::new()
  };

  let junit_path = matches.remove_one::<String>("junit-path");

  let reporter =
    if let Some(reporter) = matches.remove_one::<String>("reporter") {
      match reporter.as_str() {
        "pretty" => TestReporterConfig::Pretty,
        "junit" => TestReporterConfig::Junit,
        "dot" => TestReporterConfig::Dot,
        "tap" => TestReporterConfig::Tap,
        _ => unreachable!(),
      }
    } else {
      TestReporterConfig::Pretty
    };

  if matches!(reporter, TestReporterConfig::Dot | TestReporterConfig::Tap) {
    flags.log_level = Some(Level::Error);
  }

  let hide_stacktraces = matches.get_flag("hide-stacktraces");

  flags.subcommand = DenoSubcommand::Test(TestFlags {
    no_run,
    doc,
    coverage_dir: matches.remove_one::<String>("coverage"),
    clean,
    fail_fast,
    files: FileFlags { include, ignore },
    filter,
    shuffle,
    permit_no_files,
    concurrent_jobs,
    trace_leaks,
    watch: watch_arg_parse_with_paths(matches)?,
    reporter,
    junit_path,
    hide_stacktraces,
  });
  Ok(())
}

fn types_parse(flags: &mut Flags, _matches: &mut ArgMatches) {
  flags.subcommand = DenoSubcommand::Types;
}

fn upgrade_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  ca_file_arg_parse(flags, matches);
  unsafely_ignore_certificate_errors_parse(flags, matches);

  let dry_run = matches.get_flag("dry-run");
  let force = matches.get_flag("force");
  let canary = matches.get_flag("canary");
  let release_candidate = matches.get_flag("release-candidate");
  let version = matches.remove_one::<String>("version");
  let output = matches.remove_one::<String>("output");
  let version_or_hash_or_channel =
    matches.remove_one::<String>("version-or-hash-or-channel");
  flags.subcommand = DenoSubcommand::Upgrade(UpgradeFlags {
    dry_run,
    force,
    release_candidate,
    canary,
    version,
    output,
    version_or_hash_or_channel,
  });
}

fn vendor_parse(flags: &mut Flags, _matches: &mut ArgMatches) {
  flags.subcommand = DenoSubcommand::Vendor
}

fn publish_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.type_check_mode = TypeCheckMode::Local; // local by default
  unstable_args_parse(flags, matches, UnstableArgsConfig::ResolutionOnly);
  no_check_arg_parse(flags, matches);
  check_arg_parse(flags, matches);
  config_args_parse(flags, matches);

  flags.subcommand = DenoSubcommand::Publish(PublishFlags {
    token: matches.remove_one("token"),
    dry_run: matches.get_flag("dry-run"),
    allow_slow_types: matches.get_flag("allow-slow-types"),
    allow_dirty: matches.get_flag("allow-dirty"),
    no_provenance: matches.get_flag("no-provenance"),
  });
}

fn compile_args_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  compile_args_without_check_parse(flags, matches)?;
  no_check_arg_parse(flags, matches);
  check_arg_parse(flags, matches);
  Ok(())
}

fn compile_args_without_check_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  import_map_arg_parse(flags, matches);
  no_remote_arg_parse(flags, matches);
  no_npm_arg_parse(flags, matches);
  node_modules_and_vendor_dir_arg_parse(flags, matches);
  config_args_parse(flags, matches);
  reload_arg_parse(flags, matches)?;
  lock_args_parse(flags, matches);
  ca_file_arg_parse(flags, matches);
  unsafely_ignore_certificate_errors_parse(flags, matches);
  Ok(())
}

fn escape_and_split_commas(s: String) -> Result<Vec<String>, clap::Error> {
  let mut result = vec![];
  let mut current = String::new();
  let mut chars = s.chars();

  while let Some(c) = chars.next() {
    if c == ',' {
      if let Some(next) = chars.next() {
        if next == ',' {
          current.push(',');
        } else {
          if current.is_empty() {
            return Err(
              std::io::Error::new(
                std::io::ErrorKind::Other,
                String::from("Empty values are not allowed"),
              )
              .into(),
            );
          }

          result.push(current.clone());
          current.clear();
          current.push(next);
        }
      } else {
        return Err(
          std::io::Error::new(
            std::io::ErrorKind::Other,
            String::from("Empty values are not allowed"),
          )
          .into(),
        );
      }
    } else {
      current.push(c);
    }
  }

  if current.is_empty() {
    return Err(
      std::io::Error::new(
        std::io::ErrorKind::Other,
        String::from("Empty values are not allowed"),
      )
      .into(),
    );
  }

  result.push(current);

  Ok(result)
}

fn flat_escape_split_commas(str: String) -> Vec<Result<String, clap::Error>> {
  match escape_and_split_commas(str) {
    Ok(vec) => vec.into_iter().map(Ok).collect::<Vec<_>>(),
    Err(e) => vec![Err(e)],
  }
}

fn permission_args_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  if let Some(read_wl) = matches.remove_many::<String>("allow-read") {
    let read_wl = read_wl
      .flat_map(flat_escape_split_commas)
      .collect::<Result<Vec<_>, _>>()?;
    flags.permissions.allow_read = Some(read_wl);
  }

  if let Some(read_wl) = matches.remove_many::<String>("deny-read") {
    let read_wl = read_wl
      .flat_map(flat_escape_split_commas)
      .collect::<Result<Vec<_>, _>>()?;
    flags.permissions.deny_read = Some(read_wl);
  }

  if let Some(write_wl) = matches.remove_many::<String>("allow-write") {
    let write_wl = write_wl
      .flat_map(flat_escape_split_commas)
      .collect::<Result<Vec<_>, _>>()?;
    flags.permissions.allow_write = Some(write_wl);
  }

  if let Some(write_wl) = matches.remove_many::<String>("deny-write") {
    let write_wl = write_wl
      .flat_map(flat_escape_split_commas)
      .collect::<Result<Vec<_>, _>>()?;
    flags.permissions.deny_write = Some(write_wl);
  }

  if let Some(net_wl) = matches.remove_many::<String>("allow-net") {
    let net_allowlist = flags_net::parse(net_wl.collect())?;
    flags.permissions.allow_net = Some(net_allowlist);
  }

  if let Some(net_wl) = matches.remove_many::<String>("deny-net") {
    let net_denylist = flags_net::parse(net_wl.collect())?;
    flags.permissions.deny_net = Some(net_denylist);
  }

  if let Some(env_wl) = matches.remove_many::<String>("allow-env") {
    flags.permissions.allow_env = Some(env_wl.collect());
    debug!("env allowlist: {:#?}", &flags.permissions.allow_env);
  }

  if let Some(env_wl) = matches.remove_many::<String>("deny-env") {
    flags.permissions.deny_env = Some(env_wl.collect());
    debug!("env denylist: {:#?}", &flags.permissions.deny_env);
  }

  if let Some(run_wl) = matches.remove_many::<String>("allow-run") {
    flags.permissions.allow_run = Some(run_wl.collect());
    debug!("run allowlist: {:#?}", &flags.permissions.allow_run);
  }

  if let Some(run_wl) = matches.remove_many::<String>("deny-run") {
    flags.permissions.deny_run = Some(run_wl.collect());
    debug!("run denylist: {:#?}", &flags.permissions.deny_run);
  }

  if let Some(sys_wl) = matches.remove_many::<String>("allow-sys") {
    flags.permissions.allow_sys = Some(sys_wl.collect());
    debug!("sys info allowlist: {:#?}", &flags.permissions.allow_sys);
  }

  if let Some(sys_wl) = matches.remove_many::<String>("deny-sys") {
    flags.permissions.deny_sys = Some(sys_wl.collect());
    debug!("sys info denylist: {:#?}", &flags.permissions.deny_sys);
  }

  if let Some(ffi_wl) = matches.remove_many::<String>("allow-ffi") {
    let ffi_wl = ffi_wl
      .flat_map(flat_escape_split_commas)
      .collect::<Result<Vec<_>, _>>()?;
    flags.permissions.allow_ffi = Some(ffi_wl);
    debug!("ffi allowlist: {:#?}", &flags.permissions.allow_ffi);
  }

  if let Some(ffi_wl) = matches.remove_many::<String>("deny-ffi") {
    let ffi_wl = ffi_wl
      .flat_map(flat_escape_split_commas)
      .collect::<Result<Vec<_>, _>>()?;
    flags.permissions.deny_ffi = Some(ffi_wl);
    debug!("ffi denylist: {:#?}", &flags.permissions.deny_ffi);
  }

  if matches.get_flag("allow-hrtime") || matches.get_flag("deny-hrtime") {
    // use eprintln instead of log::warn because logging hasn't been initialized yet
    #[allow(clippy::print_stderr)]
    {
      eprintln!(
        "{} `allow-hrtime` and `deny-hrtime` have been removed in Deno 2, as high resolution time is now always allowed",
        deno_runtime::colors::yellow("Warning")
      );
    }
  }

  if matches.get_flag("allow-all") {
    flags.allow_all();
  }

  allow_import_parse(flags, matches);

  if matches.get_flag("no-prompt") {
    flags.permissions.no_prompt = true;
  }

  Ok(())
}

fn allow_import_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if let Some(imports_wl) = matches.remove_many::<String>("allow-import") {
    let imports_allowlist = flags_net::parse(imports_wl.collect()).unwrap();
    flags.permissions.allow_import = Some(imports_allowlist);
  }
}

fn unsafely_ignore_certificate_errors_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) {
  if let Some(ic_wl) =
    matches.remove_many::<String>("unsafely-ignore-certificate-errors")
  {
    let ic_allowlist = flags_net::parse(ic_wl.collect()).unwrap();
    flags.unsafely_ignore_certificate_errors = Some(ic_allowlist);
  }
}

fn runtime_args_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
  include_perms: bool,
  include_inspector: bool,
) -> clap::error::Result<()> {
  unstable_args_parse(flags, matches, UnstableArgsConfig::ResolutionAndRuntime);
  compile_args_parse(flags, matches)?;
  cached_only_arg_parse(flags, matches);
  frozen_lockfile_arg_parse(flags, matches);
  if include_perms {
    permission_args_parse(flags, matches)?;
  }
  if include_inspector {
    inspect_arg_parse(flags, matches);
  }
  location_arg_parse(flags, matches);
  v8_flags_arg_parse(flags, matches);
  seed_arg_parse(flags, matches);
  enable_testing_features_arg_parse(flags, matches);
  env_file_arg_parse(flags, matches);
  strace_ops_parse(flags, matches);
  Ok(())
}

fn inspect_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.inspect = matches.remove_one::<SocketAddr>("inspect");
  flags.inspect_brk = matches.remove_one::<SocketAddr>("inspect-brk");
  flags.inspect_wait = matches.remove_one::<SocketAddr>("inspect-wait");
}

fn import_map_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.import_map_path = matches.remove_one::<String>("import-map");
}

fn env_file_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.env_file = matches.remove_one::<String>("env-file");
}

fn reload_arg_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) -> clap::error::Result<()> {
  if let Some(cache_bl) = matches.remove_many::<String>("reload") {
    let raw_cache_blocklist: Vec<String> = cache_bl
      .flat_map(flat_escape_split_commas)
      .map(|s| s.and_then(reload_arg_validate))
      .collect::<Result<Vec<_>, _>>()?;
    if raw_cache_blocklist.is_empty() {
      flags.reload = true;
    } else {
      flags.cache_blocklist = resolve_urls(raw_cache_blocklist);
      debug!("cache blocklist: {:#?}", &flags.cache_blocklist);
      flags.reload = false;
    }
  }

  Ok(())
}

fn ca_file_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.ca_data = matches.remove_one::<String>("cert").map(CaData::File);
}

fn enable_testing_features_arg_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) {
  if matches.get_flag("enable-testing-features-do-not-use") {
    flags.enable_testing_features = true
  }
}

fn strace_ops_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if let Some(patterns) = matches.remove_many::<String>("strace-ops") {
    flags.strace_ops = Some(patterns.collect());
  }
}

fn cached_only_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if matches.get_flag("cached-only") {
    flags.cached_only = true;
  }
}

fn frozen_lockfile_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if let Some(&v) = matches.get_one::<bool>("frozen") {
    flags.frozen_lockfile = Some(v);
  }
}

fn ext_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.ext = matches.remove_one::<String>("ext");
}

fn location_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.location = matches.remove_one::<Url>("location");
}

fn v8_flags_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if let Some(v8_flags) = matches.remove_many::<String>("v8-flags") {
    flags.v8_flags = v8_flags.collect();
  }
}

fn seed_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if let Some(seed) = matches.remove_one::<u64>("seed") {
    flags.seed = Some(seed);

    flags.v8_flags.push(format!("--random-seed={seed}"));
  }
}

fn no_check_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if let Some(cache_type) = matches.get_one::<String>("no-check") {
    match cache_type.as_str() {
      "remote" => flags.type_check_mode = TypeCheckMode::Local,
      _ => debug!(
        "invalid value for 'no-check' of '{}' using default",
        cache_type
      ),
    }
  } else if matches.contains_id("no-check") {
    flags.type_check_mode = TypeCheckMode::None;
  }
}

fn check_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if let Some(cache_type) = matches.get_one::<String>("check") {
    match cache_type.as_str() {
      "all" => flags.type_check_mode = TypeCheckMode::All,
      _ => debug!(
        "invalid value for 'check' of '{}' using default",
        cache_type
      ),
    }
  } else if matches.contains_id("check") {
    flags.type_check_mode = TypeCheckMode::Local;
  }
}

fn lock_args_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  lock_arg_parse(flags, matches);
  no_lock_arg_parse(flags, matches);
}

fn lock_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if matches.contains_id("lock") {
    let lockfile = matches.remove_one::<String>("lock").unwrap();
    flags.lock = Some(lockfile);
  }
}

fn no_lock_arg_parse(flags: &mut Flags, matches: &ArgMatches) {
  if matches.get_flag("no-lock") {
    flags.no_lock = true;
  }
}

fn config_args_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.config_flag = if matches.get_flag("no-config") {
    ConfigFlag::Disabled
  } else if let Some(config) = matches.remove_one::<String>("config") {
    ConfigFlag::Path(config)
  } else {
    ConfigFlag::Discover
  };
}

fn no_remote_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if matches.get_flag("no-remote") {
    flags.no_remote = true;
  }
}

fn no_npm_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if matches.get_flag("no-npm") {
    flags.no_npm = true;
  }
}

fn node_modules_and_vendor_dir_arg_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) {
  node_modules_arg_parse(flags, matches);
  flags.vendor = matches.remove_one::<bool>("vendor");
}

fn reload_arg_validate(urlstr: String) -> Result<String, clap::Error> {
  if urlstr.is_empty() {
    return Err(
      std::io::Error::new(
        std::io::ErrorKind::Other,
        String::from("Missing url. Check for extra commas."),
      )
      .into(),
    );
  }
  match Url::from_str(&urlstr) {
    Ok(_) => Ok(urlstr),
    Err(e) => {
      Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()).into())
    }
  }
}

fn watch_arg_parse(
  matches: &mut ArgMatches,
) -> clap::error::Result<Option<WatchFlags>> {
  if matches.get_flag("watch") {
    Ok(Some(WatchFlags {
      hmr: false,
      no_clear_screen: matches.get_flag("no-clear-screen"),
      exclude: matches
        .remove_many::<String>("watch-exclude")
        .map(|f| {
          f.flat_map(flat_escape_split_commas)
            .collect::<Result<_, _>>()
        })
        .transpose()?
        .unwrap_or_default(),
    }))
  } else {
    Ok(None)
  }
}

fn watch_arg_parse_with_paths(
  matches: &mut ArgMatches,
) -> clap::error::Result<Option<WatchFlagsWithPaths>> {
  if let Some(paths) = matches.remove_many::<String>("watch") {
    return Ok(Some(WatchFlagsWithPaths {
      paths: paths
        .flat_map(flat_escape_split_commas)
        .collect::<Result<Vec<_>, _>>()?,
      hmr: false,
      no_clear_screen: matches.get_flag("no-clear-screen"),
      exclude: matches
        .remove_many::<String>("watch-exclude")
        .map(|f| {
          f.flat_map(flat_escape_split_commas)
            .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default(),
    }));
  }

  if matches.try_contains_id("hmr").is_ok() {
    return matches
      .remove_many::<String>("hmr")
      .map(|paths| {
        Ok(WatchFlagsWithPaths {
          paths: paths
            .flat_map(flat_escape_split_commas)
            .collect::<Result<Vec<_>, _>>()?,
          hmr: true,
          no_clear_screen: matches.get_flag("no-clear-screen"),
          exclude: matches
            .remove_many::<String>("watch-exclude")
            .map(|f| {
              f.flat_map(flat_escape_split_commas)
                .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default(),
        })
      })
      .transpose();
  }

  Ok(None)
}

fn unstable_args_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
  cfg: UnstableArgsConfig,
) {
  // TODO(bartlomieju): remove in Deno 2.5
  if matches.get_flag("unstable") {
    flags.unstable_config.legacy_flag_enabled = true;
  }

  flags.unstable_config.bare_node_builtins =
    matches.get_flag("unstable-bare-node-builtins");
  flags.unstable_config.sloppy_imports =
    matches.get_flag("unstable-sloppy-imports");

  if matches!(cfg, UnstableArgsConfig::ResolutionAndRuntime) {
    for granular_flag in crate::UNSTABLE_GRANULAR_FLAGS {
      if matches.get_flag(&format!("unstable-{}", granular_flag.name)) {
        flags
          .unstable_config
          .features
          .push(granular_flag.name.to_string());
      }
    }
  }
}

// TODO(ry) move this to utility module and add test.
/// Strips fragment part of URL. Panics on bad URL.
pub fn resolve_urls(urls: Vec<String>) -> Vec<String> {
  let mut out: Vec<String> = vec![];
  for urlstr in urls.iter() {
    if let Ok(mut url) = Url::from_str(urlstr) {
      url.set_fragment(None);
      let mut full_url = String::from(url.as_str());
      if full_url.len() > 1 && full_url.ends_with('/') {
        full_url.pop();
      }
      out.push(full_url);
    } else {
      panic!("Bad Url: {urlstr}");
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  /// Creates vector of strings, Vec<String>
  macro_rules! svec {
    ($($x:expr),* $(,)?) => (vec![$($x.to_string().into()),*]);
  }

  #[test]
  fn global_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "--log-level", "debug", "--quiet", "run", "script.ts"]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string()
        )),
        log_level: Some(Level::Error),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    #[rustfmt::skip]
    let r2 = flags_from_vec(svec!["deno", "run", "--log-level", "debug", "--quiet", "script.ts"]);
    let flags2 = r2.unwrap();
    assert_eq!(flags2, flags);
  }

  #[test]
  fn upgrade() {
    let r = flags_from_vec(svec!["deno", "upgrade", "--dry-run", "--force"]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
          force: true,
          dry_run: true,
          canary: false,
          release_candidate: false,
          version: None,
          output: None,
          version_or_hash_or_channel: None,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn upgrade_with_output_flag() {
    let r = flags_from_vec(svec!["deno", "upgrade", "--output", "example.txt"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
          force: false,
          dry_run: false,
          canary: false,
          release_candidate: false,
          version: None,
          output: Some(String::from("example.txt")),
          version_or_hash_or_channel: None,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn version() {
    let r = flags_from_vec(svec!["deno", "--version"]);
    assert_eq!(
      r.unwrap_err().kind(),
      clap::error::ErrorKind::DisplayVersion
    );
    let r = flags_from_vec(svec!["deno", "-V"]);
    assert_eq!(
      r.unwrap_err().kind(),
      clap::error::ErrorKind::DisplayVersion
    );
  }

  #[test]
  fn run_reload() {
    let r = flags_from_vec(svec!["deno", "run", "-r", "script.ts"]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string()
        )),
        reload: true,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_watch() {
    let r = flags_from_vec(svec!["deno", "run", "--watch", "script.ts"]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: false,
            exclude: vec![],
          }),
          bare: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "--watch",
      "--no-clear-screen",
      "script.ts"
    ]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: true,
            exclude: vec![],
          }),
          bare: true,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--watch-hmr",
      "--no-clear-screen",
      "script.ts"
    ]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: true,
            paths: vec![],
            no_clear_screen: true,
            exclude: vec![],
          }),
          bare: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--unstable-hmr",
      "--no-clear-screen",
      "script.ts"
    ]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: true,
            paths: vec![],
            no_clear_screen: true,
            exclude: vec![],
          }),
          bare: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--watch-hmr=foo.txt",
      "--no-clear-screen",
      "script.ts"
    ]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: true,
            paths: vec![String::from("foo.txt")],
            no_clear_screen: true,
            exclude: vec![],
          }),
          bare: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "run", "--hmr", "--watch", "script.ts"]);
    assert!(r.is_err());
  }

  #[test]
  fn run_watch_with_external() {
    let r = flags_from_vec(svec!["deno", "--watch=file1,file2", "script.ts"]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("file1"), String::from("file2")],
            no_clear_screen: false,
            exclude: vec![],
          }),
          bare: true,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_watch_with_no_clear_screen() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--watch",
      "--no-clear-screen",
      "script.ts"
    ]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: true,
            exclude: vec![],
          }),
          bare: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_watch_with_excluded_paths() {
    let r = flags_from_vec(svec!(
      "deno",
      "--watch",
      "--watch-exclude=foo",
      "script.ts"
    ));

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: false,
            exclude: vec![String::from("foo")],
          }),
          bare: true,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!(
      "deno",
      "run",
      "--watch=foo",
      "--watch-exclude=bar",
      "script.ts"
    ));
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("foo")],
            no_clear_screen: false,
            exclude: vec![String::from("bar")],
          }),
          bare: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--watch",
      "--watch-exclude=foo,bar",
      "script.ts"
    ]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: false,
            exclude: vec![String::from("foo"), String::from("bar")],
          }),
          bare: false,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "--watch=foo,bar",
      "--watch-exclude=baz,qux",
      "script.ts"
    ]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("foo"), String::from("bar")],
            no_clear_screen: false,
            exclude: vec![String::from("baz"), String::from("qux"),],
          }),
          bare: true,
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_reload_allow_write() {
    let r =
      flags_from_vec(svec!["deno", "run", "-r", "--allow-write", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        reload: true,
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string()
        )),
        permissions: PermissionFlags {
          allow_write: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_v8_flags() {
    let r = flags_from_vec(svec!["deno", "run", "--v8-flags=--help"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default("_".to_string())),
        v8_flags: svec!["--help"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--v8-flags=--expose-gc,--gc-stats=1",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        v8_flags: svec!["--expose-gc", "--gc-stats=1"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "run", "--v8-flags=--expose-gc"]);
    assert!(r.is_err());
  }

  #[test]
  fn serve_flags() {
    let r = flags_from_vec(svec!["deno", "serve", "main.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
          "main.ts".to_string(),
          8000,
          "0.0.0.0"
        )),
        permissions: PermissionFlags {
          allow_net: Some(vec![
            "0.0.0.0:8000".to_string(),
            "127.0.0.1:8000".to_string(),
            "localhost:8000".to_string()
          ]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    let r = flags_from_vec(svec!["deno", "serve", "--port", "5000", "main.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
          "main.ts".to_string(),
          5000,
          "0.0.0.0"
        )),
        permissions: PermissionFlags {
          allow_net: Some(vec![
            "0.0.0.0:5000".to_string(),
            "127.0.0.1:5000".to_string(),
            "localhost:5000".to_string()
          ]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    let r = flags_from_vec(svec![
      "deno",
      "serve",
      "--port",
      "5000",
      "--allow-net=example.com",
      "main.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
          "main.ts".to_string(),
          5000,
          "0.0.0.0"
        )),
        permissions: PermissionFlags {
          allow_net: Some(vec![
            "example.com".to_string(),
            "0.0.0.0:5000".to_string(),
            "127.0.0.1:5000".to_string(),
            "localhost:5000".to_string()
          ]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    let r = flags_from_vec(svec![
      "deno",
      "serve",
      "--port",
      "5000",
      "--allow-net",
      "main.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
          "main.ts".to_string(),
          5000,
          "0.0.0.0"
        )),
        permissions: PermissionFlags {
          allow_net: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    let r = flags_from_vec(svec![
      "deno",
      "serve",
      "--port",
      "5000",
      "--host",
      "example.com",
      "main.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
          "main.ts".to_string(),
          5000,
          "example.com"
        )),
        permissions: PermissionFlags {
          allow_net: Some(vec!["example.com:5000".to_owned()]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "serve",
      "--port",
      "0",
      "--host",
      "example.com",
      "main.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
          "main.ts".to_string(),
          0,
          "example.com"
        )),
        permissions: PermissionFlags {
          allow_net: Some(vec!["example.com:0".to_owned()]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn has_permission() {
    let r = flags_from_vec(svec!["deno", "--allow-read", "x.ts"]);
    assert_eq!(r.unwrap().has_permission(), true);

    let r = flags_from_vec(svec!["deno", "run", "--deny-read", "x.ts"]);
    assert_eq!(r.unwrap().has_permission(), true);

    let r = flags_from_vec(svec!["deno", "run", "x.ts"]);
    assert_eq!(r.unwrap().has_permission(), false);
  }

  #[test]
  fn has_permission_in_argv() {
    let r = flags_from_vec(svec!["deno", "run", "x.ts", "--allow-read"]);
    assert_eq!(r.unwrap().has_permission_in_argv(), true);

    let r = flags_from_vec(svec!["deno", "x.ts", "--deny-read"]);
    assert_eq!(r.unwrap().has_permission_in_argv(), true);

    let r = flags_from_vec(svec!["deno", "run", "x.ts"]);
    assert_eq!(r.unwrap().has_permission_in_argv(), false);
  }

  #[test]
  fn script_args() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-net",
      "gist.ts",
      "--title",
      "X"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "gist.ts".to_string()
        )),
        argv: svec!["--title", "X"],
        permissions: PermissionFlags {
          allow_net: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_all() {
    let r = flags_from_vec(svec!["deno", "run", "--allow-all", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "gist.ts".to_string()
        )),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_read() {
    let r = flags_from_vec(svec!["deno", "run", "--allow-read", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "gist.ts".to_string()
        )),
        permissions: PermissionFlags {
          allow_read: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn short_permission_flags() {
    let r = flags_from_vec(svec!["deno", "run", "-RNESWI", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "gist.ts".to_string()
        )),
        permissions: PermissionFlags {
          allow_read: Some(vec![]),
          allow_write: Some(vec![]),
          allow_env: Some(vec![]),
          allow_import: Some(vec![]),
          allow_net: Some(vec![]),
          allow_sys: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_read() {
    let r = flags_from_vec(svec!["deno", "--deny-read", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "gist.ts".to_string(),
          watch: None,
          bare: true,
        }),
        permissions: PermissionFlags {
          deny_read: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn double_hyphen() {
    // notice that flags passed after double dash will not
    // be parsed to Flags but instead forwarded to
    // script args as Deno.args
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-write",
      "script.ts",
      "--",
      "-D",
      "--allow-net"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        argv: svec!["--", "-D", "--allow-net"],
        permissions: PermissionFlags {
          allow_write: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn fmt() {
    let r = flags_from_vec(svec!["deno", "fmt", "script_1.ts", "script_2.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          files: FileFlags {
            include: vec!["script_1.ts".to_string(), "script_2.ts".to_string()],
            ignore: vec![],
          },
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "fmt", "--check"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: true,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "fmt"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "fmt", "--watch"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          watch: Some(Default::default()),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "fmt",
      "--watch",
      "--no-clear-screen",
      "--unstable-css",
      "--unstable-html",
      "--unstable-component",
      "--unstable-yaml"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: true,
          watch: Some(WatchFlags {
            hmr: false,
            no_clear_screen: true,
            exclude: vec![],
          })
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "fmt",
      "--check",
      "--watch",
      "foo.ts",
      "--ignore=bar.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: true,
          files: FileFlags {
            include: vec!["foo.ts".to_string()],
            ignore: vec!["bar.js".to_string()],
          },
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          watch: Some(Default::default()),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "fmt", "--config", "deno.jsonc"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          watch: Default::default(),
        }),
        config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "fmt",
      "--config",
      "deno.jsonc",
      "--watch",
      "foo.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          files: FileFlags {
            include: vec!["foo.ts".to_string()],
            ignore: vec![],
          },
          use_tabs: None,
          line_width: None,
          indent_width: None,
          single_quote: None,
          prose_wrap: None,
          no_semicolons: None,
          unstable_component: false,
          watch: Some(Default::default()),
        }),
        config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "fmt",
      "--use-tabs",
      "--line-width",
      "60",
      "--indent-width",
      "4",
      "--single-quote",
      "--prose-wrap",
      "never",
      "--no-semicolons",
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          use_tabs: Some(true),
          line_width: Some(NonZeroU32::new(60).unwrap()),
          indent_width: Some(NonZeroU8::new(4).unwrap()),
          single_quote: Some(true),
          prose_wrap: Some("never".to_string()),
          no_semicolons: Some(true),
          unstable_component: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    // try providing =false to the booleans
    let r = flags_from_vec(svec![
      "deno",
      "fmt",
      "--use-tabs=false",
      "--single-quote=false",
      "--no-semicolons=false",
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Fmt(FmtFlags {
          check: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          use_tabs: Some(false),
          line_width: None,
          indent_width: None,
          single_quote: Some(false),
          prose_wrap: None,
          no_semicolons: Some(false),
          unstable_component: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn lint() {
    let r = flags_from_vec(svec!["deno", "lint", "script_1.ts", "script_2.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec!["script_1.ts".to_string(), "script_2.ts".to_string(),],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          json: false,
          compact: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--watch",
      "script_1.ts",
      "script_2.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec!["script_1.ts".to_string(), "script_2.ts".to_string()],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          json: false,
          compact: false,
          watch: Some(Default::default()),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--watch",
      "--no-clear-screen",
      "script_1.ts",
      "script_2.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec!["script_1.ts".to_string(), "script_2.ts".to_string()],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          json: false,
          compact: false,
          watch: Some(WatchFlags {
            hmr: false,
            no_clear_screen: true,
            exclude: vec![],
          }),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--fix",
      "--ignore=script_1.ts,script_2.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec![],
            ignore: vec!["script_1.ts".to_string(), "script_2.ts".to_string()],
          },
          fix: true,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          json: false,
          compact: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "lint", "--rules"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          fix: false,
          rules: true,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          json: false,
          compact: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--rules",
      "--rules-tags=recommended"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          fix: false,
          rules: true,
          maybe_rules_tags: Some(svec!["recommended"]),
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          json: false,
          compact: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--rules-tags=",
      "--rules-include=ban-untagged-todo,no-undef",
      "--rules-exclude=no-const-assign"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: Some(svec![""]),
          maybe_rules_include: Some(svec!["ban-untagged-todo", "no-undef"]),
          maybe_rules_exclude: Some(svec!["no-const-assign"]),
          json: false,
          compact: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "lint", "--json", "script_1.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec!["script_1.ts".to_string()],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          json: true,
          compact: false,
          watch: Default::default(),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--config",
      "Deno.jsonc",
      "--json",
      "script_1.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec!["script_1.ts".to_string()],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          json: true,
          compact: false,
          watch: Default::default(),
        }),
        config_flag: ConfigFlag::Path("Deno.jsonc".to_string()),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "lint",
      "--config",
      "Deno.jsonc",
      "--compact",
      "script_1.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Lint(LintFlags {
          files: FileFlags {
            include: vec!["script_1.ts".to_string()],
            ignore: vec![],
          },
          fix: false,
          rules: false,
          maybe_rules_tags: None,
          maybe_rules_include: None,
          maybe_rules_exclude: None,
          json: false,
          compact: true,
          watch: Default::default(),
        }),
        config_flag: ConfigFlag::Path("Deno.jsonc".to_string()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn types() {
    let r = flags_from_vec(svec!["deno", "types"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Types,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cache() {
    let r = flags_from_vec(svec!["deno", "cache", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache(CacheFlags {
          files: svec!["script.ts"],
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn check() {
    let r = flags_from_vec(svec!["deno", "check", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Check(CheckFlags {
          files: svec!["script.ts"],
          doc: false,
          doc_only: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "check", "--doc", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Check(CheckFlags {
          files: svec!["script.ts"],
          doc: true,
          doc_only: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "check", "--doc-only", "markdown.md"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Check(CheckFlags {
          files: svec!["markdown.md"],
          doc: false,
          doc_only: true,
        }),
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );

    // `--doc` and `--doc-only` are mutually exclusive
    let r = flags_from_vec(svec![
      "deno",
      "check",
      "--doc",
      "--doc-only",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap_err().kind(),
      clap::error::ErrorKind::ArgumentConflict
    );

    for all_flag in ["--remote", "--all"] {
      let r = flags_from_vec(svec!["deno", "check", all_flag, "script.ts"]);
      assert_eq!(
        r.unwrap(),
        Flags {
          subcommand: DenoSubcommand::Check(CheckFlags {
            files: svec!["script.ts"],
            doc: false,
            doc_only: false,
          }),
          type_check_mode: TypeCheckMode::All,
          ..Flags::default()
        }
      );

      let r = flags_from_vec(svec![
        "deno",
        "check",
        all_flag,
        "--no-remote",
        "script.ts"
      ]);
      assert_eq!(
        r.unwrap_err().kind(),
        clap::error::ErrorKind::ArgumentConflict
      );
    }
  }

  #[test]
  fn info() {
    let r = flags_from_vec(svec!["deno", "info", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: false,
          file: Some("script.ts".to_string()),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "info", "--reload", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: false,
          file: Some("script.ts".to_string()),
        }),
        reload: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "info", "--json", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: true,
          file: Some("script.ts".to_string()),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "info"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: false,
          file: None
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "info", "--json"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: true,
          file: None
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "info",
      "--no-npm",
      "--no-remote",
      "--config",
      "tsconfig.json"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: false,
          file: None
        }),
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        no_npm: true,
        no_remote: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn tsconfig() {
    let r =
      flags_from_vec(svec!["deno", "run", "-c", "tsconfig.json", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval() {
    let r = flags_from_vec(svec!["deno", "eval", "'console.log(\"hello\")'"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval(EvalFlags {
          print: false,
          code: "'console.log(\"hello\")'".to_string(),
        }),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_p() {
    let r = flags_from_vec(svec!["deno", "eval", "-p", "1+2"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval(EvalFlags {
          print: true,
          code: "1+2".to_string(),
        }),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_typescript() {
    let r = flags_from_vec(svec![
      "deno",
      "eval",
      "--ext=ts",
      "'console.log(\"hello\")'"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval(EvalFlags {
          print: false,
          code: "'console.log(\"hello\")'".to_string(),
        }),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ext: Some("ts".to_string()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "eval", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--reload", "--lock", "lock.json", "--cert", "example.crt", "--cached-only", "--location", "https:foo", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "--env=.example.env", "42"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval(EvalFlags {
          print: false,
          code: "42".to_string(),
        }),
        import_map_path: Some("import_map.json".to_string()),
        no_remote: true,
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        type_check_mode: TypeCheckMode::None,
        reload: true,
        lock: Some(String::from("lock.json")),
        ca_data: Some(CaData::File("example.crt".to_string())),
        cached_only: true,
        location: Some(Url::parse("https://foo/").unwrap()),
        v8_flags: svec!["--help", "--random-seed=1"],
        seed: Some(1),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        env_file: Some(".example.env".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_args() {
    let r = flags_from_vec(svec![
      "deno",
      "eval",
      "console.log(Deno.args)",
      "arg1",
      "arg2"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval(EvalFlags {
          print: false,
          code: "console.log(Deno.args)".to_string(),
        }),
        argv: svec!["arg1", "arg2"],
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl() {
    let r = flags_from_vec(svec!["deno"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: None,
          is_default_command: true,
        }),
        unsafely_ignore_certificate_errors: None,
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_strace_ops() {
    // Lightly test this undocumented flag
    let r = flags_from_vec(svec!["deno", "repl", "--strace-ops"]);
    assert_eq!(r.unwrap().strace_ops, Some(vec![]));
    let r =
      flags_from_vec(svec!["deno", "repl", "--strace-ops=http,websocket"]);
    assert_eq!(
      r.unwrap().strace_ops,
      Some(vec!["http".to_string(), "websocket".to_string()])
    );
  }

  #[test]
  fn repl_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "repl", "-A", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--reload", "--lock", "lock.json", "--cert", "example.crt", "--cached-only", "--location", "https:foo", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "--unsafely-ignore-certificate-errors", "--env=.example.env"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: None,
          is_default_command: false,
        }),
        import_map_path: Some("import_map.json".to_string()),
        no_remote: true,
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        type_check_mode: TypeCheckMode::None,
        reload: true,
        lock: Some(String::from("lock.json")),
        ca_data: Some(CaData::File("example.crt".to_string())),
        cached_only: true,
        location: Some(Url::parse("https://foo/").unwrap()),
        v8_flags: svec!["--help", "--random-seed=1"],
        seed: Some(1),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        env_file: Some(".example.env".to_owned()),
        unsafely_ignore_certificate_errors: Some(vec![]),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_with_eval_flag() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "repl", "--allow-write", "--eval", "console.log('hello');"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: Some("console.log('hello');".to_string()),
          is_default_command: false,
        }),
        permissions: PermissionFlags {
          allow_write: Some(vec![]),
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::None,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_with_eval_file_flag() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "repl", "--eval-file=./a.js,./b.ts,https://examples.deno.land/hello-world.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: Some(vec![
            "./a.js".to_string(),
            "./b.ts".to_string(),
            "https://examples.deno.land/hello-world.ts".to_string()
          ]),
          eval: None,
          is_default_command: false,
        }),
        type_check_mode: TypeCheckMode::None,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_read_allowlist() {
    use test_util::TempDir;
    let temp_dir_guard = TempDir::new();
    let temp_dir = temp_dir_guard.path().to_string();

    let r = flags_from_vec(svec![
      "deno",
      "run",
      format!("--allow-read=.,{}", temp_dir),
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        permissions: PermissionFlags {
          allow_read: Some(vec![String::from("."), temp_dir]),
          ..Default::default()
        },
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_read_denylist() {
    use test_util::TempDir;
    let temp_dir_guard = TempDir::new();
    let temp_dir = temp_dir_guard.path().to_string();

    let r = flags_from_vec(svec![
      "deno",
      "run",
      format!("--deny-read=.,{}", temp_dir),
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        permissions: PermissionFlags {
          deny_read: Some(vec![String::from("."), temp_dir]),
          ..Default::default()
        },
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_write_allowlist() {
    use test_util::TempDir;
    let temp_dir_guard = TempDir::new();
    let temp_dir = temp_dir_guard.path().to_string();

    let r = flags_from_vec(svec![
      "deno",
      "run",
      format!("--allow-write=.,{}", temp_dir),
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        permissions: PermissionFlags {
          allow_write: Some(vec![String::from("."), temp_dir]),
          ..Default::default()
        },
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_write_denylist() {
    use test_util::TempDir;
    let temp_dir_guard = TempDir::new();
    let temp_dir = temp_dir_guard.path().to_string();

    let r = flags_from_vec(svec![
      "deno",
      "run",
      format!("--deny-write=.,{}", temp_dir),
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        permissions: PermissionFlags {
          deny_write: Some(vec![String::from("."), temp_dir]),
          ..Default::default()
        },
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_net_allowlist() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-net=127.0.0.1",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_net: Some(svec!["127.0.0.1"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_net_denylist() {
    let r = flags_from_vec(svec!["deno", "--deny-net=127.0.0.1", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
        }),
        permissions: PermissionFlags {
          deny_net: Some(svec!["127.0.0.1"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_env_allowlist() {
    let r =
      flags_from_vec(svec!["deno", "run", "--allow-env=HOME", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_env: Some(svec!["HOME"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_env_denylist() {
    let r =
      flags_from_vec(svec!["deno", "run", "--deny-env=HOME", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          deny_env: Some(svec!["HOME"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_env_allowlist_multiple() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-env=HOME,PATH",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_env: Some(svec!["HOME", "PATH"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_env_denylist_multiple() {
    let r =
      flags_from_vec(svec!["deno", "run", "--deny-env=HOME,PATH", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          deny_env: Some(svec!["HOME", "PATH"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_env_allowlist_validator() {
    let r =
      flags_from_vec(svec!["deno", "run", "--allow-env=HOME", "script.ts"]);
    assert!(r.is_ok());
    let r = flags_from_vec(svec!["deno", "--allow-env=H=ME", "script.ts"]);
    assert!(r.is_err());
    let r =
      flags_from_vec(svec!["deno", "run", "--allow-env=H\0ME", "script.ts"]);
    assert!(r.is_err());
  }

  #[test]
  fn deny_env_denylist_validator() {
    let r =
      flags_from_vec(svec!["deno", "run", "--deny-env=HOME", "script.ts"]);
    assert!(r.is_ok());
    let r =
      flags_from_vec(svec!["deno", "run", "--deny-env=H=ME", "script.ts"]);
    assert!(r.is_err());
    let r = flags_from_vec(svec!["deno", "--deny-env=H\0ME", "script.ts"]);
    assert!(r.is_err());
  }

  #[test]
  fn allow_sys() {
    let r = flags_from_vec(svec!["deno", "run", "--allow-sys", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_sys: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_sys() {
    let r = flags_from_vec(svec!["deno", "run", "--deny-sys", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          deny_sys: Some(vec![]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_sys_allowlist() {
    let r =
      flags_from_vec(svec!["deno", "run", "--allow-sys=hostname", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_sys: Some(svec!["hostname"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_sys_denylist() {
    let r = flags_from_vec(svec!["deno", "--deny-sys=hostname", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
        }),
        permissions: PermissionFlags {
          deny_sys: Some(svec!["hostname"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_sys_allowlist_multiple() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-sys=hostname,osRelease",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_sys: Some(svec!["hostname", "osRelease"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_sys_denylist_multiple() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--deny-sys=hostname,osRelease",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          deny_sys: Some(svec!["hostname", "osRelease"]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_sys_allowlist_validator() {
    let r =
      flags_from_vec(svec!["deno", "run", "--allow-sys=hostname", "script.ts"]);
    assert!(r.is_ok());
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-sys=hostname,osRelease",
      "script.ts"
    ]);
    assert!(r.is_ok());
    let r =
      flags_from_vec(svec!["deno", "run", "--allow-sys=foo", "script.ts"]);
    assert!(r.is_err());
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-sys=hostname,foo",
      "script.ts"
    ]);
    assert!(r.is_err());
  }

  #[test]
  fn deny_sys_denylist_validator() {
    let r =
      flags_from_vec(svec!["deno", "run", "--deny-sys=hostname", "script.ts"]);
    assert!(r.is_ok());
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--deny-sys=hostname,osRelease",
      "script.ts"
    ]);
    assert!(r.is_ok());
    let r = flags_from_vec(svec!["deno", "run", "--deny-sys=foo", "script.ts"]);
    assert!(r.is_err());
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--deny-sys=hostname,foo",
      "script.ts"
    ]);
    assert!(r.is_err());
  }

  #[test]
  fn reload_validator() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--reload=http://deno.land/",
      "script.ts"
    ]);
    assert!(r.is_ok(), "should accept valid urls");

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--reload=http://deno.land/a,http://deno.land/b",
      "script.ts"
    ]);
    assert!(r.is_ok(), "should accept accept multiple valid urls");

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--reload=./relativeurl/",
      "script.ts"
    ]);
    assert!(r.is_err(), "Should reject relative urls that start with ./");

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--reload=relativeurl/",
      "script.ts"
    ]);
    assert!(r.is_err(), "Should reject relative urls");

    let r =
      flags_from_vec(svec!["deno", "run", "--reload=/absolute", "script.ts"]);
    assert!(r.is_err(), "Should reject absolute urls");

    let r = flags_from_vec(svec!["deno", "--reload=/", "script.ts"]);
    assert!(r.is_err(), "Should reject absolute root url");

    let r = flags_from_vec(svec!["deno", "run", "--reload=", "script.ts"]);
    assert!(r.is_err(), "Should reject when nothing is provided");

    let r = flags_from_vec(svec!["deno", "run", "--reload=,", "script.ts"]);
    assert!(r.is_err(), "Should reject when a single comma is provided");

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--reload=,http://deno.land/a",
      "script.ts"
    ]);
    assert!(r.is_err(), "Should reject a leading comma");

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--reload=http://deno.land/a,",
      "script.ts"
    ]);
    assert!(r.is_err(), "Should reject a trailing comma");
  }

  #[test]
  fn run_import_map() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--import-map=import_map.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        import_map_path: Some("import_map.json".to_owned()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn info_import_map() {
    let r = flags_from_vec(svec![
      "deno",
      "info",
      "--import-map=import_map.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          file: Some("script.ts".to_string()),
          json: false,
        }),
        import_map_path: Some("import_map.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cache_import_map() {
    let r = flags_from_vec(svec![
      "deno",
      "cache",
      "--import-map=import_map.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache(CacheFlags {
          files: svec!["script.ts"],
        }),
        import_map_path: Some("import_map.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn doc_import_map() {
    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--import-map=import_map.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          source_files: DocSourceFileFlag::Paths(vec!["script.ts".to_owned()]),
          private: false,
          json: false,
          html: None,
          lint: false,
          filter: None,
        }),
        import_map_path: Some("import_map.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_env_default() {
    let r = flags_from_vec(svec!["deno", "run", "--env", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        env_file: Some(".env".to_owned()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_env_file_default() {
    let r = flags_from_vec(svec!["deno", "run", "--env-file", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        env_file: Some(".env".to_owned()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_no_code_cache() {
    let r = flags_from_vec(svec!["deno", "--no-code-cache", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_env_defined() {
    let r =
      flags_from_vec(svec!["deno", "run", "--env=.another_env", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        env_file: Some(".another_env".to_owned()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_env_file_defined() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--env-file=.another_env",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        env_file: Some(".another_env".to_owned()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cache_multiple() {
    let r =
      flags_from_vec(svec!["deno", "cache", "script.ts", "script_two.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache(CacheFlags {
          files: svec!["script.ts", "script_two.ts"],
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_seed() {
    let r = flags_from_vec(svec!["deno", "run", "--seed", "250", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        seed: Some(250_u64),
        v8_flags: svec!["--random-seed=250"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_seed_with_v8_flags() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--seed",
      "250",
      "--v8-flags=--expose-gc",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        seed: Some(250_u64),
        v8_flags: svec!["--expose-gc", "--random-seed=250"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn install() {
    let r = flags_from_vec(svec![
      "deno",
      "install",
      "-g",
      "jsr:@std/http/file-server"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Install(InstallFlags {
          kind: InstallKind::Global(InstallFlagsGlobal {
            name: None,
            module_url: "jsr:@std/http/file-server".to_string(),
            args: vec![],
            root: None,
            force: false,
          }),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "install",
      "-g",
      "jsr:@std/http/file-server"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Install(InstallFlags {
          kind: InstallKind::Global(InstallFlagsGlobal {
            name: None,
            module_url: "jsr:@std/http/file-server".to_string(),
            args: vec![],
            root: None,
            force: false,
          }),
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn install_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "install", "--global", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--unsafely-ignore-certificate-errors", "--reload", "--lock", "lock.json", "--cert", "example.crt", "--cached-only", "--allow-read", "--allow-net", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "--name", "file_server", "--root", "/foo", "--force", "--env=.example.env", "jsr:@std/http/file-server", "foo", "bar"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Install(InstallFlags {
          kind: InstallKind::Global(InstallFlagsGlobal {
            name: Some("file_server".to_string()),
            module_url: "jsr:@std/http/file-server".to_string(),
            args: svec!["foo", "bar"],
            root: Some("/foo".to_string()),
            force: true,
          }),
        }),
        import_map_path: Some("import_map.json".to_string()),
        no_remote: true,
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        type_check_mode: TypeCheckMode::None,
        reload: true,
        lock: Some(String::from("lock.json")),
        ca_data: Some(CaData::File("example.crt".to_string())),
        cached_only: true,
        v8_flags: svec!["--help", "--random-seed=1"],
        seed: Some(1),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        unsafely_ignore_certificate_errors: Some(vec![]),
        permissions: PermissionFlags {
          allow_net: Some(vec![]),
          allow_read: Some(vec![]),
          ..Default::default()
        },
        env_file: Some(".example.env".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn uninstall() {
    let r = flags_from_vec(svec!["deno", "uninstall"]);
    assert!(r.is_err(),);

    let r = flags_from_vec(svec!["deno", "uninstall", "@std/load"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Uninstall(UninstallFlags {
          kind: UninstallKind::Local(RemoveFlags {
            packages: vec!["@std/load".to_string()],
          }),
        }),
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "uninstall", "file_server", "@std/load"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Uninstall(UninstallFlags {
          kind: UninstallKind::Local(RemoveFlags {
            packages: vec!["file_server".to_string(), "@std/load".to_string()],
          }),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "uninstall", "-g", "file_server"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Uninstall(UninstallFlags {
          kind: UninstallKind::Global(UninstallFlagsGlobal {
            name: "file_server".to_string(),
            root: None,
          }),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "uninstall",
      "-g",
      "--root",
      "/user/foo/bar",
      "file_server"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Uninstall(UninstallFlags {
          kind: UninstallKind::Global(UninstallFlagsGlobal {
            name: "file_server".to_string(),
            root: Some("/user/foo/bar".to_string()),
          }),
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn uninstall_with_help_flag() {
    let r = flags_from_vec(svec!["deno", "uninstall", "--help"]);
    assert!(r.is_ok());
  }

  #[test]
  fn log_level() {
    let r =
      flags_from_vec(svec!["deno", "run", "--log-level=debug", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        log_level: Some(Level::Debug),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn quiet() {
    let r = flags_from_vec(svec!["deno", "-q", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
        }),
        log_level: Some(Level::Error),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn completions() {
    let r = flags_from_vec(svec!["deno", "completions", "zsh"]).unwrap();

    match r.subcommand {
      DenoSubcommand::Completions(CompletionsFlags { buf }) => {
        assert!(!buf.is_empty())
      }
      _ => unreachable!(),
    }
  }

  #[test]
  fn run_with_args() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "script.ts",
      "--allow-read",
      "--allow-net"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        argv: svec!["--allow-read", "--allow-net"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--location",
      "https:foo",
      "--allow-read",
      "script.ts",
      "--allow-net",
      "-r",
      "--help",
      "--foo",
      "bar"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        location: Some(Url::parse("https://foo/").unwrap()),
        permissions: PermissionFlags {
          allow_read: Some(vec![]),
          ..Default::default()
        },
        argv: svec!["--allow-net", "-r", "--help", "--foo", "bar"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "run", "script.ts", "foo", "bar"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        argv: svec!["foo", "bar"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    let r = flags_from_vec(svec!["deno", "run", "script.ts", "-"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        argv: svec!["-"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "run", "script.ts", "-", "foo", "bar"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        argv: svec!["-", "foo", "bar"],
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn no_check() {
    let r = flags_from_vec(svec!["deno", "--no-check", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
        }),
        type_check_mode: TypeCheckMode::None,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn no_check_remote() {
    let r =
      flags_from_vec(svec!["deno", "run", "--no-check=remote", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        type_check_mode: TypeCheckMode::Local,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_with_unsafely_ignore_certificate_errors() {
    let r = flags_from_vec(svec![
      "deno",
      "repl",
      "--eval",
      "console.log('hello');",
      "--unsafely-ignore-certificate-errors"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: Some("console.log('hello');".to_string()),
          is_default_command: false,
        }),
        unsafely_ignore_certificate_errors: Some(vec![]),
        type_check_mode: TypeCheckMode::None,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_unsafely_ignore_certificate_errors() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--unsafely-ignore-certificate-errors",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        unsafely_ignore_certificate_errors: Some(vec![]),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_unsafely_treat_insecure_origin_as_secure_with_ipv6_address() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--unsafely-ignore-certificate-errors=deno.land,localhost,[::],127.0.0.1,[::1],1.2.3.4",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        unsafely_ignore_certificate_errors: Some(svec![
          "deno.land",
          "localhost",
          "[::]",
          "127.0.0.1",
          "[::1]",
          "1.2.3.4"
        ]),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_with_unsafely_treat_insecure_origin_as_secure_with_ipv6_address() {
    let r = flags_from_vec(svec![
      "deno",
      "repl",
      "--unsafely-ignore-certificate-errors=deno.land,localhost,[::],127.0.0.1,[::1],1.2.3.4"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: None,
          is_default_command: false,
        }),
        unsafely_ignore_certificate_errors: Some(svec![
          "deno.land",
          "localhost",
          "[::]",
          "127.0.0.1",
          "[::1]",
          "1.2.3.4"
        ]),
        type_check_mode: TypeCheckMode::None,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn no_remote() {
    let r = flags_from_vec(svec!["deno", "run", "--no-remote", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        no_remote: true,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn no_npm() {
    let r = flags_from_vec(svec!["deno", "run", "--no-npm", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        no_npm: true,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn local_npm() {
    let r = flags_from_vec(svec!["deno", "--node-modules-dir", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
        }),
        node_modules_dir: Some(NodeModulesDirMode::Auto),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn vendor_flag() {
    let r = flags_from_vec(svec!["deno", "run", "--vendor", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        vendor: Some(true),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "run", "--vendor=false", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        vendor: Some(false),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn cached_only() {
    let r = flags_from_vec(svec!["deno", "run", "--cached-only", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        cached_only: true,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_net_allowlist_with_ports() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-net=deno.land,:8000,:4545",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_net: Some(svec![
            "deno.land",
            "0.0.0.0:8000",
            "127.0.0.1:8000",
            "localhost:8000",
            "0.0.0.0:4545",
            "127.0.0.1:4545",
            "localhost:4545"
          ]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_net_denylist_with_ports() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--deny-net=deno.land,:8000,:4545",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          deny_net: Some(svec![
            "deno.land",
            "0.0.0.0:8000",
            "127.0.0.1:8000",
            "localhost:8000",
            "0.0.0.0:4545",
            "127.0.0.1:4545",
            "localhost:4545"
          ]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn allow_net_allowlist_with_ipv6_address() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-net=deno.land,deno.land:80,[::],127.0.0.1,[::1],1.2.3.4:5678,:5678,[::1]:8080",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_net: Some(svec![
            "deno.land",
            "deno.land:80",
            "[::]",
            "127.0.0.1",
            "[::1]",
            "1.2.3.4:5678",
            "0.0.0.0:5678",
            "127.0.0.1:5678",
            "localhost:5678",
            "[::1]:8080"
          ]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_net_denylist_with_ipv6_address() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--deny-net=deno.land,deno.land:80,[::],127.0.0.1,[::1],1.2.3.4:5678,:5678,[::1]:8080",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        permissions: PermissionFlags {
          deny_net: Some(svec![
            "deno.land",
            "deno.land:80",
            "[::]",
            "127.0.0.1",
            "[::1]",
            "1.2.3.4:5678",
            "0.0.0.0:5678",
            "127.0.0.1:5678",
            "localhost:5678",
            "[::1]:8080"
          ]),
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_no_colon_in_value_name() {
    let app =
      runtime_args(Command::new("test_inspect_completion_value"), true, true);
    let inspect_args = app
      .get_arguments()
      .filter(|arg| arg.get_id() == "inspect")
      .collect::<Vec<_>>();
    // The value_name cannot have a : otherwise it breaks shell completions for zsh.
    let value_name = "HOST_AND_PORT";
    let arg = inspect_args
      .iter()
      .any(|v| v.get_value_names().unwrap() == [value_name]);

    assert_eq!(arg, true);
  }

  #[test]
  fn test_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "test", "--no-npm", "--no-remote", "--trace-leaks", "--no-run", "--filter", "- foo", "--coverage=cov", "--clean", "--location", "https:foo", "--allow-net", "--permit-no-files", "dir1/", "dir2/", "--", "arg1", "arg2"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: true,
          doc: false,
          fail_fast: None,
          filter: Some("- foo".to_string()),
          permit_no_files: true,
          files: FileFlags {
            include: vec!["dir1/".to_string(), "dir2/".to_string()],
            ignore: vec![],
          },
          shuffle: None,
          concurrent_jobs: None,
          trace_leaks: true,
          coverage_dir: Some("cov".to_string()),
          clean: true,
          watch: Default::default(),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        no_npm: true,
        no_remote: true,
        location: Some(Url::parse("https://foo/").unwrap()),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          allow_net: Some(vec![]),
          ..Default::default()
        },
        argv: svec!["arg1", "arg2"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_cafile() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--cert",
      "example.crt",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        ca_data: Some(CaData::File("example.crt".to_owned())),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_enable_testing_features() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--enable-testing-features-do-not-use",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        enable_testing_features: true,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_with_fail_fast() {
    let r = flags_from_vec(svec!["deno", "test", "--fail-fast=3"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: false,
          doc: false,
          fail_fast: Some(NonZeroUsize::new(3).unwrap()),
          filter: None,
          permit_no_files: false,
          shuffle: None,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          concurrent_jobs: None,
          trace_leaks: false,
          coverage_dir: None,
          clean: false,
          watch: Default::default(),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "test", "--fail-fast=0"]);
    assert!(r.is_err());
  }

  #[test]
  fn test_with_enable_testing_features() {
    let r = flags_from_vec(svec![
      "deno",
      "test",
      "--enable-testing-features-do-not-use"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: false,
          doc: false,
          fail_fast: None,
          filter: None,
          permit_no_files: false,
          shuffle: None,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          concurrent_jobs: None,
          trace_leaks: false,
          coverage_dir: None,
          clean: false,
          watch: Default::default(),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        enable_testing_features: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_reporter() {
    let r = flags_from_vec(svec!["deno", "test", "--reporter=pretty"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          reporter: TestReporterConfig::Pretty,
          ..Default::default()
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "test", "--reporter=dot"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          reporter: TestReporterConfig::Dot,
          ..Default::default()
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        log_level: Some(Level::Error),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "test", "--reporter=junit"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          reporter: TestReporterConfig::Junit,
          ..Default::default()
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "test", "--reporter=tap"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          reporter: TestReporterConfig::Tap,
          ..Default::default()
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        log_level: Some(Level::Error),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "test",
      "--reporter=dot",
      "--junit-path=report.xml"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          reporter: TestReporterConfig::Dot,
          junit_path: Some("report.xml".to_string()),
          ..Default::default()
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        log_level: Some(Level::Error),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "test", "--junit-path"]);
    assert!(r.is_err());
  }

  #[test]
  fn test_shuffle() {
    let r = flags_from_vec(svec!["deno", "test", "--shuffle=1"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: false,
          doc: false,
          fail_fast: None,
          filter: None,
          permit_no_files: false,
          shuffle: Some(1),
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          concurrent_jobs: None,
          trace_leaks: false,
          coverage_dir: None,
          clean: false,
          watch: Default::default(),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_watch() {
    let r = flags_from_vec(svec!["deno", "test", "--watch"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: false,
          doc: false,
          fail_fast: None,
          filter: None,
          permit_no_files: false,
          shuffle: None,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          concurrent_jobs: None,
          trace_leaks: false,
          coverage_dir: None,
          clean: false,
          watch: Some(Default::default()),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }
  #[test]
  fn test_watch_explicit_cwd() {
    let r = flags_from_vec(svec!["deno", "test", "--watch", "./"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: false,
          doc: false,
          fail_fast: None,
          filter: None,
          permit_no_files: false,
          shuffle: None,
          files: FileFlags {
            include: vec!["./".to_string()],
            ignore: vec![],
          },
          concurrent_jobs: None,
          trace_leaks: false,
          coverage_dir: None,
          clean: false,
          watch: Some(Default::default()),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_watch_with_no_clear_screen() {
    let r =
      flags_from_vec(svec!["deno", "test", "--watch", "--no-clear-screen"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: false,
          doc: false,
          fail_fast: None,
          filter: None,
          permit_no_files: false,
          shuffle: None,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          concurrent_jobs: None,
          trace_leaks: false,
          coverage_dir: None,
          clean: false,
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            no_clear_screen: true,
            exclude: vec![],
            paths: vec![],
          }),
          reporter: Default::default(),
          junit_path: None,
          hide_stacktraces: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_watch_with_paths() {
    let r = flags_from_vec(svec!("deno", "test", "--watch=foo"));

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("foo")],
            no_clear_screen: false,
            exclude: vec![],
          }),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "test", "--watch=foo,bar"]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("foo"), String::from("bar")],
            no_clear_screen: false,
            exclude: vec![],
          }),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_watch_with_excluded_paths() {
    let r =
      flags_from_vec(svec!("deno", "test", "--watch", "--watch-exclude=foo",));

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: false,
            exclude: vec![String::from("foo")],
          }),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!(
      "deno",
      "test",
      "--watch=foo",
      "--watch-exclude=bar",
    ));
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("foo")],
            no_clear_screen: false,
            exclude: vec![String::from("bar")],
          }),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "test",
      "--watch",
      "--watch-exclude=foo,bar",
    ]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![],
            no_clear_screen: false,
            exclude: vec![String::from("foo"), String::from("bar")],
          }),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "test",
      "--watch=foo,bar",
      "--watch-exclude=baz,qux",
    ]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          watch: Some(WatchFlagsWithPaths {
            hmr: false,
            paths: vec![String::from("foo"), String::from("bar")],
            no_clear_screen: false,
            exclude: vec![String::from("baz"), String::from("qux"),],
          }),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_coverage_default_dir() {
    let r = flags_from_vec(svec!["deno", "test", "--coverage"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          coverage_dir: Some("coverage".to_string()),
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn test_hide_stacktraces() {
    let r = flags_from_vec(svec!["deno", "test", "--hide-stacktraces"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          hide_stacktraces: true,
          ..TestFlags::default()
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn upgrade_with_ca_file() {
    let r = flags_from_vec(svec!["deno", "upgrade", "--cert", "example.crt"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
          force: false,
          dry_run: false,
          canary: false,
          release_candidate: false,
          version: None,
          output: None,
          version_or_hash_or_channel: None,
        }),
        ca_data: Some(CaData::File("example.crt".to_owned())),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn upgrade_release_candidate() {
    let r = flags_from_vec(svec!["deno", "upgrade", "--rc"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
          force: false,
          dry_run: false,
          canary: false,
          release_candidate: true,
          version: None,
          output: None,
          version_or_hash_or_channel: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "upgrade", "--rc", "--canary"]);
    assert!(r.is_err());

    let r = flags_from_vec(svec!["deno", "upgrade", "--rc", "--version"]);
    assert!(r.is_err());
  }

  #[test]
  fn cache_with_cafile() {
    let r = flags_from_vec(svec![
      "deno",
      "cache",
      "--cert",
      "example.crt",
      "script.ts",
      "script_two.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Cache(CacheFlags {
          files: svec!["script.ts", "script_two.ts"],
        }),
        ca_data: Some(CaData::File("example.crt".to_owned())),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn info_with_cafile() {
    let r = flags_from_vec(svec![
      "deno",
      "info",
      "--cert",
      "example.crt",
      "https://example.com"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Info(InfoFlags {
          json: false,
          file: Some("https://example.com".to_string()),
        }),
        ca_data: Some(CaData::File("example.crt".to_owned())),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn doc() {
    let r = flags_from_vec(svec!["deno", "doc", "--json", "path/to/module.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          json: true,
          html: None,
          lint: false,
          source_files: DocSourceFileFlag::Paths(svec!["path/to/module.ts"]),
          filter: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "doc", "--html", "path/to/module.ts"]);
    assert!(r.is_ok());

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--html",
      "--name=My library",
      "path/to/module.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          json: false,
          lint: false,
          html: Some(DocHtmlFlag {
            name: Some("My library".to_string()),
            category_docs_path: None,
            symbol_redirect_map_path: None,
            default_symbol_map_path: None,
            strip_trailing_html: false,
            output: String::from("./docs/"),
          }),
          source_files: DocSourceFileFlag::Paths(svec!["path/to/module.ts"]),
          filter: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--html",
      "--name=My library",
      "--lint",
      "--output=./foo",
      "path/to/module.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          json: false,
          html: Some(DocHtmlFlag {
            name: Some("My library".to_string()),
            category_docs_path: None,
            symbol_redirect_map_path: None,
            default_symbol_map_path: None,
            strip_trailing_html: false,
            output: String::from("./foo"),
          }),
          lint: true,
          source_files: DocSourceFileFlag::Paths(svec!["path/to/module.ts"]),
          filter: None,
        }),
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "doc", "--html", "--name=My library",]);
    assert!(r.is_err());

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--filter",
      "SomeClass.someField",
      "path/to/module.ts",
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          json: false,
          html: None,
          lint: false,
          source_files: DocSourceFileFlag::Paths(vec![
            "path/to/module.ts".to_string()
          ]),
          filter: Some("SomeClass.someField".to_string()),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "doc"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          json: false,
          html: None,
          lint: false,
          source_files: Default::default(),
          filter: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--filter",
      "Deno.Listener",
      "--builtin"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          lint: false,
          json: false,
          html: None,
          source_files: DocSourceFileFlag::Builtin,
          filter: Some("Deno.Listener".to_string()),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--no-npm",
      "--no-remote",
      "--private",
      "path/to/module.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: true,
          lint: false,
          json: false,
          html: None,
          source_files: DocSourceFileFlag::Paths(svec!["path/to/module.js"]),
          filter: None,
        }),
        no_npm: true,
        no_remote: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "path/to/module.js",
      "path/to/module2.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          lint: false,
          json: false,
          html: None,
          source_files: DocSourceFileFlag::Paths(vec![
            "path/to/module.js".to_string(),
            "path/to/module2.js".to_string()
          ]),
          filter: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "path/to/module.js",
      "--builtin",
      "path/to/module2.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          json: false,
          html: None,
          lint: false,
          source_files: DocSourceFileFlag::Paths(vec![
            "path/to/module.js".to_string(),
            "path/to/module2.js".to_string()
          ]),
          filter: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "doc", "--lint",]);
    assert!(r.is_err());

    let r = flags_from_vec(svec![
      "deno",
      "doc",
      "--lint",
      "path/to/module.js",
      "path/to/module2.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Doc(DocFlags {
          private: false,
          lint: true,
          json: false,
          html: None,
          source_files: DocSourceFileFlag::Paths(vec![
            "path/to/module.js".to_string(),
            "path/to/module2.js".to_string()
          ]),
          filter: None,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn inspect_default_host() {
    let r = flags_from_vec(svec!["deno", "run", "--inspect", "foo.js"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "foo.js".to_string(),
        )),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn inspect_wait() {
    let r = flags_from_vec(svec!["deno", "--inspect-wait", "foo.js"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "foo.js".to_string(),
          watch: None,
          bare: true,
        }),
        inspect_wait: Some("127.0.0.1:9229".parse().unwrap()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--inspect-wait=127.0.0.1:3567",
      "foo.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "foo.js".to_string(),
        )),
        inspect_wait: Some("127.0.0.1:3567".parse().unwrap()),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn compile() {
    let r = flags_from_vec(svec![
      "deno",
      "compile",
      "https://examples.deno.land/color-logging.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Compile(CompileFlags {
          source_file: "https://examples.deno.land/color-logging.ts"
            .to_string(),
          output: None,
          args: vec![],
          target: None,
          no_terminal: false,
          icon: None,
          include: vec![]
        }),
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn compile_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "compile", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--unsafely-ignore-certificate-errors", "--reload", "--lock", "lock.json", "--cert", "example.crt", "--cached-only", "--location", "https:foo", "--allow-read", "--allow-net", "--v8-flags=--help", "--seed", "1", "--no-terminal", "--icon", "favicon.ico", "--output", "colors", "--env=.example.env", "https://examples.deno.land/color-logging.ts", "foo", "bar", "-p", "8080"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Compile(CompileFlags {
          source_file: "https://examples.deno.land/color-logging.ts"
            .to_string(),
          output: Some(String::from("colors")),
          args: svec!["foo", "bar", "-p", "8080"],
          target: None,
          no_terminal: true,
          icon: Some(String::from("favicon.ico")),
          include: vec![]
        }),
        import_map_path: Some("import_map.json".to_string()),
        no_remote: true,
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        type_check_mode: TypeCheckMode::None,
        reload: true,
        lock: Some(String::from("lock.json")),
        ca_data: Some(CaData::File("example.crt".to_string())),
        cached_only: true,
        location: Some(Url::parse("https://foo/").unwrap()),
        permissions: PermissionFlags {
          allow_read: Some(vec![]),
          allow_net: Some(vec![]),
          ..Default::default()
        },
        unsafely_ignore_certificate_errors: Some(vec![]),
        v8_flags: svec!["--help", "--random-seed=1"],
        seed: Some(1),
        env_file: Some(".example.env".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn coverage() {
    let r = flags_from_vec(svec!["deno", "coverage", "foo.json"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Coverage(CoverageFlags {
          files: FileFlags {
            include: vec!["foo.json".to_string()],
            ignore: vec![],
          },
          include: vec![r"^file:".to_string()],
          exclude: vec![r"test\.(js|mjs|ts|jsx|tsx)$".to_string()],
          ..CoverageFlags::default()
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn coverage_with_lcov_and_out_file() {
    let r = flags_from_vec(svec![
      "deno",
      "coverage",
      "--lcov",
      "--output=foo.lcov",
      "foo.json"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Coverage(CoverageFlags {
          files: FileFlags {
            include: vec!["foo.json".to_string()],
            ignore: vec![],
          },
          include: vec![r"^file:".to_string()],
          exclude: vec![r"test\.(js|mjs|ts|jsx|tsx)$".to_string()],
          r#type: CoverageType::Lcov,
          output: Some(String::from("foo.lcov")),
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn coverage_with_default_files() {
    let r = flags_from_vec(svec!["deno", "coverage",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Coverage(CoverageFlags {
          files: FileFlags {
            include: vec!["coverage".to_string()],
            ignore: vec![],
          },
          include: vec![r"^file:".to_string()],
          exclude: vec![r"test\.(js|mjs|ts|jsx|tsx)$".to_string()],
          ..CoverageFlags::default()
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn location_with_bad_scheme() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "run", "--location", "foo:", "mod.ts"]);
    assert!(r.is_err());
    assert!(r
      .unwrap_err()
      .to_string()
      .contains("Expected protocol \"http\" or \"https\""));
  }

  #[test]
  fn test_config_path_args() {
    let flags = flags_from_vec(svec!["deno", "run", "foo.js"]).unwrap();
    let cwd = std::env::current_dir().unwrap();

    assert_eq!(flags.config_path_args(&cwd), Some(vec![cwd.clone()]));

    let flags = flags_from_vec(svec!["deno", "run", "sub_dir/foo.js"]).unwrap();
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(
      flags.config_path_args(&cwd),
      Some(vec![cwd.join("sub_dir").clone()])
    );

    let flags =
      flags_from_vec(svec!["deno", "https://example.com/foo.js"]).unwrap();
    assert_eq!(flags.config_path_args(&cwd), None);

    let flags =
      flags_from_vec(svec!["deno", "lint", "dir/a/a.js", "dir/b/b.js"])
        .unwrap();
    assert_eq!(
      flags.config_path_args(&cwd),
      Some(vec![cwd.join("dir/a/a.js"), cwd.join("dir/b/b.js")])
    );

    let flags = flags_from_vec(svec!["deno", "lint"]).unwrap();
    assert_eq!(flags.config_path_args(&cwd), Some(vec![cwd.clone()]));

    let flags = flags_from_vec(svec![
      "deno",
      "fmt",
      "dir/a/a.js",
      "dir/a/a2.js",
      "dir/b.js"
    ])
    .unwrap();
    assert_eq!(
      flags.config_path_args(&cwd),
      Some(vec![
        cwd.join("dir/a/a.js"),
        cwd.join("dir/a/a2.js"),
        cwd.join("dir/b.js")
      ])
    );
  }

  #[test]
  fn test_no_clear_watch_flag_without_watch_flag() {
    let r = flags_from_vec(svec!["deno", "run", "--no-clear-screen", "foo.js"]);
    assert!(r.is_err());
    let error_message = r.unwrap_err().to_string();
    assert!(&error_message
      .contains("error: the following required arguments were not provided:"));
    assert!(&error_message.contains("--watch[=<FILES>...]"));
  }

  #[test]
  fn task_subcommand() {
    let r = flags_from_vec(svec!["deno", "task", "build", "hello", "world",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
        }),
        argv: svec!["hello", "world"],
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "task", "build"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "task", "--cwd", "foo", "build"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: Some("foo".to_string()),
          task: Some("build".to_string()),
          is_run: false,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_subcommand_double_hyphen() {
    let r = flags_from_vec(svec![
      "deno",
      "task",
      "-c",
      "deno.json",
      "build",
      "--",
      "hello",
      "world",
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
        }),
        argv: svec!["--", "hello", "world"],
        config_flag: ConfigFlag::Path("deno.json".to_owned()),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno", "task", "--cwd", "foo", "build", "--", "hello", "world"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: Some("foo".to_string()),
          task: Some("build".to_string()),
          is_run: false,
        }),
        argv: svec!["--", "hello", "world"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_subcommand_double_hyphen_only() {
    // edge case, but it should forward
    let r = flags_from_vec(svec!["deno", "task", "build", "--"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
        }),
        argv: svec!["--"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_following_arg() {
    let r = flags_from_vec(svec!["deno", "task", "build", "-1", "--test"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
        }),
        argv: svec!["-1", "--test"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_following_double_hyphen_arg() {
    let r = flags_from_vec(svec!["deno", "task", "build", "--test"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
        }),
        argv: svec!["--test"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_with_global_flags() {
    // can fail if the custom parser in task_parse() starts at the wrong index
    let r = flags_from_vec(svec!["deno", "--quiet", "task", "build"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
          is_run: false,
        }),
        log_level: Some(log::Level::Error),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_subcommand_empty() {
    let r = flags_from_vec(svec!["deno", "task"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: None,
          is_run: false,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_subcommand_config() {
    let r = flags_from_vec(svec!["deno", "task", "--config", "deno.jsonc"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: None,
          is_run: false,
        }),
        config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_subcommand_config_short() {
    let r = flags_from_vec(svec!["deno", "task", "-c", "deno.jsonc"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: None,
          is_run: false,
        }),
        config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_subcommand_noconfig_invalid() {
    let r = flags_from_vec(svec!["deno", "task", "--no-config"]);
    assert_eq!(
      r.unwrap_err().kind(),
      clap::error::ErrorKind::UnknownArgument
    );
  }

  #[test]
  fn bench_with_flags() {
    let r = flags_from_vec(svec![
      "deno",
      "bench",
      "--json",
      "--no-npm",
      "--no-remote",
      "--no-run",
      "--filter",
      "- foo",
      "--location",
      "https:foo",
      "--allow-net",
      "dir1/",
      "dir2/",
      "--",
      "arg1",
      "arg2"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bench(BenchFlags {
          filter: Some("- foo".to_string()),
          json: true,
          no_run: true,
          files: FileFlags {
            include: vec!["dir1/".to_string(), "dir2/".to_string()],
            ignore: vec![],
          },
          watch: Default::default(),
        }),
        no_npm: true,
        no_remote: true,
        type_check_mode: TypeCheckMode::Local,
        location: Some(Url::parse("https://foo/").unwrap()),
        permissions: PermissionFlags {
          allow_net: Some(vec![]),
          no_prompt: true,
          ..Default::default()
        },
        argv: svec!["arg1", "arg2"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bench_watch() {
    let r = flags_from_vec(svec!["deno", "bench", "--watch"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bench(BenchFlags {
          filter: None,
          json: false,
          no_run: false,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          watch: Some(Default::default()),
        }),
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_check() {
    let r = flags_from_vec(svec!["deno", "run", "--check", "script.ts",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        type_check_mode: TypeCheckMode::Local,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "run", "--check=all", "script.ts",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        type_check_mode: TypeCheckMode::All,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "--check=foo", "script.ts",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
        }),
        type_check_mode: TypeCheckMode::None,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--no-check",
      "--check",
      "script.ts",
    ]);
    assert!(r.is_err());
  }

  #[test]
  fn no_config() {
    let r = flags_from_vec(svec!["deno", "run", "--no-config", "script.ts",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        config_flag: ConfigFlag::Disabled,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--config",
      "deno.json",
      "--no-config",
      "script.ts",
    ]);
    assert!(r.is_err());
  }

  #[test]
  fn init() {
    let r = flags_from_vec(svec!["deno", "init"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          dir: None,
          lib: false,
          serve: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "foo"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          dir: Some(String::from("foo")),
          lib: false,
          serve: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "--quiet"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          dir: None,
          lib: false,
          serve: false,
        }),
        log_level: Some(Level::Error),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "--lib"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          dir: None,
          lib: true,
          serve: false,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "--serve"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          dir: None,
          lib: false,
          serve: true,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "foo", "--lib"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          dir: Some(String::from("foo")),
          lib: true,
          serve: false,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn jupyter() {
    let r = flags_from_vec(svec!["deno", "jupyter"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Jupyter(JupyterFlags {
          install: false,
          kernel: false,
          conn_file: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "jupyter", "--install"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Jupyter(JupyterFlags {
          install: true,
          kernel: false,
          conn_file: None,
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "jupyter",
      "--kernel",
      "--conn",
      "path/to/conn/file"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Jupyter(JupyterFlags {
          install: false,
          kernel: true,
          conn_file: Some(String::from("path/to/conn/file")),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "jupyter",
      "--install",
      "--conn",
      "path/to/conn/file"
    ]);
    r.unwrap_err();
    let r = flags_from_vec(svec!["deno", "jupyter", "--kernel",]);
    r.unwrap_err();
    let r = flags_from_vec(svec!["deno", "jupyter", "--install", "--kernel",]);
    r.unwrap_err();
  }

  #[test]
  fn publish_args() {
    let r = flags_from_vec(svec![
      "deno",
      "publish",
      "--no-provenance",
      "--dry-run",
      "--allow-slow-types",
      "--allow-dirty",
      "--token=asdf",
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Publish(PublishFlags {
          token: Some("asdf".to_string()),
          dry_run: true,
          allow_slow_types: true,
          allow_dirty: true,
          no_provenance: true,
        }),
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn add_or_install_subcommand() {
    let r = flags_from_vec(svec!["deno", "add"]);
    r.unwrap_err();
    for cmd in ["add", "install"] {
      let mk_flags = |flags: AddFlags| -> Flags {
        match cmd {
          "add" => Flags {
            subcommand: DenoSubcommand::Add(flags),
            ..Flags::default()
          },
          "install" => Flags {
            subcommand: DenoSubcommand::Install(InstallFlags {
              kind: InstallKind::Local(InstallFlagsLocal::Add(flags)),
            }),
            ..Flags::default()
          },
          _ => unreachable!(),
        }
      };

      let r = flags_from_vec(svec!["deno", cmd, "@david/which"]);
      assert_eq!(
        r.unwrap(),
        mk_flags(AddFlags {
          packages: svec!["@david/which"],
          dev: false,
        }) // default is false
      );

      let r = flags_from_vec(svec!["deno", cmd, "@david/which", "@luca/hello"]);
      assert_eq!(
        r.unwrap(),
        mk_flags(AddFlags {
          packages: svec!["@david/which", "@luca/hello"],
          dev: false,
        })
      );

      let r = flags_from_vec(svec!["deno", cmd, "--dev", "npm:chalk"]);
      assert_eq!(
        r.unwrap(),
        mk_flags(AddFlags {
          packages: svec!["npm:chalk"],
          dev: true,
        }),
      );
    }
  }

  #[test]
  fn remove_subcommand() {
    let r = flags_from_vec(svec!["deno", "remove"]);
    r.unwrap_err();

    let r = flags_from_vec(svec!["deno", "remove", "@david/which"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Remove(RemoveFlags {
          packages: svec!["@david/which"],
        }),
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "remove", "@david/which", "@luca/hello"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Remove(RemoveFlags {
          packages: svec!["@david/which", "@luca/hello"],
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_with_frozen_lockfile() {
    let cases = [
      (Some("--frozen"), Some(true)),
      (Some("--frozen=true"), Some(true)),
      (Some("--frozen=false"), Some(false)),
      (None, None),
    ];
    for (flag, frozen) in cases {
      let mut args = svec!["deno", "run"];
      if let Some(f) = flag {
        args.push(f.into());
      }
      args.push("script.ts".into());
      let r = flags_from_vec(args);
      assert_eq!(
        r.unwrap(),
        Flags {
          subcommand: DenoSubcommand::Run(RunFlags::new_default(
            "script.ts".to_string(),
          )),
          frozen_lockfile: frozen,
          code_cache_enabled: true,
          ..Flags::default()
        }
      );
    }
  }

  #[test]
  fn allow_scripts() {
    let cases = [
      (Some("--allow-scripts"), Ok(PackagesAllowedScripts::All)),
      (None, Ok(PackagesAllowedScripts::None)),
      (
        Some("--allow-scripts=npm:foo"),
        Ok(PackagesAllowedScripts::Some(svec!["npm:foo"])),
      ),
      (
        Some("--allow-scripts=npm:foo,npm:bar"),
        Ok(PackagesAllowedScripts::Some(svec!["npm:foo", "npm:bar"])),
      ),
      (Some("--allow-scripts=foo"), Err("Invalid package")),
    ];
    for (flag, value) in cases {
      let mut args = svec!["deno", "cache"];
      if let Some(flag) = flag {
        args.push(flag.into());
      }
      args.push("script.ts".into());
      let r = flags_from_vec(args);
      match value {
        Ok(value) => {
          assert_eq!(
            r.unwrap(),
            Flags {
              subcommand: DenoSubcommand::Cache(CacheFlags {
                files: svec!["script.ts"],
              }),
              allow_scripts: value,
              ..Flags::default()
            }
          );
        }
        Err(e) => {
          let err = r.unwrap_err();
          assert!(
            err.to_string().contains(e),
            "expected to contain '{e}' got '{err}'"
          );
        }
      }
    }
  }

  #[test]
  fn bare_run() {
    let r = flags_from_vec(svec!["deno", "--no-config", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "script.ts".to_string(),
          watch: None,
          bare: true,
        }),
        config_flag: ConfigFlag::Disabled,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bare_global() {
    let r = flags_from_vec(svec!["deno", "--log-level=debug"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: None,
          is_default_command: true,
        }),
        log_level: Some(Level::Debug),
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn repl_user_args() {
    let r = flags_from_vec(svec!["deno", "repl", "foo"]);
    assert!(r.is_err());
    let r = flags_from_vec(svec!["deno", "repl", "--", "foo"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Repl(ReplFlags {
          eval_files: None,
          eval: None,
          is_default_command: false,
        }),
        argv: svec!["foo"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bare_with_flag_no_file() {
    let r = flags_from_vec(svec!["deno", "--no-config"]);

    let err = r.unwrap_err();
    assert!(err.to_string().contains("error: [SCRIPT_ARG] may only be omitted with --v8-flags=--help, else to use the repl with arguments, please use the `deno repl` subcommand"));
    assert!(err
      .to_string()
      .contains("Usage: deno [OPTIONS] [COMMAND] [SCRIPT_ARG]..."));
  }

  #[test]
  fn equal_help_output() {
    for command in clap_root().get_subcommands() {
      if command.get_name() == "help" {
        continue;
      }

      let long_flag = if let DenoSubcommand::Help(help) =
        flags_from_vec(svec!["deno", command.get_name(), "--help"])
          .unwrap()
          .subcommand
      {
        help.help.to_string()
      } else {
        unreachable!()
      };
      let short_flag = if let DenoSubcommand::Help(help) =
        flags_from_vec(svec!["deno", command.get_name(), "-h"])
          .unwrap()
          .subcommand
      {
        help.help.to_string()
      } else {
        unreachable!()
      };
      let subcommand = if let DenoSubcommand::Help(help) =
        flags_from_vec(svec!["deno", "help", command.get_name()])
          .unwrap()
          .subcommand
      {
        help.help.to_string()
      } else {
        unreachable!()
      };
      assert_eq!(long_flag, short_flag, "{} subcommand", command.get_name());
      assert_eq!(long_flag, subcommand, "{} subcommand", command.get_name());
    }
  }

  #[test]
  fn install_permissions_non_global() {
    let r =
      flags_from_vec(svec!["deno", "install", "--allow-net", "jsr:@std/fs"]);

    assert!(r
      .unwrap_err()
      .to_string()
      .contains("Note: Permission flags can only be used in a global setting"));
  }

  #[test]
  fn jupyter_unstable_flags() {
    let r = flags_from_vec(svec![
      "deno",
      "jupyter",
      "--unstable-ffi",
      "--unstable-bare-node-builtins",
      "--unstable-worker-options"
    ]);

    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Jupyter(JupyterFlags {
          install: false,
          kernel: false,
          conn_file: None,
        }),
        unstable_config: UnstableConfig {
          bare_node_builtins: true,
          sloppy_imports: false,
          features: svec!["ffi", "worker-options"],
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn serve_with_allow_all() {
    let r = flags_from_vec(svec!["deno", "serve", "--allow-all", "./main.ts"]);
    let flags = r.unwrap();
    assert_eq!(
      &flags,
      &Flags {
        subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
          "./main.ts".into(),
          8000,
          "0.0.0.0"
        )),
        permissions: PermissionFlags {
          allow_all: true,
          allow_net: None,
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Default::default()
      }
    );
    // just make sure this doesn't panic
    let _ = flags.permissions.to_options(&[]);
  }

  #[test]
  fn escape_and_split_commas_test() {
    assert_eq!(escape_and_split_commas("foo".to_string()).unwrap(), ["foo"]);
    assert!(escape_and_split_commas("foo,".to_string()).is_err());
    assert_eq!(
      escape_and_split_commas("foo,,".to_string()).unwrap(),
      ["foo,"]
    );
    assert!(escape_and_split_commas("foo,,,".to_string()).is_err());
    assert_eq!(
      escape_and_split_commas("foo,,,,".to_string()).unwrap(),
      ["foo,,"]
    );
    assert_eq!(
      escape_and_split_commas("foo,bar".to_string()).unwrap(),
      ["foo", "bar"]
    );
    assert_eq!(
      escape_and_split_commas("foo,,bar".to_string()).unwrap(),
      ["foo,bar"]
    );
    assert_eq!(
      escape_and_split_commas("foo,,,bar".to_string()).unwrap(),
      ["foo,", "bar"]
    );
  }

  #[test]
  fn net_flag_with_url() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--allow-net=https://example.com",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap_err().to_string(),
      "error: invalid value 'https://example.com': URLs are not supported, only domains and ips"
    );
  }

  #[test]
  fn node_modules_dir_default() {
    let r =
      flags_from_vec(svec!["deno", "run", "--node-modules-dir", "./foo.ts"]);
    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags {
          script: "./foo.ts".into(),
          ..Default::default()
        }),
        node_modules_dir: Some(NodeModulesDirMode::Auto),
        code_cache_enabled: true,
        ..Default::default()
      }
    )
  }

  #[test]
  fn flag_before_subcommand() {
    let r = flags_from_vec(svec!["deno", "--allow-net", "repl"]);
    assert_eq!(
      r.unwrap_err().to_string(),
      "error: unexpected argument '--allow-net' found

  tip: 'repl --allow-net' exists

Usage: deno repl [OPTIONS] [-- [ARGS]...]\n"
    )
  }

  #[test]
  fn test_allow_import_host_from_url() {
    fn parse(text: &str) -> Option<String> {
      allow_import_host_from_url(&Url::parse(text).unwrap())
    }

    assert_eq!(parse("https://jsr.io"), None);
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
  fn allow_all_conflicts_allow_perms() {
    let flags = [
      "--allow-read",
      "--allow-write",
      "--allow-net",
      "--allow-env",
      "--allow-run",
      "--allow-sys",
      "--allow-ffi",
      "--allow-import",
    ];
    for flag in flags {
      let r =
        flags_from_vec(svec!["deno", "run", "--allow-all", flag, "foo.ts"]);
      assert!(r.is_err());
    }
  }
}
