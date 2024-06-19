// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use clap::builder::styling::AnsiColor;
use clap::builder::FalseyValueParser;
use clap::value_parser;
use clap::Arg;
use clap::ArgAction;
use clap::ArgMatches;
use clap::ColorChoice;
use clap::Command;
use clap::ValueHint;
use deno_config::glob::PathOrPatternSet;
use deno_config::ConfigFlag;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::url::Url;
use deno_graph::GraphKind;
use deno_runtime::deno_permissions::parse_sys_kind;
use deno_runtime::deno_permissions::PermissionsOptions;
use log::debug;
use log::Level;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::ffi::OsString;
use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::num::NonZeroU8;
use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use crate::args::resolve_no_prompt;
use crate::util::fs::canonicalize_path;

use super::flags_net;
use super::DENO_FUTURE;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileFlags {
  pub ignore: Vec<String>,
  pub include: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AddFlags {
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
pub struct BundleFlags {
  pub source_file: String,
  pub out_file: Option<String>,
  pub watch: Option<WatchFlags>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheFlags {
  pub files: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckFlags {
  pub files: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileFlags {
  pub source_file: String,
  pub output: Option<String>,
  pub args: Vec<String>,
  pub target: Option<String>,
  pub no_terminal: bool,
  pub include: Vec<String>,
  pub eszip: bool,
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

#[derive(Clone, Debug, Eq, PartialEq)]
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
  #[allow(unused)]
  Local(Option<AddFlags>),
  Global(InstallFlagsGlobal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallFlags {
  pub global: bool,
  pub kind: InstallKind,
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
  #[allow(unused)]
  Local,
  Global(UninstallFlagsGlobal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallFlags {
  pub global: bool,
  pub kind: UninstallKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
}

impl RunFlags {
  #[cfg(test)]
  pub fn new_default(script: String) -> Self {
    Self {
      script,
      watch: None,
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
}

impl ServeFlags {
  #[cfg(test)]
  pub fn new_default(script: String, port: u16, host: &str) -> Self {
    Self {
      script,
      watch: None,
      port,
      host: host.to_owned(),
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
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
  pub allow_none: bool,
  pub filter: Option<String>,
  pub shuffle: Option<u64>,
  pub concurrent_jobs: Option<NonZeroUsize>,
  pub trace_leaks: bool,
  pub watch: Option<WatchFlags>,
  pub reporter: TestReporterConfig,
  pub junit_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeFlags {
  pub dry_run: bool,
  pub force: bool,
  pub canary: bool,
  pub version: Option<String>,
  pub output: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VendorFlags {
  pub specifiers: Vec<String>,
  pub output_path: Option<String>,
  pub force: bool,
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
pub enum DenoSubcommand {
  Add(AddFlags),
  Bench(BenchFlags),
  Bundle(BundleFlags),
  Cache(CacheFlags),
  Check(CheckFlags),
  Compile(CompileFlags),
  Completions(CompletionsFlags),
  Coverage(CoverageFlags),
  Doc(DocFlags),
  Eval(EvalFlags),
  Fmt(FmtFlags),
  Init(InitFlags),
  Info(InfoFlags),
  Install(InstallFlags),
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
  Vendor(VendorFlags),
  Publish(PublishFlags),
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

#[derive(
  Clone, Default, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize,
)]
pub struct UnstableConfig {
  pub legacy_flag_enabled: bool, // --unstable
  pub bare_node_builtins: bool,  // --unstable-bare-node-builts
  pub byonm: bool,
  pub sloppy_imports: bool,
  pub features: Vec<String>, // --unstabe-kv --unstable-cron
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct Flags {
  /// Vector of CLI arguments - these are user script arguments, all Deno
  /// specific flags are removed.
  pub argv: Vec<String>,
  pub subcommand: DenoSubcommand,

  pub ca_stores: Option<Vec<String>>,
  pub ca_data: Option<CaData>,
  pub cache_blocklist: Vec<String>,
  /// This is not exposed as an option in the CLI, it is used internally when
  /// the language server is configured with an explicit cache option.
  pub cache_path: Option<PathBuf>,
  pub cached_only: bool,
  pub type_check_mode: TypeCheckMode,
  pub config_flag: ConfigFlag,
  pub node_modules_dir: Option<bool>,
  pub vendor: Option<bool>,
  pub enable_op_summary_metrics: bool,
  pub enable_testing_features: bool,
  pub ext: Option<String>,
  pub ignore: Vec<String>,
  pub import_map_path: Option<String>,
  pub env_file: Option<String>,
  pub inspect_brk: Option<SocketAddr>,
  pub inspect_wait: Option<SocketAddr>,
  pub inspect: Option<SocketAddr>,
  pub location: Option<Url>,
  pub lock_write: bool,
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
  pub eszip: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct PermissionFlags {
  pub allow_all: bool,
  pub allow_env: Option<Vec<String>>,
  pub deny_env: Option<Vec<String>>,
  pub allow_hrtime: bool,
  pub deny_hrtime: bool,
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
}

impl PermissionFlags {
  pub fn has_permission(&self) -> bool {
    self.allow_all
      || self.allow_env.is_some()
      || self.deny_env.is_some()
      || self.allow_hrtime
      || self.deny_hrtime
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
  }

  pub fn to_options(
    &self,
    // will be None when `deno compile` can't resolve the cwd
    initial_cwd: Option<&Path>,
  ) -> Result<PermissionsOptions, AnyError> {
    fn convert_option_str_to_path_buf(
      flag: &Option<Vec<String>>,
      initial_cwd: Option<&Path>,
    ) -> Result<Option<Vec<PathBuf>>, AnyError> {
      let Some(paths) = &flag else {
        return Ok(None);
      };

      let mut new_paths = Vec::with_capacity(paths.len());
      for path in paths {
        if let Some(initial_cwd) = initial_cwd {
          new_paths.push(initial_cwd.join(path))
        } else {
          let path = PathBuf::from(path);
          if path.is_absolute() {
            new_paths.push(path);
          } else {
            bail!("Could not resolve relative permission path '{}' when current working directory could not be resolved.", path.display())
          }
        }
      }
      Ok(Some(new_paths))
    }

    Ok(PermissionsOptions {
      allow_all: self.allow_all,
      allow_env: self.allow_env.clone(),
      deny_env: self.deny_env.clone(),
      allow_hrtime: self.allow_hrtime,
      deny_hrtime: self.deny_hrtime,
      allow_net: self.allow_net.clone(),
      deny_net: self.deny_net.clone(),
      allow_ffi: convert_option_str_to_path_buf(&self.allow_ffi, initial_cwd)?,
      deny_ffi: convert_option_str_to_path_buf(&self.deny_ffi, initial_cwd)?,
      allow_read: convert_option_str_to_path_buf(
        &self.allow_read,
        initial_cwd,
      )?,
      deny_read: convert_option_str_to_path_buf(&self.deny_read, initial_cwd)?,
      allow_run: self.allow_run.clone(),
      deny_run: self.deny_run.clone(),
      allow_sys: self.allow_sys.clone(),
      deny_sys: self.deny_sys.clone(),
      allow_write: convert_option_str_to_path_buf(
        &self.allow_write,
        initial_cwd,
      )?,
      deny_write: convert_option_str_to_path_buf(
        &self.deny_write,
        initial_cwd,
      )?,
      prompt: !resolve_no_prompt(self),
    })
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

    if self.permissions.allow_hrtime {
      args.push("--allow-hrtime".to_string());
    }

    if self.permissions.deny_hrtime {
      args.push("--deny-hrtime".to_string());
    }

    args
  }

  /// Extract path arguments for config search paths.
  /// If it returns Some(vec), the config should be discovered
  /// from the passed `current_dir` after trying to discover from each entry in
  /// the returned vector.
  /// If it returns None, the config file shouldn't be discovered at all.
  pub fn config_path_args(&self, current_dir: &Path) -> Option<Vec<PathBuf>> {
    use DenoSubcommand::*;

    match &self.subcommand {
      Fmt(FmtFlags { files, .. }) => {
        Some(files.include.iter().map(|p| current_dir.join(p)).collect())
      }
      Lint(LintFlags { files, .. }) => {
        Some(files.include.iter().map(|p| current_dir.join(p)).collect())
      }
      Run(RunFlags { script, .. }) => {
        if let Ok(module_specifier) = resolve_url_or_path(script, current_dir) {
          if module_specifier.scheme() == "file"
            || module_specifier.scheme() == "npm"
          {
            if let Ok(p) = module_specifier.to_file_path() {
              Some(vec![p])
            } else {
              Some(vec![])
            }
          } else {
            // When the entrypoint doesn't have file: scheme (it's the remote
            // script), then we don't auto discover config file.
            None
          }
        } else {
          Some(vec![])
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
          Err(_) => Some(vec![]),
        }
      }
      _ => Some(vec![]),
    }
  }

  /// Extract path argument for `package.json` search paths.
  /// If it returns Some(path), the `package.json` should be discovered
  /// from the `path` dir.
  /// If it returns None, the `package.json` file shouldn't be discovered at
  /// all.
  pub fn package_json_search_dir(&self, current_dir: &Path) -> Option<PathBuf> {
    use DenoSubcommand::*;

    match &self.subcommand {
      Run(RunFlags { script, .. }) | Serve(ServeFlags { script, .. }) => {
        let module_specifier = resolve_url_or_path(script, current_dir).ok()?;
        if module_specifier.scheme() == "file" {
          let p = module_specifier
            .to_file_path()
            .unwrap()
            .parent()?
            .to_owned();
          Some(p)
        } else if module_specifier.scheme() == "npm" {
          Some(current_dir.to_path_buf())
        } else {
          None
        }
      }
      Task(TaskFlags { cwd: Some(cwd), .. }) => {
        resolve_url_or_path(cwd, current_dir)
          .ok()?
          .to_file_path()
          .ok()
      }
      Task(_) | Check(_) | Coverage(_) | Cache(_) | Info(_) | Eval(_)
      | Test(_) | Bench(_) | Repl(_) | Compile(_) | Publish(_) => {
        Some(current_dir.to_path_buf())
      }
      Add(_) | Bundle(_) | Completions(_) | Doc(_) | Fmt(_) | Init(_)
      | Uninstall(_) | Jupyter(_) | Lsp | Lint(_) | Types | Upgrade(_)
      | Vendor(_) => None,
      Install(_) => {
        if *DENO_FUTURE {
          Some(current_dir.to_path_buf())
        } else {
          None
        }
      }
    }
  }

  pub fn has_permission(&self) -> bool {
    self.permissions.has_permission()
  }

  pub fn has_permission_in_argv(&self) -> bool {
    self.argv.iter().any(|arg| {
      arg == "--allow-all"
        || arg == "--allow-hrtime"
        || arg == "--deny-hrtime"
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
    self.permissions.allow_read = Some(vec![]);
    self.permissions.allow_env = Some(vec![]);
    self.permissions.allow_net = Some(vec![]);
    self.permissions.allow_run = Some(vec![]);
    self.permissions.allow_write = Some(vec![]);
    self.permissions.allow_sys = Some(vec![]);
    self.permissions.allow_ffi = Some(vec![]);
    self.permissions.allow_hrtime = true;
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
    | DenoSubcommand::Bundle(BundleFlags {
      watch:
        Some(WatchFlags {
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
        Some(WatchFlags {
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

static ENV_VARIABLES_HELP: &str = color_print::cstr!(
  r#"<y>Environment variables:</>
    <g>DENO_AUTH_TOKENS</>     A semi-colon separated list of bearer tokens and
                         hostnames to use when fetching remote modules from
                         private repositories
                         (e.g. "abcde12345@deno.land;54321edcba@github.com")

    <g>DENO_FUTURE</>          Set to "1" to enable APIs that will take effect in
                         Deno 2

    <g>DENO_CERT</>            Load certificate authorities from PEM encoded file

    <g>DENO_DIR</>             Set the cache directory

    <g>DENO_INSTALL_ROOT</>    Set deno install's output directory
                         (defaults to $HOME/.deno/bin)

    <g>DENO_JOBS</>            Number of parallel workers used for the --parallel
                         flag with the test subcommand. Defaults to number
                         of available CPUs.

    <g>DENO_REPL_HISTORY</>    Set REPL history file path
                         History file is disabled when the value is empty
                         (defaults to $DENO_DIR/deno_history.txt)

    <g>DENO_NO_PACKAGE_JSON</> Disables auto-resolution of package.json

    <g>DENO_NO_PROMPT</>       Set to disable permission prompts on access
                         (alternative to passing --no-prompt on invocation)

    <g>DENO_NO_UPDATE_CHECK</> Set to disable checking if a newer Deno version is
                         available

    <g>DENO_TLS_CA_STORE</>    Comma-separated list of order dependent certificate
                         stores. Possible values: "system", "mozilla".
                         Defaults to "mozilla".

    <g>DENO_V8_FLAGS</>        Set V8 command line options

    <g>DENO_WEBGPU_TRACE</>    Directory to use for wgpu traces

    <g>HTTP_PROXY</>           Proxy address for HTTP requests
                         (module downloads, fetch)

    <g>HTTPS_PROXY</>          Proxy address for HTTPS requests
                         (module downloads, fetch)

    <g>NO_COLOR</>             Set to disable color

    <g>NO_PROXY</>             Comma-separated list of hosts which do not use a proxy
                         (module downloads, fetch)

    <g>NPM_CONFIG_REGISTRY</>  URL to use for the npm registry."#
);

static DENO_HELP: &str = concat!(
  color_print::cstr!("<g>A modern JavaScript and TypeScript runtime</>"),
  "

Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  color_print::cstr!(
    "
Standard Library: https://jsr.io/@std
Modules: https://jsr.io/ https://deno.land/x/
Bugs: https://github.com/denoland/deno/issues

To start the REPL:

  <g>deno</>

To execute a script:

  <g>deno run https://examples.deno.land/hello-world.ts</>

To evaluate code in the shell:

  <g>deno eval \"console.log(30933 + 404)\"</>
"
  )
);

/// Main entry point for parsing deno's command line flags.
pub fn flags_from_vec(args: Vec<OsString>) -> clap::error::Result<Flags> {
  let mut app = clap_root();
  let mut matches = app.try_get_matches_from_mut(&args)?;

  let mut flags = Flags::default();

  if matches.get_flag("unstable") {
    flags.unstable_config.legacy_flag_enabled = true;
  }

  for (name, _, _) in crate::UNSTABLE_GRANULAR_FLAGS {
    if matches.get_flag(&format!("unstable-{}", name)) {
      flags.unstable_config.features.push(name.to_string());
    }
  }

  flags.unstable_config.bare_node_builtins =
    matches.get_flag("unstable-bare-node-builtins");
  flags.unstable_config.byonm = matches.get_flag("unstable-byonm");
  flags.unstable_config.sloppy_imports =
    matches.get_flag("unstable-sloppy-imports");

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

  if let Some((subcommand, mut m)) = matches.remove_subcommand() {
    match subcommand.as_str() {
      "add" => add_parse(&mut flags, &mut m),
      "bench" => bench_parse(&mut flags, &mut m),
      "bundle" => bundle_parse(&mut flags, &mut m),
      "cache" => cache_parse(&mut flags, &mut m),
      "check" => check_parse(&mut flags, &mut m),
      "compile" => compile_parse(&mut flags, &mut m),
      "completions" => completions_parse(&mut flags, &mut m, app),
      "coverage" => coverage_parse(&mut flags, &mut m),
      "doc" => doc_parse(&mut flags, &mut m),
      "eval" => eval_parse(&mut flags, &mut m),
      "fmt" => fmt_parse(&mut flags, &mut m),
      "init" => init_parse(&mut flags, &mut m),
      "info" => info_parse(&mut flags, &mut m),
      "install" => install_parse(&mut flags, &mut m),
      "jupyter" => jupyter_parse(&mut flags, &mut m),
      "lint" => lint_parse(&mut flags, &mut m),
      "lsp" => lsp_parse(&mut flags, &mut m),
      "repl" => repl_parse(&mut flags, &mut m),
      "run" => run_parse(&mut flags, &mut m, app)?,
      "serve" => serve_parse(&mut flags, &mut m, app)?,
      "task" => task_parse(&mut flags, &mut m),
      "test" => test_parse(&mut flags, &mut m),
      "types" => types_parse(&mut flags, &mut m),
      "uninstall" => uninstall_parse(&mut flags, &mut m),
      "upgrade" => upgrade_parse(&mut flags, &mut m),
      "vendor" => vendor_parse(&mut flags, &mut m),
      "publish" => publish_parse(&mut flags, &mut m),
      _ => unreachable!(),
    }
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

  Ok(flags)
}

fn handle_repl_flags(flags: &mut Flags, repl_flags: ReplFlags) {
  // If user runs just `deno` binary we enter REPL and allow all permissions.
  if repl_flags.is_default_command {
    flags.permissions.allow_net = Some(vec![]);
    flags.permissions.allow_env = Some(vec![]);
    flags.permissions.allow_run = Some(vec![]);
    flags.permissions.allow_read = Some(vec![]);
    flags.permissions.allow_sys = Some(vec![]);
    flags.permissions.allow_write = Some(vec![]);
    flags.permissions.allow_ffi = Some(vec![]);
    flags.permissions.allow_hrtime = true;
  }
  flags.subcommand = DenoSubcommand::Repl(repl_flags);
}

fn clap_root() -> Command {
  let long_version = format!(
    "{} ({}, {})\nv8 {}\ntypescript {}",
    crate::version::deno(),
    if crate::version::is_canary() {
      "canary"
    } else {
      env!("PROFILE")
    },
    env!("TARGET"),
    deno_core::v8_version(),
    crate::version::TYPESCRIPT
  );

  let mut cmd = Command::new("deno")
    .bin_name("deno")
    .styles(
      clap::builder::Styles::styled()
        .header(AnsiColor::Yellow.on_default())
        .usage(AnsiColor::White.on_default())
        .literal(AnsiColor::Green.on_default())
        .placeholder(AnsiColor::Green.on_default())
    )
    .color(ColorChoice::Auto)
    .max_term_width(80)
    .version(crate::version::deno())
    .long_version(long_version)
    // cause --unstable flags to display at the bottom of the help text
    .next_display_order(1000)
    .disable_version_flag(true)
    .arg(
      Arg::new("version")
        .short('V')
        .short_alias('v')
        .long("version")
        .action(ArgAction::Version)
        .help("Print version")
    )
    .arg(
      Arg::new("unstable")
        .long("unstable")
        .help("Enable unstable features and APIs")
        .action(ArgAction::SetTrue)
        .global(true),
    )
    .arg(
      Arg::new("unstable-bare-node-builtins")
        .long("unstable-bare-node-builtins")
        .help("Enable unstable bare node builtins feature")
        .env("DENO_UNSTABLE_BARE_NODE_BUILTINS")
        .value_parser(FalseyValueParser::new())
        .action(ArgAction::SetTrue)
        .global(true),
    )
    .arg(
      Arg::new("unstable-byonm")
        .long("unstable-byonm")
        .help("Enable unstable 'bring your own node_modules' feature")
        .env("DENO_UNSTABLE_BYONM")
        .value_parser(FalseyValueParser::new())
        .action(ArgAction::SetTrue)
        .global(true),
    )
    .arg(
      Arg::new("unstable-sloppy-imports")
        .long("unstable-sloppy-imports")
        .help(
          "Enable unstable resolving of specifiers by extension probing, .js to .ts, and directory probing.",
        )
        .env("DENO_UNSTABLE_SLOPPY_IMPORTS")
        .value_parser(FalseyValueParser::new())
        .action(ArgAction::SetTrue)
        .global(true),
    );

  for (flag_name, help, _) in crate::UNSTABLE_GRANULAR_FLAGS {
    cmd = cmd.arg(
      Arg::new(format!("unstable-{}", flag_name))
        .long(format!("unstable-{}", flag_name))
        .help(help)
        .action(ArgAction::SetTrue)
        .global(true),
    );
  }

  cmd
    // reset the display order after the unstable flags
    .next_display_order(0)
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
      cmd
        .subcommand(add_subcommand())
        .subcommand(bench_subcommand())
        .subcommand(bundle_subcommand())
        .subcommand(cache_subcommand())
        .subcommand(check_subcommand())
        .subcommand(compile_subcommand())
        .subcommand(completions_subcommand())
        .subcommand(coverage_subcommand())
        .subcommand(doc_subcommand())
        .subcommand(eval_subcommand())
        .subcommand(fmt_subcommand())
        .subcommand(init_subcommand())
        .subcommand(info_subcommand())
        .subcommand(if *DENO_FUTURE {
          future_install_subcommand()
        } else {
          install_subcommand()
        })
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
        .subcommand(vendor_subcommand())
    })
    .long_about(DENO_HELP)
    .after_help(ENV_VARIABLES_HELP)
}

fn add_subcommand() -> Command {
  Command::new("add")
    .about("Add dependencies")
    .long_about(
      "Add dependencies to the configuration file.

  deno add @std/path

You can add multiple dependencies at once:

  deno add @std/path @std/assert
",
    )
    .defer(|cmd| {
      cmd.arg(
        Arg::new("packages")
          .help("List of packages to add")
          .required(true)
          .num_args(1..)
          .action(ArgAction::Append),
      )
    })
}

fn bench_subcommand() -> Command {
  Command::new("bench")
    .about("Run benchmarks")
    .long_about(
      "Run benchmarks using Deno's built-in bench tool.

Evaluate the given modules, run all benches declared with 'Deno.bench()'
and report results to standard output:

  deno bench src/fetch_bench.ts src/signal_bench.ts

Directory arguments are expanded to all contained files matching the
glob {*_,*.,}bench.{js,mjs,ts,mts,jsx,tsx}:

  deno bench src/",
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
            .use_value_delimiter(true)
            .require_equals(true)
            .help("Ignore files"),
        )
        .arg(
          Arg::new("filter")
            .long("filter")
            .allow_hyphen_values(true)
            .help(
              "Run benchmarks with this string or pattern in the bench name",
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
    })
}

fn bundle_subcommand() -> Command {
  Command::new("bundle")
    .about("Bundle module and dependencies into single file")
    .long_about(
      "⚠️ Warning: `deno bundle` is deprecated and will be removed in Deno 2.0.
Use an alternative bundler like \"deno_emit\", \"esbuild\" or \"rollup\" instead.

Output a single JavaScript file with all dependencies.

  deno bundle jsr:@std/http/file-server file_server.bundle.js

If no output file is given, the output is written to standard output:

  deno bundle jsr:@std/http/file-server",
    )
    .defer(|cmd| {
      compile_args(cmd)
        .hide(true)
        .arg(check_arg(true))
        .arg(
          Arg::new("source_file")
            .required(true)
            .value_hint(ValueHint::FilePath),
        )
        .arg(Arg::new("out_file").value_hint(ValueHint::FilePath))
        .arg(watch_arg(false))
        .arg(watch_exclude_arg())
        .arg(no_clear_screen_arg())
        .arg(executable_ext_arg())
    })
}

fn cache_subcommand() -> Command {
  Command::new("cache")
    .about("Cache the dependencies")
    .long_about(
      "Cache and compile remote dependencies recursively.

Download and compile a module with all of its static dependencies and save
them in the local cache, without running any code:

  deno cache jsr:@std/http/file-server

Future runs of this module will trigger no downloads or compilation unless
--reload is specified.",
    )
    .defer(|cmd| {
      compile_args(cmd).arg(check_arg(false)).arg(
        Arg::new("file")
          .num_args(1..)
          .required(true)
          .value_hint(ValueHint::FilePath),
      )
    })
}

fn check_subcommand() -> Command {
  Command::new("check")
      .about("Type-check the dependencies")
      .long_about(
        "Download and type-check without execution.

  deno check jsr:@std/http/file-server

Unless --reload is specified, this command will not re-download already cached dependencies.",
      )
    .defer(|cmd| compile_args_without_check_args(cmd).arg(
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
          .help("Type-check all modules, including remote")
          .action(ArgAction::SetTrue)
          .conflicts_with("no-remote")
          .hide(true)
      )
      .arg(
        Arg::new("file")
          .num_args(1..)
          .required(true)
          .value_hint(ValueHint::FilePath),
      )
    )
}

fn compile_subcommand() -> Command {
  Command::new("compile")
    .about("Compile the script into a self contained executable")
    .long_about(
      "Compiles the given script into a self contained executable.

  deno compile -A jsr:@std/http/file-server
  deno compile --output file_server jsr:@std/http/file-server

Any flags passed which affect runtime behavior, such as '--unstable',
'--allow-*', '--v8-flags', etc. are encoded into the output executable and
used at runtime as if they were passed to a similar 'deno run' command.

The executable name is inferred by default: Attempt to take the file stem of
the URL path. The above example would become 'file_server'. If the file stem
is something generic like 'main', 'mod', 'index' or 'cli', and the path has no
parent, take the file name of the parent path. Otherwise settle with the
generic name. If the resulting name has an '@...' suffix, strip it.

Cross-compiling to different target architectures is supported using the
`--target` flag. On the first invocation with deno will download proper
binary and cache it in $DENO_DIR. The aarch64-apple-darwin target is not
supported in canary.
",
    )
    .defer(|cmd| {
      runtime_args(cmd, true, false)
      .arg(check_arg(true))
      .arg(
        Arg::new("include")
          .long("include")
          .help("Additional module to include in the module graph")
          .long_help(
            "Includes an additional module in the compiled executable's module
    graph. Use this flag if a dynamically imported module or a web worker main
    module fails to load in the executable. This flag can be passed multiple
    times, to include multiple additional modules.",
          )
          .action(ArgAction::Append)
          .value_hint(ValueHint::FilePath),
      )
      .arg(
        Arg::new("output")
          .long("output")
          .short('o')
          .value_parser(value_parser!(String))
          .help("Output file (defaults to $PWD/<inferred-name>)")
          .value_hint(ValueHint::FilePath),
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
          ]),
      )
      .arg(
        Arg::new("no-terminal")
          .long("no-terminal")
          .help("Hide terminal on Windows")
          .action(ArgAction::SetTrue),
      )
      .arg(executable_ext_arg())
      .arg(env_file_arg())
      .arg(script_arg().required(true).trailing_var_arg(true))
    })
}

fn completions_subcommand() -> Command {
  Command::new("completions")
    .about("Generate shell completions")
    .long_about(
      "Output shell completion script to standard output.

  deno completions bash > /usr/local/etc/bash_completion.d/deno.bash
  source /usr/local/etc/bash_completion.d/deno.bash",
    )
    .defer(|cmd| {
      cmd.disable_help_subcommand(true).arg(
        Arg::new("shell")
          .value_parser(["bash", "fish", "powershell", "zsh", "fig"])
          .required(true),
      )
    })
}

fn coverage_subcommand() -> Command {
  Command::new("coverage")
    .about("Print coverage reports")
    .long_about(
      "Print coverage reports from coverage profiles.

Collect a coverage profile with deno test:

  deno test --coverage=cov_profile

Print a report to stdout:

  deno coverage cov_profile

Include urls that start with the file schema:

  deno coverage --include=\"^file:\" cov_profile

Exclude urls ending with test.ts and test.js:

  deno coverage --exclude=\"test\\.(ts|js)\" cov_profile

Include urls that start with the file schema and exclude files ending with
test.ts and test.js, for an url to match it must match the include pattern and
not match the exclude pattern:

  deno coverage --include=\"^file:\" --exclude=\"test\\.(ts|js)\" cov_profile

Write a report using the lcov format:

  deno coverage --lcov --output=cov.lcov cov_profile/

Generate html reports from lcov:

  genhtml -o html_cov cov.lcov
",
    )
    .defer(|cmd| {
      cmd
        .arg(
          Arg::new("ignore")
            .long("ignore")
            .num_args(1..)
            .use_value_delimiter(true)
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
            .help("Output file (defaults to stdout) for lcov")
            .long_help(
              "Exports the coverage report in lcov format to the given file.
    Filename should be passed along with '=' For example '--output=foo.lcov'
    If no --output arg is specified then the report is written to stdout.",
            )
            .require_equals(true)
            .value_hint(ValueHint::FilePath),
        )
        .arg(
          Arg::new("html")
            .long("html")
            .help(
              "Output coverage report in HTML format in the given directory",
            )
            .action(ArgAction::SetTrue),
        )
        .arg(
          Arg::new("detailed")
            .long("detailed")
            .help("Output coverage report in detailed format in the terminal.")
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
  Command::new("doc")
    .about("Show documentation for a module")
    .long_about(
      "Show documentation for a module.

Output documentation to standard output:

    deno doc ./path/to/module.ts

Output documentation in HTML format:

    deno doc --html --name=\"My library\" ./path/to/module.ts
    deno doc --html --name=\"My library\" ./main.ts ./dev.ts
    deno doc --html --name=\"My library\" --output=./documentation/ ./path/to/module.ts

Output private documentation to standard output:

    deno doc --private ./path/to/module.ts

Output documentation in JSON format:

    deno doc --json ./path/to/module.ts

Lint a module for documentation diagnostics:

    deno doc --lint ./path/to/module.ts

Target a specific symbol:

    deno doc ./path/to/module.ts MyClass.someField

Show documentation for runtime built-ins:

    deno doc
    deno doc --filter Deno.Listener",
    )
    .defer(|cmd| {
      cmd
        .arg(import_map_arg())
        .arg(reload_arg())
        .arg(lock_arg())
        .arg(no_lock_arg())
        .arg(no_npm_arg())
        .arg(no_remote_arg())
        .arg(
          Arg::new("json")
            .long("json")
            .help("Output documentation in JSON format")
            .action(ArgAction::SetTrue),
        )
        .arg(
          Arg::new("html")
            .long("html")
            .help("Output documentation in HTML format")
            .action(ArgAction::SetTrue)
            .conflicts_with("json")
        )
        .arg(
          Arg::new("name")
            .long("name")
            .help("The name that will be displayed in the docs")
            .action(ArgAction::Set)
            .require_equals(true)
        )
        .arg(
          Arg::new("output")
            .long("output")
            .help("Directory for HTML documentation output")
            .action(ArgAction::Set)
            .require_equals(true)
            .value_hint(ValueHint::DirPath)
            .value_parser(value_parser!(String))
        )
        .arg(
          Arg::new("private")
            .long("private")
            .help("Output private documentation")
            .action(ArgAction::SetTrue),
        )
        .arg(
          Arg::new("filter")
            .long("filter")
            .help("Dot separated path to symbol")
            .required(false)
            .conflicts_with("json")
            .conflicts_with("lint")
            .conflicts_with("html"),
        )
        .arg(
          Arg::new("lint")
            .long("lint")
            .help("Output documentation diagnostics.")
            .action(ArgAction::SetTrue),
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
  Command::new("eval")
    .about("Eval script")
    .long_about(
      "Evaluate JavaScript from the command line.

  deno eval \"console.log('hello world')\"

To evaluate as TypeScript:

  deno eval --ext=ts \"const v: string = 'hello'; console.log(v)\"

This command has implicit access to all permissions (--allow-all).",
    )
    .defer(|cmd| {
      runtime_args(cmd, false, true)
        .arg(check_arg(false))
        .arg(
          // TODO(@satyarohith): remove this argument in 2.0.
          Arg::new("ts")
            .conflicts_with("ext")
            .long("ts")
            .short('T')
            .help("deprecated: Use `--ext=ts` instead. The `--ts` and `-T` flags are deprecated and will be removed in Deno 2.0.")
            .action(ArgAction::SetTrue)
            .hide(true),
        )
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
            .help("Code arg")
            .value_name("CODE_ARG")
            .required(true),
        )
        .arg(env_file_arg())
    })
}

fn fmt_subcommand() -> Command {
  Command::new("fmt")
    .about("Format source files")
    .long_about(
      "Auto-format JavaScript, TypeScript, Markdown, and JSON files.

  deno fmt
  deno fmt myfile1.ts myfile2.ts
  deno fmt --check

Format stdin and write to stdout:

  cat file.ts | deno fmt -

Ignore formatting code by preceding it with an ignore comment:

  // deno-fmt-ignore

Ignore formatting a file by adding an ignore comment at the top of the file:

  // deno-fmt-ignore-file",
    )
    .defer(|cmd| {
      cmd
        .arg(config_arg())
        .arg(no_config_arg())
        .arg(
          Arg::new("check")
            .long("check")
            .help("Check if the source files are formatted")
            .num_args(0),
        )
        .arg(
          Arg::new("ext")
            .long("ext")
            .help("Set content type of the supplied file")
            // prefer using ts for formatting instead of js because ts works in more scenarios
            .default_value("ts")
            .value_parser([
              "ts", "tsx", "js", "jsx", "md", "json", "jsonc", "ipynb",
            ]),
        )
        .arg(
          Arg::new("ignore")
            .long("ignore")
            .num_args(1..)
            .use_value_delimiter(true)
            .require_equals(true)
            .help("Ignore formatting particular source files")
            .value_hint(ValueHint::AnyPath),
        )
        .arg(
          Arg::new("files")
            .num_args(1..)
            .action(ArgAction::Append)
            .required(false)
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
              "Use tabs instead of spaces for indentation. Defaults to false.",
            ),
        )
        .arg(
          Arg::new("line-width")
            .long("line-width")
            .alias("options-line-width")
            .help("Define maximum line width. Defaults to 80.")
            .value_parser(value_parser!(NonZeroU32)),
        )
        .arg(
          Arg::new("indent-width")
            .long("indent-width")
            .alias("options-indent-width")
            .help("Define indentation width. Defaults to 2.")
            .value_parser(value_parser!(NonZeroU8)),
        )
        .arg(
          Arg::new("single-quote")
            .long("single-quote")
            .alias("options-single-quote")
            .num_args(0..=1)
            .value_parser(value_parser!(bool))
            .default_missing_value("true")
            .require_equals(true)
            .help("Use single quotes. Defaults to false."),
        )
        .arg(
          Arg::new("prose-wrap")
            .long("prose-wrap")
            .alias("options-prose-wrap")
            .value_parser(["always", "never", "preserve"])
            .help("Define how prose should be wrapped. Defaults to always."),
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
              "Don't use semicolons except where necessary. Defaults to false.",
            ),
        )
    })
}

fn init_subcommand() -> Command {
  Command::new("init")
    .about("Initialize a new project")
    .defer(|cmd| {
      cmd.arg(
        Arg::new("dir")
          .required(false)
          .value_hint(ValueHint::DirPath),
      )
    })
}

fn info_subcommand() -> Command {
  Command::new("info")
      .about("Show info about cache or info related to source file")
      .long_about(
        "Information about a module or the cache directories.

Get information about a module:

  deno info jsr:@std/http/file-server

The following information is shown:

local: Local path of the file.
type: JavaScript, TypeScript, or JSON.
emit: Local path of compiled source code. (TypeScript only.)
dependencies: Dependency tree of the source file.

Without any additional arguments, 'deno info' shows:

DENO_DIR: Directory containing Deno-managed files.
Remote modules cache: Subdirectory containing downloaded remote modules.
TypeScript compiler cache: Subdirectory containing TS compiler output.",
      )
    .defer(|cmd| cmd
      .arg(Arg::new("file").required(false).value_hint(ValueHint::FilePath))
      .arg(reload_arg().requires("file"))
      .arg(ca_file_arg())
      .arg(
        location_arg()
          .conflicts_with("file")
          .help("Show files used for origin bound APIs like the Web Storage API when running a script with '--location=<HREF>'")
      )
      .arg(no_check_arg().hide(true)) // TODO(lucacasonato): remove for 2.0
      .arg(no_config_arg())
      .arg(no_remote_arg())
      .arg(no_npm_arg())
      .arg(lock_arg())
      .arg(lock_write_arg())
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
}

fn install_args(cmd: Command, deno_future: bool) -> Command {
  let cmd = if deno_future {
    cmd.arg(
      Arg::new("cmd")
        .required_if_eq("global", "true")
        .num_args(1..)
        .value_hint(ValueHint::FilePath),
    )
  } else {
    cmd.arg(
      Arg::new("cmd")
        .required(true)
        .num_args(1..)
        .value_hint(ValueHint::FilePath),
    )
  };
  cmd
    .arg(
      Arg::new("name")
        .long("name")
        .short('n')
        .help("Executable file name")
        .required(false),
    )
    .arg(
      Arg::new("root")
        .long("root")
        .help("Installation root")
        .value_hint(ValueHint::DirPath),
    )
    .arg(
      Arg::new("force")
        .long("force")
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
    .arg(env_file_arg())
}

fn future_install_subcommand() -> Command {
  Command::new("install")
    .visible_alias("i")
    .about("Install dependencies")
    .long_about(
"Installs dependencies either in the local project or globally to a bin directory.

Local installation
-------------------
If the --global flag is not set, adds dependencies to the local project's configuration
(package.json / deno.json) and installs them in the package cache. If no dependency
is specified, installs all dependencies listed in package.json.

  deno install
  deno install @std/bytes
  deno install npm:chalk

Global installation
-------------------
If the --global flag is set, installs a script as an executable in the installation root's bin directory.

  deno install --global --allow-net --allow-read jsr:@std/http/file-server
  deno install -g https://examples.deno.land/color-logging.ts

To change the executable name, use -n/--name:

  deno install -g --allow-net --allow-read -n serve jsr:@std/http/file-server

The executable name is inferred by default:
  - Attempt to take the file stem of the URL path. The above example would
    become 'file_server'.
  - If the file stem is something generic like 'main', 'mod', 'index' or 'cli',
    and the path has no parent, take the file name of the parent path. Otherwise
    settle with the generic name.
  - If the resulting name has an '@...' suffix, strip it.

To change the installation root, use --root:

  deno install -g --allow-net --allow-read --root /usr/local jsr:@std/http/file-server

The installation root is determined, in order of precedence:
  - --root option
  - DENO_INSTALL_ROOT environment variable
  - $HOME/.deno

These must be added to the path manually if required.")
    .defer(|cmd| {
      let cmd = runtime_args(cmd, true, true).arg(check_arg(true));
      install_args(cmd, true)
    })
}

fn install_subcommand() -> Command {
  Command::new("install")
    .about("Install script as an executable")
    .long_about(
"Installs a script as an executable in the installation root's bin directory.

  deno install --global --allow-net --allow-read jsr:@std/http/file-server
  deno install -g https://examples.deno.land/color-logging.ts

To change the executable name, use -n/--name:

  deno install -g --allow-net --allow-read -n serve jsr:@std/http/file-server

The executable name is inferred by default:
  - Attempt to take the file stem of the URL path. The above example would
    become 'file_server'.
  - If the file stem is something generic like 'main', 'mod', 'index' or 'cli',
    and the path has no parent, take the file name of the parent path. Otherwise
    settle with the generic name.
  - If the resulting name has an '@...' suffix, strip it.

To change the installation root, use --root:

  deno install -g --allow-net --allow-read --root /usr/local jsr:@std/http/file-server

The installation root is determined, in order of precedence:
  - --root option
  - DENO_INSTALL_ROOT environment variable
  - $HOME/.deno

These must be added to the path manually if required.")
    .defer(|cmd| {
      let cmd = runtime_args(cmd, true, true).arg(check_arg(true));
      install_args(cmd, false)
    })
}

fn jupyter_subcommand() -> Command {
  Command::new("jupyter")
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
    .about("Deno kernel for Jupyter notebooks")
}

fn uninstall_subcommand() -> Command {
  Command::new("uninstall")
      .about("Uninstall a script previously installed with deno install")
      .long_about(
        "Uninstalls an executable script in the installation root's bin directory.

  deno uninstall serve

To change the installation root, use --root:

  deno uninstall --root /usr/local serve

The installation root is determined, in order of precedence:
  - --root option
  - DENO_INSTALL_ROOT environment variable
  - $HOME/.deno")
    .defer(|cmd| cmd.arg(Arg::new("name").required(true))
      .arg(
        Arg::new("root")
          .long("root")
          .help("Installation root")
          .value_hint(ValueHint::DirPath)
      )
      .arg(
        Arg::new("global")
          .long("global")
          .short('g')
          .help("Remove globally installed package or module")
          .action(ArgAction::SetTrue)
      )
)
}

static LSP_HELP: &str = concat!(
  "The 'deno lsp' subcommand provides a way for code editors and IDEs to
interact with Deno using the Language Server Protocol. Usually humans do not
use this subcommand directly. For example, 'deno lsp' can provide IDEs with
go-to-definition support and automatic code formatting.

How to connect various editors and IDEs to 'deno lsp':
https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/getting_started/setup_your_environment#editors-and-ides",
);

fn lsp_subcommand() -> Command {
  Command::new("lsp")
    .about("Start the language server")
    .long_about(LSP_HELP)
}

fn lint_subcommand() -> Command {
  Command::new("lint")
    .about("Lint source files")
    .long_about(
      "Lint JavaScript/TypeScript source code.

  deno lint
  deno lint myfile1.ts myfile2.js

Print result as JSON:

  deno lint --json

Read from stdin:

  cat file.ts | deno lint -
  cat file.ts | deno lint --json -

List available rules:

  deno lint --rules

Ignore diagnostics on the next line by preceding it with an ignore comment and
rule name:

  // deno-lint-ignore no-explicit-any
  // deno-lint-ignore require-await no-empty

Names of rules to ignore must be specified after ignore comment.

Ignore linting a file by adding an ignore comment at the top of the file:

  // deno-lint-ignore-file
",
    )
    .defer(|cmd| {
      cmd
        .arg(
          Arg::new("fix")
            .long("fix")
            .help("Fix any linting errors for rules that support it")
            .action(ArgAction::SetTrue),
        )
        .arg(
          Arg::new("rules")
            .long("rules")
            .help("List available rules")
            .action(ArgAction::SetTrue),
        )
        .arg(
          Arg::new("rules-tags")
            .long("rules-tags")
            .require_equals(true)
            .num_args(1..)
            .action(ArgAction::Append)
            .use_value_delimiter(true)
            .help("Use set of rules with a tag"),
        )
        .arg(
          Arg::new("rules-include")
            .long("rules-include")
            .require_equals(true)
            .num_args(1..)
            .use_value_delimiter(true)
            .conflicts_with("rules")
            .help("Include lint rules"),
        )
        .arg(
          Arg::new("rules-exclude")
            .long("rules-exclude")
            .require_equals(true)
            .num_args(1..)
            .use_value_delimiter(true)
            .conflicts_with("rules")
            .help("Exclude lint rules"),
        )
        .arg(no_config_arg())
        .arg(config_arg())
        .arg(
          Arg::new("ignore")
            .long("ignore")
            .num_args(1..)
            .use_value_delimiter(true)
            .require_equals(true)
            .help("Ignore linting particular source files")
            .value_hint(ValueHint::AnyPath),
        )
        .arg(
          Arg::new("json")
            .long("json")
            .help("Output lint result in JSON format")
            .action(ArgAction::SetTrue),
        )
        .arg(
          Arg::new("compact")
            .long("compact")
            .help("Output lint result in compact format")
            .action(ArgAction::SetTrue)
            .conflicts_with("json"),
        )
        .arg(
          Arg::new("files")
            .num_args(1..)
            .action(ArgAction::Append)
            .required(false)
            .value_hint(ValueHint::AnyPath),
        )
        .arg(watch_arg(false))
        .arg(watch_exclude_arg())
        .arg(no_clear_screen_arg())
    })
}

fn repl_subcommand() -> Command {
  Command::new("repl")
    .about("Read Eval Print Loop")
    .defer(|cmd| runtime_args(cmd, true, true)
      .arg(check_arg(false))
      .arg(
        Arg::new("eval-file")
          .long("eval-file")
          .num_args(1..)
          .use_value_delimiter(true)
          .require_equals(true)
          .help("Evaluates the provided file(s) as scripts when the REPL starts. Accepts file paths and URLs.")
          .value_hint(ValueHint::AnyPath),
      )
      .arg(
        Arg::new("eval")
          .long("eval")
          .help("Evaluates the provided code when the REPL starts.")
          .value_name("code"),
      ))
      .arg(env_file_arg())
}

fn run_subcommand() -> Command {
  runtime_args(Command::new("run"), true, true)
    .arg(check_arg(false))
    .arg(watch_arg(true))
    .arg(watch_exclude_arg())
    .arg(hmr_arg(true))
    .arg(no_clear_screen_arg())
    .arg(executable_ext_arg())
    .arg(
      script_arg()
        .required_unless_present("v8-flags")
        .trailing_var_arg(true),
    )
    .arg(env_file_arg())
    .arg(no_code_cache_arg())
    .about("Run a JavaScript or TypeScript program")
    .long_about(
      "Run a JavaScript or TypeScript program

By default all programs are run in sandbox without access to disk, network or
ability to spawn subprocesses.

  deno run https://examples.deno.land/hello-world.ts

Grant all permissions:

  deno run -A jsr:@std/http/file-server

Grant permission to read from disk and listen to network:

  deno run --allow-read --allow-net jsr:@std/http/file-server

Grant permission to read allow-listed files from disk:

  deno run --allow-read=/etc jsr:@std/http/file-server

Specifying the filename '-' to read the file from stdin.

  curl https://examples.deno.land/hello-world.ts | deno run -",
    )
}

fn serve_host_validator(host: &str) -> Result<String, String> {
  if Url::parse(&format!("internal://{host}:9999")).is_ok() {
    Ok(host.to_owned())
  } else {
    Err(format!("Bad serve host: {host}"))
  }
}

fn serve_subcommand() -> Command {
  runtime_args(Command::new("serve"), true, true)
    .arg(
      Arg::new("port")
        .long("port")
        .help("The TCP port to serve on, defaulting to 8000. Pass 0 to pick a random free port.")
        .value_parser(value_parser!(u16)),
    )
    .arg(
      Arg::new("host")
        .long("host")
        .help("The TCP address to serve on, defaulting to 0.0.0.0 (all interfaces).")
        .value_parser(serve_host_validator),
    )
    .arg(check_arg(false))
    .arg(watch_arg(true))
    .arg(watch_exclude_arg())
    .arg(hmr_arg(true))
    .arg(no_clear_screen_arg())
    .arg(executable_ext_arg())
    .arg(
      script_arg()
        .required_unless_present("v8-flags")
        .trailing_var_arg(true),
    )
    .arg(env_file_arg())
    .arg(no_code_cache_arg())
    .about("Run a server")
    .long_about("Run a server defined in a main module

The serve command uses the default exports of the main module to determine which
servers to start.

See https://docs.deno.com/runtime/manual/tools/serve for
more detailed information.

Start a server defined in server.ts:

  deno serve server.ts

Start a server defined in server.ts, watching for changes and running on port 5050:

  deno serve --watch --port 5050 server.ts")
}

fn task_subcommand() -> Command {
  Command::new("task")
    .about("Run a task defined in the configuration file")
    .long_about(
      "Run a task defined in the configuration file

  deno task build",
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
    })
}

fn test_subcommand() -> Command {
  Command::new("test")
    .about("Run tests")
    .long_about(
      "Run tests using Deno's built-in test runner.

Evaluate the given modules, run all tests declared with 'Deno.test()' and
report results to standard output:

  deno test src/fetch_test.ts src/signal_test.ts

Directory arguments are expanded to all contained files matching the glob
{*_,*.,}test.{js,mjs,ts,mts,jsx,tsx}:

  deno test src/",
    )
  .defer(|cmd| runtime_args(cmd, true, true)
    .arg(check_arg(true))
    .arg(
      Arg::new("ignore")
        .long("ignore")
        .num_args(1..)
        .use_value_delimiter(true)
        .require_equals(true)
        .help("Ignore files")
        .value_hint(ValueHint::AnyPath),
    )
    .arg(
      Arg::new("no-run")
        .long("no-run")
        .help("Cache test modules, but don't run tests")
        .action(ArgAction::SetTrue),
    )
    .arg(
      Arg::new("trace-ops")
        .long("trace-ops")
        .help("Deprecated alias for --trace-leaks.")
        .hide(true)
        .action(ArgAction::SetTrue),
    )
    .arg(
      Arg::new("trace-leaks")
        .long("trace-leaks")
        .help("Enable tracing of leaks. Useful when debugging leaking ops in test, but impacts test execution time.")
        .action(ArgAction::SetTrue),
    )
    .arg(
      Arg::new("doc")
        .long("doc")
        .help("Type-check code blocks in JSDoc and Markdown")
        .action(ArgAction::SetTrue),
    )
    .arg(
      Arg::new("fail-fast")
        .long("fail-fast")
        .alias("failfast")
        .help("Stop after N errors. Defaults to stopping after first failure.")
        .num_args(0..=1)
        .require_equals(true)
        .value_name("N")
        .value_parser(value_parser!(NonZeroUsize)),
    )
    .arg(
      Arg::new("allow-none")
        .long("allow-none")
        .help("Don't return error code if no test files are found")
        .action(ArgAction::SetTrue),
    )
    .arg(
      Arg::new("filter")
        .allow_hyphen_values(true)
        .long("filter")
        .help("Run tests with this string or pattern in the test name"),
    )
    .arg(
      Arg::new("shuffle")
        .long("shuffle")
        .value_name("NUMBER")
        .help("Shuffle the order in which the tests are run")
        .num_args(0..=1)
        .require_equals(true)
        .value_parser(value_parser!(u64)),
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
        .help("Collect coverage profile data into DIR. If DIR is not specified, it uses 'coverage/'."),
    )
    .arg(
      Arg::new("clean")
        .long("clean")
        .help("Empty the temporary coverage profile data directory before running tests.
        
Note: running multiple `deno test --clean` calls in series or parallel for the same coverage directory may cause race conditions.")
        .action(ArgAction::SetTrue),
    )
    .arg(
      Arg::new("parallel")
        .long("parallel")
        .help("Run test modules in parallel. Parallelism defaults to the number of available CPUs or the value in the DENO_JOBS environment variable.")
        .conflicts_with("jobs")
        .action(ArgAction::SetTrue)
    )
    .arg(
      Arg::new("jobs")
        .short('j')
        .long("jobs")
        .help("deprecated: The `--jobs` flag is deprecated and will be removed in Deno 2.0. Use the `--parallel` flag with possibly the `DENO_JOBS` environment variable instead.")
        .hide(true)
        .num_args(0..=1)
        .value_parser(value_parser!(NonZeroUsize)),
    )
    .arg(
      Arg::new("files")
        .help("List of file names to run")
        .num_args(0..)
        .action(ArgAction::Append)
        .value_hint(ValueHint::AnyPath),
    )
    .arg(
      watch_arg(false)
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
        .help("Write a JUnit XML test report to PATH. Use '-' to write to stdout which is the default when PATH is not provided.")
    )
    .arg(
      Arg::new("reporter")
        .long("reporter")
        .help("Select reporter to use. Default to 'pretty'.")
        .value_parser(["pretty", "dot", "junit", "tap"])
    )
    .arg(env_file_arg())
  )
}

fn types_subcommand() -> Command {
  Command::new("types")
    .about("Print runtime TypeScript declarations")
    .long_about(
      "Print runtime TypeScript declarations.

  deno types > lib.deno.d.ts

The declaration file could be saved and used for typing information.",
    )
}

fn upgrade_subcommand() -> Command {
  Command::new("upgrade")
    .about("Upgrade deno executable to given version")
    .long_about(
      "Upgrade deno executable to the given version.
Defaults to latest.

The version is downloaded from
https://github.com/denoland/deno/releases
and is used to replace the current executable.

If you want to not replace the current Deno executable but instead download an
update to a different location, use the --output flag

  deno upgrade --output $HOME/my_deno",
    )
    .hide(cfg!(not(feature = "upgrade")))
    .defer(|cmd| {
      cmd
        .arg(
          Arg::new("version")
            .long("version")
            .help("The version to upgrade to"),
        )
        .arg(
          Arg::new("output")
            .long("output")
            .help("The path to output the updated version to")
            .value_parser(value_parser!(String))
            .value_hint(ValueHint::FilePath),
        )
        .arg(
          Arg::new("dry-run")
            .long("dry-run")
            .help("Perform all checks without replacing old exe")
            .action(ArgAction::SetTrue),
        )
        .arg(
          Arg::new("force")
            .long("force")
            .short('f')
            .help("Replace current exe even if not out-of-date")
            .action(ArgAction::SetTrue),
        )
        .arg(
          Arg::new("canary")
            .long("canary")
            .help("Upgrade to canary builds")
            .action(ArgAction::SetTrue),
        )
        .arg(ca_file_arg())
    })
}

fn vendor_subcommand() -> Command {
  Command::new("vendor")
      .about("Vendor remote modules into a local directory")
      .long_about(
        "Vendor remote modules into a local directory.

Analyzes the provided modules along with their dependencies, downloads
remote modules to the output directory, and produces an import map that
maps remote specifiers to the downloaded files.

  deno vendor main.ts
  deno run --import-map vendor/import_map.json main.ts

Remote modules and multiple modules may also be specified:

  deno vendor main.ts test.deps.ts jsr:@std/path",
      )
    .defer(|cmd| cmd
      .arg(
        Arg::new("specifiers")
          .num_args(1..)
          .action(ArgAction::Append)
          .required(true),
      )
      .arg(
        Arg::new("output")
          .long("output")
          .help("The directory to output the vendored modules to")
          .value_parser(value_parser!(String))
          .value_hint(ValueHint::DirPath),
      )
      .arg(
        Arg::new("force")
          .long("force")
          .short('f')
          .help(
            "Forcefully overwrite conflicting files in existing output directory",
          )
          .action(ArgAction::SetTrue),
      )
      .arg(no_config_arg())
      .arg(config_arg())
      .arg(import_map_arg())
      .arg(lock_arg())
      .arg(node_modules_dir_arg())
      .arg(vendor_arg())
      .arg(reload_arg())
      .arg(ca_file_arg()))
}

fn publish_subcommand() -> Command {
  Command::new("publish")
    .hide(true)
    .about("Unstable preview feature: Publish the current working directory's package or workspace")
    // TODO: .long_about()
    .defer(|cmd| {
      cmd.arg(
        Arg::new("token")
          .long("token")
          .help("The API token to use when publishing. If unset, interactive authentication is be used")
      )
      .arg(config_arg())
      .arg(no_config_arg())
      .arg(
        Arg::new("dry-run")
          .long("dry-run")
          .help("Prepare the package for publishing performing all checks and validations without uploading")
          .action(ArgAction::SetTrue),
      )
      .arg(
        Arg::new("allow-slow-types")
          .long("allow-slow-types")
          .help("Allow publishing with slow types")
          .action(ArgAction::SetTrue),
      )
      .arg(
        Arg::new("allow-dirty")
        .long("allow-dirty")
        .help("Allow publishing if the repository has uncommitted changed")
        .action(ArgAction::SetTrue),
      ).arg(
        Arg::new("no-provenance")
          .long("no-provenance")
          .help("Disable provenance attestation. Enabled by default on Github actions, publicly links the package to where it was built and published from.")
          .action(ArgAction::SetTrue)
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
    .arg(lock_write_arg())
    .arg(no_lock_arg())
    .arg(ca_file_arg())
}

static ALLOW_READ_HELP: &str = concat!(
  "Allow file system read access. Optionally specify allowed paths.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --allow-read\n",
  "  --allow-read=\"/etc,/var/log.txt\""
);

static DENY_READ_HELP: &str = concat!(
  "Deny file system read access. Optionally specify denied paths.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --deny-read\n",
  "  --deny-read=\"/etc,/var/log.txt\""
);

static ALLOW_WRITE_HELP: &str = concat!(
  "Allow file system write access. Optionally specify allowed paths.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --allow-write\n",
  "  --allow-write=\"/etc,/var/log.txt\""
);

static DENY_WRITE_HELP: &str = concat!(
  "Deny file system write access. Optionally specify denied paths.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --deny-write\n",
  "  --deny-write=\"/etc,/var/log.txt\""
);

static ALLOW_NET_HELP: &str = concat!(
  "Allow network access. Optionally specify allowed IP addresses and host names, with ports as necessary.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --allow-net\n",
  "  --allow-net=\"localhost:8080,deno.land\""
);

static DENY_NET_HELP: &str = concat!(
  "Deny network access. Optionally specify denied IP addresses and host names, with ports as necessary.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --deny-net\n",
  "  --deny-net=\"localhost:8080,deno.land\""
);

static ALLOW_ENV_HELP: &str = concat!(
  "Allow access to system environment information. Optionally specify accessible environment variables.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --allow-env\n",
  "  --allow-env=\"PORT,HOME,PATH\""
);

static DENY_ENV_HELP: &str = concat!(
  "Deny access to system environment information. Optionally specify accessible environment variables.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --deny-env\n",
  "  --deny-env=\"PORT,HOME,PATH\""
);

static ALLOW_SYS_HELP: &str = concat!(
  "Allow access to OS information. Optionally allow specific APIs by function name.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --allow-sys\n",
  "  --allow-sys=\"systemMemoryInfo,osRelease\""
);

static DENY_SYS_HELP: &str = concat!(
  "Deny access to OS information. Optionally deny specific APIs by function name.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --deny-sys\n",
  "  --deny-sys=\"systemMemoryInfo,osRelease\""
);

static ALLOW_RUN_HELP: &str = concat!(
  "Allow running subprocesses. Optionally specify allowed runnable program names.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --allow-run\n",
  "  --allow-run=\"whoami,ps\""
);

static DENY_RUN_HELP: &str = concat!(
  "Deny running subprocesses. Optionally specify denied runnable program names.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --deny-run\n",
  "  --deny-run=\"whoami,ps\""
);

static ALLOW_FFI_HELP: &str = concat!(
  "(Unstable) Allow loading dynamic libraries. Optionally specify allowed directories or files.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --allow-ffi\n",
  "  --allow-ffi=\"./libfoo.so\""
);

static DENY_FFI_HELP: &str = concat!(
  "(Unstable) Deny loading dynamic libraries. Optionally specify denied directories or files.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n",
  "Examples:\n",
  "  --deny-ffi\n",
  "  --deny-ffi=\"./libfoo.so\""
);

static ALLOW_HRTIME_HELP: &str = concat!(
  "Allow high-resolution time measurement. Note: this can enable timing attacks and fingerprinting.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n"
);

static DENY_HRTIME_HELP: &str = concat!(
  "Deny high-resolution time measurement. Note: this can prevent timing attacks and fingerprinting.\n",
  "Docs: https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n"
);

static ALLOW_ALL_HELP: &str = concat!(
  "Allow all permissions. Learn more about permissions in Deno:\n",
  "https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/basics/permissions\n"
);

fn permission_args(app: Command) -> Command {
  app
    .arg(
      Arg::new("allow-read")
        .long("allow-read")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("PATH")
        .help(ALLOW_READ_HELP)
        .value_parser(value_parser!(String))
        .value_hint(ValueHint::AnyPath),
    )
    .arg(
      Arg::new("deny-read")
        .long("deny-read")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("PATH")
        .help(DENY_READ_HELP)
        .value_parser(value_parser!(String))
        .value_hint(ValueHint::AnyPath),
    )
    .arg(
      Arg::new("allow-write")
        .long("allow-write")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("PATH")
        .help(ALLOW_WRITE_HELP)
        .value_parser(value_parser!(String))
        .value_hint(ValueHint::AnyPath),
    )
    .arg(
      Arg::new("deny-write")
        .long("deny-write")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("PATH")
        .help(DENY_WRITE_HELP)
        .value_parser(value_parser!(String))
        .value_hint(ValueHint::AnyPath),
    )
    .arg(
      Arg::new("allow-net")
        .long("allow-net")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("IP_OR_HOSTNAME")
        .help(ALLOW_NET_HELP)
        .value_parser(flags_net::validator),
    )
    .arg(
      Arg::new("deny-net")
        .long("deny-net")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("IP_OR_HOSTNAME")
        .help(DENY_NET_HELP)
        .value_parser(flags_net::validator),
    )
    .arg(unsafely_ignore_certificate_errors_arg())
    .arg(
      Arg::new("allow-env")
        .long("allow-env")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("VARIABLE_NAME")
        .help(ALLOW_ENV_HELP)
        .value_parser(|key: &str| {
          if key.is_empty() || key.contains(&['=', '\0'] as &[char]) {
            return Err(format!("invalid key \"{key}\""));
          }

          Ok(if cfg!(windows) {
            key.to_uppercase()
          } else {
            key.to_string()
          })
        }),
    )
    .arg(
      Arg::new("deny-env")
        .long("deny-env")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("VARIABLE_NAME")
        .help(DENY_ENV_HELP)
        .value_parser(|key: &str| {
          if key.is_empty() || key.contains(&['=', '\0'] as &[char]) {
            return Err(format!("invalid key \"{key}\""));
          }

          Ok(if cfg!(windows) {
            key.to_uppercase()
          } else {
            key.to_string()
          })
        }),
    )
    .arg(
      Arg::new("allow-sys")
        .long("allow-sys")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("API_NAME")
        .help(ALLOW_SYS_HELP)
        .value_parser(|key: &str| parse_sys_kind(key).map(ToString::to_string)),
    )
    .arg(
      Arg::new("deny-sys")
        .long("deny-sys")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("API_NAME")
        .help(DENY_SYS_HELP)
        .value_parser(|key: &str| parse_sys_kind(key).map(ToString::to_string)),
    )
    .arg(
      Arg::new("allow-run")
        .long("allow-run")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("PROGRAM_NAME")
        .help(ALLOW_RUN_HELP),
    )
    .arg(
      Arg::new("deny-run")
        .long("deny-run")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("PROGRAM_NAME")
        .help(DENY_RUN_HELP),
    )
    .arg(
      Arg::new("allow-ffi")
        .long("allow-ffi")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("PATH")
        .help(ALLOW_FFI_HELP)
        .value_parser(value_parser!(String))
        .value_hint(ValueHint::AnyPath),
    )
    .arg(
      Arg::new("deny-ffi")
        .long("deny-ffi")
        .num_args(0..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("PATH")
        .help(DENY_FFI_HELP)
        .value_parser(value_parser!(String))
        .value_hint(ValueHint::AnyPath),
    )
    .arg(
      Arg::new("allow-hrtime")
        .long("allow-hrtime")
        .action(ArgAction::SetTrue)
        .help(ALLOW_HRTIME_HELP),
    )
    .arg(
      Arg::new("deny-hrtime")
        .long("deny-hrtime")
        .action(ArgAction::SetTrue)
        .help(DENY_HRTIME_HELP),
    )
    .arg(
      Arg::new("allow-all")
        .short('A')
        .long("allow-all")
        .action(ArgAction::SetTrue)
        .help(ALLOW_ALL_HELP),
    )
    .arg(
      Arg::new("no-prompt")
        .long("no-prompt")
        .action(ArgAction::SetTrue)
        .help("Always throw if required permission wasn't passed"),
    )
}

fn runtime_args(
  app: Command,
  include_perms: bool,
  include_inspector: bool,
) -> Command {
  let app = compile_args(app);
  let app = if include_perms {
    permission_args(app)
  } else {
    app
  };
  let app = if include_inspector {
    inspect_args(app)
  } else {
    app
  };
  app
    .arg(cached_only_arg())
    .arg(location_arg())
    .arg(v8_flags_arg())
    .arg(seed_arg())
    .arg(enable_testing_features_arg())
    .arg(strace_ops_arg())
    .arg(eszip_arg())
}

fn inspect_args(app: Command) -> Command {
  app
    .arg(
      Arg::new("inspect")
        .long("inspect")
        .value_name("HOST_AND_PORT")
        .help("Activate inspector on host:port (default: 127.0.0.1:9229)")
        .num_args(0..=1)
        .require_equals(true)
        .value_parser(value_parser!(SocketAddr)),
    )
    .arg(
      Arg::new("inspect-brk")
        .long("inspect-brk")
        .value_name("HOST_AND_PORT")
        .help(
          "Activate inspector on host:port, wait for debugger to connect and break at the start of user script",
        )
        .num_args(0..=1)
        .require_equals(true)
        .value_parser(value_parser!(SocketAddr)),
    )
    .arg(
      Arg::new("inspect-wait")
        .long("inspect-wait")
        .value_name("HOST_AND_PORT")
        .help(
          "Activate inspector on host:port and wait for debugger to connect before running user code",
        )
        .num_args(0..=1)
        .require_equals(true)
        .value_parser(value_parser!(SocketAddr)),
    )
}

static IMPORT_MAP_HELP: &str = concat!(
  "Load import map file from local file or remote URL.
Docs: https://docs.deno.com/runtime/manual/basics/import_maps
Specification: https://wicg.github.io/import-maps/
Examples: https://github.com/WICG/import-maps#the-import-map",
);

fn import_map_arg() -> Arg {
  Arg::new("import-map")
    .long("import-map")
    .alias("importmap")
    .value_name("FILE")
    .help("Load import map file")
    .long_help(IMPORT_MAP_HELP)
    .value_hint(ValueHint::FilePath)
}

fn env_file_arg() -> Arg {
  Arg::new("env")
    .long("env")
    .value_name("FILE")
    .help("Load .env file")
    .long_help("UNSTABLE: Load environment variables from local file. Only the first environment variable with a given key is used. Existing process environment variables are not overwritten.")
    .value_hint(ValueHint::FilePath)
    .default_missing_value(".env")
    .require_equals(true)
    .num_args(0..=1)
}

fn reload_arg() -> Arg {
  Arg::new("reload")
    .short('r')
    .num_args(0..)
    .use_value_delimiter(true)
    .require_equals(true)
    .long("reload")
    .help("Reload source code cache (recompile TypeScript)")
    .value_name("CACHE_BLOCKLIST")
    .long_help(
      "Reload source code cache (recompile TypeScript)
--reload
  Reload everything
--reload=jsr:@std/http/file-server
  Reload only standard modules
--reload=jsr:@std/http/file-server,jsr:@std/assert/assert-equals
  Reloads specific modules
--reload=npm:
  Reload all npm modules
--reload=npm:chalk
  Reload specific npm module",
    )
    .value_hint(ValueHint::FilePath)
    .value_parser(reload_arg_validate)
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
}

fn eszip_arg() -> Arg {
  Arg::new("eszip-internal-do-not-use")
    .hide(true)
    .long("eszip-internal-do-not-use")
    .action(ArgAction::SetTrue)
    .help("Run eszip")
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
    .help("Value of 'globalThis.location' used by some web APIs")
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
    .help("Set V8 command line options")
    .long_help("To see a list of all available flags use --v8-flags=--help.
    Any flags set with this flag are appended after the DENO_V8_FLAGS environmental variable")
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
    .long("unstable-hmr")
    .help("UNSTABLE: Watch for file changes and hot replace modules")
    .conflicts_with("watch");

  if takes_files {
    arg
      .value_name("FILES")
      .num_args(0..)
      .value_parser(value_parser!(String))
      .use_value_delimiter(true)
      .require_equals(true)
      .long_help(
        "Watch for file changes and restart process automatically.
Local files from entry point module graph are watched by default.
Additional paths might be watched by passing them as arguments to this flag.",
      )
      .value_hint(ValueHint::AnyPath)
  } else {
    arg.action(ArgAction::SetTrue).long_help(
      "Watch for file changes and restart process automatically.
      Only local files from entry point module graph are watched.",
    )
  }
}

fn watch_arg(takes_files: bool) -> Arg {
  let arg = Arg::new("watch")
    .long("watch")
    .help("Watch for file changes and restart automatically");

  if takes_files {
    arg
      .value_name("FILES")
      .num_args(0..)
      .value_parser(value_parser!(String))
      .use_value_delimiter(true)
      .require_equals(true)
      .long_help(
        "Watch for file changes and restart process automatically.
Local files from entry point module graph are watched by default.
Additional paths might be watched by passing them as arguments to this flag.",
      )
      .value_hint(ValueHint::AnyPath)
  } else {
    arg.action(ArgAction::SetTrue).long_help(
      "Watch for file changes and restart process automatically.
      Only local files from entry point module graph are watched.",
    )
  }
}

fn no_clear_screen_arg() -> Arg {
  Arg::new("no-clear-screen")
    .requires("watch")
    .long("no-clear-screen")
    .action(ArgAction::SetTrue)
    .help("Do not clear terminal screen when under watch mode")
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
    .value_parser(value_parser!(String))
    .use_value_delimiter(true)
    .require_equals(true)
    .value_hint(ValueHint::AnyPath)
}

fn no_check_arg() -> Arg {
  Arg::new("no-check")
    .num_args(0..=1)
    .require_equals(true)
    .value_name("NO_CHECK_TYPE")
    .long("no-check")
    .help("Skip type-checking modules")
    .long_help(
      "Skip type-checking. If the value of '--no-check=remote' is supplied,
diagnostic errors from remote modules will be ignored.",
    )
}

fn check_arg(checks_local_by_default: bool) -> Arg {
  let arg = Arg::new("check")
    .conflicts_with("no-check")
    .long("check")
    .num_args(0..=1)
    .require_equals(true)
    .value_name("CHECK_TYPE")
    .help("Type-check modules");

  if checks_local_by_default {
    arg.long_help(
      "Set type-checking behavior. This subcommand type-checks local modules by
default, so adding --check is redundant.
If the value of '--check=all' is supplied, diagnostic errors from remote modules
will be included.

Alternatively, the 'deno check' subcommand can be used.",
    )
  } else {
    arg.long_help(
      "Enable type-checking. This subcommand does not type-check by default.
If the value of '--check=all' is supplied, diagnostic errors from remote modules
will be included.

Alternatively, the 'deno check' subcommand can be used.",
    )
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
    .help("Check the specified lock file.

If value is not provided, defaults to \"deno.lock\" in the current working directory.")
    .num_args(0..=1)
    .value_parser(value_parser!(String))
    .value_hint(ValueHint::FilePath)
}

fn lock_write_arg() -> Arg {
  Arg::new("lock-write")
    .action(ArgAction::SetTrue)
    .long("lock-write")
    .help("Force overwriting the lock file.")
    .conflicts_with("no-lock")
}

fn no_lock_arg() -> Arg {
  Arg::new("no-lock")
    .long("no-lock")
    .action(ArgAction::SetTrue)
    .help("Disable auto discovery of the lock file.")
    .conflicts_with("lock")
}

static CONFIG_HELP: &str = concat!(
  "The configuration file can be used to configure different aspects of
deno including TypeScript, linting, and code formatting. Typically the
configuration file will be called `deno.json` or `deno.jsonc` and
automatically detected; in that case this flag is not necessary.
See https://deno.land/manual@v",
  env!("CARGO_PKG_VERSION"),
  "/getting_started/configuration_file"
);

fn config_arg() -> Arg {
  Arg::new("config")
    .short('c')
    .long("config")
    .value_name("FILE")
    .help("Specify the configuration file")
    .long_help(CONFIG_HELP)
    .value_hint(ValueHint::FilePath)
}

fn no_config_arg() -> Arg {
  Arg::new("no-config")
    .long("no-config")
    .action(ArgAction::SetTrue)
    .help("Disable automatic loading of the configuration file.")
    .conflicts_with("config")
}

fn no_remote_arg() -> Arg {
  Arg::new("no-remote")
    .long("no-remote")
    .action(ArgAction::SetTrue)
    .help("Do not resolve remote modules")
}

fn no_npm_arg() -> Arg {
  Arg::new("no-npm")
    .long("no-npm")
    .action(ArgAction::SetTrue)
    .help("Do not resolve npm modules")
}

fn node_modules_dir_arg() -> Arg {
  Arg::new("node-modules-dir")
    .long("node-modules-dir")
    .num_args(0..=1)
    .value_parser(value_parser!(bool))
    .default_missing_value("true")
    .require_equals(true)
    .help("Enables or disables the use of a local node_modules folder for npm packages")
}

fn vendor_arg() -> Arg {
  Arg::new("vendor")
    .long("vendor")
    .num_args(0..=1)
    .value_parser(value_parser!(bool))
    .default_missing_value("true")
    .require_equals(true)
    .help("UNSTABLE: Enables or disables the use of a local vendor folder for remote modules and node_modules folder for npm packages")
}

fn unsafely_ignore_certificate_errors_arg() -> Arg {
  Arg::new("unsafely-ignore-certificate-errors")
    .long("unsafely-ignore-certificate-errors")
    .num_args(0..)
    .use_value_delimiter(true)
    .require_equals(true)
    .value_name("HOSTNAMES")
    .help("DANGER: Disables verification of TLS certificates")
    .value_parser(flags_net::validator)
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
  AddFlags { packages }
}

fn bench_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.type_check_mode = TypeCheckMode::Local;

  runtime_args_parse(flags, matches, true, false);

  // NOTE: `deno bench` always uses `--no-prompt`, tests shouldn't ever do
  // interactive prompts, unless done by user code
  flags.permissions.no_prompt = true;

  let json = matches.get_flag("json");

  let ignore = match matches.remove_many::<String>("ignore") {
    Some(f) => f.collect(),
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
    watch: watch_arg_parse(matches),
  });
}

fn bundle_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.type_check_mode = TypeCheckMode::Local;

  compile_args_parse(flags, matches);

  let source_file = matches.remove_one::<String>("source_file").unwrap();

  let out_file =
    if let Some(out_file) = matches.remove_one::<String>("out_file") {
      flags.permissions.allow_write = Some(vec![]);
      Some(out_file)
    } else {
      None
    };

  ext_arg_parse(flags, matches);

  flags.subcommand = DenoSubcommand::Bundle(BundleFlags {
    source_file,
    out_file,
    watch: watch_arg_parse(matches),
  });
}

fn cache_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  compile_args_parse(flags, matches);
  let files = matches.remove_many::<String>("file").unwrap().collect();
  flags.subcommand = DenoSubcommand::Cache(CacheFlags { files });
}

fn check_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.type_check_mode = TypeCheckMode::Local;
  compile_args_without_check_parse(flags, matches);
  let files = matches.remove_many::<String>("file").unwrap().collect();
  if matches.get_flag("all") || matches.get_flag("remote") {
    flags.type_check_mode = TypeCheckMode::All;
  }
  flags.subcommand = DenoSubcommand::Check(CheckFlags { files });
}

fn compile_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.type_check_mode = TypeCheckMode::Local;
  runtime_args_parse(flags, matches, true, false);

  let mut script = matches.remove_many::<String>("script_arg").unwrap();
  let source_file = script.next().unwrap();
  let args = script.collect();
  let output = matches.remove_one::<String>("output");
  let target = matches.remove_one::<String>("target");
  let no_terminal = matches.get_flag("no-terminal");
  let eszip = matches.get_flag("eszip-internal-do-not-use");
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
    include,
    eszip,
  });
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

fn coverage_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  let files = match matches.remove_many::<String>("files") {
    Some(f) => f.collect(),
    None => vec!["coverage".to_string()], // default
  };
  let ignore = match matches.remove_many::<String>("ignore") {
    Some(f) => f.collect(),
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
}

fn doc_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  import_map_arg_parse(flags, matches);
  reload_arg_parse(flags, matches);
  lock_arg_parse(flags, matches);
  no_lock_arg_parse(flags, matches);
  no_npm_arg_parse(flags, matches);
  no_remote_arg_parse(flags, matches);

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
    let output = matches
      .remove_one::<String>("output")
      .unwrap_or(String::from("./docs/"));
    Some(DocHtmlFlag { name, output })
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
}

fn eval_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  runtime_args_parse(flags, matches, false, true);
  flags.allow_all();

  ext_arg_parse(flags, matches);

  // TODO(@satyarohith): remove this flag in 2.0.
  let as_typescript = matches.get_flag("ts");

  #[allow(clippy::print_stderr)]
  if as_typescript {
    eprintln!(
      "⚠️ {}",
      crate::colors::yellow(
        "Use `--ext=ts` instead. The `--ts` and `-T` flags are deprecated and will be removed in Deno 2.0."
      ),
    );

    flags.ext = Some("ts".to_string());
  }

  let print = matches.get_flag("print");
  let mut code_args = matches.remove_many::<String>("code_arg").unwrap();
  let code = code_args.next().unwrap();
  flags.argv.extend(code_args);

  flags.subcommand = DenoSubcommand::Eval(EvalFlags { print, code });
}

fn fmt_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  config_args_parse(flags, matches);
  ext_arg_parse(flags, matches);

  let include = match matches.remove_many::<String>("files") {
    Some(f) => f.collect(),
    None => vec![],
  };
  let ignore = match matches.remove_many::<String>("ignore") {
    Some(f) => f.collect(),
    None => vec![],
  };

  let use_tabs = matches.remove_one::<bool>("use-tabs");
  let line_width = matches.remove_one::<NonZeroU32>("line-width");
  let indent_width = matches.remove_one::<NonZeroU8>("indent-width");
  let single_quote = matches.remove_one::<bool>("single-quote");
  let prose_wrap = matches.remove_one::<String>("prose-wrap");
  let no_semicolons = matches.remove_one::<bool>("no-semicolons");

  flags.subcommand = DenoSubcommand::Fmt(FmtFlags {
    check: matches.get_flag("check"),
    files: FileFlags { include, ignore },
    use_tabs,
    line_width,
    indent_width,
    single_quote,
    prose_wrap,
    no_semicolons,
    watch: watch_arg_parse(matches),
  });
}

fn init_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.subcommand = DenoSubcommand::Init(InitFlags {
    dir: matches.remove_one::<String>("dir"),
  });
}

fn info_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  reload_arg_parse(flags, matches);
  config_args_parse(flags, matches);
  import_map_arg_parse(flags, matches);
  location_arg_parse(flags, matches);
  ca_file_arg_parse(flags, matches);
  node_modules_and_vendor_dir_arg_parse(flags, matches);
  lock_args_parse(flags, matches);
  no_remote_arg_parse(flags, matches);
  no_npm_arg_parse(flags, matches);
  let json = matches.get_flag("json");
  flags.subcommand = DenoSubcommand::Info(InfoFlags {
    file: matches.remove_one::<String>("file"),
    json,
  });
}

fn install_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  runtime_args_parse(flags, matches, true, true);

  let global = matches.get_flag("global");
  if global || !*DENO_FUTURE {
    let root = matches.remove_one::<String>("root");
    let force = matches.get_flag("force");
    let name = matches.remove_one::<String>("name");
    let mut cmd_values =
      matches.remove_many::<String>("cmd").unwrap_or_default();

    let module_url = cmd_values.next().unwrap();
    let args = cmd_values.collect();

    flags.subcommand = DenoSubcommand::Install(InstallFlags {
      // TODO(bartlomieju): remove for 2.0
      global,
      kind: InstallKind::Global(InstallFlagsGlobal {
        name,
        module_url,
        args,
        root,
        force,
      }),
    });
  } else {
    let local_flags = matches
      .remove_many("cmd")
      .map(|packages| add_parse_inner(matches, Some(packages)));
    flags.subcommand = DenoSubcommand::Install(InstallFlags {
      global,
      kind: InstallKind::Local(local_flags),
    })
  }
}

fn jupyter_parse(flags: &mut Flags, matches: &mut ArgMatches) {
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
  let root = matches.remove_one::<String>("root");
  let global = matches.get_flag("global");
  let name = matches.remove_one::<String>("name").unwrap();
  flags.subcommand = DenoSubcommand::Uninstall(UninstallFlags {
    // TODO(bartlomieju): remove once `deno uninstall` supports both local and
    // global installs
    global,
    kind: UninstallKind::Global(UninstallFlagsGlobal { name, root }),
  });
}

fn lsp_parse(flags: &mut Flags, _matches: &mut ArgMatches) {
  flags.subcommand = DenoSubcommand::Lsp;
}

fn lint_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  config_args_parse(flags, matches);
  let files = match matches.remove_many::<String>("files") {
    Some(f) => f.collect(),
    None => vec![],
  };
  let ignore = match matches.remove_many::<String>("ignore") {
    Some(f) => f.collect(),
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
    watch: watch_arg_parse(matches),
  });
}

fn repl_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  runtime_args_parse(flags, matches, true, true);
  unsafely_ignore_certificate_errors_parse(flags, matches);

  let eval_files = matches
    .remove_many::<String>("eval-file")
    .map(|values| values.collect());

  handle_repl_flags(
    flags,
    ReplFlags {
      eval_files,
      eval: matches.remove_one::<String>("eval"),
      is_default_command: false,
    },
  );
}

fn run_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
  app: Command,
) -> clap::error::Result<()> {
  runtime_args_parse(flags, matches, true, true);

  flags.code_cache_enabled = !matches.get_flag("no-code-cache");

  let mut script_arg =
    matches.remove_many::<String>("script_arg").ok_or_else(|| {
      let mut app = app;
      let subcommand = &mut app.find_subcommand_mut("run").unwrap();
      subcommand.error(
        clap::error::ErrorKind::MissingRequiredArgument,
        "[SCRIPT_ARG] may only be omitted with --v8-flags=--help",
      )
    })?;

  let script = script_arg.next().unwrap();
  flags.argv.extend(script_arg);

  ext_arg_parse(flags, matches);

  flags.subcommand = DenoSubcommand::Run(RunFlags {
    script,
    watch: watch_arg_parse_with_paths(matches),
  });

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

  runtime_args_parse(flags, matches, true, true);
  // If the user didn't pass --allow-net, add this port to the network
  // allowlist. If the host is 0.0.0.0, we add :{port} and allow the same network perms
  // as if it was passed to --allow-net directly.
  let allowed = flags_net::parse(vec![if host == "0.0.0.0" {
    format!(":{port}")
  } else {
    format!("{host}:{port}")
  }])?;
  match &mut flags.permissions.allow_net {
    None => flags.permissions.allow_net = Some(allowed),
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
    watch: watch_arg_parse_with_paths(matches),
    port,
    host,
  });

  Ok(())
}

fn task_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.config_flag = matches
    .remove_one::<String>("config")
    .map(ConfigFlag::Path)
    .unwrap_or(ConfigFlag::Discover);

  let mut task_flags = TaskFlags {
    cwd: matches.remove_one::<String>("cwd"),
    task: None,
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

fn test_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.type_check_mode = TypeCheckMode::Local;
  runtime_args_parse(flags, matches, true, true);
  // NOTE: `deno test` always uses `--no-prompt`, tests shouldn't ever do
  // interactive prompts, unless done by user code
  flags.permissions.no_prompt = true;

  let ignore = match matches.remove_many::<String>("ignore") {
    Some(f) => f.collect(),
    None => vec![],
  };

  let no_run = matches.get_flag("no-run");
  let trace_leaks =
    matches.get_flag("trace-ops") || matches.get_flag("trace-leaks");

  #[allow(clippy::print_stderr)]
  if trace_leaks && matches.get_flag("trace-ops") {
    // We can't change this to use the log crate because its not configured
    // yet at this point since the flags haven't been parsed. This flag is
    // deprecated though so it's not worth changing the code to use the log
    // crate here and this is only done for testing anyway.
    eprintln!(
      "⚠️ {}",
      crate::colors::yellow("The `--trace-ops` flag is deprecated and will be removed in Deno 2.0.\nUse the `--trace-leaks` flag instead."),
    );
  }
  let doc = matches.get_flag("doc");
  let allow_none = matches.get_flag("allow-none");
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

  let concurrent_jobs = if matches.get_flag("parallel") {
    if let Ok(value) = env::var("DENO_JOBS") {
      value.parse::<NonZeroUsize>().ok()
    } else {
      std::thread::available_parallelism().ok()
    }
  } else if matches.contains_id("jobs") {
    // We can't change this to use the log crate because its not configured
    // yet at this point since the flags haven't been parsed. This flag is
    // deprecated though so it's not worth changing the code to use the log
    // crate here and this is only done for testing anyway.
    #[allow(clippy::print_stderr)]
    {
      eprintln!(
        "⚠️ {}",
        crate::colors::yellow(concat!(
          "The `--jobs` flag is deprecated and will be removed in Deno 2.0.\n",
          "Use the `--parallel` flag with possibly the `DENO_JOBS` environment variable instead.\n",
          "Learn more at: https://docs.deno.com/runtime/manual/basics/env_variables"
        )),
      );
    }
    if let Some(value) = matches.remove_one::<NonZeroUsize>("jobs") {
      Some(value)
    } else {
      std::thread::available_parallelism().ok()
    }
  } else {
    None
  };

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

  flags.subcommand = DenoSubcommand::Test(TestFlags {
    no_run,
    doc,
    coverage_dir: matches.remove_one::<String>("coverage"),
    clean,
    fail_fast,
    files: FileFlags { include, ignore },
    filter,
    shuffle,
    allow_none,
    concurrent_jobs,
    trace_leaks,
    watch: watch_arg_parse(matches),
    reporter,
    junit_path,
  });
}

fn types_parse(flags: &mut Flags, _matches: &mut ArgMatches) {
  flags.subcommand = DenoSubcommand::Types;
}

fn upgrade_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  ca_file_arg_parse(flags, matches);

  let dry_run = matches.get_flag("dry-run");
  let force = matches.get_flag("force");
  let canary = matches.get_flag("canary");
  let version = matches.remove_one::<String>("version");
  let output = matches.remove_one::<String>("output");
  flags.subcommand = DenoSubcommand::Upgrade(UpgradeFlags {
    dry_run,
    force,
    canary,
    version,
    output,
  });
}

fn vendor_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  ca_file_arg_parse(flags, matches);
  config_args_parse(flags, matches);
  import_map_arg_parse(flags, matches);
  lock_arg_parse(flags, matches);
  node_modules_and_vendor_dir_arg_parse(flags, matches);
  reload_arg_parse(flags, matches);

  flags.subcommand = DenoSubcommand::Vendor(VendorFlags {
    specifiers: matches
      .remove_many::<String>("specifiers")
      .map(|p| p.collect())
      .unwrap_or_default(),
    output_path: matches.remove_one::<String>("output"),
    force: matches.get_flag("force"),
  });
}

fn publish_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.type_check_mode = TypeCheckMode::Local; // local by default
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

fn compile_args_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  compile_args_without_check_parse(flags, matches);
  no_check_arg_parse(flags, matches);
  check_arg_parse(flags, matches);
}

fn compile_args_without_check_parse(
  flags: &mut Flags,
  matches: &mut ArgMatches,
) {
  import_map_arg_parse(flags, matches);
  no_remote_arg_parse(flags, matches);
  no_npm_arg_parse(flags, matches);
  node_modules_and_vendor_dir_arg_parse(flags, matches);
  config_args_parse(flags, matches);
  reload_arg_parse(flags, matches);
  lock_args_parse(flags, matches);
  ca_file_arg_parse(flags, matches);
}

fn permission_args_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  unsafely_ignore_certificate_errors_parse(flags, matches);
  if let Some(read_wl) = matches.remove_many::<String>("allow-read") {
    flags.permissions.allow_read = Some(read_wl.collect());
  }

  if let Some(read_wl) = matches.remove_many::<String>("deny-read") {
    flags.permissions.deny_read = Some(read_wl.collect());
  }

  if let Some(write_wl) = matches.remove_many::<String>("allow-write") {
    flags.permissions.allow_write = Some(write_wl.collect());
  }

  if let Some(write_wl) = matches.remove_many::<String>("deny-write") {
    flags.permissions.deny_write = Some(write_wl.collect());
  }

  if let Some(net_wl) = matches.remove_many::<String>("allow-net") {
    let net_allowlist = flags_net::parse(net_wl.collect()).unwrap();
    flags.permissions.allow_net = Some(net_allowlist);
  }

  if let Some(net_wl) = matches.remove_many::<String>("deny-net") {
    let net_denylist = flags_net::parse(net_wl.collect()).unwrap();
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
    flags.permissions.allow_ffi = Some(ffi_wl.collect());
    debug!("ffi allowlist: {:#?}", &flags.permissions.allow_ffi);
  }

  if let Some(ffi_wl) = matches.remove_many::<String>("deny-ffi") {
    flags.permissions.deny_ffi = Some(ffi_wl.collect());
    debug!("ffi denylist: {:#?}", &flags.permissions.deny_ffi);
  }

  if matches.get_flag("allow-hrtime") {
    flags.permissions.allow_hrtime = true;
  }

  if matches.get_flag("deny-hrtime") {
    flags.permissions.deny_hrtime = true;
  }

  if matches.get_flag("allow-all") {
    flags.allow_all();
  }

  if matches.get_flag("no-prompt") {
    flags.permissions.no_prompt = true;
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
) {
  compile_args_parse(flags, matches);
  cached_only_arg_parse(flags, matches);
  if include_perms {
    permission_args_parse(flags, matches);
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
  eszip_arg_parse(flags, matches);
}

fn inspect_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  let default = || "127.0.0.1:9229".parse::<SocketAddr>().unwrap();
  flags.inspect = if matches.contains_id("inspect") {
    Some(
      matches
        .remove_one::<SocketAddr>("inspect")
        .unwrap_or_else(default),
    )
  } else {
    None
  };
  flags.inspect_brk = if matches.contains_id("inspect-brk") {
    Some(
      matches
        .remove_one::<SocketAddr>("inspect-brk")
        .unwrap_or_else(default),
    )
  } else {
    None
  };
  flags.inspect_wait = if matches.contains_id("inspect-wait") {
    Some(
      matches
        .remove_one::<SocketAddr>("inspect-wait")
        .unwrap_or_else(default),
    )
  } else {
    None
  };
}

fn import_map_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.import_map_path = matches.remove_one::<String>("import-map");
}

fn env_file_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  flags.env_file = matches.remove_one::<String>("env");
}

fn reload_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if let Some(cache_bl) = matches.remove_many::<String>("reload") {
    let raw_cache_blocklist: Vec<String> = cache_bl.collect();
    if raw_cache_blocklist.is_empty() {
      flags.reload = true;
    } else {
      flags.cache_blocklist = resolve_urls(raw_cache_blocklist);
      debug!("cache blocklist: {:#?}", &flags.cache_blocklist);
      flags.reload = false;
    }
  }
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

fn eszip_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if matches.get_flag("eszip-internal-do-not-use") {
    flags.eszip = true;
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
  if matches.get_flag("lock-write") {
    flags.lock_write = true;
  }
}

fn lock_arg_parse(flags: &mut Flags, matches: &mut ArgMatches) {
  if matches.contains_id("lock") {
    let lockfile = matches
      .remove_one::<String>("lock")
      .unwrap_or_else(|| String::from("./deno.lock"));
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
  flags.node_modules_dir = matches.remove_one::<bool>("node-modules-dir");
  flags.vendor = matches.remove_one::<bool>("vendor");
}

fn reload_arg_validate(urlstr: &str) -> Result<String, String> {
  if urlstr.is_empty() {
    return Err(String::from("Missing url. Check for extra commas."));
  }
  match Url::from_str(urlstr) {
    Ok(_) => Ok(urlstr.to_string()),
    Err(e) => Err(e.to_string()),
  }
}

fn watch_arg_parse(matches: &mut ArgMatches) -> Option<WatchFlags> {
  if matches.get_flag("watch") {
    Some(WatchFlags {
      hmr: false,
      no_clear_screen: matches.get_flag("no-clear-screen"),
      exclude: matches
        .remove_many::<String>("watch-exclude")
        .map(|f| f.collect::<Vec<String>>())
        .unwrap_or_default(),
    })
  } else {
    None
  }
}

fn watch_arg_parse_with_paths(
  matches: &mut ArgMatches,
) -> Option<WatchFlagsWithPaths> {
  if let Some(paths) = matches.remove_many::<String>("watch") {
    return Some(WatchFlagsWithPaths {
      paths: paths.collect(),
      hmr: false,
      no_clear_screen: matches.get_flag("no-clear-screen"),
      exclude: matches
        .remove_many::<String>("watch-exclude")
        .map(|f| f.collect::<Vec<String>>())
        .unwrap_or_default(),
    });
  }

  matches
    .remove_many::<String>("hmr")
    .map(|paths| WatchFlagsWithPaths {
      paths: paths.collect(),
      hmr: true,
      no_clear_screen: matches.get_flag("no-clear-screen"),
      exclude: matches
        .remove_many::<String>("watch-exclude")
        .map(|f| f.collect::<Vec<String>>())
        .unwrap_or_default(),
    })
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
    let r = flags_from_vec(svec!["deno", "--unstable", "--log-level", "debug", "--quiet", "run", "script.ts"]);

    let flags = r.unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string()
        )),
        unstable_config: UnstableConfig {
          legacy_flag_enabled: true,
          ..Default::default()
        },
        log_level: Some(Level::Error),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
    #[rustfmt::skip]
    let r2 = flags_from_vec(svec!["deno", "run", "--unstable", "--log-level", "debug", "--quiet", "script.ts"]);
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
          version: None,
          output: None,
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
          version: None,
          output: Some(String::from("example.txt")),
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
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

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
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--unstable-hmr=foo.txt",
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
    let r =
      flags_from_vec(svec!["deno", "run", "--watch=file1,file2", "script.ts"]);
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
      "run",
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
        }),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
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
    assert!(r
      .unwrap_err()
      .to_string()
      .contains("[SCRIPT_ARG] may only be omitted with --v8-flags=--help"));
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
    let r = flags_from_vec(svec!["deno", "run", "--allow-read", "x.ts"]);
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

    let r = flags_from_vec(svec!["deno", "run", "x.ts", "--deny-read"]);
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
          allow_net: Some(vec![]),
          allow_env: Some(vec![]),
          allow_run: Some(vec![]),
          allow_read: Some(vec![]),
          allow_sys: Some(vec![]),
          allow_write: Some(vec![]),
          allow_ffi: Some(vec![]),
          allow_hrtime: true,
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
  fn deny_read() {
    let r = flags_from_vec(svec!["deno", "run", "--deny-read", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "gist.ts".to_string()
        )),
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
  fn allow_hrtime() {
    let r = flags_from_vec(svec!["deno", "run", "--allow-hrtime", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "gist.ts".to_string(),
        )),
        permissions: PermissionFlags {
          allow_hrtime: true,
          ..Default::default()
        },
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn deny_hrtime() {
    let r = flags_from_vec(svec!["deno", "run", "--deny-hrtime", "gist.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "gist.ts".to_string(),
        )),
        permissions: PermissionFlags {
          deny_hrtime: true,
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
          watch: Default::default(),
        }),
        ext: Some("ts".to_string()),
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
          watch: Default::default(),
        }),
        ext: Some("ts".to_string()),
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
          watch: Default::default(),
        }),
        ext: Some("ts".to_string()),
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
          watch: Some(Default::default()),
        }),
        ext: Some("ts".to_string()),
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "fmt", "--watch", "--no-clear-screen"]);
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
          watch: Some(WatchFlags {
            hmr: false,
            no_clear_screen: true,
            exclude: vec![],
          })
        }),
        ext: Some("ts".to_string()),
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
          watch: Some(Default::default()),
        }),
        ext: Some("ts".to_string()),
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
          watch: Default::default(),
        }),
        ext: Some("ts".to_string()),
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
          watch: Some(Default::default()),
        }),
        config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
        ext: Some("ts".to_string()),
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
          watch: Default::default(),
        }),
        ext: Some("ts".to_string()),
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
          watch: Default::default(),
        }),
        ext: Some("ts".to_string()),
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
          })
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
        }),
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );

    for all_flag in ["--remote", "--all"] {
      let r = flags_from_vec(svec!["deno", "check", all_flag, "script.ts"]);
      assert_eq!(
        r.unwrap(),
        Flags {
          subcommand: DenoSubcommand::Check(CheckFlags {
            files: svec!["script.ts"],
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
          allow_net: Some(vec![]),
          allow_env: Some(vec![]),
          allow_run: Some(vec![]),
          allow_read: Some(vec![]),
          allow_sys: Some(vec![]),
          allow_write: Some(vec![]),
          allow_ffi: Some(vec![]),
          allow_hrtime: true,
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
          allow_net: Some(vec![]),
          allow_env: Some(vec![]),
          allow_run: Some(vec![]),
          allow_read: Some(vec![]),
          allow_sys: Some(vec![]),
          allow_write: Some(vec![]),
          allow_ffi: Some(vec![]),
          allow_hrtime: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn eval_typescript() {
    let r =
      flags_from_vec(svec!["deno", "eval", "-T", "'console.log(\"hello\")'"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Eval(EvalFlags {
          print: false,
          code: "'console.log(\"hello\")'".to_string(),
        }),
        permissions: PermissionFlags {
          allow_all: true,
          allow_net: Some(vec![]),
          allow_env: Some(vec![]),
          allow_run: Some(vec![]),
          allow_read: Some(vec![]),
          allow_sys: Some(vec![]),
          allow_write: Some(vec![]),
          allow_ffi: Some(vec![]),
          allow_hrtime: true,
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
    let r = flags_from_vec(svec!["deno", "eval", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--reload", "--lock", "lock.json", "--lock-write", "--cert", "example.crt", "--cached-only", "--location", "https:foo", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "--env=.example.env", "42"]);
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
        lock_write: true,
        ca_data: Some(CaData::File("example.crt".to_string())),
        cached_only: true,
        location: Some(Url::parse("https://foo/").unwrap()),
        v8_flags: svec!["--help", "--random-seed=1"],
        seed: Some(1),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        permissions: PermissionFlags {
          allow_all: true,
          allow_net: Some(vec![]),
          allow_env: Some(vec![]),
          allow_run: Some(vec![]),
          allow_read: Some(vec![]),
          allow_sys: Some(vec![]),
          allow_write: Some(vec![]),
          allow_ffi: Some(vec![]),
          allow_hrtime: true,
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
          allow_net: Some(vec![]),
          allow_env: Some(vec![]),
          allow_run: Some(vec![]),
          allow_read: Some(vec![]),
          allow_sys: Some(vec![]),
          allow_write: Some(vec![]),
          allow_ffi: Some(vec![]),
          allow_hrtime: true,
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
          allow_net: Some(vec![]),
          allow_env: Some(vec![]),
          deny_env: None,
          allow_run: Some(vec![]),
          deny_run: None,
          allow_read: Some(vec![]),
          deny_read: None,
          allow_sys: Some(vec![]),
          deny_sys: None,
          allow_write: Some(vec![]),
          deny_write: None,
          allow_ffi: Some(vec![]),
          deny_ffi: None,
          allow_hrtime: true,
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
    let r = flags_from_vec(svec!["deno", "repl", "-A", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--reload", "--lock", "lock.json", "--lock-write", "--cert", "example.crt", "--cached-only", "--location", "https:foo", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "--unsafely-ignore-certificate-errors", "--env=.example.env"]);
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
        lock_write: true,
        ca_data: Some(CaData::File("example.crt".to_string())),
        cached_only: true,
        location: Some(Url::parse("https://foo/").unwrap()),
        v8_flags: svec!["--help", "--random-seed=1"],
        seed: Some(1),
        inspect: Some("127.0.0.1:9229".parse().unwrap()),
        permissions: PermissionFlags {
          allow_all: true,
          allow_net: Some(vec![]),
          allow_env: Some(vec![]),
          allow_run: Some(vec![]),
          allow_read: Some(vec![]),
          allow_sys: Some(vec![]),
          allow_write: Some(vec![]),
          allow_ffi: Some(vec![]),
          allow_hrtime: true,
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
    let r =
      flags_from_vec(svec!["deno", "run", "--deny-net=127.0.0.1", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
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
    let r =
      flags_from_vec(svec!["deno", "run", "--allow-env=H=ME", "script.ts"]);
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
    let r =
      flags_from_vec(svec!["deno", "run", "--deny-env=H\0ME", "script.ts"]);
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
    let r =
      flags_from_vec(svec!["deno", "run", "--deny-sys=hostname", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
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

    let r = flags_from_vec(svec!["deno", "run", "--reload=/", "script.ts"]);
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

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--reload=http://deno.land/a,,http://deno.land/b",
      "script.ts"
    ]);
    assert!(r.is_err(), "Should reject adjacent commas");
  }

  #[test]
  fn bundle() {
    let r = flags_from_vec(svec!["deno", "bundle", "source.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bundle(BundleFlags {
          source_file: "source.ts".to_string(),
          out_file: None,
          watch: Default::default(),
        }),
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle_with_config() {
    let r = flags_from_vec(svec![
      "deno",
      "bundle",
      "--no-remote",
      "--config",
      "tsconfig.json",
      "source.ts",
      "bundle.js"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bundle(BundleFlags {
          source_file: "source.ts".to_string(),
          out_file: Some("bundle.js".to_string()),
          watch: Default::default(),
        }),
        permissions: PermissionFlags {
          allow_write: Some(vec![]),
          ..Default::default()
        },
        no_remote: true,
        type_check_mode: TypeCheckMode::Local,
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle_with_output() {
    let r = flags_from_vec(svec!["deno", "bundle", "source.ts", "bundle.js"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bundle(BundleFlags {
          source_file: "source.ts".to_string(),
          out_file: Some("bundle.js".to_string()),
          watch: Default::default(),
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          allow_write: Some(vec![]),
          ..Default::default()
        },
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle_with_lock() {
    let r = flags_from_vec(svec![
      "deno",
      "bundle",
      "--lock-write",
      "--lock=lock.json",
      "source.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bundle(BundleFlags {
          source_file: "source.ts".to_string(),
          out_file: None,
          watch: Default::default(),
        }),
        type_check_mode: TypeCheckMode::Local,
        lock_write: true,
        lock: Some(String::from("lock.json")),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle_with_reload() {
    let r = flags_from_vec(svec!["deno", "bundle", "--reload", "source.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        reload: true,
        subcommand: DenoSubcommand::Bundle(BundleFlags {
          source_file: "source.ts".to_string(),
          out_file: None,
          watch: Default::default(),
        }),
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle_nocheck() {
    let r = flags_from_vec(svec!["deno", "bundle", "--no-check", "script.ts"])
      .unwrap();
    assert_eq!(
      r,
      Flags {
        subcommand: DenoSubcommand::Bundle(BundleFlags {
          source_file: "script.ts".to_string(),
          out_file: None,
          watch: Default::default(),
        }),
        type_check_mode: TypeCheckMode::None,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn bundle_watch() {
    let r = flags_from_vec(svec!["deno", "bundle", "--watch", "source.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bundle(BundleFlags {
          source_file: "source.ts".to_string(),
          out_file: None,
          watch: Some(Default::default()),
        }),
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    )
  }

  #[test]
  fn bundle_watch_with_no_clear_screen() {
    let r = flags_from_vec(svec![
      "deno",
      "bundle",
      "--watch",
      "--no-clear-screen",
      "source.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bundle(BundleFlags {
          source_file: "source.ts".to_string(),
          out_file: None,
          watch: Some(WatchFlags {
            hmr: false,
            no_clear_screen: true,
            exclude: vec![],
          }),
        }),
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    )
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
  fn run_env_file_default() {
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
  fn run_no_code_cache() {
    let r =
      flags_from_vec(svec!["deno", "run", "--no-code-cache", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn run_env_file_defined() {
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
    let r =
      flags_from_vec(svec!["deno", "install", "jsr:@std/http/file-server"]);
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
          global: false,
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
          global: true,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn install_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "install", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--unsafely-ignore-certificate-errors", "--reload", "--lock", "lock.json", "--lock-write", "--cert", "example.crt", "--cached-only", "--allow-read", "--allow-net", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "--name", "file_server", "--root", "/foo", "--force", "--env=.example.env", "jsr:@std/http/file-server", "foo", "bar"]);
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
          global: false,
        }),
        import_map_path: Some("import_map.json".to_string()),
        no_remote: true,
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        type_check_mode: TypeCheckMode::None,
        reload: true,
        lock: Some(String::from("lock.json")),
        lock_write: true,
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
    let r = flags_from_vec(svec!["deno", "uninstall", "file_server"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Uninstall(UninstallFlags {
          kind: UninstallKind::Global(UninstallFlagsGlobal {
            name: "file_server".to_string(),
            root: None,
          }),
          global: false,
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
          global: true,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn uninstall_with_help_flag() {
    let r = flags_from_vec(svec!["deno", "uninstall", "--help"]);
    assert_eq!(r.err().unwrap().kind(), clap::error::ErrorKind::DisplayHelp);
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
    let r = flags_from_vec(svec!["deno", "run", "-q", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
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
    let r = flags_from_vec(svec!["deno", "run", "--no-check", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
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
      "--unsafely-ignore-certificate-errors=deno.land,localhost,::,127.0.0.1,[::1],1.2.3.4",
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
          "::",
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
      "--unsafely-ignore-certificate-errors=deno.land,localhost,::,127.0.0.1,[::1],1.2.3.4"]);
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
          "::",
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
    let r =
      flags_from_vec(svec!["deno", "run", "--node-modules-dir", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        node_modules_dir: Some(true),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--node-modules-dir=false",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        node_modules_dir: Some(false),
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
      "--allow-net=deno.land,deno.land:80,::,127.0.0.1,[::1],1.2.3.4:5678,:5678,[::1]:8080",
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
            "::",
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
      "--deny-net=deno.land,deno.land:80,::,127.0.0.1,[::1],1.2.3.4:5678,:5678,[::1]:8080",
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
            "::",
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
  fn lock_write() {
    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--lock-write",
      "--lock=lock.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        lock_write: true,
        lock: Some(String::from("lock.json")),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "run", "--no-lock", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        no_lock: true,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--lock",
      "--lock-write",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        lock_write: true,
        lock: Some(String::from("./deno.lock")),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--lock-write",
      "--lock",
      "lock.json",
      "script.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        lock_write: true,
        lock: Some(String::from("lock.json")),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "run", "--lock-write", "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        lock_write: true,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    let r =
      flags_from_vec(svec!["deno", "run", "--lock", "--no-lock", "script.ts"]);
    assert!(r.is_err(),);

    let r = flags_from_vec(svec![
      "deno",
      "run",
      "--lock-write",
      "--no-lock",
      "script.ts"
    ]);
    assert!(r.is_err(),);
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
    let r = flags_from_vec(svec!["deno", "test", "--unstable", "--no-npm", "--no-remote", "--trace-leaks", "--no-run", "--filter", "- foo", "--coverage=cov", "--clean", "--location", "https:foo", "--allow-net", "--allow-none", "dir1/", "dir2/", "--", "arg1", "arg2"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: true,
          doc: false,
          fail_fast: None,
          filter: Some("- foo".to_string()),
          allow_none: true,
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
        }),
        unstable_config: UnstableConfig {
          legacy_flag_enabled: true,
          ..Default::default()
        },
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
  fn test_with_concurrent_jobs() {
    let r = flags_from_vec(svec!["deno", "test", "--jobs=4"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Test(TestFlags {
          no_run: false,
          reporter: Default::default(),
          doc: false,
          fail_fast: None,
          filter: None,
          allow_none: false,
          shuffle: None,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          concurrent_jobs: Some(NonZeroUsize::new(4).unwrap()),
          trace_leaks: false,
          coverage_dir: None,
          clean: false,
          watch: Default::default(),
          junit_path: None,
        }),
        type_check_mode: TypeCheckMode::Local,
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "test", "--jobs=0"]);
    assert!(r.is_err());
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
          allow_none: false,
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
          allow_none: false,
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
          allow_none: false,
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
          allow_none: false,
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
          allow_none: false,
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
          allow_none: false,
          shuffle: None,
          files: FileFlags {
            include: vec![],
            ignore: vec![],
          },
          concurrent_jobs: None,
          trace_leaks: false,
          coverage_dir: None,
          clean: false,
          watch: Some(WatchFlags {
            hmr: false,
            no_clear_screen: true,
            exclude: vec![],
          }),
          reporter: Default::default(),
          junit_path: None,
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
  fn bundle_with_cafile() {
    let r = flags_from_vec(svec![
      "deno",
      "bundle",
      "--cert",
      "example.crt",
      "source.ts"
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Bundle(BundleFlags {
          source_file: "source.ts".to_string(),
          out_file: None,
          watch: Default::default(),
        }),
        type_check_mode: TypeCheckMode::Local,
        ca_data: Some(CaData::File("example.crt".to_owned())),
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
          version: None,
          output: None,
        }),
        ca_data: Some(CaData::File("example.crt".to_owned())),
        ..Flags::default()
      }
    );
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
    let r = flags_from_vec(svec!["deno", "run", "--inspect-wait", "foo.js"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "foo.js".to_string(),
        )),
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
          include: vec![],
          eszip: false,
        }),
        type_check_mode: TypeCheckMode::Local,
        ..Flags::default()
      }
    );
  }

  #[test]
  fn compile_with_flags() {
    #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "compile", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--unsafely-ignore-certificate-errors", "--reload", "--lock", "lock.json", "--lock-write", "--cert", "example.crt", "--cached-only", "--location", "https:foo", "--allow-read", "--allow-net", "--v8-flags=--help", "--seed", "1", "--no-terminal", "--output", "colors", "--env=.example.env", "https://examples.deno.land/color-logging.ts", "foo", "bar", "-p", "8080"]);
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
          include: vec![],
          eszip: false,
        }),
        import_map_path: Some("import_map.json".to_string()),
        no_remote: true,
        config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
        type_check_mode: TypeCheckMode::None,
        reload: true,
        lock: Some(String::from("lock.json")),
        lock_write: true,
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
    assert_eq!(flags.config_path_args(&cwd), Some(vec![cwd.join("foo.js")]));

    let flags =
      flags_from_vec(svec!["deno", "run", "https://example.com/foo.js"])
        .unwrap();
    assert_eq!(flags.config_path_args(&cwd), None);

    let flags =
      flags_from_vec(svec!["deno", "lint", "dir/a.js", "dir/b.js"]).unwrap();
    assert_eq!(
      flags.config_path_args(&cwd),
      Some(vec![cwd.join("dir/a.js"), cwd.join("dir/b.js")])
    );

    let flags = flags_from_vec(svec!["deno", "lint"]).unwrap();
    assert!(flags.config_path_args(&cwd).unwrap().is_empty());

    let flags =
      flags_from_vec(svec!["deno", "fmt", "dir/a.js", "dir/b.js"]).unwrap();
    assert_eq!(
      flags.config_path_args(&cwd),
      Some(vec![cwd.join("dir/a.js"), cwd.join("dir/b.js")])
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
  fn vendor_minimal() {
    let r = flags_from_vec(svec!["deno", "vendor", "mod.ts",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Vendor(VendorFlags {
          specifiers: svec!["mod.ts"],
          force: false,
          output_path: None,
        }),
        ..Flags::default()
      }
    );
  }

  #[test]
  fn vendor_all() {
    let r = flags_from_vec(svec![
      "deno",
      "vendor",
      "--config",
      "deno.json",
      "--import-map",
      "import_map.json",
      "--lock",
      "lock.json",
      "--force",
      "--output",
      "out_dir",
      "--reload",
      "mod.ts",
      "deps.test.ts",
    ]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Vendor(VendorFlags {
          specifiers: svec!["mod.ts", "deps.test.ts"],
          force: true,
          output_path: Some(String::from("out_dir")),
        }),
        config_flag: ConfigFlag::Path("deno.json".to_owned()),
        import_map_path: Some("import_map.json".to_string()),
        lock: Some(String::from("lock.json")),
        reload: true,
        ..Flags::default()
      }
    );
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
        }),
        argv: svec!["--test"],
        ..Flags::default()
      }
    );
  }

  #[test]
  fn task_with_global_flags() {
    // can fail if the custom parser in task_parse() starts at the wrong index
    let r =
      flags_from_vec(svec!["deno", "--quiet", "--unstable", "task", "build"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Task(TaskFlags {
          cwd: None,
          task: Some("build".to_string()),
        }),
        unstable_config: UnstableConfig {
          legacy_flag_enabled: true,
          ..Default::default()
        },
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
      "--unstable",
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
        unstable_config: UnstableConfig {
          legacy_flag_enabled: true,
          ..Default::default()
        },
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

    let r = flags_from_vec(svec!["deno", "run", "--check=foo", "script.ts",]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
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
        subcommand: DenoSubcommand::Init(InitFlags { dir: None }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "foo"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags {
          dir: Some(String::from("foo")),
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "init", "--quiet"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Init(InitFlags { dir: None }),
        log_level: Some(Level::Error),
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
  fn add_subcommand() {
    let r = flags_from_vec(svec!["deno", "add"]);
    r.unwrap_err();

    let r = flags_from_vec(svec!["deno", "add", "@david/which"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Add(AddFlags {
          packages: svec!["@david/which"],
        }),
        ..Flags::default()
      }
    );

    let r = flags_from_vec(svec!["deno", "add", "@david/which", "@luca/hello"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Add(AddFlags {
          packages: svec!["@david/which", "@luca/hello"],
        }),
        ..Flags::default()
      }
    );
  }
}
