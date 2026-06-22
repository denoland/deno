// Copyright 2018-2026 the Deno authors. MIT license.
#![allow(dead_code, reason = "types are used by the deno CLI crate")]

// Flag types using real Deno dependencies. These are the canonical type
// definitions shared by both the parser crate and `cli/args/flags.rs`.

use std::net::SocketAddr;
use std::num::NonZeroU8;
use std::num::NonZeroU32;
use std::num::NonZeroUsize;
use std::path::PathBuf;
#[allow(
  clippy::disallowed_types,
  reason = "Arc needed for CompletionsFlags::Dynamic"
)]
use std::sync::Arc;

pub use deno_bundle_runtime::BundleFormat;
use deno_bundle_runtime::BundleOptions;
pub use deno_bundle_runtime::BundlePlatform;
pub use deno_bundle_runtime::PackageHandling;
pub use deno_bundle_runtime::SourceMapType;
pub use deno_config::deno_json::NewestDependencyDate;
pub use deno_config::deno_json::NodeModulesDirMode;
pub use deno_config::deno_json::NodeModulesLinkerMode;
use deno_core::error::AnyError;
pub use deno_core::url::Url;
pub use deno_lib::args::CaData;
pub use deno_lib::args::UnstableConfig;
pub use deno_npm_installer::PackagesAllowedScripts;
pub use deno_runtime::deno_inspector_server::InspectPublishUid;
use deno_semver::package::PackageReq;
pub use log::Level;
use serde::Deserialize;
use serde::Serialize;

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

#[derive(Clone, Debug, Copy, Eq, PartialEq)]
pub enum DefaultRegistry {
  Npm,
  Jsr,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AddFlags {
  pub packages: Vec<String>,
  pub dev: bool,
  pub default_registry: Option<DefaultRegistry>,
  pub lockfile_only: bool,
  pub save_exact: bool,
  pub package_json: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WhyFlags {
  pub package: String,
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
  pub fix: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RemoveFlags {
  pub packages: Vec<String>,
  pub lockfile_only: bool,
  pub package_json: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LinkFlags {
  pub paths: Vec<String>,
  pub lockfile_only: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UnlinkFlags {
  pub names_or_paths: Vec<String>,
  pub lockfile_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct VersionFlags {
  pub increment: Option<VersionIncrement>,
  /// Bump every package in the workspace. Defaults to true when invoked at the
  /// workspace root and the workspace has more than one member with a version.
  pub workspace: Option<bool>,
  /// When in workspace mode without an explicit increment, derive bumps from
  /// commit messages between `start` and `base`. The default for both is to
  /// fall back to git (latest tag and current branch respectively).
  pub start: Option<String>,
  pub base: Option<String>,
  /// Path to the import map to rewrite jsr: version constraints in. Defaults
  /// to the root deno.json (or whatever its `importMap` field points to).
  pub import_map: Option<String>,
  /// Path to the release notes markdown file to prepend in conventional-commits
  /// mode. Defaults to `Releases.md`.
  pub release_notes: Option<String>,
  /// Don't write any files; just print what would happen.
  pub dry_run: bool,
  /// Explicit path to the manifest file to bump. May point to a `deno.json`
  /// (or `.jsonc`) or a `package.json`. When set, single-file mode is forced
  /// (workspace auto-detection is bypassed). Useful when both `deno.json` and
  /// `package.json` exist in the same directory.
  pub config: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VersionIncrement {
  Major,
  Minor,
  Patch,
  Premajor,
  Preminor,
  Prepatch,
  Prerelease,
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
  pub watch: Option<WatchFlags>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileFlags {
  pub source_file: String,
  pub output: Option<String>,
  pub args: Vec<String>,
  pub target: Option<String>,
  pub watch: Option<WatchFlags>,
  pub no_terminal: bool,
  pub icon: Option<String>,
  pub include: Vec<String>,
  pub exclude: Vec<String>,
  pub eszip: bool,
  pub self_extracting: bool,
  /// Bundle the entrypoint with esbuild before embedding it, instead of
  /// shipping the entire node_modules tree. Experimental.
  pub bundle: bool,
  /// Minify the bundle. Only meaningful with `bundle: true`.
  pub minify: bool,
  /// Prune the embedded managed npm snapshot to only those packages reachable
  /// from npm specifiers in the module graph. Opt-in because non-statically
  /// analyzable dynamic imports may not appear in the graph; pass
  /// `--include npm:<pkg>` for any such packages.
  pub exclude_unused_npm: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IconSetEntry {
  pub path: String,
  pub size: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IconConfig {
  /// A single icon file (`.icns`, `.ico`, or `.png`).
  Single(String),
  /// Multiple PNGs at specific pixel sizes.
  Set(Vec<IconSetEntry>),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DesktopFlags {
  pub source_file: String,
  pub output: Option<String>,
  pub args: Vec<String>,
  pub target: Option<String>,
  pub icon: Option<IconConfig>,
  pub include: Vec<String>,
  pub exclude: Vec<String>,
  pub hmr: bool,
  pub backend: Option<String>,
  pub all_targets: bool,
  /// Reverse-DNS bundle / application identifier (e.g. `com.acme.foo`).
  /// Used for the macOS `CFBundleIdentifier`, the Linux `.desktop` file
  /// identifier, and (eventually) the Windows AppUserModelID. When unset
  /// a synthetic `com.deno.desktop.<app-slug>` is generated.
  pub identifier: Option<String>,
  /// macOS codesigning identity (e.g. `Developer ID Application: Acme,
  /// Inc. (TEAMID)`, or `-` for ad-hoc). When unset the bundle is left
  /// unsigned; the system will quarantine it on download.
  pub codesign_identity: Option<String>,
  /// Optional override for the CEF renderer debugger port. When unset, a free
  /// port is allocated. The user-visible inspector port (from `--inspect`) is
  /// separate and is carried on `Flags::inspect`.
  pub inspect_renderer: Option<SocketAddr>,
  /// When set, emit a compressed distribution archive of the packaged app
  /// next to it (`<output>.tar.xz` or `<output>.tar.zst`). The installed
  /// `.app`/app dir is left untouched (and still code-signed); the archive is
  /// just a small download artifact. Value is the compressor: `"xz"` (LZMA)
  /// or `"zstd"`.
  pub compress: Option<String>,
}

#[derive(Clone)]
pub enum CompletionsFlags {
  Static(Box<[u8]>),
  #[allow(
    clippy::disallowed_types,
    reason = "Arc needed for dynamic completion callback"
  )]
  Dynamic(Arc<dyn Fn() -> Result<(), AnyError> + Send + Sync + 'static>),
}

#[allow(clippy::disallowed_types, reason = "Arc used in Dynamic variant")]
impl PartialEq for CompletionsFlags {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Static(l0), Self::Static(r0)) => l0 == r0,
      (Self::Dynamic(l0), Self::Dynamic(r0)) => Arc::ptr_eq(l0, r0),
      _ => false,
    }
  }
}

impl Eq for CompletionsFlags {}

impl std::fmt::Debug for CompletionsFlags {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Static(arg0) => f.debug_tuple("Static").field(arg0).finish(),
      Self::Dynamic(_) => f.debug_tuple("Dynamic").finish(),
    }
  }
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
  /// Minimum coverage percentage (0-100) applied to line, branch, and function
  /// coverage. Overrides per-metric thresholds from `deno.json`.
  pub threshold: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct DeployFlags {
  pub sandbox: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
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

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct CpuProfFlags {
  pub dir: Option<String>,
  pub name: Option<String>,
  pub interval: Option<u32>,
  pub md: bool,
  pub flamegraph: bool,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct EvalFlags {
  pub print: bool,
  pub code: String,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct FmtFlags {
  pub check: bool,
  pub fail_fast: bool,
  pub files: FileFlags,
  pub permit_no_files: bool,
  pub use_tabs: Option<bool>,
  pub line_width: Option<NonZeroU32>,
  pub indent_width: Option<NonZeroU8>,
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
  Local(InstallFlagsLocal, NpmInstallTargetFlags),
  Global(InstallFlagsGlobal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallFlagsLocal {
  Add(AddFlags),
  TopLevel(InstallTopLevelFlags),
  Entrypoints(InstallEntrypointsFlags),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CiFlags {
  pub production: bool,
  pub skip_types: bool,
}

/// Overrides for the target OS and architecture when installing npm packages.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NpmInstallTargetFlags {
  pub os: Option<String>,
  pub arch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallTopLevelFlags {
  pub lockfile_only: bool,
  pub production: bool,
  pub skip_types: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallEntrypointsFlags {
  pub entrypoints: Vec<String>,
  pub lockfile_only: bool,
  pub production: bool,
  pub skip_types: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JSONReferenceFlags {
  pub json: deno_core::serde_json::Value,
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

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct ReplFlags {
  pub eval_files: Option<Vec<String>>,
  pub eval: Option<String>,
  pub is_default_command: bool,
  pub json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct RunFlags {
  pub script: String,
  pub watch: Option<WatchFlagsWithPaths>,
  pub bare: bool,
  pub coverage_dir: Option<String>,
  pub print_task_list: bool,
}

impl RunFlags {
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

#[derive(Clone, Debug, Eq, PartialEq, Default)]
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
  pub ignore_scripts: PackagesAllowedScripts,
  /// When set via `--package`/`-p`, specifies the package to install
  /// while `command` is the binary name to execute from that package.
  pub package: Option<String>,
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

pub enum WatchFlagsRef<'a> {
  Watch(&'a WatchFlags),
  WithPaths(&'a WatchFlagsWithPaths),
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
  pub no_prefix: bool,
  /// Maximum number of workspace tasks to run concurrently. Overrides the
  /// `DENO_JOBS` env var and the `available_parallelism()` default. Only
  /// meaningful for multi-task (`-r`/`--filter`) runs.
  pub concurrency: Option<NonZeroUsize>,
  /// Exit with code 0 instead of an error when the named task is not found.
  pub if_present: bool,
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
  /// Minimum coverage percentage (0-100) required when `--coverage` is set.
  /// Overrides per-metric thresholds from `deno.json`.
  pub coverage_threshold: Option<u32>,
  pub clean: bool,
  pub fail_fast: Option<NonZeroUsize>,
  pub files: FileFlags,
  pub parallel: bool,
  pub permit_no_files: bool,
  pub filter: Option<String>,
  pub shuffle: Option<u64>,
  pub retry: u32,
  pub repeats: u32,
  /// Run only a subset of test files, as `(index, count)` with a 1-based
  /// index. Used to split a run across machines, e.g. `--shard=2/3`.
  pub shard: Option<(usize, usize)>,
  pub trace_leaks: bool,
  pub sanitize_ops: bool,
  pub sanitize_resources: bool,
  pub watch: Option<WatchFlagsWithPaths>,
  pub reporter: TestReporterConfig,
  pub junit_path: Option<String>,
  pub hide_stacktraces: bool,
  pub update_snapshots: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum SourceMapMode {
  #[default]
  None,
  Inline,
  Separate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranspileFlags {
  pub files: Vec<String>,
  pub output: Option<String>,
  pub output_dir: Option<String>,
  pub declaration: bool,
  pub source_map: SourceMapMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeFlags {
  pub dry_run: bool,
  pub force: bool,
  pub release_candidate: bool,
  pub canary: bool,
  pub no_delta: bool,
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
pub struct PackFlags {
  pub files: FileFlags,
  pub output: Option<String>,
  pub dry_run: bool,
  pub allow_slow_types: bool,
  pub allow_dirty: bool,
  pub set_version: Option<String>,
  pub no_source_maps: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelpFlags {
  pub help: String,
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
  pub declaration: bool,
}

impl From<BundleOptions> for BundleFlags {
  fn from(value: BundleOptions) -> Self {
    Self {
      entrypoints: value.entrypoints,
      output_path: value.output_path,
      output_dir: value.output_dir,
      external: value.external,
      format: value.format,
      minify: value.minify,
      keep_names: value.keep_names,
      code_splitting: value.code_splitting,
      platform: value.platform,
      watch: false,
      sourcemap: value.sourcemap,
      inline_imports: value.inline_imports,
      packages: value.packages,
      declaration: false,
    }
  }
}

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
  Ci(CiFlags),
  Clean(CleanFlags),
  Compile(CompileFlags),
  Completions(CompletionsFlags),
  Desktop(DesktopFlags),
  Coverage(CoverageFlags),
  Deploy(DeployFlags),
  Doc(DocFlags),
  Eval(EvalFlags),
  Fmt(FmtFlags),
  Init(InitFlags),
  Info(InfoFlags),
  Install(InstallFlags),
  JSONReference(JSONReferenceFlags),
  Jupyter(JupyterFlags),
  Uninstall(UninstallFlags),
  Link(LinkFlags),
  Unlink(UnlinkFlags),
  Lsp,
  Lint(LintFlags),
  Repl(ReplFlags),
  Run(RunFlags),
  Serve(ServeFlags),
  Task(TaskFlags),
  Test(TestFlags),
  Transpile(TranspileFlags),
  Outdated(OutdatedFlags),
  Types,
  Upgrade(UpgradeFlags),
  Vendor,
  Why(WhyFlags),
  BumpVersion(VersionFlags),
  Publish(PublishFlags),
  Pack(PackFlags),
  Help(HelpFlags),
  X(XFlags),
}

impl DenoSubcommand {
  pub fn watch_flags(&self) -> Option<WatchFlagsRef<'_>> {
    match self {
      Self::Run(RunFlags {
        watch: Some(flags), ..
      })
      | Self::Serve(ServeFlags {
        watch: Some(flags), ..
      })
      | Self::Test(TestFlags {
        watch: Some(flags), ..
      }) => Some(WatchFlagsRef::WithPaths(flags)),
      Self::Bench(BenchFlags {
        watch: Some(flags), ..
      })
      | Self::Check(CheckFlags {
        watch: Some(flags), ..
      })
      | Self::Lint(LintFlags {
        watch: Some(flags), ..
      })
      | Self::Fmt(FmtFlags {
        watch: Some(flags), ..
      })
      | Self::Compile(CompileFlags {
        watch: Some(flags), ..
      }) => Some(WatchFlagsRef::Watch(flags)),
      _ => None,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutdatedFlags {
  pub filters: Vec<String>,
  pub recursive: bool,
  pub kind: OutdatedKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApproveScriptsFlags {
  pub lockfile_only: bool,
  pub packages: Vec<String>,
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
        | Self::Lint(_)
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
      json: false,
    })
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub enum TypeCheckMode {
  /// Type-check all modules.
  All,
  /// Skip type-checking of all modules. The default value for "deno run" and
  /// several other subcommands.
  #[default]
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
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct InternalFlags {
  /// Used when the language server is configured with an
  /// explicit cache option.
  pub cache_path: Option<PathBuf>,
  /// Override the path to use for the node_modules directory.
  pub root_node_modules_dir_override: Option<PathBuf>,
  /// Only reads to the lockfile instead of writing to it.
  pub lockfile_skip_write: bool,
  /// Set when running the desktop subcommand to use desktop type libs.
  pub is_desktop: bool,
  /// Set by `deno compile --bundle` when the bundled output contains
  /// references that need to resolve against npm packages at runtime
  /// (CJS dependencies, native addons). When true, the standalone
  /// binary writer embeds (a subset of) the npm tree. Pure-ESM bundles
  /// leave this false and ship a tiny binary.
  pub compile_bundle_embed_node_modules: bool,
  /// Absolute paths the bundle path-rewriter resolved at build time —
  /// the on-disk files the compiled binary will require() at runtime.
  /// The standalone binary writer maps these back to npm packages so
  /// it can embed only what's reachable, instead of the whole tree.
  pub compile_bundle_referenced_paths: Vec<PathBuf>,
  /// Force-enable bundle-style resolution config regardless of the
  /// current subcommand. Set by `compile --bundle` so the bundle phase
  /// of compilation pulls deep CJS files (e.g. `jiti.cjs`) into the
  /// module graph the same way `deno bundle` does.
  pub force_bundle_mode: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct Flags {
  pub initial_cwd: Option<PathBuf>,
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
  pub node_modules_linker: Option<NodeModulesLinkerMode>,
  pub vendor: Option<bool>,
  pub enable_testing_features: bool,
  pub ext: Option<String>,
  /// Flags that aren't exposed in the CLI, but are used internally.
  pub internal: InternalFlags,
  pub ignore: Vec<String>,
  pub import_map_path: Option<String>,
  pub env_file: Option<Vec<String>>,
  pub inspect_brk: Option<SocketAddr>,
  pub inspect_wait: Option<SocketAddr>,
  pub inspect: Option<SocketAddr>,
  pub inspect_publish_uid: Option<InspectPublishUid>,
  pub location: Option<Url>,
  pub lock: Option<String>,
  pub log_level: Option<Level>,
  pub minimum_dependency_age: Option<NewestDependencyDate>,
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
  pub deny_scripts: Vec<PackageReq>,
  pub permission_set: Option<String>,
  pub eszip: bool,
  pub node_conditions: Vec<String>,
  pub preload: Vec<String>,
  pub require: Vec<String>,
  pub tunnel: bool,
  pub cpu_prof: Option<CpuProfFlags>,
}

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
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
