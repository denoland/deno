// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod config_file;
mod flags;
mod lockfile;

mod flags_allow_net;

pub use config_file::BenchConfig;
pub use config_file::CompilerOptions;
pub use config_file::ConfigFile;
pub use config_file::EmitConfigOptions;
pub use config_file::FilesConfig;
pub use config_file::FmtOptionsConfig;
pub use config_file::JsxImportSourceConfig;
pub use config_file::LintRulesConfig;
pub use config_file::ProseWrap;
pub use config_file::TsConfig;
pub use config_file::TsConfigForEmit;
pub use config_file::TsConfigType;
pub use config_file::TsTypeLib;
pub use flags::*;
pub use lockfile::Lockfile;
pub use lockfile::LockfileError;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::normalize_path;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_runtime::colors;
use deno_runtime::deno_tls::rustls;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::rustls_native_certs::load_native_certs;
use deno_runtime::deno_tls::rustls_pemfile;
use deno_runtime::deno_tls::webpki_roots;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::permissions::PermissionsOptions;
use std::collections::BTreeMap;
use std::env;
use std::io::BufReader;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

use crate::cache::DenoDir;
use crate::util::fs::canonicalize_path_maybe_not_exists;
use crate::version;

use self::config_file::FmtConfig;
use self::config_file::LintConfig;
use self::config_file::MaybeImportsResult;
use self::config_file::TestConfig;

/// Indicates how cached source files should be handled.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CacheSetting {
  /// Only the cached files should be used.  Any files not in the cache will
  /// error.  This is the equivalent of `--cached-only` in the CLI.
  Only,
  /// No cached source files should be used, and all files should be reloaded.
  /// This is the equivalent of `--reload` in the CLI.
  ReloadAll,
  /// Only some cached resources should be used.  This is the equivalent of
  /// `--reload=https://deno.land/std` or
  /// `--reload=https://deno.land/std,https://deno.land/x/example`.
  ReloadSome(Vec<String>),
  /// The usability of a cached value is determined by analyzing the cached
  /// headers and other metadata associated with a cached response, reloading
  /// any cached "non-fresh" cached responses.
  RespectHeaders,
  /// The cached source files should be used for local modules.  This is the
  /// default behavior of the CLI.
  Use,
}

impl CacheSetting {
  pub fn should_use_for_npm_package(&self, package_name: &str) -> bool {
    match self {
      CacheSetting::ReloadAll => false,
      CacheSetting::ReloadSome(list) => {
        if list.iter().any(|i| i == "npm:") {
          return false;
        }
        let specifier = format!("npm:{}", package_name);
        if list.contains(&specifier) {
          return false;
        }
        true
      }
      _ => true,
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchOptions {
  pub files: FilesConfig,
  pub filter: Option<String>,
}

impl BenchOptions {
  pub fn resolve(
    maybe_bench_config: Option<BenchConfig>,
    maybe_bench_flags: Option<BenchFlags>,
  ) -> Result<Self, AnyError> {
    let bench_flags = maybe_bench_flags.unwrap_or_default();
    Ok(Self {
      files: resolve_files(
        maybe_bench_config.map(|c| c.files),
        Some(bench_flags.files),
      ),
      filter: bench_flags.filter,
    })
  }
}

#[derive(Clone, Debug, Default)]
pub struct FmtOptions {
  pub is_stdin: bool,
  pub check: bool,
  pub ext: String,
  pub options: FmtOptionsConfig,
  pub files: FilesConfig,
}

impl FmtOptions {
  pub fn resolve(
    maybe_fmt_config: Option<FmtConfig>,
    mut maybe_fmt_flags: Option<FmtFlags>,
  ) -> Result<Self, AnyError> {
    let is_stdin = if let Some(fmt_flags) = maybe_fmt_flags.as_mut() {
      let args = &mut fmt_flags.files.include;
      if args.len() == 1 && args[0].to_string_lossy() == "-" {
        args.pop(); // remove the "-" arg
        true
      } else {
        false
      }
    } else {
      false
    };
    let (maybe_config_options, maybe_config_files) =
      maybe_fmt_config.map(|c| (c.options, c.files)).unzip();

    Ok(Self {
      is_stdin,
      check: maybe_fmt_flags.as_ref().map(|f| f.check).unwrap_or(false),
      ext: maybe_fmt_flags
        .as_ref()
        .map(|f| f.ext.to_string())
        .unwrap_or_else(|| "ts".to_string()),
      options: resolve_fmt_options(
        maybe_fmt_flags.as_ref(),
        maybe_config_options,
      ),
      files: resolve_files(
        maybe_config_files,
        maybe_fmt_flags.map(|f| f.files),
      ),
    })
  }
}

fn resolve_fmt_options(
  fmt_flags: Option<&FmtFlags>,
  options: Option<FmtOptionsConfig>,
) -> FmtOptionsConfig {
  let mut options = options.unwrap_or_default();

  if let Some(fmt_flags) = fmt_flags {
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
  }

  options
}

#[derive(Clone)]
pub struct TestOptions {
  pub files: FilesConfig,
  pub doc: bool,
  pub no_run: bool,
  pub fail_fast: Option<NonZeroUsize>,
  pub allow_none: bool,
  pub filter: Option<String>,
  pub shuffle: Option<u64>,
  pub concurrent_jobs: NonZeroUsize,
  pub trace_ops: bool,
}

impl TestOptions {
  pub fn resolve(
    maybe_test_config: Option<TestConfig>,
    maybe_test_flags: Option<TestFlags>,
  ) -> Result<Self, AnyError> {
    let test_flags = maybe_test_flags.unwrap_or_default();

    Ok(Self {
      files: resolve_files(
        maybe_test_config.map(|c| c.files),
        Some(test_flags.files),
      ),
      allow_none: test_flags.allow_none,
      concurrent_jobs: test_flags
        .concurrent_jobs
        .unwrap_or_else(|| NonZeroUsize::new(1).unwrap()),
      doc: test_flags.doc,
      fail_fast: test_flags.fail_fast,
      filter: test_flags.filter,
      no_run: test_flags.no_run,
      shuffle: test_flags.shuffle,
      trace_ops: test_flags.trace_ops,
    })
  }
}

#[derive(Clone, Debug)]
pub enum LintReporterKind {
  Pretty,
  Json,
  Compact,
}

impl Default for LintReporterKind {
  fn default() -> Self {
    LintReporterKind::Pretty
  }
}

#[derive(Clone, Debug, Default)]
pub struct LintOptions {
  pub rules: LintRulesConfig,
  pub files: FilesConfig,
  pub is_stdin: bool,
  pub reporter_kind: LintReporterKind,
}

impl LintOptions {
  pub fn resolve(
    maybe_lint_config: Option<LintConfig>,
    mut maybe_lint_flags: Option<LintFlags>,
  ) -> Result<Self, AnyError> {
    let is_stdin = if let Some(lint_flags) = maybe_lint_flags.as_mut() {
      let args = &mut lint_flags.files.include;
      if args.len() == 1 && args[0].to_string_lossy() == "-" {
        args.pop(); // remove the "-" arg
        true
      } else {
        false
      }
    } else {
      false
    };

    let mut maybe_reporter_kind =
      maybe_lint_flags.as_ref().and_then(|lint_flags| {
        if lint_flags.json {
          Some(LintReporterKind::Json)
        } else if lint_flags.compact {
          Some(LintReporterKind::Compact)
        } else {
          None
        }
      });

    if maybe_reporter_kind.is_none() {
      // Flag not set, so try to get lint reporter from the config file.
      if let Some(lint_config) = &maybe_lint_config {
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
    }

    let (
      maybe_file_flags,
      maybe_rules_tags,
      maybe_rules_include,
      maybe_rules_exclude,
    ) = maybe_lint_flags
      .map(|f| {
        (
          f.files,
          f.maybe_rules_tags,
          f.maybe_rules_include,
          f.maybe_rules_exclude,
        )
      })
      .unwrap_or_default();

    let (maybe_config_files, maybe_config_rules) =
      maybe_lint_config.map(|c| (c.files, c.rules)).unzip();
    Ok(Self {
      reporter_kind: maybe_reporter_kind.unwrap_or_default(),
      is_stdin,
      files: resolve_files(maybe_config_files, Some(maybe_file_flags)),
      rules: resolve_lint_rules_options(
        maybe_config_rules,
        maybe_rules_tags,
        maybe_rules_include,
        maybe_rules_exclude,
      ),
    })
  }
}

fn resolve_lint_rules_options(
  maybe_lint_rules_config: Option<LintRulesConfig>,
  mut maybe_rules_tags: Option<Vec<String>>,
  mut maybe_rules_include: Option<Vec<String>>,
  mut maybe_rules_exclude: Option<Vec<String>>,
) -> LintRulesConfig {
  if let Some(config_rules) = maybe_lint_rules_config {
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
  }
  LintRulesConfig {
    exclude: maybe_rules_exclude,
    include: maybe_rules_include,
    tags: maybe_rules_tags,
  }
}

/// Create and populate a root cert store based on the passed options and
/// environment.
pub fn get_root_cert_store(
  maybe_root_path: Option<PathBuf>,
  maybe_ca_stores: Option<Vec<String>>,
  maybe_ca_file: Option<String>,
) -> Result<RootCertStore, AnyError> {
  let mut root_cert_store = RootCertStore::empty();
  let ca_stores: Vec<String> = maybe_ca_stores
    .or_else(|| {
      let env_ca_store = env::var("DENO_TLS_CA_STORE").ok()?;
      Some(
        env_ca_store
          .split(',')
          .map(|s| s.trim().to_string())
          .filter(|s| !s.is_empty())
          .collect(),
      )
    })
    .unwrap_or_else(|| vec!["mozilla".to_string()]);

  for store in ca_stores.iter() {
    match store.as_str() {
      "mozilla" => {
        root_cert_store.add_server_trust_anchors(
          webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
            rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
              ta.subject,
              ta.spki,
              ta.name_constraints,
            )
          }),
        );
      }
      "system" => {
        let roots = load_native_certs().expect("could not load platform certs");
        for root in roots {
          root_cert_store
            .add(&rustls::Certificate(root.0))
            .expect("Failed to add platform cert to root cert store");
        }
      }
      _ => {
        return Err(anyhow!("Unknown certificate store \"{}\" specified (allowed: \"system,mozilla\")", store));
      }
    }
  }

  let ca_file = maybe_ca_file.or_else(|| env::var("DENO_CERT").ok());
  if let Some(ca_file) = ca_file {
    let ca_file = if let Some(root) = &maybe_root_path {
      root.join(&ca_file)
    } else {
      PathBuf::from(ca_file)
    };
    let certfile = std::fs::File::open(ca_file)?;
    let mut reader = BufReader::new(certfile);

    match rustls_pemfile::certs(&mut reader) {
      Ok(certs) => {
        root_cert_store.add_parsable_certificates(&certs);
      }
      Err(e) => {
        return Err(anyhow!(
          "Unable to add pem file to certificate store: {}",
          e
        ));
      }
    }
  }

  Ok(root_cert_store)
}

/// Overrides for the options below that when set will
/// use these values over the values derived from the
/// CLI flags or config file.
#[derive(Default)]
struct CliOptionOverrides {
  import_map_specifier: Option<Option<ModuleSpecifier>>,
}

/// Holds the resolved options of many sources used by sub commands
/// and provides some helper function for creating common objects.
pub struct CliOptions {
  // the source of the options is a detail the rest of the
  // application need not concern itself with, so keep these private
  flags: Flags,
  maybe_config_file: Option<ConfigFile>,
  maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  overrides: CliOptionOverrides,
}

impl CliOptions {
  pub fn new(
    flags: Flags,
    maybe_config_file: Option<ConfigFile>,
    maybe_lockfile: Option<Lockfile>,
  ) -> Self {
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
      // use eprintln instead of log::warn so this always gets shown
      eprintln!("{}", colors::yellow(msg));
    }

    let maybe_lockfile = maybe_lockfile.map(|l| Arc::new(Mutex::new(l)));

    Self {
      maybe_config_file,
      maybe_lockfile,
      flags,
      overrides: Default::default(),
    }
  }

  pub fn from_flags(flags: Flags) -> Result<Self, AnyError> {
    let maybe_config_file = ConfigFile::discover(&flags)?;
    let maybe_lock_file =
      Lockfile::discover(&flags, maybe_config_file.as_ref())?;
    Ok(Self::new(flags, maybe_config_file, maybe_lock_file))
  }

  pub fn maybe_config_file_specifier(&self) -> Option<ModuleSpecifier> {
    self.maybe_config_file.as_ref().map(|f| f.specifier.clone())
  }

  pub fn ts_type_lib_window(&self) -> TsTypeLib {
    if self.flags.unstable {
      TsTypeLib::UnstableDenoWindow
    } else {
      TsTypeLib::DenoWindow
    }
  }

  pub fn ts_type_lib_worker(&self) -> TsTypeLib {
    if self.flags.unstable {
      TsTypeLib::UnstableDenoWorker
    } else {
      TsTypeLib::DenoWorker
    }
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

  pub fn resolve_deno_dir(&self) -> Result<DenoDir, AnyError> {
    Ok(DenoDir::new(self.maybe_custom_root())?)
  }

  /// Based on an optional command line import map path and an optional
  /// configuration file, return a resolved module specifier to an import map.
  pub fn resolve_import_map_specifier(
    &self,
  ) -> Result<Option<ModuleSpecifier>, AnyError> {
    match self.overrides.import_map_specifier.clone() {
      Some(path) => Ok(path),
      None => resolve_import_map_specifier(
        self.flags.import_map_path.as_deref(),
        self.maybe_config_file.as_ref(),
      ),
    }
  }

  /// Overrides the import map specifier to use.
  pub fn set_import_map_specifier(&mut self, path: Option<ModuleSpecifier>) {
    self.overrides.import_map_specifier = Some(path);
  }

  pub fn node_modules_dir(&self) -> bool {
    self.flags.node_modules_dir
  }

  /// Resolves the path to use for a local node_modules folder.
  pub fn resolve_local_node_modules_folder(
    &self,
  ) -> Result<Option<PathBuf>, AnyError> {
    let path = if !self.flags.node_modules_dir {
      return Ok(None);
    } else if let Some(config_path) = self
      .maybe_config_file
      .as_ref()
      .and_then(|c| c.specifier.to_file_path().ok())
    {
      config_path.parent().unwrap().join("node_modules")
    } else {
      std::env::current_dir()?.join("node_modules")
    };
    Ok(Some(canonicalize_path_maybe_not_exists(&path)?))
  }

  pub fn resolve_root_cert_store(&self) -> Result<RootCertStore, AnyError> {
    get_root_cert_store(
      None,
      self.flags.ca_stores.clone(),
      self.flags.ca_file.clone(),
    )
  }

  pub fn resolve_ts_config_for_emit(
    &self,
    config_type: TsConfigType,
  ) -> Result<TsConfigForEmit, AnyError> {
    config_file::get_ts_config_for_emit(
      config_type,
      self.maybe_config_file.as_ref(),
    )
  }

  /// Resolves the storage key to use based on the current flags, config, or main module.
  pub fn resolve_storage_key(
    &self,
    main_module: &ModuleSpecifier,
  ) -> Option<String> {
    if let Some(location) = &self.flags.location {
      // if a location is set, then the ascii serialization of the location is
      // used, unless the origin is opaque, and then no storage origin is set, as
      // we can't expect the origin to be reproducible
      let storage_origin = location.origin();
      if storage_origin.is_tuple() {
        Some(storage_origin.ascii_serialization())
      } else {
        None
      }
    } else if let Some(config_file) = &self.maybe_config_file {
      // otherwise we will use the path to the config file
      Some(config_file.specifier.to_string())
    } else {
      // otherwise we will use the path to the main module
      Some(main_module.to_string())
    }
  }

  pub fn resolve_inspector_server(&self) -> Option<InspectorServer> {
    let maybe_inspect_host = self
      .flags
      .inspect
      .or(self.flags.inspect_brk)
      .or(self.flags.inspect_wait);
    maybe_inspect_host
      .map(|host| InspectorServer::new(host, version::get_user_agent()))
  }

  pub fn maybe_lock_file(&self) -> Option<Arc<Mutex<Lockfile>>> {
    self.maybe_lockfile.clone()
  }

  pub fn resolve_tasks_config(
    &self,
  ) -> Result<BTreeMap<String, String>, AnyError> {
    if let Some(config_file) = &self.maybe_config_file {
      config_file.resolve_tasks_config()
    } else {
      bail!("No config file found")
    }
  }

  /// Return the JSX import source configuration.
  pub fn to_maybe_jsx_import_source_config(
    &self,
  ) -> Option<JsxImportSourceConfig> {
    self
      .maybe_config_file
      .as_ref()
      .and_then(|c| c.to_maybe_jsx_import_source_config())
  }

  /// Return any imports that should be brought into the scope of the module
  /// graph.
  pub fn to_maybe_imports(&self) -> MaybeImportsResult {
    let mut imports = Vec::new();
    if let Some(config_file) = &self.maybe_config_file {
      if let Some(config_imports) = config_file.to_maybe_imports()? {
        imports.extend(config_imports);
      }
    }
    if imports.is_empty() {
      Ok(None)
    } else {
      Ok(Some(imports))
    }
  }

  pub fn get_maybe_config_file(&self) -> &Option<ConfigFile> {
    &self.maybe_config_file
  }

  pub fn resolve_fmt_options(
    &self,
    fmt_flags: FmtFlags,
  ) -> Result<FmtOptions, AnyError> {
    let maybe_fmt_config = if let Some(config_file) = &self.maybe_config_file {
      config_file.to_fmt_config()?
    } else {
      None
    };
    FmtOptions::resolve(maybe_fmt_config, Some(fmt_flags))
  }

  pub fn resolve_lint_options(
    &self,
    lint_flags: LintFlags,
  ) -> Result<LintOptions, AnyError> {
    let maybe_lint_config = if let Some(config_file) = &self.maybe_config_file {
      config_file.to_lint_config()?
    } else {
      None
    };
    LintOptions::resolve(maybe_lint_config, Some(lint_flags))
  }

  pub fn resolve_test_options(
    &self,
    test_flags: TestFlags,
  ) -> Result<TestOptions, AnyError> {
    let maybe_test_config = if let Some(config_file) = &self.maybe_config_file {
      config_file.to_test_config()?
    } else {
      None
    };
    TestOptions::resolve(maybe_test_config, Some(test_flags))
  }

  pub fn resolve_bench_options(
    &self,
    bench_flags: BenchFlags,
  ) -> Result<BenchOptions, AnyError> {
    let maybe_bench_config = if let Some(config_file) = &self.maybe_config_file
    {
      config_file.to_bench_config()?
    } else {
      None
    };
    BenchOptions::resolve(maybe_bench_config, Some(bench_flags))
  }

  /// Vector of user script CLI arguments.
  pub fn argv(&self) -> &Vec<String> {
    &self.flags.argv
  }

  pub fn ca_file(&self) -> &Option<String> {
    &self.flags.ca_file
  }

  pub fn ca_stores(&self) -> &Option<Vec<String>> {
    &self.flags.ca_stores
  }

  pub fn check_js(&self) -> bool {
    self
      .maybe_config_file
      .as_ref()
      .map(|cf| cf.get_check_js())
      .unwrap_or(false)
  }

  pub fn coverage_dir(&self) -> Option<String> {
    fn allow_coverage(sub_command: &DenoSubcommand) -> bool {
      match sub_command {
        DenoSubcommand::Test(_) => true,
        DenoSubcommand::Run(flags) => !flags.is_stdin(),
        _ => false,
      }
    }

    if allow_coverage(self.sub_command()) {
      self
        .flags
        .coverage_dir
        .as_ref()
        .map(ToOwned::to_owned)
        .or_else(|| env::var("DENO_UNSTABLE_COVERAGE_DIR").ok())
    } else {
      None
    }
  }

  pub fn enable_testing_features(&self) -> bool {
    self.flags.enable_testing_features
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

  pub fn maybe_custom_root(&self) -> Option<PathBuf> {
    self
      .flags
      .cache_path
      .clone()
      .or_else(|| env::var("DENO_DIR").map(String::into).ok())
  }

  pub fn no_clear_screen(&self) -> bool {
    self.flags.no_clear_screen
  }

  pub fn no_prompt(&self) -> bool {
    resolve_no_prompt(&self.flags)
  }

  pub fn no_remote(&self) -> bool {
    self.flags.no_remote
  }

  pub fn no_npm(&self) -> bool {
    self.flags.no_npm
  }

  pub fn permissions_options(&self) -> PermissionsOptions {
    PermissionsOptions {
      allow_env: self.flags.allow_env.clone(),
      allow_hrtime: self.flags.allow_hrtime,
      allow_net: self.flags.allow_net.clone(),
      allow_ffi: self.flags.allow_ffi.clone(),
      allow_read: self.flags.allow_read.clone(),
      allow_run: self.flags.allow_run.clone(),
      allow_sys: self.flags.allow_sys.clone(),
      allow_write: self.flags.allow_write.clone(),
      prompt: !self.no_prompt(),
    }
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

  pub fn trace_ops(&self) -> bool {
    match self.sub_command() {
      DenoSubcommand::Test(flags) => flags.trace_ops,
      _ => false,
    }
  }

  pub fn shuffle_tests(&self) -> Option<u64> {
    match self.sub_command() {
      DenoSubcommand::Test(flags) => flags.shuffle,
      _ => None,
    }
  }

  pub fn type_check_mode(&self) -> TypeCheckMode {
    self.flags.type_check_mode
  }

  pub fn unsafely_ignore_certificate_errors(&self) -> &Option<Vec<String>> {
    &self.flags.unsafely_ignore_certificate_errors
  }

  pub fn unstable(&self) -> bool {
    self.flags.unstable
  }

  pub fn v8_flags(&self) -> &Vec<String> {
    &self.flags.v8_flags
  }

  pub fn watch_paths(&self) -> &Option<Vec<PathBuf>> {
    &self.flags.watch
  }
}

fn resolve_import_map_specifier(
  maybe_import_map_path: Option<&str>,
  maybe_config_file: Option<&ConfigFile>,
) -> Result<Option<ModuleSpecifier>, AnyError> {
  if let Some(import_map_path) = maybe_import_map_path {
    if let Some(config_file) = &maybe_config_file {
      if config_file.to_import_map_path().is_some() {
        log::warn!("{} the configuration file \"{}\" contains an entry for \"importMap\" that is being ignored.", colors::yellow("Warning"), config_file.specifier);
      }
    }
    let specifier = deno_core::resolve_url_or_path(import_map_path)
      .context(format!("Bad URL (\"{}\") for import map.", import_map_path))?;
    return Ok(Some(specifier));
  } else if let Some(config_file) = &maybe_config_file {
    // when the import map is specifier in a config file, it needs to be
    // resolved relative to the config file, versus the CWD like with the flag
    // and with config files, we support both local and remote config files,
    // so we have treat them differently.
    if let Some(import_map_path) = config_file.to_import_map_path() {
      // if the import map is an absolute URL, use it as is
      if let Ok(specifier) = deno_core::resolve_url(&import_map_path) {
        return Ok(Some(specifier));
      }
      let specifier =
          // with local config files, it might be common to specify an import
          // map like `"importMap": "import-map.json"`, which is resolvable if
          // the file is resolved like a file path, so we will coerce the config
          // file into a file path if possible and join the import map path to
          // the file path.
          if let Ok(config_file_path) = config_file.specifier.to_file_path() {
            let import_map_file_path = normalize_path(config_file_path
              .parent()
              .ok_or_else(|| {
                anyhow!("Bad config file specifier: {}", config_file.specifier)
              })?
              .join(&import_map_path));
            ModuleSpecifier::from_file_path(import_map_file_path).unwrap()
          // otherwise if the config file is remote, we have no choice but to
          // use "import resolution" with the config file as the base.
          } else {
            deno_core::resolve_import(&import_map_path, config_file.specifier.as_str())
              .context(format!(
                "Bad URL (\"{}\") for import map.",
                import_map_path
              ))?
          };
      return Ok(Some(specifier));
    }
  }
  Ok(None)
}

/// Collect included and ignored files. CLI flags take precedence
/// over config file, i.e. if there's `files.ignore` in config file
/// and `--ignore` CLI flag, only the flag value is taken into account.
fn resolve_files(
  maybe_files_config: Option<FilesConfig>,
  maybe_file_flags: Option<FileFlags>,
) -> FilesConfig {
  let mut result = maybe_files_config.unwrap_or_default();
  if let Some(file_flags) = maybe_file_flags {
    if !file_flags.include.is_empty() {
      result.include = file_flags.include;
    }
    if !file_flags.ignore.is_empty() {
      result.exclude = file_flags.ignore;
    }
  }
  result
}

/// Resolves the no_prompt value based on the cli flags and environment.
pub fn resolve_no_prompt(flags: &Flags) -> bool {
  flags.no_prompt || {
    let value = env::var("DENO_NO_PROMPT");
    matches!(value.as_ref().map(|s| s.as_str()), Ok("1"))
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[cfg(not(windows))]
  #[test]
  fn resolve_import_map_config_file() {
    let config_text = r#"{
      "importMap": "import_map.json"
    }"#;
    let config_specifier =
      ModuleSpecifier::parse("file:///deno/deno.jsonc").unwrap();
    let config_file = ConfigFile::new(config_text, &config_specifier).unwrap();
    let actual = resolve_import_map_specifier(None, Some(&config_file));
    assert!(actual.is_ok());
    let actual = actual.unwrap();
    assert_eq!(
      actual,
      Some(ModuleSpecifier::parse("file:///deno/import_map.json").unwrap())
    );
  }

  #[test]
  fn resolve_import_map_remote_config_file_local() {
    let config_text = r#"{
      "importMap": "https://example.com/import_map.json"
    }"#;
    let config_specifier =
      ModuleSpecifier::parse("file:///deno/deno.jsonc").unwrap();
    let config_file = ConfigFile::new(config_text, &config_specifier).unwrap();
    let actual = resolve_import_map_specifier(None, Some(&config_file));
    assert!(actual.is_ok());
    let actual = actual.unwrap();
    assert_eq!(
      actual,
      Some(
        ModuleSpecifier::parse("https://example.com/import_map.json").unwrap()
      )
    );
  }

  #[test]
  fn resolve_import_map_config_file_remote() {
    let config_text = r#"{
      "importMap": "./import_map.json"
    }"#;
    let config_specifier =
      ModuleSpecifier::parse("https://example.com/deno.jsonc").unwrap();
    let config_file = ConfigFile::new(config_text, &config_specifier).unwrap();
    let actual = resolve_import_map_specifier(None, Some(&config_file));
    assert!(actual.is_ok());
    let actual = actual.unwrap();
    assert_eq!(
      actual,
      Some(
        ModuleSpecifier::parse("https://example.com/import_map.json").unwrap()
      )
    );
  }

  #[test]
  fn resolve_import_map_flags_take_precedence() {
    let config_text = r#"{
      "importMap": "import_map.json"
    }"#;
    let config_specifier =
      ModuleSpecifier::parse("file:///deno/deno.jsonc").unwrap();
    let config_file = ConfigFile::new(config_text, &config_specifier).unwrap();
    let actual =
      resolve_import_map_specifier(Some("import-map.json"), Some(&config_file));
    let import_map_path =
      std::env::current_dir().unwrap().join("import-map.json");
    let expected_specifier =
      ModuleSpecifier::from_file_path(import_map_path).unwrap();
    assert!(actual.is_ok());
    let actual = actual.unwrap();
    assert_eq!(actual, Some(expected_specifier));
  }

  #[test]
  fn resolve_import_map_none() {
    let config_text = r#"{}"#;
    let config_specifier =
      ModuleSpecifier::parse("file:///deno/deno.jsonc").unwrap();
    let config_file = ConfigFile::new(config_text, &config_specifier).unwrap();
    let actual = resolve_import_map_specifier(None, Some(&config_file));
    assert!(actual.is_ok());
    let actual = actual.unwrap();
    assert_eq!(actual, None);
  }

  #[test]
  fn resolve_import_map_no_config() {
    let actual = resolve_import_map_specifier(None, None);
    assert!(actual.is_ok());
    let actual = actual.unwrap();
    assert_eq!(actual, None);
  }
}
