// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub mod deno_json;
mod flags;
mod flags_net;
mod import_map;
mod lockfile;
mod package_json;

use deno_ast::SourceMapOption;
use deno_config::deno_json::NodeModulesDirMode;
use deno_config::workspace::CreateResolverOptions;
use deno_config::workspace::FolderConfigs;
use deno_config::workspace::PackageJsonDepResolution;
use deno_config::workspace::VendorEnablement;
use deno_config::workspace::Workspace;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceDirectoryEmptyOptions;
use deno_config::workspace::WorkspaceDiscoverOptions;
use deno_config::workspace::WorkspaceDiscoverStart;
use deno_config::workspace::WorkspaceLintConfig;
use deno_config::workspace::WorkspaceResolver;
use deno_core::resolve_url_or_path;
use deno_graph::GraphKind;
use deno_npm::npm_rc::NpmRc;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm::NpmSystemInfo;
use deno_path_util::normalize_path;
use deno_semver::npm::NpmPackageReqReference;
use import_map::resolve_import_map_value_from_specifier;

pub use deno_config::deno_json::BenchConfig;
pub use deno_config::deno_json::ConfigFile;
pub use deno_config::deno_json::FmtOptionsConfig;
pub use deno_config::deno_json::JsxImportSourceConfig;
pub use deno_config::deno_json::LintRulesConfig;
pub use deno_config::deno_json::ProseWrap;
pub use deno_config::deno_json::TsConfig;
pub use deno_config::deno_json::TsConfigForEmit;
pub use deno_config::deno_json::TsConfigType;
pub use deno_config::deno_json::TsTypeLib;
pub use deno_config::glob::FilePatterns;
pub use deno_json::check_warn_tsconfig;
pub use flags::*;
pub use lockfile::CliLockfile;
pub use lockfile::CliLockfileReadFromPathOptions;
pub use package_json::NpmInstallDepsProvider;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_runtime::deno_permissions::PermissionsOptions;
use deno_runtime::deno_tls::deno_native_certs::load_native_certs;
use deno_runtime::deno_tls::rustls;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::rustls_pemfile;
use deno_runtime::deno_tls::webpki_roots;
use deno_runtime::inspector_server::InspectorServer;
use deno_terminal::colors;
use dotenvy::from_filename;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Read;
use std::io::Seek;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

use crate::cache;
use crate::cache::DenoDirProvider;
use crate::file_fetcher::FileFetcher;
use crate::util::fs::canonicalize_path_maybe_not_exists;
use crate::version;

use deno_config::deno_json::FmtConfig;
use deno_config::deno_json::LintConfig;
use deno_config::deno_json::TestConfig;

pub fn npm_registry_url() -> &'static Url {
  static NPM_REGISTRY_DEFAULT_URL: Lazy<Url> = Lazy::new(|| {
    let env_var_name = "NPM_CONFIG_REGISTRY";
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

    Url::parse("https://registry.npmjs.org").unwrap()
  });

  &NPM_REGISTRY_DEFAULT_URL
}

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

pub fn ts_config_to_transpile_and_emit_options(
  config: deno_config::deno_json::TsConfig,
) -> Result<(deno_ast::TranspileOptions, deno_ast::EmitOptions), AnyError> {
  let options: deno_config::deno_json::EmitConfigOptions =
    serde_json::from_value(config.0)
      .context("Failed to parse compilerOptions")?;
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
  Ok((
    deno_ast::TranspileOptions {
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
    },
    deno_ast::EmitOptions {
      inline_sources: options.inline_sources,
      remove_comments: false,
      source_map,
      source_map_base: None,
      source_map_file: None,
    },
  ))
}

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
  /// `--reload=jsr:@std/http/file-server` or
  /// `--reload=jsr:@std/http/file-server,jsr:@std/assert/assert-equals`.
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
        let specifier = format!("npm:{package_name}");
        if list.contains(&specifier) {
          return false;
        }
        true
      }
      _ => true,
    }
  }
}

pub struct WorkspaceBenchOptions {
  pub filter: Option<String>,
  pub json: bool,
  pub no_run: bool,
}

impl WorkspaceBenchOptions {
  pub fn resolve(bench_flags: &BenchFlags) -> Self {
    Self {
      filter: bench_flags.filter.clone(),
      json: bench_flags.json,
      no_run: bench_flags.no_run,
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
    }
  }

  pub fn resolve(lint_config: LintConfig, lint_flags: &LintFlags) -> Self {
    Self {
      files: lint_config.files,
      rules: resolve_lint_rules_options(
        lint_config.options.rules,
        lint_flags.maybe_rules_tags.clone(),
        lint_flags.maybe_rules_include.clone(),
        lint_flags.maybe_rules_exclude.clone(),
      ),
      fix: lint_flags.fix,
    }
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

pub fn discover_npmrc_from_workspace(
  workspace: &Workspace,
) -> Result<(Arc<ResolvedNpmRc>, Option<PathBuf>), AnyError> {
  let root_folder = workspace.root_folder_configs();
  discover_npmrc(
    root_folder.pkg_json.as_ref().map(|p| p.path.clone()),
    root_folder.deno_json.as_ref().and_then(|cf| {
      if cf.specifier.scheme() == "file" {
        Some(cf.specifier.to_file_path().unwrap())
      } else {
        None
      }
    }),
  )
}

/// Discover `.npmrc` file - currently we only support it next to `package.json`
/// or next to `deno.json`.
///
/// In the future we will need to support it in user directory or global directory
/// as per https://docs.npmjs.com/cli/v10/configuring-npm/npmrc#files.
fn discover_npmrc(
  maybe_package_json_path: Option<PathBuf>,
  maybe_deno_json_path: Option<PathBuf>,
) -> Result<(Arc<ResolvedNpmRc>, Option<PathBuf>), AnyError> {
  const NPMRC_NAME: &str = ".npmrc";

  fn get_env_var(var_name: &str) -> Option<String> {
    std::env::var(var_name).ok()
  }

  #[derive(Debug, Error)]
  #[error("Error loading .npmrc at {}.", path.display())]
  struct NpmRcLoadError {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  }

  fn try_to_read_npmrc(
    dir: &Path,
  ) -> Result<Option<(String, PathBuf)>, NpmRcLoadError> {
    let path = dir.join(NPMRC_NAME);
    let maybe_source = match std::fs::read_to_string(&path) {
      Ok(source) => Some(source),
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
      Err(err) => return Err(NpmRcLoadError { path, source: err }),
    };

    Ok(maybe_source.map(|source| (source, path)))
  }

  fn try_to_parse_npmrc(
    source: String,
    path: &Path,
  ) -> Result<Arc<ResolvedNpmRc>, AnyError> {
    let npmrc = NpmRc::parse(&source, &get_env_var).with_context(|| {
      format!("Failed to parse .npmrc at {}", path.display())
    })?;
    let resolved = npmrc
      .as_resolved(npm_registry_url())
      .context("Failed to resolve .npmrc options")?;
    Ok(Arc::new(resolved))
  }

  // 1. Try `.npmrc` next to `package.json`
  if let Some(package_json_path) = maybe_package_json_path {
    if let Some(package_json_dir) = package_json_path.parent() {
      if let Some((source, path)) = try_to_read_npmrc(package_json_dir)? {
        return try_to_parse_npmrc(source, &path).map(|r| (r, Some(path)));
      }
    }
  }

  // 2. Try `.npmrc` next to `deno.json(c)`
  if let Some(deno_json_path) = maybe_deno_json_path {
    if let Some(deno_json_dir) = deno_json_path.parent() {
      if let Some((source, path)) = try_to_read_npmrc(deno_json_dir)? {
        return try_to_parse_npmrc(source, &path).map(|r| (r, Some(path)));
      }
    }
  }

  // TODO(bartlomieju): update to read both files - one in the project root and one and
  // home dir and then merge them.
  // 3. Try `.npmrc` in the user's home directory
  if let Some(home_dir) = cache::home_dir() {
    match try_to_read_npmrc(&home_dir) {
      Ok(Some((source, path))) => {
        return try_to_parse_npmrc(source, &path).map(|r| (r, Some(path)));
      }
      Ok(None) => {}
      Err(err) if err.source.kind() == std::io::ErrorKind::PermissionDenied => {
        log::debug!(
          "Skipping .npmrc in home directory due to permission denied error. {:#}",
          err
        );
      }
      Err(err) => {
        return Err(err.into());
      }
    }
  }

  log::debug!("No .npmrc file found");
  Ok((create_default_npmrc(), None))
}

pub fn create_default_npmrc() -> Arc<ResolvedNpmRc> {
  Arc::new(ResolvedNpmRc {
    default_config: deno_npm::npm_rc::RegistryConfigWithUrl {
      registry_url: npm_registry_url().clone(),
      config: Default::default(),
    },
    scopes: Default::default(),
    registry_configs: Default::default(),
  })
}

#[derive(Error, Debug, Clone)]
pub enum RootCertStoreLoadError {
  #[error(
    "Unknown certificate store \"{0}\" specified (allowed: \"system,mozilla\")"
  )]
  UnknownStore(String),
  #[error("Unable to add pem file to certificate store: {0}")]
  FailedAddPemFile(String),
  #[error("Failed opening CA file: {0}")]
  CaFileOpenError(String),
}

/// Create and populate a root cert store based on the passed options and
/// environment.
pub fn get_root_cert_store(
  maybe_root_path: Option<PathBuf>,
  maybe_ca_stores: Option<Vec<String>>,
  maybe_ca_data: Option<CaData>,
) -> Result<RootCertStore, RootCertStoreLoadError> {
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
        root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.to_vec());
      }
      "system" => {
        let roots = load_native_certs().expect("could not load platform certs");
        for root in roots {
          if let Err(err) = root_cert_store
            .add(rustls::pki_types::CertificateDer::from(root.0.clone()))
          {
            log::error!(
              "{}",
              colors::yellow(&format!(
                "Unable to add system certificate to certificate store: {:?}",
                err
              ))
            );
            let hex_encoded_root = faster_hex::hex_string(&root.0);
            log::error!("{}", colors::gray(&hex_encoded_root));
          }
        }
      }
      _ => {
        return Err(RootCertStoreLoadError::UnknownStore(store.clone()));
      }
    }
  }

  let ca_data =
    maybe_ca_data.or_else(|| env::var("DENO_CERT").ok().map(CaData::File));
  if let Some(ca_data) = ca_data {
    let result = match ca_data {
      CaData::File(ca_file) => {
        let ca_file = if let Some(root) = &maybe_root_path {
          root.join(&ca_file)
        } else {
          PathBuf::from(ca_file)
        };
        let certfile = std::fs::File::open(ca_file).map_err(|err| {
          RootCertStoreLoadError::CaFileOpenError(err.to_string())
        })?;
        let mut reader = BufReader::new(certfile);
        rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()
      }
      CaData::Bytes(data) => {
        let mut reader = BufReader::new(Cursor::new(data));
        rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()
      }
    };

    match result {
      Ok(certs) => {
        root_cert_store.add_parsable_certificates(certs);
      }
      Err(e) => {
        return Err(RootCertStoreLoadError::FailedAddPemFile(e.to_string()));
      }
    }
  }

  Ok(root_cert_store)
}

/// State provided to the process via an environment variable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NpmProcessState {
  pub kind: NpmProcessStateKind,
  pub local_node_modules_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NpmProcessStateKind {
  Snapshot(deno_npm::resolution::SerializedNpmResolutionSnapshot),
  Byonm,
}

static NPM_PROCESS_STATE: Lazy<Option<NpmProcessState>> = Lazy::new(|| {
  use deno_runtime::ops::process::NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME;
  let fd = std::env::var(NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME).ok()?;
  std::env::remove_var(NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME);
  let fd = fd.parse::<usize>().ok()?;
  let mut file = {
    use deno_runtime::deno_io::FromRawIoHandle;
    unsafe { std::fs::File::from_raw_io_handle(fd as _) }
  };
  let mut buf = Vec::new();
  // seek to beginning. after the file is written the position will be inherited by this subprocess,
  // and also this file might have been read before
  file.seek(std::io::SeekFrom::Start(0)).unwrap();
  file
    .read_to_end(&mut buf)
    .inspect_err(|e| {
      log::error!("failed to read npm process state from fd {fd}: {e}");
    })
    .ok()?;
  let state: NpmProcessState = serde_json::from_slice(&buf)
    .inspect_err(|e| {
      log::error!(
        "failed to deserialize npm process state: {e} {}",
        String::from_utf8_lossy(&buf)
      )
    })
    .ok()?;
  Some(state)
});

/// Overrides for the options below that when set will
/// use these values over the values derived from the
/// CLI flags or config file.
#[derive(Default, Clone)]
struct CliOptionOverrides {
  import_map_specifier: Option<Option<ModuleSpecifier>>,
}

/// Holds the resolved options of many sources used by subcommands
/// and provides some helper function for creating common objects.
pub struct CliOptions {
  // the source of the options is a detail the rest of the
  // application need not concern itself with, so keep these private
  flags: Arc<Flags>,
  initial_cwd: PathBuf,
  main_module_cell: std::sync::OnceLock<Result<ModuleSpecifier, AnyError>>,
  maybe_node_modules_folder: Option<PathBuf>,
  npmrc: Arc<ResolvedNpmRc>,
  maybe_lockfile: Option<Arc<CliLockfile>>,
  overrides: CliOptionOverrides,
  pub start_dir: Arc<WorkspaceDirectory>,
  pub deno_dir_provider: Arc<DenoDirProvider>,
}

impl CliOptions {
  pub fn new(
    flags: Arc<Flags>,
    initial_cwd: PathBuf,
    maybe_lockfile: Option<Arc<CliLockfile>>,
    npmrc: Arc<ResolvedNpmRc>,
    start_dir: Arc<WorkspaceDirectory>,
    force_global_cache: bool,
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
      #[allow(clippy::print_stderr)]
      {
        // use eprintln instead of log::warn so this always gets shown
        eprintln!("{}", colors::yellow(msg));
      }
    }

    let maybe_lockfile = maybe_lockfile.filter(|_| !force_global_cache);
    let deno_dir_provider =
      Arc::new(DenoDirProvider::new(flags.internal.cache_path.clone()));
    let maybe_node_modules_folder = resolve_node_modules_folder(
      &initial_cwd,
      &flags,
      &start_dir.workspace,
      &deno_dir_provider,
    )
    .with_context(|| "Resolving node_modules folder.")?;

    load_env_variables_from_env_file(flags.env_file.as_ref());

    Ok(Self {
      flags,
      initial_cwd,
      maybe_lockfile,
      npmrc,
      maybe_node_modules_folder,
      overrides: Default::default(),
      main_module_cell: std::sync::OnceLock::new(),
      start_dir,
      deno_dir_provider,
    })
  }

  pub fn from_flags(flags: Arc<Flags>) -> Result<Self, AnyError> {
    let initial_cwd =
      std::env::current_dir().with_context(|| "Failed getting cwd.")?;
    let maybe_vendor_override = flags.vendor.map(|v| match v {
      true => VendorEnablement::Enable { cwd: &initial_cwd },
      false => VendorEnablement::Disable,
    });
    let resolve_workspace_discover_options = || {
      let additional_config_file_names: &'static [&'static str] =
        if matches!(flags.subcommand, DenoSubcommand::Publish(..)) {
          &["jsr.json", "jsr.jsonc"]
        } else {
          &[]
        };
      let config_parse_options = deno_config::deno_json::ConfigParseOptions {
        include_task_comments: matches!(
          flags.subcommand,
          DenoSubcommand::Task(..)
        ),
      };
      let discover_pkg_json = flags.config_flag != ConfigFlag::Disabled
        && !flags.no_npm
        && !has_flag_env_var("DENO_NO_PACKAGE_JSON");
      if !discover_pkg_json {
        log::debug!("package.json auto-discovery is disabled");
      }
      WorkspaceDiscoverOptions {
        fs: Default::default(), // use real fs
        deno_json_cache: None,
        pkg_json_cache: Some(&node_resolver::PackageJsonThreadLocalCache),
        workspace_cache: None,
        config_parse_options,
        additional_config_file_names,
        discover_pkg_json,
        maybe_vendor_override,
      }
    };
    let resolve_empty_options = || WorkspaceDirectoryEmptyOptions {
      root_dir: Arc::new(
        ModuleSpecifier::from_directory_path(&initial_cwd).unwrap(),
      ),
      use_vendor_dir: maybe_vendor_override
        .unwrap_or(VendorEnablement::Disable),
    };

    let start_dir = match &flags.config_flag {
      ConfigFlag::Discover => {
        if let Some(start_paths) = flags.config_path_args(&initial_cwd) {
          WorkspaceDirectory::discover(
            WorkspaceDiscoverStart::Paths(&start_paths),
            &resolve_workspace_discover_options(),
          )?
        } else {
          WorkspaceDirectory::empty(resolve_empty_options())
        }
      }
      ConfigFlag::Path(path) => {
        let config_path = normalize_path(initial_cwd.join(path));
        WorkspaceDirectory::discover(
          WorkspaceDiscoverStart::ConfigFile(&config_path),
          &resolve_workspace_discover_options(),
        )?
      }
      ConfigFlag::Disabled => {
        WorkspaceDirectory::empty(resolve_empty_options())
      }
    };

    for diagnostic in start_dir.workspace.diagnostics() {
      log::warn!("{} {}", colors::yellow("Warning"), diagnostic);
    }

    let (npmrc, _) = discover_npmrc_from_workspace(&start_dir.workspace)?;

    let maybe_lock_file = CliLockfile::discover(&flags, &start_dir.workspace)?;

    log::debug!("Finished config loading.");

    Self::new(
      flags,
      initial_cwd,
      maybe_lock_file.map(Arc::new),
      npmrc,
      Arc::new(start_dir),
      false,
    )
  }

  /// This method is purposefully verbose to disourage its use. Do not use it
  /// except in the factory structs. Instead, prefer specific methods on `CliOptions`
  /// that can take all sources of information into account (ex. config files or env vars).
  pub fn into_self_and_flags(
    self: Arc<CliOptions>,
  ) -> (Arc<CliOptions>, Arc<Flags>) {
    let flags = self.flags.clone();
    (self, flags)
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
    match self.sub_command() {
      DenoSubcommand::Compile(CompileFlags {
        target: Some(target),
        ..
      }) => {
        // the values of NpmSystemInfo align with the possible values for the
        // `arch` and `platform` fields of Node.js' `process` global:
        // https://nodejs.org/api/process.html
        match target.as_str() {
          "aarch64-apple-darwin" => NpmSystemInfo {
            os: "darwin".to_string(),
            cpu: "arm64".to_string(),
          },
          "aarch64-unknown-linux-gnu" => NpmSystemInfo {
            os: "linux".to_string(),
            cpu: "arm64".to_string(),
          },
          "x86_64-apple-darwin" => NpmSystemInfo {
            os: "darwin".to_string(),
            cpu: "x64".to_string(),
          },
          "x86_64-unknown-linux-gnu" => NpmSystemInfo {
            os: "linux".to_string(),
            cpu: "x64".to_string(),
          },
          "x86_64-pc-windows-msvc" => NpmSystemInfo {
            os: "win32".to_string(),
            cpu: "x64".to_string(),
          },
          value => {
            log::warn!(
              concat!(
                "Not implemented npm system info for target '{}'. Using current ",
                "system default. This may impact architecture specific dependencies."
              ),
              value,
            );
            NpmSystemInfo::default()
          }
        }
      }
      _ => NpmSystemInfo::default(),
    }
  }

  /// Resolve the specifier for a specified import map.
  ///
  /// This will NOT include the config file if it
  /// happens to be an import map.
  pub fn resolve_specified_import_map_specifier(
    &self,
  ) -> Result<Option<ModuleSpecifier>, AnyError> {
    match self.overrides.import_map_specifier.clone() {
      Some(maybe_url) => Ok(maybe_url),
      None => resolve_import_map_specifier(
        self.flags.import_map_path.as_deref(),
        self.workspace().root_deno_json().map(|c| c.as_ref()),
        &self.initial_cwd,
      ),
    }
  }

  pub async fn create_workspace_resolver(
    &self,
    file_fetcher: &FileFetcher,
    pkg_json_dep_resolution: PackageJsonDepResolution,
  ) -> Result<WorkspaceResolver, AnyError> {
    let overrode_no_import_map = self
      .overrides
      .import_map_specifier
      .as_ref()
      .map(|s| s.is_none())
      == Some(true);
    let cli_arg_specified_import_map = if overrode_no_import_map {
      // use a fake empty import map
      Some(deno_config::workspace::SpecifiedImportMap {
        base_url: self.workspace().root_dir().join("import_map.json").unwrap(),
        value: serde_json::Value::Object(Default::default()),
      })
    } else {
      let maybe_import_map_specifier =
        self.resolve_specified_import_map_specifier()?;
      match maybe_import_map_specifier {
        Some(specifier) => {
          let value =
            resolve_import_map_value_from_specifier(&specifier, file_fetcher)
              .await
              .with_context(|| {
                format!("Unable to load '{}' import map", specifier)
              })?;
          Some(deno_config::workspace::SpecifiedImportMap {
            base_url: specifier,
            value,
          })
        }
        None => None,
      }
    };
    Ok(self.workspace().create_resolver(
      CreateResolverOptions {
        pkg_json_dep_resolution,
        specified_import_map: cli_arg_specified_import_map,
      },
      |path| Ok(std::fs::read_to_string(path)?),
    )?)
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

  pub fn env_file_name(&self) -> Option<&String> {
    self.flags.env_file.as_ref()
  }

  pub fn resolve_main_module(&self) -> Result<&ModuleSpecifier, AnyError> {
    self
      .main_module_cell
      .get_or_init(|| {
        let main_module = match &self.flags.subcommand {
          DenoSubcommand::Compile(compile_flags) => {
            resolve_url_or_path(&compile_flags.source_file, self.initial_cwd())?
          }
          DenoSubcommand::Eval(_) => {
            resolve_url_or_path("./$deno$eval.ts", self.initial_cwd())?
          }
          DenoSubcommand::Repl(_) => {
            resolve_url_or_path("./$deno$repl.ts", self.initial_cwd())?
          }
          DenoSubcommand::Run(run_flags) => {
            if run_flags.is_stdin() {
              resolve_url_or_path("./$deno$stdin.ts", self.initial_cwd())?
            } else {
              resolve_url_or_path(&run_flags.script, self.initial_cwd())?
            }
          }
          DenoSubcommand::Serve(run_flags) => {
            resolve_url_or_path(&run_flags.script, self.initial_cwd())?
          }
          _ => {
            bail!("No main module.")
          }
        };

        Ok(main_module)
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

  pub fn resolve_npm_resolution_snapshot(
    &self,
  ) -> Result<Option<ValidSerializedNpmResolutionSnapshot>, AnyError> {
    if let Some(NpmProcessStateKind::Snapshot(snapshot)) =
      NPM_PROCESS_STATE.as_ref().map(|s| &s.kind)
    {
      // TODO(bartlomieju): remove this clone
      Ok(Some(snapshot.clone().into_valid()?))
    } else {
      Ok(None)
    }
  }

  // If the main module should be treated as being in an npm package.
  // This is triggered via a secret environment variable which is used
  // for functionality like child_process.fork. Users should NOT depend
  // on this functionality.
  pub fn is_npm_main(&self) -> bool {
    NPM_PROCESS_STATE.is_some()
  }

  pub fn has_node_modules_dir(&self) -> bool {
    self.maybe_node_modules_folder.is_some()
  }

  pub fn node_modules_dir_path(&self) -> Option<&PathBuf> {
    self.maybe_node_modules_folder.as_ref()
  }

  pub fn node_modules_dir(
    &self,
  ) -> Result<Option<NodeModulesDirMode>, AnyError> {
    if let Some(flag) = self.flags.node_modules_dir {
      return Ok(Some(flag));
    }
    self.workspace().node_modules_dir().map_err(Into::into)
  }

  pub fn vendor_dir_path(&self) -> Option<&PathBuf> {
    self.workspace().vendor_dir_path()
  }

  pub fn resolve_ts_config_for_emit(
    &self,
    config_type: TsConfigType,
  ) -> Result<TsConfigForEmit, AnyError> {
    self.workspace().resolve_ts_config_for_emit(config_type)
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
      version::DENO_VERSION_INFO.user_agent,
    )?))
  }

  pub fn maybe_lockfile(&self) -> Option<&Arc<CliLockfile>> {
    self.maybe_lockfile.as_ref()
  }

  pub fn to_compiler_option_types(
    &self,
  ) -> Result<Vec<deno_graph::ReferrerImports>, AnyError> {
    self
      .workspace()
      .to_compiler_option_types()
      .map(|maybe_imports| {
        maybe_imports
          .into_iter()
          .map(|(referrer, imports)| deno_graph::ReferrerImports {
            referrer,
            imports,
          })
          .collect()
      })
  }

  pub fn npmrc(&self) -> &Arc<ResolvedNpmRc> {
    &self.npmrc
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
      let options = LintOptions::resolve(config, lint_flags);
      result.push((ctx, options));
    }
    Ok(result)
  }

  pub fn resolve_deno_lint_config(
    &self,
  ) -> Result<deno_lint::linter::LintConfig, AnyError> {
    let ts_config_result =
      self.resolve_ts_config_for_emit(TsConfigType::Emit)?;

    let (transpile_options, _) =
      crate::args::ts_config_to_transpile_and_emit_options(
        ts_config_result.ts_config,
      )?;

    Ok(deno_lint::linter::LintConfig {
      default_jsx_factory: (!transpile_options.jsx_automatic)
        .then(|| transpile_options.jsx_factory.clone()),
      default_jsx_fragment_factory: (!transpile_options.jsx_automatic)
        .then(|| transpile_options.jsx_fragment_factory.clone()),
    })
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

  pub fn check_js(&self) -> bool {
    self.workspace().check_js()
  }

  pub fn coverage_dir(&self) -> Option<String> {
    match &self.flags.subcommand {
      DenoSubcommand::Test(test) => test
        .coverage_dir
        .as_ref()
        .map(ToOwned::to_owned)
        .or_else(|| env::var("DENO_UNSTABLE_COVERAGE_DIR").ok()),
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

  pub fn permission_flags(&self) -> &PermissionFlags {
    &self.flags.permissions
  }

  pub fn permissions_options(&self) -> PermissionsOptions {
    fn files_to_urls(files: &[String]) -> Vec<Cow<'_, Url>> {
      files
        .iter()
        .filter_map(|f| Url::parse(f).ok().map(Cow::Owned))
        .collect()
    }

    // get a list of urls to imply for --allow-import
    let cli_arg_urls = self
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
        DenoSubcommand::Install(InstallFlags {
          kind: InstallKind::Global(flags),
        }) => Url::parse(&flags.module_url)
          .ok()
          .map(|url| vec![Cow::Owned(url)]),
        _ => None,
      })
      .unwrap_or_default();
    self.flags.permissions.to_options(&cli_arg_urls)
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

  fn byonm_enabled(&self) -> bool {
    // check if enabled via unstable
    self.node_modules_dir().ok().flatten() == Some(NodeModulesDirMode::Manual)
      || NPM_PROCESS_STATE
        .as_ref()
        .map(|s| matches!(s.kind, NpmProcessStateKind::Byonm))
        .unwrap_or(false)
  }

  pub fn use_byonm(&self) -> bool {
    if self.node_modules_dir().ok().flatten().is_none()
      && self.maybe_node_modules_folder.is_some()
      && self
        .workspace()
        .config_folders()
        .values()
        .any(|f| f.pkg_json.is_some())
    {
      return true;
    }

    self.byonm_enabled()
  }

  pub fn unstable_sloppy_imports(&self) -> bool {
    self.flags.unstable_config.sloppy_imports
      || self.workspace().has_unstable("sloppy-imports")
  }

  pub fn unstable_features(&self) -> Vec<String> {
    let mut from_config_file = self.workspace().unstable_features().to_vec();

    self
      .flags
      .unstable_config
      .features
      .iter()
      .for_each(|feature| {
        if !from_config_file.contains(feature) {
          from_config_file.push(feature.to_string());
        }
      });

    if !from_config_file.is_empty() {
      // collect unstable granular flags
      let mut all_valid_unstable_flags: Vec<&str> =
        crate::UNSTABLE_GRANULAR_FLAGS
          .iter()
          .map(|granular_flag| granular_flag.name)
          .collect();

      let mut another_unstable_flags = Vec::from([
        "sloppy-imports",
        "byonm",
        "bare-node-builtins",
        "fmt-component",
      ]);
      // add more unstable flags to the same vector holding granular flags
      all_valid_unstable_flags.append(&mut another_unstable_flags);

      // check and warn if the unstable flag of config file isn't supported, by
      // iterating through the vector holding the unstable flags
      for unstable_value_from_config_file in &from_config_file {
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

    from_config_file
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
}

/// Resolves the path to use for a local node_modules folder.
fn resolve_node_modules_folder(
  cwd: &Path,
  flags: &Flags,
  workspace: &Workspace,
  deno_dir_provider: &Arc<DenoDirProvider>,
) -> Result<Option<PathBuf>, AnyError> {
  fn resolve_from_root(root_folder: &FolderConfigs, cwd: &Path) -> PathBuf {
    root_folder
      .deno_json
      .as_ref()
      .map(|c| Cow::Owned(c.dir_path()))
      .or_else(|| {
        root_folder
          .pkg_json
          .as_ref()
          .map(|c| Cow::Borrowed(c.dir_path()))
      })
      .unwrap_or(Cow::Borrowed(cwd))
      .join("node_modules")
  }

  let root_folder = workspace.root_folder_configs();
  let use_node_modules_dir = if let Some(mode) = flags.node_modules_dir {
    Some(mode.uses_node_modules_dir())
  } else {
    workspace
      .node_modules_dir()?
      .map(|m| m.uses_node_modules_dir())
      .or(flags.vendor)
      .or_else(|| root_folder.deno_json.as_ref().and_then(|c| c.json.vendor))
  };
  let path = if use_node_modules_dir == Some(false) {
    return Ok(None);
  } else if let Some(state) = &*NPM_PROCESS_STATE {
    return Ok(state.local_node_modules_path.as_ref().map(PathBuf::from));
  } else if root_folder.pkg_json.is_some() {
    let node_modules_dir = resolve_from_root(root_folder, cwd);
    if let Ok(deno_dir) = deno_dir_provider.get_or_create() {
      // `deno_dir.root` can be symlink in macOS
      if let Ok(root) = canonicalize_path_maybe_not_exists(&deno_dir.root) {
        if node_modules_dir.starts_with(root) {
          // if the package.json is in deno_dir, then do not use node_modules
          // next to it as local node_modules dir
          return Ok(None);
        }
      }
    }
    node_modules_dir
  } else if use_node_modules_dir.is_none() {
    return Ok(None);
  } else {
    resolve_from_root(root_folder, cwd)
  };
  Ok(Some(canonicalize_path_maybe_not_exists(&path)?))
}

fn resolve_import_map_specifier(
  maybe_import_map_path: Option<&str>,
  maybe_config_file: Option<&ConfigFile>,
  current_dir: &Path,
) -> Result<Option<ModuleSpecifier>, AnyError> {
  if let Some(import_map_path) = maybe_import_map_path {
    if let Some(config_file) = &maybe_config_file {
      if config_file.json.import_map.is_some() {
        log::warn!("{} the configuration file \"{}\" contains an entry for \"importMap\" that is being ignored.", colors::yellow("Warning"), config_file.specifier);
      }
    }
    let specifier =
      deno_core::resolve_url_or_path(import_map_path, current_dir)
        .with_context(|| {
          format!("Bad URL (\"{import_map_path}\") for import map.")
        })?;
    Ok(Some(specifier))
  } else {
    Ok(None)
  }
}

pub struct StorageKeyResolver(Option<Option<String>>);

impl StorageKeyResolver {
  pub fn from_options(options: &CliOptions) -> Self {
    Self(if let Some(location) = &options.flags.location {
      // if a location is set, then the ascii serialization of the location is
      // used, unless the origin is opaque, and then no storage origin is set, as
      // we can't expect the origin to be reproducible
      let storage_origin = location.origin();
      if storage_origin.is_tuple() {
        Some(Some(storage_origin.ascii_serialization()))
      } else {
        Some(None)
      }
    } else {
      // otherwise we will use the path to the config file or None to
      // fall back to using the main module's path
      options
        .start_dir
        .maybe_deno_json()
        .map(|config_file| Some(config_file.specifier.to_string()))
    })
  }

  /// Creates a storage key resolver that will always resolve to being empty.
  pub fn empty() -> Self {
    Self(Some(None))
  }

  /// Resolves the storage key to use based on the current flags, config, or main module.
  pub fn resolve_storage_key(
    &self,
    main_module: &ModuleSpecifier,
  ) -> Option<String> {
    // use the stored value or fall back to using the path of the main module.
    if let Some(maybe_value) = &self.0 {
      maybe_value.clone()
    } else {
      Some(main_module.to_string())
    }
  }
}

/// Resolves the no_prompt value based on the cli flags and environment.
pub fn resolve_no_prompt(flags: &PermissionFlags) -> bool {
  flags.no_prompt || has_flag_env_var("DENO_NO_PROMPT")
}

pub fn has_flag_env_var(name: &str) -> bool {
  let value = env::var(name);
  matches!(value.as_ref().map(|s| s.as_str()), Ok("1"))
}

pub fn npm_pkg_req_ref_to_binary_command(
  req_ref: &NpmPackageReqReference,
) -> String {
  let binary_name = req_ref.sub_path().unwrap_or(req_ref.req().name.as_str());
  binary_name.to_string()
}

pub fn config_to_deno_graph_workspace_member(
  config: &ConfigFile,
) -> Result<deno_graph::WorkspaceMember, AnyError> {
  let name = match &config.json.name {
    Some(name) => name.clone(),
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

fn load_env_variables_from_env_file(filename: Option<&String>) {
  let Some(env_file_name) = filename else {
    return;
  };
  match from_filename(env_file_name) {
    Ok(_) => (),
    Err(error) => {
      match error {
          dotenvy::Error::LineParse(line, index)=> log::info!("{} Parsing failed within the specified environment file: {} at index: {} of the value: {}",colors::yellow("Warning"), env_file_name, index, line),
          dotenvy::Error::Io(_)=> log::info!("{} The `--env-file` flag was used, but the environment file specified '{}' was not found.",colors::yellow("Warning"),env_file_name),
          dotenvy::Error::EnvVar(_)=> log::info!("{} One or more of the environment variables isn't present or not unicode within the specified environment file: {}",colors::yellow("Warning"),env_file_name),
          _ => log::info!("{} Unknown failure occurred with the specified environment file: {}", colors::yellow("Warning"), env_file_name),
        }
    }
  }
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
    let config_specifier =
      ModuleSpecifier::parse("file:///deno/deno.jsonc").unwrap();
    let config_file = ConfigFile::new(
      config_text,
      config_specifier,
      &deno_config::deno_json::ConfigParseOptions::default(),
    )
    .unwrap();
    let actual = resolve_import_map_specifier(
      Some("import-map.json"),
      Some(&config_file),
      cwd,
    );
    let import_map_path = cwd.join("import-map.json");
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
    let config_file = ConfigFile::new(
      config_text,
      config_specifier,
      &deno_config::deno_json::ConfigParseOptions::default(),
    )
    .unwrap();
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
  fn storage_key_resolver_test() {
    let resolver = StorageKeyResolver(None);
    let specifier = ModuleSpecifier::parse("file:///a.ts").unwrap();
    assert_eq!(
      resolver.resolve_storage_key(&specifier),
      Some(specifier.to_string())
    );
    let resolver = StorageKeyResolver(Some(None));
    assert_eq!(resolver.resolve_storage_key(&specifier), None);
    let resolver = StorageKeyResolver(Some(Some("value".to_string())));
    assert_eq!(
      resolver.resolve_storage_key(&specifier),
      Some("value".to_string())
    );

    // test empty
    let resolver = StorageKeyResolver::empty();
    assert_eq!(resolver.resolve_storage_key(&specifier), None);
  }

  #[test]
  fn jsr_urls() {
    let reg_url = jsr_url();
    assert!(reg_url.as_str().ends_with('/'));
    let reg_api_url = jsr_api_url();
    assert!(reg_api_url.as_str().ends_with('/'));
  }
}
