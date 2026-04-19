#![allow(dead_code)]

// Simplified versions of Deno's CLI flag types.
// External dependencies are replaced with simpler types:
// - Url, PathBuf, SocketAddr -> String
// - NonZeroU32 -> u32, NonZeroU8 -> u8, NonZeroUsize -> usize
// - log::Level -> LogLevel enum
// - clap::builder::StyledStr -> String
// Self-contained: no external crate dependencies.

// ---------------------------------------------------------------------------
// Simple replacement enums for external types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogLevel {
  Trace,
  Debug,
  Info,
  Warn,
  Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaData {
  /// The string is a file path.
  File(String),
  /// The bytes hold the actual certificate data.
  Bytes(Vec<u8>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeModulesDirMode {
  Auto,
  Manual,
  None,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum PackagesAllowedScripts {
  All,
  Some(Vec<String>),
  #[default]
  None,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InspectPublishUid {
  pub console: bool,
  pub http: bool,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct UnstableConfig {
  pub legacy_flag_enabled: bool,
  pub bare_node_builtins: bool,
  pub detect_cjs: bool,
  pub lazy_dynamic_imports: bool,
  pub raw_imports: bool,
  pub sloppy_imports: bool,
  pub npm_lazy_caching: bool,
  pub tsgo: bool,
  pub features: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BundleFormat {
  #[default]
  Esm,
  Cjs,
  Iife,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BundlePlatform {
  Browser,
  #[default]
  Deno,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PackageHandling {
  #[default]
  Bundle,
  External,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SourceMapType {
  #[default]
  Linked,
  Inline,
  External,
}

// ---------------------------------------------------------------------------
// Config flag
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ConfigFlag {
  #[default]
  Discover,
  Path(String),
  Disabled,
}

// ---------------------------------------------------------------------------
// File flags (shared by several subcommands)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileFlags {
  pub ignore: Vec<String>,
  pub include: Vec<String>,
}

// ---------------------------------------------------------------------------
// DefaultRegistry
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultRegistry {
  Npm,
  Jsr,
}

// ---------------------------------------------------------------------------
// Subcommand flag structs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AddFlags {
  pub packages: Vec<String>,
  pub dev: bool,
  pub default_registry: Option<DefaultRegistry>,
  pub lockfile_only: bool,
  pub save_exact: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AuditFlags {
  pub severity: String,
  pub ignore_registry_errors: bool,
  pub ignore_unfixable: bool,
  pub dev: bool,
  pub prod: bool,
  pub optional: bool,
  pub ignore: Vec<String>,
  pub socket: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RemoveFlags {
  pub packages: Vec<String>,
  pub lockfile_only: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BenchFlags {
  pub files: FileFlags,
  pub filter: Option<String>,
  pub json: bool,
  pub no_run: bool,
  pub permit_no_files: bool,
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
  pub check_js: bool,
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
  pub exclude: Vec<String>,
  pub eszip: bool,
  pub self_extracting: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum CoverageType {
  #[default]
  Summary,
  Detailed,
  Lcov,
  Html,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CoverageFlags {
  pub files: FileFlags,
  pub output: Option<String>,
  pub include: Vec<String>,
  pub exclude: Vec<String>,
  pub r#type: CoverageType,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeployFlags {
  pub sandbox: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum DocSourceFileFlag {
  #[default]
  Builtin,
  Paths(Vec<String>),
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CpuProfFlags {
  pub dir: Option<String>,
  pub name: Option<String>,
  pub interval: Option<u32>,
  pub md: bool,
  pub flamegraph: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EvalFlags {
  pub print: bool,
  pub code: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FmtFlags {
  pub check: bool,
  pub fail_fast: bool,
  pub files: FileFlags,
  pub permit_no_files: bool,
  pub use_tabs: Option<bool>,
  pub line_width: Option<u32>,
  pub indent_width: Option<u8>,
  pub single_quote: Option<bool>,
  pub prose_wrap: Option<String>,
  pub no_semicolons: Option<bool>,
  pub watch: Option<WatchFlags>,
  pub unstable_component: bool,
  pub unstable_sql: bool,
}

impl FmtFlags {
  pub fn is_stdin(&self) -> bool {
    let args = &self.files.include;
    args.len() == 1 && args[0] == "-"
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitFlags {
  pub package: Option<String>,
  pub package_args: Vec<String>,
  pub dir: Option<String>,
  pub lib: bool,
  pub serve: bool,
  pub empty: bool,
  pub yes: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InfoFlags {
  pub json: bool,
  pub file: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallFlagsGlobal {
  pub module_urls: Vec<String>,
  pub args: Vec<String>,
  pub name: Option<String>,
  pub root: Option<String>,
  pub force: bool,
  pub compile: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallFlags {
  Local(InstallFlagsLocal),
  Global(InstallFlagsGlobal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallFlagsLocal {
  Add(AddFlags),
  TopLevel(InstallTopLevelFlags),
  Entrypoints(InstallEntrypointsFlags),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallTopLevelFlags {
  pub lockfile_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallEntrypointsFlags {
  pub entrypoints: Vec<String>,
  pub lockfile_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JupyterFlags {
  pub install: bool,
  pub name: Option<String>,
  pub display: Option<String>,
  pub kernel: bool,
  pub conn_file: Option<String>,
  pub force: bool,
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
  pub permit_no_files: bool,
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReplFlags {
  pub eval_files: Option<Vec<String>>,
  pub eval: Option<String>,
  pub is_default_command: bool,
  pub json: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RunFlags {
  pub script: String,
  pub watch: Option<WatchFlagsWithPaths>,
  pub bare: bool,
  pub coverage_dir: Option<String>,
  pub print_task_list: bool,
}

impl RunFlags {
  #[cfg(test)]
  pub fn new_default(script: String) -> Self {
    Self {
      script,
      watch: None,
      bare: false,
      coverage_dir: None,
      print_task_list: false,
    }
  }

  pub fn is_stdin(&self) -> bool {
    self.script == "-"
  }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum DenoXShimName {
  #[default]
  Dx,
  Denox,
  Dnx,
  Other(String),
}

impl DenoXShimName {
  pub fn name(&self) -> &str {
    match self {
      Self::Dx => "dx",
      Self::Denox => "denox",
      Self::Dnx => "dnx",
      Self::Other(name) => name,
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XFlagsKind {
  InstallAlias(DenoXShimName),
  Command(XCommandFlags),
  Print,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XCommandFlags {
  pub yes: bool,
  pub command: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XFlags {
  pub kind: XFlagsKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServeFlags {
  pub script: String,
  pub watch: Option<WatchFlagsWithPaths>,
  pub port: u16,
  pub host: String,
  pub parallel: bool,
  pub open_site: bool,
}

impl ServeFlags {
  #[cfg(test)]
  pub fn new_default(script: String, port: u16, host: &str) -> Self {
    Self {
      script,
      watch: None,
      port,
      host: host.to_owned(),
      parallel: false,
      open_site: false,
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
  pub recursive: bool,
  pub filter: Option<String>,
  pub eval: bool,
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
  pub coverage_raw_data_only: bool,
  pub clean: bool,
  pub fail_fast: Option<usize>,
  pub files: FileFlags,
  pub parallel: bool,
  pub permit_no_files: bool,
  pub filter: Option<String>,
  pub shuffle: Option<u64>,
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
  pub checksum: Option<String>,
  pub pr: Option<u64>,
  pub branch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishFlags {
  pub token: Option<String>,
  pub dry_run: bool,
  pub allow_slow_types: bool,
  pub allow_dirty: bool,
  pub no_provenance: bool,
  pub set_version: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelpFlags {
  pub help: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JSONReferenceFlags {
  pub json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanFlags {
  pub except_paths: Vec<String>,
  pub dry_run: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleFlags {
  pub entrypoints: Vec<String>,
  pub output_path: Option<String>,
  pub output_dir: Option<String>,
  pub external: Vec<String>,
  pub format: BundleFormat,
  pub minify: bool,
  pub keep_names: bool,
  pub code_splitting: bool,
  pub inline_imports: bool,
  pub packages: PackageHandling,
  pub sourcemap: Option<SourceMapType>,
  pub platform: BundlePlatform,
  pub watch: bool,
}

// ---------------------------------------------------------------------------
// Outdated
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutdatedKind {
  Update {
    latest: bool,
    interactive: bool,
    lockfile_only: bool,
  },
  PrintOutdated {
    compatible: bool,
  },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutdatedFlags {
  pub filters: Vec<String>,
  pub recursive: bool,
  pub kind: OutdatedKind,
}

// ---------------------------------------------------------------------------
// ApproveScripts
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApproveScriptsFlags {
  pub lockfile_only: bool,
  pub packages: Vec<String>,
}

// ---------------------------------------------------------------------------
// TypeCheckMode
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TypeCheckMode {
  /// Type-check all modules.
  All,
  /// Skip type-checking of all modules. The default value for `deno run` and
  /// several other subcommands.
  #[default]
  None,
  /// Only type-check local modules. The default value for `deno test` and
  /// several other subcommands.
  Local,
}

impl TypeCheckMode {
  /// Returns `true` if type checking will occur under this mode.
  pub fn is_true(&self) -> bool {
    match self {
      Self::None => false,
      Self::Local | Self::All => true,
    }
  }
}

// ---------------------------------------------------------------------------
// InternalFlags
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InternalFlags {
  /// Used when the language server is configured with an
  /// explicit cache option.
  pub cache_path: Option<String>,
  /// Override the path to use for the node_modules directory.
  pub root_node_modules_dir_override: Option<String>,
  /// Only reads to the lockfile instead of writing to it.
  pub lockfile_skip_write: bool,
}

// ---------------------------------------------------------------------------
// DenoSubcommand
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DenoSubcommand {
  Add(AddFlags),
  Audit(AuditFlags),
  ApproveScripts(ApproveScriptsFlags),
  Remove(RemoveFlags),
  Bench(BenchFlags),
  Bundle(BundleFlags),
  Cache(CacheFlags),
  Check(CheckFlags),
  Clean(CleanFlags),
  Compile(CompileFlags),
  Completions(String),
  Coverage(CoverageFlags),
  Deploy(DeployFlags),
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
  Outdated(OutdatedFlags),
  Types,
  Upgrade(UpgradeFlags),
  Vendor,
  Publish(PublishFlags),
  Help(HelpFlags),
  X(XFlags),
  JSONReference(JSONReferenceFlags),
}

impl Default for DenoSubcommand {
  fn default() -> Self {
    DenoSubcommand::Repl(ReplFlags {
      eval_files: None,
      eval: None,
      is_default_command: true,
      json: false,
    })
  }
}

impl DenoSubcommand {
  pub fn is_run(&self) -> bool {
    matches!(self, Self::Run(_))
  }

  /// Returns `true` if the subcommand depends on testing infrastructure.
  pub fn needs_test(&self) -> bool {
    matches!(
      self,
      Self::Test(_)
        | Self::Jupyter(_)
        | Self::Repl(_)
        | Self::Bench(_)
        | Self::Lint(_)
        | Self::Lsp
    )
  }
}

// ---------------------------------------------------------------------------
// PermissionFlags
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PermissionFlags {
  pub allow_all: bool,
  pub allow_env: Option<Vec<String>>,
  pub deny_env: Option<Vec<String>>,
  pub ignore_env: Option<Vec<String>>,
  pub allow_ffi: Option<Vec<String>>,
  pub deny_ffi: Option<Vec<String>>,
  pub allow_net: Option<Vec<String>>,
  pub deny_net: Option<Vec<String>>,
  pub allow_read: Option<Vec<String>>,
  pub deny_read: Option<Vec<String>>,
  pub ignore_read: Option<Vec<String>>,
  pub allow_run: Option<Vec<String>>,
  pub deny_run: Option<Vec<String>>,
  pub allow_sys: Option<Vec<String>>,
  pub deny_sys: Option<Vec<String>>,
  pub allow_write: Option<Vec<String>>,
  pub deny_write: Option<Vec<String>>,
  pub no_prompt: bool,
  pub allow_import: Option<Vec<String>>,
  pub deny_import: Option<Vec<String>>,
}

impl PermissionFlags {
  pub fn has_permission(&self) -> bool {
    self.allow_all
      || self.allow_env.is_some()
      || self.deny_env.is_some()
      || self.ignore_env.is_some()
      || self.allow_ffi.is_some()
      || self.deny_ffi.is_some()
      || self.allow_net.is_some()
      || self.deny_net.is_some()
      || self.allow_read.is_some()
      || self.deny_read.is_some()
      || self.ignore_read.is_some()
      || self.allow_run.is_some()
      || self.deny_run.is_some()
      || self.allow_sys.is_some()
      || self.deny_sys.is_some()
      || self.allow_write.is_some()
      || self.deny_write.is_some()
      || self.allow_import.is_some()
      || self.deny_import.is_some()
  }
}

// ---------------------------------------------------------------------------
// Flags (main struct)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Flags {
  pub initial_cwd: Option<String>,
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
  pub enable_testing_features: bool,
  pub ext: Option<String>,
  /// Flags that aren't exposed in the CLI, but are used internally.
  pub internal: InternalFlags,
  pub ignore: Vec<String>,
  pub import_map_path: Option<String>,
  pub env_file: Option<Vec<String>>,
  pub inspect_brk: Option<String>,
  pub inspect_wait: Option<String>,
  pub inspect: Option<String>,
  pub inspect_publish_uid: Option<InspectPublishUid>,
  pub location: Option<String>,
  pub lock: Option<String>,
  pub log_level: Option<LogLevel>,
  pub minimum_dependency_age: Option<String>,
  pub no_remote: bool,
  pub no_lock: bool,
  pub no_npm: bool,
  pub reload: bool,
  pub seed: Option<u64>,
  pub trace_ops: Option<Vec<String>>,
  pub unstable_config: UnstableConfig,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub v8_flags: Vec<String>,
  pub code_cache_enabled: bool,
  pub permissions: PermissionFlags,
  pub allow_scripts: PackagesAllowedScripts,
  pub permission_set: Option<String>,
  pub eszip: bool,
  pub node_conditions: Vec<String>,
  pub preload: Vec<String>,
  pub require: Vec<String>,
  pub tunnel: bool,
  pub cpu_prof: Option<CpuProfFlags>,
}

impl Flags {
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
}
