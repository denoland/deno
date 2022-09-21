// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

pub mod config_file;
pub mod flags;

mod flags_allow_net;

pub use config_file::CompilerOptions;
pub use config_file::ConfigFile;
pub use config_file::EmitConfigOptions;
pub use config_file::FmtConfig;
pub use config_file::FmtOptionsConfig;
pub use config_file::LintConfig;
pub use config_file::LintRulesConfig;
pub use config_file::MaybeImportsResult;
pub use config_file::ProseWrap;
pub use config_file::TestConfig;
pub use config_file::TsConfig;
pub use flags::*;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::normalize_path;
use deno_core::url::Url;
use deno_runtime::colors;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::permissions::PermissionsOptions;
use std::collections::BTreeMap;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use crate::args::config_file::JsxImportSourceConfig;
use crate::deno_dir::DenoDir;
use crate::emit::get_ts_config_for_emit;
use crate::emit::TsConfigType;
use crate::emit::TsConfigWithIgnoredOptions;
use crate::emit::TsTypeLib;
use crate::file_fetcher::get_root_cert_store;
use crate::file_fetcher::CacheSetting;
use crate::lockfile::Lockfile;
use crate::version;

/// Overrides for the options below that when set will
/// use these values over the values derived from the
/// CLI flags or config file.
#[derive(Default)]
struct CliOptionOverrides {
  import_map_specifier: Option<Option<ModuleSpecifier>>,
}

/// Holds the common options used by many sub commands
/// and provides some helper function for creating common objects.
pub struct CliOptions {
  // the source of the options is a detail the rest of the
  // application need not concern itself with, so keep these private
  flags: Flags,
  maybe_config_file: Option<ConfigFile>,
  overrides: CliOptionOverrides,
}

impl CliOptions {
  pub fn new(flags: Flags, maybe_config_file: Option<ConfigFile>) -> Self {
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

    Self {
      maybe_config_file,
      flags,
      overrides: Default::default(),
    }
  }

  pub fn from_flags(flags: Flags) -> Result<Self, AnyError> {
    let maybe_config_file = ConfigFile::discover(&flags)?;
    Ok(Self::new(flags, maybe_config_file))
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
  ) -> Result<TsConfigWithIgnoredOptions, AnyError> {
    get_ts_config_for_emit(config_type, self.maybe_config_file.as_ref())
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
      let storage_origin = location.origin().ascii_serialization();
      if storage_origin == "null" {
        None
      } else {
        Some(storage_origin)
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
    let maybe_inspect_host = self.flags.inspect.or(self.flags.inspect_brk);
    maybe_inspect_host
      .map(|host| InspectorServer::new(host, version::get_user_agent()))
  }

  pub fn resolve_lock_file(&self) -> Result<Option<Lockfile>, AnyError> {
    if let Some(filename) = &self.flags.lock {
      let lockfile = Lockfile::new(filename.clone(), self.flags.lock_write)?;
      Ok(Some(lockfile))
    } else {
      Ok(None)
    }
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

  pub fn to_lint_config(&self) -> Result<Option<LintConfig>, AnyError> {
    if let Some(config_file) = &self.maybe_config_file {
      config_file.to_lint_config()
    } else {
      Ok(None)
    }
  }

  pub fn to_test_config(&self) -> Result<Option<TestConfig>, AnyError> {
    if let Some(config_file) = &self.maybe_config_file {
      config_file.to_test_config()
    } else {
      Ok(None)
    }
  }

  pub fn to_fmt_config(&self) -> Result<Option<FmtConfig>, AnyError> {
    if let Some(config) = &self.maybe_config_file {
      config.to_fmt_config()
    } else {
      Ok(None)
    }
  }

  /// Vector of user script CLI arguments.
  pub fn argv(&self) -> &Vec<String> {
    &self.flags.argv
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
    self.flags.inspect.is_some() || self.flags.inspect_brk.is_some()
  }

  pub fn inspect_brk(&self) -> Option<SocketAddr> {
    self.flags.inspect_brk
  }

  pub fn log_level(&self) -> Option<log::Level> {
    self.flags.log_level
  }

  pub fn location_flag(&self) -> Option<&Url> {
    self.flags.location.as_ref()
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

  pub fn no_remote(&self) -> bool {
    self.flags.no_remote
  }

  pub fn no_npm(&self) -> bool {
    self.flags.no_npm
  }

  pub fn permissions_options(&self) -> PermissionsOptions {
    self.flags.permissions_options()
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

  pub fn unsafely_ignore_certificate_errors(&self) -> Option<&Vec<String>> {
    self.flags.unsafely_ignore_certificate_errors.as_ref()
  }

  pub fn unstable(&self) -> bool {
    self.flags.unstable
  }

  pub fn watch_paths(&self) -> Option<&Vec<PathBuf>> {
    self.flags.watch.as_ref()
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
      ModuleSpecifier::from_file_path(&import_map_path).unwrap();
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
