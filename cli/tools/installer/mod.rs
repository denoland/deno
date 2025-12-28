// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Write;
#[cfg(not(windows))]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use dashmap::DashSet;
use deno_cache_dir::file_fetcher::CacheSetting;
use deno_core::anyhow::Context;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_lib::args::CaData;
use deno_npm::NpmPackageId;
use deno_npm_installer::lifecycle_scripts::LifecycleScriptsWarning;
use deno_path_util::resolve_url_or_path;
use deno_resolver::workspace::WorkspaceResolver;
use deno_semver::npm::NpmPackageReqReference;
use log::Level;
use once_cell::sync::Lazy;
use regex::Regex;
use regex::RegexBuilder;

pub use self::bin_name_resolver::BinNameResolver;
use crate::args::AddFlags;
use crate::args::ConfigFlag;
use crate::args::Flags;
use crate::args::InstallEntrypointsFlags;
use crate::args::InstallFlags;
use crate::args::InstallFlagsGlobal;
use crate::args::InstallFlagsLocal;
use crate::args::InstallTopLevelFlags;
use crate::args::TypeCheckMode;
use crate::args::UninstallFlags;
use crate::args::UninstallKind;
use crate::args::resolve_no_prompt;
use crate::factory::CliFactory;
use crate::file_fetcher::CreateCliFileFetcherOptions;
use crate::file_fetcher::create_cli_file_fetcher;
use crate::graph_container::CollectSpecifiersOptions;
use crate::graph_container::ModuleGraphContainer;
use crate::jsr::JsrFetchResolver;
use crate::npm::CliNpmResolver;
use crate::npm::NpmFetchResolver;
use crate::sys::CliSys;
use crate::util::display;
use crate::util::fs::canonicalize_path_maybe_not_exists;

mod bin_name_resolver;

#[derive(Debug, Default)]
pub struct Count {
  value: AtomicUsize,
}

impl Count {
  pub fn inc(&self) {
    self.value.fetch_add(1, Ordering::Relaxed);
  }

  pub fn get(&self) -> usize {
    self.value.load(Ordering::Relaxed)
  }
}

#[derive(Default)]
pub struct InstallStats {
  pub resolved_jsr: DashSet<String>,
  pub downloaded_jsr: DashSet<String>,
  pub reused_jsr: DashSet<String>,
  pub resolved_npm: DashSet<String>,
  pub downloaded_npm: DashSet<String>,
  pub intialized_npm: DashSet<String>,
  pub reused_npm: Count,
}

impl std::fmt::Debug for InstallStats {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("InstallStats")
      .field(
        "resolved_jsr",
        &self
          .resolved_jsr
          .iter()
          .map(|s| s.as_str().to_string())
          .collect::<Vec<_>>(),
      )
      .field(
        "downloaded_jsr",
        &self
          .downloaded_jsr
          .iter()
          .map(|s| s.as_str().to_string())
          .collect::<Vec<_>>(),
      )
      .field("resolved_npm", &self.resolved_npm.len())
      .field("resolved_jsr_count", &self.resolved_jsr.len())
      .field("downloaded_npm", &self.downloaded_npm.len())
      .field("downloaded_jsr_count", &self.downloaded_jsr.len())
      .field(
        "intialized_npm",
        &self
          .intialized_npm
          .iter()
          .map(|s| s.as_str().to_string())
          .collect::<Vec<_>>(),
      )
      .field("intialized_npm_count", &self.intialized_npm.len())
      .field("reused_npm", &self.reused_npm.get())
      .finish()
  }
}

#[derive(Debug)]
pub struct InstallReporter {
  stats: Arc<InstallStats>,

  scripts_warnings: Arc<Mutex<Vec<LifecycleScriptsWarning>>>,

  deprecation_messages: Arc<Mutex<Vec<String>>>,
}

impl InstallReporter {
  pub fn new() -> Self {
    Self {
      stats: Arc::new(InstallStats::default()),
      scripts_warnings: Arc::new(Mutex::new(Vec::new())),
      deprecation_messages: Arc::new(Mutex::new(Vec::new())),
    }
  }

  pub fn take_scripts_warnings(&self) -> Vec<LifecycleScriptsWarning> {
    std::mem::take(&mut *self.scripts_warnings.lock())
  }

  pub fn take_deprecation_message(&self) -> Vec<String> {
    std::mem::take(&mut *self.deprecation_messages.lock())
  }
}

impl deno_npm_installer::InstallProgressReporter for InstallReporter {
  fn initializing(&self, _nv: &deno_semver::package::PackageNv) {}

  fn initialized(&self, nv: &deno_semver::package::PackageNv) {
    self.stats.intialized_npm.insert(nv.to_string());
  }

  fn blocking(&self, _message: &str) {}

  fn scripts_not_run_warning(
    &self,
    warning: deno_npm_installer::lifecycle_scripts::LifecycleScriptsWarning,
  ) {
    self.scripts_warnings.lock().push(warning);
  }

  fn deprecated_message(&self, message: String) {
    self.deprecation_messages.lock().push(message);
  }
}

fn package_nv_from_url(url: &Url) -> Option<String> {
  if !matches!(url.scheme(), "http" | "https") {
    return None;
  }
  if !url.host_str().is_some_and(|h| h.contains("jsr.io")) {
    return None;
  }
  let mut parts = url.path_segments()?;
  let scope = parts.next()?;
  let name = parts.next()?;
  let version = parts.next()?;
  if version.ends_with(".json") {
    // don't include meta.json urls
    return None;
  }
  Some(format!("{scope}/{name}@{version}"))
}

impl deno_graph::source::Reporter for InstallReporter {
  fn on_resolve(
    &self,
    _req: &deno_semver::package::PackageReq,
    package_nv: &deno_semver::package::PackageNv,
  ) {
    self.stats.resolved_jsr.insert(package_nv.to_string());
  }
}

impl deno_npm::resolution::Reporter for InstallReporter {
  fn on_resolved(
    &self,
    package_req: &deno_semver::package::PackageReq,
    _nv: &deno_semver::package::PackageNv,
  ) {
    self.stats.resolved_npm.insert(package_req.to_string());
  }
}

impl deno_npm_cache::TarballCacheReporter for InstallReporter {
  fn download_started(&self, _nv: &deno_semver::package::PackageNv) {}

  fn downloaded(&self, nv: &deno_semver::package::PackageNv) {
    self.stats.downloaded_npm.insert(nv.to_string());
  }

  fn reused_cache(&self, _nv: &deno_semver::package::PackageNv) {
    self.stats.reused_npm.inc();
  }
}

impl deno_resolver::file_fetcher::GraphLoaderReporter for InstallReporter {
  fn on_load(
    &self,
    specifier: &Url,
    loaded_from: deno_cache_dir::file_fetcher::LoadedFrom,
  ) {
    if let Some(nv) = package_nv_from_url(specifier) {
      match loaded_from {
        deno_cache_dir::file_fetcher::LoadedFrom::Cache => {
          self.stats.reused_jsr.insert(nv);
        }
        deno_cache_dir::file_fetcher::LoadedFrom::Remote => {
          self.stats.downloaded_jsr.insert(nv);
        }
        _ => {}
      }
    } else {
      // it's a local file or http/https specifier
    }
  }
}

static EXEC_NAME_RE: Lazy<Regex> = Lazy::new(|| {
  RegexBuilder::new(r"^[a-z0-9][\w-]*$")
    .case_insensitive(true)
    .build()
    .expect("invalid regex")
});

fn validate_name(exec_name: &str) -> Result<(), AnyError> {
  if EXEC_NAME_RE.is_match(exec_name) {
    Ok(())
  } else {
    Err(anyhow!("Invalid executable name: {exec_name}"))
  }
}

#[cfg(windows)]
/// On Windows, 2 files are generated.
/// One compatible with cmd & powershell with a .cmd extension
/// A second compatible with git bash / MINGW64
/// Generate batch script to satisfy that.
fn generate_executable_file(shim_data: &ShimData) -> Result<(), AnyError> {
  let args: Vec<String> =
    shim_data.args.iter().map(|c| format!("\"{c}\"")).collect();
  let template = format!(
    "% generated by deno install %\n@deno {} %*\n",
    args
      .iter()
      .map(|arg| arg.replace('%', "%%"))
      .collect::<Vec<_>>()
      .join(" ")
  );
  let mut file = File::create(&shim_data.file_path)?;
  file.write_all(template.as_bytes())?;

  // write file for bash
  // create filepath without extensions
  let template = format!(
    r#"#!/bin/sh
# generated by deno install
deno {} "$@"
"#,
    args.join(" "),
  );
  let mut file = File::create(shim_data.file_path.with_extension(""))?;
  file.write_all(template.as_bytes())?;
  Ok(())
}

#[cfg(not(windows))]
fn generate_executable_file(shim_data: &ShimData) -> Result<(), AnyError> {
  use shell_escape::escape;
  let args: Vec<String> = shim_data
    .args
    .iter()
    .map(|c| escape(c.into()).into_owned())
    .collect();
  let template = format!(
    r#"#!/bin/sh
# generated by deno install
exec deno {} "$@"
"#,
    args.join(" "),
  );
  let mut file = File::create(&shim_data.file_path)?;
  file.write_all(template.as_bytes())?;
  let _metadata = fs::metadata(&shim_data.file_path)?;
  let mut permissions = _metadata.permissions();
  permissions.set_mode(0o755);
  fs::set_permissions(&shim_data.file_path, permissions)?;
  Ok(())
}

fn get_installer_bin_dir(
  cwd: &Path,
  root_flag: Option<&str>,
) -> Result<PathBuf, AnyError> {
  let root = if let Some(root) = root_flag {
    canonicalize_path_maybe_not_exists(&cwd.join(root))?
  } else {
    get_installer_root()?
  };

  Ok(if !root.ends_with("bin") {
    root.join("bin")
  } else {
    root
  })
}

fn get_installer_root() -> Result<PathBuf, AnyError> {
  if let Some(env_dir) = env::var_os("DENO_INSTALL_ROOT")
    && !env_dir.is_empty()
  {
    let env_dir = PathBuf::from(env_dir);
    return canonicalize_path_maybe_not_exists(&env_dir).with_context(|| {
      format!(
        "Canonicalizing DENO_INSTALL_ROOT ('{}').",
        env_dir.display()
      )
    });
  }
  // Note: on Windows, the $HOME environment variable may be set by users or by
  // third party software, but it is non-standard and should not be relied upon.
  let home_env_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
  let mut home_path =
    env::var_os(home_env_var)
      .map(PathBuf::from)
      .ok_or_else(|| {
        io::Error::new(
          io::ErrorKind::NotFound,
          format!("${home_env_var} is not defined"),
        )
      })?;
  home_path.push(".deno");
  Ok(home_path)
}

pub async fn uninstall(
  flags: Arc<Flags>,
  uninstall_flags: UninstallFlags,
) -> Result<(), AnyError> {
  let uninstall_flags = match uninstall_flags.kind {
    UninstallKind::Global(flags) => flags,
    UninstallKind::Local(remove_flags) => {
      return super::pm::remove(flags, remove_flags).await;
    }
  };

  let cwd = std::env::current_dir().context("Unable to get CWD")?;
  let installation_dir =
    get_installer_bin_dir(&cwd, uninstall_flags.root.as_deref())?;

  // ensure directory exists
  if let Ok(metadata) = fs::metadata(&installation_dir)
    && !metadata.is_dir()
  {
    return Err(anyhow!("Installation path is not a directory"));
  }

  let file_path = installation_dir.join(&uninstall_flags.name);

  let mut removed = remove_file_if_exists(&file_path)?;

  if cfg!(windows) {
    let file_path = file_path.with_extension("cmd");
    removed |= remove_file_if_exists(&file_path)?;
  }

  if !removed {
    return Err(anyhow!(
      "No installation found for {}",
      uninstall_flags.name
    ));
  }

  // There might be some extra files to delete
  // Note: tsconfig.json is legacy. We renamed it to deno.json.
  // Remove cleaning it up after January 2024
  for ext in ["tsconfig.json", "deno.json", "lock.json"] {
    let file_path = file_path.with_extension(ext);
    remove_file_if_exists(&file_path)?;
  }

  log::info!("✅ Successfully uninstalled {}", uninstall_flags.name);
  Ok(())
}

fn remove_file_if_exists(file_path: &Path) -> Result<bool, AnyError> {
  if !file_path.exists() {
    return Ok(false);
  }

  fs::remove_file(file_path)
    .with_context(|| format!("Failed removing: {}", file_path.display()))?;
  log::info!("deleted {}", file_path.display());
  Ok(true)
}

pub(crate) async fn install_from_entrypoints(
  flags: Arc<Flags>,
  entrypoints_flags: InstallEntrypointsFlags,
) -> Result<(), AnyError> {
  let started = std::time::Instant::now();
  let factory = CliFactory::from_flags(flags.clone());
  let emitter = factory.emitter()?;
  let main_graph_container = factory.main_module_graph_container().await?;
  let specifiers = main_graph_container.collect_specifiers(
    &entrypoints_flags.entrypoints,
    CollectSpecifiersOptions {
      include_ignored_specified: true,
    },
  )?;
  main_graph_container
    .check_specifiers(
      &specifiers,
      crate::graph_container::CheckSpecifiersOptions {
        ext_overwrite: None,
        allow_unknown_media_types: true,
      },
    )
    .await?;
  emitter
    .cache_module_emits(&main_graph_container.graph())
    .await?;

  print_install_report(
    &factory.sys(),
    started.elapsed(),
    &factory.install_reporter()?.unwrap().clone(),
    factory.workspace_resolver().await?,
    factory.npm_resolver().await?,
  );
  Ok(())
}

async fn install_local(
  flags: Arc<Flags>,
  install_flags: InstallFlagsLocal,
) -> Result<(), AnyError> {
  match install_flags {
    InstallFlagsLocal::Add(add_flags) => {
      super::pm::add(flags, add_flags, super::pm::AddCommandName::Install).await
    }
    InstallFlagsLocal::Entrypoints(entrypoints) => {
      install_from_entrypoints(flags, entrypoints).await
    }
    InstallFlagsLocal::TopLevel(top_level_flags) => {
      install_top_level(flags, top_level_flags).await
    }
  }
}

#[derive(Debug, Default)]
struct CategorizedInstalledDeps {
  normal_deps: Vec<NpmPackageId>,
  dev_deps: Vec<NpmPackageId>,
}

fn categorize_installed_npm_deps(
  npm_resolver: &CliNpmResolver,
  workspace: &WorkspaceResolver<CliSys>,
  install_reporter: &InstallReporter,
) -> CategorizedInstalledDeps {
  let Some(managed_resolver) = npm_resolver.as_managed() else {
    return CategorizedInstalledDeps::default();
  };
  // compute the summary info
  let snapshot = managed_resolver.resolution().snapshot();

  let top_level_packages = snapshot.top_level_packages();

  // all this nonsense is to categorize into normal and dev deps
  let mut normal_deps = HashSet::new();
  let mut dev_deps = HashSet::new();

  for package_json in workspace.package_jsons() {
    let deps = package_json.resolve_local_package_json_deps();
    for (_k, v) in deps.dependencies.iter() {
      let Ok(s) = v else {
        continue;
      };
      match s {
        deno_package_json::PackageJsonDepValue::File(_) => {
          // TODO(nathanwhit)
          // TODO(bartlomieju)
        }
        deno_package_json::PackageJsonDepValue::Req(package_req) => {
          normal_deps.insert(package_req.name.to_string());
        }
        deno_package_json::PackageJsonDepValue::Workspace(
          _package_json_dep_workspace_req,
        ) => {
          // ignore workspace deps
        }
        deno_package_json::PackageJsonDepValue::JsrReq(_package_req) => {
          // ignore jsr deps
        }
      }
    }

    for (_k, v) in deps.dev_dependencies.iter() {
      let Ok(s) = v else {
        continue;
      };
      match s {
        deno_package_json::PackageJsonDepValue::File(_) => {
          // TODO(nathanwhit)
          // TODO(bartlomieju)
        }
        deno_package_json::PackageJsonDepValue::Req(package_req) => {
          dev_deps.insert(package_req.name.to_string());
        }
        deno_package_json::PackageJsonDepValue::Workspace(
          _package_json_dep_workspace_req,
        ) => {
          // ignore workspace deps
        }
        deno_package_json::PackageJsonDepValue::JsrReq(_package_req) => {
          // ignore jsr deps
        }
      }
    }
  }

  let mut installed_normal_deps = Vec::new();
  let mut installed_dev_deps = Vec::new();

  let npm_installed_set = if npm_resolver.root_node_modules_path().is_some() {
    &install_reporter.stats.intialized_npm
  } else {
    &install_reporter.stats.downloaded_npm
  };
  for pkg in top_level_packages {
    if !npm_installed_set.contains(&pkg.nv.to_string()) {
      continue;
    }
    if normal_deps.contains(&pkg.nv.name.to_string()) {
      installed_normal_deps.push(pkg);
    } else if dev_deps.contains(&pkg.nv.name.to_string()) {
      installed_dev_deps.push(pkg);
    } else {
      installed_normal_deps.push(pkg);
    }
  }

  installed_normal_deps.sort_by(|a, b| a.nv.name.cmp(&b.nv.name));
  installed_dev_deps.sort_by(|a, b| a.nv.name.cmp(&b.nv.name));

  CategorizedInstalledDeps {
    normal_deps: installed_normal_deps.into_iter().cloned().collect(),
    dev_deps: installed_dev_deps.into_iter().cloned().collect(),
  }
}

pub fn print_install_report(
  sys: &dyn sys_traits::boxed::FsOpenBoxed,
  elapsed: std::time::Duration,
  install_reporter: &InstallReporter,
  workspace: &WorkspaceResolver<CliSys>,
  npm_resolver: &CliNpmResolver,
) {
  fn human_elapsed(elapsed: u128) -> String {
    display::human_elapsed_with_ms_limit(elapsed, 3_000)
  }

  let rep = install_reporter;

  if !rep.stats.intialized_npm.is_empty()
    || !rep.stats.downloaded_jsr.is_empty()
  {
    let total_installed =
      rep.stats.intialized_npm.len() + rep.stats.downloaded_jsr.len();
    log::info!(
      "{} {} {} {} {}",
      deno_terminal::colors::gray("Installed"),
      total_installed,
      deno_terminal::colors::gray(format!(
        "package{}",
        if total_installed > 1 { "s" } else { "" },
      )),
      deno_terminal::colors::gray("in"),
      human_elapsed(elapsed.as_millis())
    );

    let total_reused = rep.stats.reused_npm.get() + rep.stats.reused_jsr.len();
    log::info!(
      "{} {} {}",
      deno_terminal::colors::gray("Reused"),
      total_reused,
      deno_terminal::colors::gray(format!(
        "package{} from cache",
        if total_reused == 1 { "" } else { "s" },
      )),
    );
    if total_reused > 0 {
      log::info!(
        "{}",
        deno_terminal::colors::yellow_bold("+".repeat(total_reused))
      );
    }

    let jsr_downloaded = rep.stats.downloaded_jsr.len();
    log::info!(
      "{} {} {}",
      deno_terminal::colors::gray("Downloaded"),
      jsr_downloaded,
      deno_terminal::colors::gray(format!(
        "package{} from JSR",
        if jsr_downloaded == 1 { "" } else { "s" },
      )),
    );
    if jsr_downloaded > 0 {
      log::info!(
        "{}",
        deno_terminal::colors::green("+".repeat(jsr_downloaded))
      );
    }

    let npm_download = rep.stats.downloaded_npm.len();
    log::info!(
      "{} {} {}",
      deno_terminal::colors::gray("Downloaded"),
      npm_download,
      deno_terminal::colors::gray(format!(
        "package{} from npm",
        if npm_download == 1 { "" } else { "s" },
      )),
    );
    if npm_download > 0 {
      log::info!("{}", deno_terminal::colors::green("+".repeat(npm_download)));
    }
  }

  let CategorizedInstalledDeps {
    normal_deps: installed_normal_deps,
    dev_deps: installed_dev_deps,
  } = categorize_installed_npm_deps(npm_resolver, workspace, install_reporter);

  if !installed_normal_deps.is_empty() || !rep.stats.downloaded_jsr.is_empty() {
    log::info!("");
    log::info!("{}", deno_terminal::colors::cyan("Dependencies:"));
    let mut jsr_packages = rep
      .stats
      .downloaded_jsr
      .clone()
      .into_iter()
      .collect::<Vec<_>>();
    jsr_packages.sort();
    for pkg in jsr_packages {
      let (name, version) = pkg.rsplit_once("@").unwrap();
      log::info!(
        "{} {}{} {}",
        deno_terminal::colors::green("+"),
        deno_terminal::colors::gray("jsr:"),
        name,
        deno_terminal::colors::gray(version)
      );
    }
    for pkg in &installed_normal_deps {
      log::info!(
        "{} {}{} {}",
        deno_terminal::colors::green("+"),
        deno_terminal::colors::gray("npm:"),
        pkg.nv.name,
        deno_terminal::colors::gray(pkg.nv.version.to_string())
      );
    }
    log::info!("");
  }
  if !installed_dev_deps.is_empty() {
    log::info!("{}", deno_terminal::colors::cyan("Dev dependencies:"));
    for pkg in &installed_dev_deps {
      log::info!(
        "{} {}{} {}",
        deno_terminal::colors::green("+"),
        deno_terminal::colors::gray("npm:"),
        pkg.nv.name,
        deno_terminal::colors::gray(pkg.nv.version.to_string())
      );
    }
  }

  let warnings = install_reporter.take_scripts_warnings();
  for warning in warnings {
    log::warn!("{}", warning.into_message(sys));
  }

  let deprecation_messages = install_reporter.take_deprecation_message();
  for message in deprecation_messages {
    log::warn!("{}", message);
  }
}

async fn install_top_level(
  flags: Arc<Flags>,
  top_level_flags: InstallTopLevelFlags,
) -> Result<(), AnyError> {
  let start_instant = std::time::Instant::now();
  let factory = CliFactory::from_flags(flags);
  // surface any errors in the package.json
  factory
    .npm_installer()
    .await?
    .ensure_no_pkg_json_dep_errors()?;
  let npm_installer = factory.npm_installer().await?;
  npm_installer.ensure_no_pkg_json_dep_errors()?;

  // the actual work
  crate::tools::pm::cache_top_level_deps(
    &factory,
    None,
    crate::tools::pm::CacheTopLevelDepsOptions {
      lockfile_only: top_level_flags.lockfile_only,
    },
  )
  .await?;

  if let Some(lockfile) = factory.maybe_lockfile().await? {
    lockfile.write_if_changed()?;
  }

  let install_reporter = factory.install_reporter()?.unwrap().clone();
  let workspace = factory.workspace_resolver().await?;
  let npm_resolver = factory.npm_resolver().await?;
  print_install_report(
    &factory.sys(),
    start_instant.elapsed(),
    &install_reporter,
    workspace,
    npm_resolver,
  );

  Ok(())
}

fn check_if_installs_a_single_package_globally(
  maybe_add_flags: Option<&AddFlags>,
) -> Result<(), AnyError> {
  let Some(add_flags) = maybe_add_flags else {
    return Ok(());
  };
  if add_flags.packages.len() != 1 {
    return Ok(());
  }
  let Ok(url) = Url::parse(&add_flags.packages[0]) else {
    return Ok(());
  };
  if matches!(url.scheme(), "http" | "https") {
    bail!(
      "Failed to install \"{}\" specifier. If you are trying to install {} globally, run again with `-g` flag:\n  deno install -g {}",
      url.scheme(),
      url.as_str(),
      url.as_str()
    );
  }
  Ok(())
}

pub async fn install_command(
  flags: Arc<Flags>,
  install_flags: InstallFlags,
) -> Result<(), AnyError> {
  match install_flags {
    InstallFlags::Global(global_flags) => {
      install_global(flags, global_flags).await
    }
    InstallFlags::Local(local_flags) => {
      if let InstallFlagsLocal::Add(add_flags) = &local_flags {
        check_if_installs_a_single_package_globally(Some(add_flags))?;
      }
      install_local(flags, local_flags).await
    }
  }
}

async fn install_global(
  flags: Arc<Flags>,
  install_flags_global: InstallFlagsGlobal,
) -> Result<(), AnyError> {
  // ensure the module is cached
  let factory = CliFactory::from_flags(flags.clone());

  let cli_options = factory.cli_options()?;
  let http_client = factory.http_client_provider();
  let deps_http_cache = factory.global_http_cache()?;
  let deps_file_fetcher = create_cli_file_fetcher(
    Default::default(),
    deno_cache_dir::GlobalOrLocalHttpCache::Global(deps_http_cache.clone()),
    http_client.clone(),
    factory.memory_files().clone(),
    factory.sys(),
    CreateCliFileFetcherOptions {
      allow_remote: true,
      cache_setting: CacheSetting::ReloadAll,
      download_log_level: log::Level::Trace,
      progress_bar: None,
    },
  );

  let npmrc = factory.npmrc()?;

  let deps_file_fetcher = Arc::new(deps_file_fetcher);
  let jsr_resolver = Arc::new(JsrFetchResolver::new(
    deps_file_fetcher.clone(),
    factory.jsr_version_resolver()?.clone(),
  ));
  let npm_resolver = Arc::new(NpmFetchResolver::new(
    deps_file_fetcher.clone(),
    npmrc.clone(),
    factory.npm_version_resolver()?.clone(),
  ));

  if matches!(flags.config_flag, ConfigFlag::Discover)
    && cli_options.workspace().deno_jsons().next().is_some()
  {
    log::warn!(
      "{} discovered config file will be ignored in the installed command. Use the --config flag if you wish to include it.",
      crate::colors::yellow("Warning")
    );
  }

  for (i, module_url) in install_flags_global.module_urls.iter().enumerate() {
    let entry_text = module_url;
    if !cli_options.initial_cwd().join(entry_text).exists() {
      // provide a helpful error message for users migrating from Deno < 3.0
      if i == 1
        && install_flags_global.args.is_empty()
        && Url::parse(entry_text).is_err()
      {
        bail!(
          concat!(
            "{} is missing a prefix. Deno 3.0 requires `--` before script arguments in `deno install -g`. ",
            "Did you mean `deno install -g {} -- {}`? Or maybe provide a `jsr:` or `npm:` prefix?",
          ),
          entry_text,
          &install_flags_global.module_urls[0],
          install_flags_global.module_urls[1..].join(" "),
        )
      }
      // check for package requirement missing prefix
      if let Ok(Err(package_req)) =
        super::pm::AddRmPackageReq::parse(entry_text, None)
      {
        if package_req.name.starts_with("@")
          && jsr_resolver
            .req_to_nv(&package_req)
            .await
            .ok()
            .flatten()
            .is_some()
        {
          bail!(
            "{entry_text} is missing a prefix. Did you mean `{}`?",
            crate::colors::yellow(format!("deno install -g jsr:{package_req}"))
          );
        } else if npm_resolver
          .req_to_nv(&package_req)
          .await
          .ok()
          .flatten()
          .is_some()
        {
          bail!(
            "{entry_text} is missing a prefix. Did you mean `{}`?",
            crate::colors::yellow(format!("deno install -g npm:{package_req}"))
          );
        }
      }
    }

    factory
      .main_module_graph_container()
      .await?
      .load_and_type_check_files(
        std::slice::from_ref(module_url),
        CollectSpecifiersOptions {
          include_ignored_specified: true,
        },
      )
      .await?;

    let bin_name_resolver = factory.bin_name_resolver()?;
    // create the install shim
    create_install_shim(
      &bin_name_resolver,
      cli_options.initial_cwd(),
      &flags,
      &install_flags_global,
      module_url,
    )
    .await?;
  }
  Ok(())
}

async fn create_install_shim(
  bin_name_resolver: &BinNameResolver<'_>,
  cwd: &Path,
  flags: &Flags,
  install_flags_global: &InstallFlagsGlobal,
  module_url: &str,
) -> Result<(), AnyError> {
  let shim_data = resolve_shim_data(
    bin_name_resolver,
    cwd,
    flags,
    install_flags_global,
    module_url,
  )
  .await?;

  // ensure directory exists
  if let Ok(metadata) = fs::metadata(&shim_data.installation_dir) {
    if !metadata.is_dir() {
      return Err(anyhow!("Installation path is not a directory"));
    }
  } else {
    fs::create_dir_all(&shim_data.installation_dir)?;
  };

  if shim_data.file_path.exists() && !install_flags_global.force {
    return Err(anyhow!(
      "Existing installation found. Aborting (Use -f to overwrite).",
    ));
  };

  generate_executable_file(&shim_data)?;
  for (path, contents) in shim_data.extra_files {
    fs::write(path, contents)?;
  }

  log::info!("✅ Successfully installed {}", shim_data.name);
  log::info!("{}", shim_data.file_path.display());
  if cfg!(windows) {
    let display_path = shim_data.file_path.with_extension("");
    log::info!("{} (shell)", display_path.display());
  }
  let installation_dir_str = shim_data.installation_dir.to_string_lossy();

  if !is_in_path(&shim_data.installation_dir) {
    log::info!("ℹ️  Add {} to PATH", installation_dir_str);
    if cfg!(windows) {
      log::info!("    set PATH=%PATH%;{}", installation_dir_str);
    } else {
      log::info!("    export PATH=\"{}:$PATH\"", installation_dir_str);
    }
  }

  Ok(())
}

struct ShimData {
  name: String,
  installation_dir: PathBuf,
  file_path: PathBuf,
  args: Vec<String>,
  extra_files: Vec<(PathBuf, String)>,
}

async fn resolve_shim_data(
  bin_name_resolver: &BinNameResolver<'_>,
  cwd: &Path,
  flags: &Flags,
  install_flags_global: &InstallFlagsGlobal,
  module_url: &str,
) -> Result<ShimData, AnyError> {
  let installation_dir =
    get_installer_bin_dir(cwd, install_flags_global.root.as_deref())?;

  // Check if module_url is remote
  let module_url = resolve_url_or_path(module_url, cwd)?;
  let name = if install_flags_global.name.is_some() {
    install_flags_global.name.clone()
  } else {
    bin_name_resolver.infer_name_from_url(&module_url).await
  };

  let name = match name {
    Some(name) => name,
    None => {
      return Err(anyhow!(
        "An executable name was not provided. One could not be inferred from the URL. Aborting.\n  {} {}",
        deno_runtime::colors::cyan("hint:"),
        "provide one with the `--name` flag"
      ));
    }
  };

  validate_name(name.as_str())?;
  let mut file_path = installation_dir.join(&name);

  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }

  let mut extra_files: Vec<(PathBuf, String)> = vec![];

  let mut executable_args = vec!["run".to_string()];
  executable_args.extend_from_slice(&flags.to_permission_args());
  if let Some(url) = flags.location.as_ref() {
    executable_args.push("--location".to_string());
    executable_args.push(url.to_string());
  }
  if let Some(CaData::File(ca_file)) = &flags.ca_data {
    executable_args.push("--cert".to_string());
    executable_args.push(ca_file.to_owned())
  }
  if let Some(log_level) = flags.log_level {
    if log_level == Level::Error {
      executable_args.push("--quiet".to_string());
    } else {
      executable_args.push("--log-level".to_string());
      let log_level = match log_level {
        Level::Debug => "debug",
        Level::Info => "info",
        _ => return Err(anyhow!(format!("invalid log level {log_level}"))),
      };
      executable_args.push(log_level.to_string());
    }
  }

  // we should avoid a default branch here to ensure we continue to cover any
  // changes to this flag.
  match flags.type_check_mode {
    TypeCheckMode::All => executable_args.push("--check=all".to_string()),
    TypeCheckMode::None => {}
    TypeCheckMode::Local => executable_args.push("--check".to_string()),
  }

  for feature in &flags.unstable_config.features {
    executable_args.push(format!("--unstable-{}", feature));
  }

  if flags.no_remote {
    executable_args.push("--no-remote".to_string());
  }

  if flags.no_npm {
    executable_args.push("--no-npm".to_string());
  }

  if flags.cached_only {
    executable_args.push("--cached-only".to_string());
  }

  if flags.frozen_lockfile.unwrap_or(false) {
    executable_args.push("--frozen".to_string());
  }

  if resolve_no_prompt(&flags.permissions) {
    executable_args.push("--no-prompt".to_string());
  }

  if !flags.v8_flags.is_empty() {
    executable_args.push(format!("--v8-flags={}", flags.v8_flags.join(",")));
  }

  if let Some(seed) = flags.seed {
    executable_args.push("--seed".to_string());
    executable_args.push(seed.to_string());
  }

  if let Some(inspect) = flags.inspect {
    executable_args.push(format!("--inspect={inspect}"));
  }

  if let Some(inspect_brk) = flags.inspect_brk {
    executable_args.push(format!("--inspect-brk={inspect_brk}"));
  }

  if let Some(import_map_path) = &flags.import_map_path {
    let import_map_url = resolve_url_or_path(import_map_path, cwd)?;
    executable_args.push("--import-map".to_string());
    executable_args.push(import_map_url.to_string());
  }

  if let ConfigFlag::Path(config_path) = &flags.config_flag {
    let copy_path = get_hidden_file_with_ext(&file_path, "deno.json");
    executable_args.push("--config".to_string());
    executable_args.push(copy_path.to_str().unwrap().to_string());
    let mut config_text = fs::read_to_string(config_path)
      .with_context(|| format!("error reading {config_path}"))?;
    // always remove the import map field because when someone specifies `--import-map` we
    // don't want that file to be attempted to be loaded and when they don't specify that
    // (which is just something we haven't implemented yet)
    if let Some(new_text) = remove_import_map_field_from_text(&config_text) {
      if flags.import_map_path.is_none() {
        log::warn!(
          "{} \"importMap\" field in the specified config file we be ignored. Use the --import-map flag instead.",
          crate::colors::yellow("Warning"),
        );
      }
      config_text = new_text;
    }

    extra_files.push((copy_path, config_text));
  } else {
    executable_args.push("--no-config".to_string());
  }

  if flags.no_lock {
    executable_args.push("--no-lock".to_string());
  } else if flags.lock.is_some()
    // always use a lockfile for an npm entrypoint unless --no-lock
    || NpmPackageReqReference::from_specifier(&module_url).is_ok()
  {
    let copy_path = get_hidden_file_with_ext(&file_path, "lock.json");
    executable_args.push("--lock".to_string());
    executable_args.push(copy_path.to_str().unwrap().to_string());

    if let Some(lock_path) = &flags.lock {
      extra_files.push((
        copy_path,
        fs::read_to_string(lock_path)
          .with_context(|| format!("error reading {}", lock_path))?,
      ));
    } else {
      // Provide an empty lockfile so that this overwrites any existing lockfile
      // from a previous installation. This will get populated on first run.
      extra_files.push((copy_path, "{}".to_string()));
    }
  }

  executable_args.push(module_url.into());
  executable_args.extend_from_slice(&install_flags_global.args);

  Ok(ShimData {
    name,
    installation_dir,
    file_path,
    args: executable_args,
    extra_files,
  })
}

fn remove_import_map_field_from_text(config_text: &str) -> Option<String> {
  let value =
    jsonc_parser::cst::CstRootNode::parse(config_text, &Default::default())
      .ok()?;
  let root_value = value.object_value()?;
  let import_map_value = root_value.get("importMap")?;
  import_map_value.remove();
  Some(value.to_string())
}

fn get_hidden_file_with_ext(file_path: &Path, ext: &str) -> PathBuf {
  // use a dot file to prevent the file from showing up in some
  // users shell auto-complete since this directory is on the PATH
  file_path
    .with_file_name(format!(
      ".{}",
      file_path.file_name().unwrap().to_string_lossy()
    ))
    .with_extension(ext)
}

fn is_in_path(dir: &Path) -> bool {
  if let Some(paths) = env::var_os("PATH") {
    for p in env::split_paths(&paths) {
      if *dir == p {
        return true;
      }
    }
  }
  false
}

#[cfg(test)]
mod tests {
  use std::process::Command;

  use deno_lib::args::UnstableConfig;
  use deno_npm::resolution::NpmVersionResolver;
  use test_util::TempDir;
  use test_util::testdata_path;

  use super::*;
  use crate::args::ConfigFlag;
  use crate::args::PermissionFlags;
  use crate::args::UninstallFlagsGlobal;
  use crate::http_util::HttpClientProvider;
  use crate::util::fs::canonicalize_path;

  async fn create_install_shim(
    flags: &Flags,
    install_flags_global: InstallFlagsGlobal,
  ) -> Result<(), AnyError> {
    let cwd = std::env::current_dir().unwrap();
    let http_client = HttpClientProvider::new(None, None);
    let registry_api = deno_npm::registry::TestNpmRegistryApi::default();
    let npm_version_resolver = NpmVersionResolver::default();
    let resolver =
      BinNameResolver::new(&http_client, &registry_api, &npm_version_resolver);
    super::create_install_shim(
      &resolver,
      &cwd,
      flags,
      &install_flags_global,
      &install_flags_global.module_urls[0],
    )
    .await
  }

  async fn resolve_shim_data(
    flags: &Flags,
    install_flags_global: &InstallFlagsGlobal,
  ) -> Result<ShimData, AnyError> {
    let cwd = std::env::current_dir().unwrap();
    let http_client = HttpClientProvider::new(None, None);
    let registry_api = deno_npm::registry::TestNpmRegistryApi::default();
    let npm_version_resolver = NpmVersionResolver::default();
    let resolver =
      BinNameResolver::new(&http_client, &registry_api, &npm_version_resolver);
    super::resolve_shim_data(
      &resolver,
      &cwd,
      flags,
      install_flags_global,
      &install_flags_global.module_urls[0],
    )
    .await
  }

  #[tokio::test]
  async fn install_unstable() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();

    create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo_server.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: false,
      },
    )
    .await
    .unwrap();

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }

    assert!(file_path.exists());

    let content = fs::read_to_string(file_path).unwrap();
    if cfg!(windows) {
      assert!(content.contains(
        r#""run" "--no-config" "http://localhost:4545/echo_server.ts""#
      ));
    } else {
      assert!(
        content.contains(
          r#"run --no-config 'http://localhost:4545/echo_server.ts'"#
        )
      );
    }
  }

  #[tokio::test]
  async fn install_inferred_name() {
    let shim_data = resolve_shim_data(
      &Flags::default(),
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo_server.ts".to_string()],
        args: vec![],
        name: None,
        root: Some(env::temp_dir().to_string_lossy().into_owned()),
        force: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(shim_data.name, "echo_server");
    assert_eq!(
      shim_data.args,
      vec!["run", "--no-config", "http://localhost:4545/echo_server.ts",]
    );
  }

  #[tokio::test]
  async fn install_unstable_legacy() {
    let shim_data = resolve_shim_data(
      &Default::default(),
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo_server.ts".to_string()],
        args: vec![],
        name: None,
        root: Some(env::temp_dir().to_string_lossy().into_owned()),
        force: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(shim_data.name, "echo_server");
    assert_eq!(
      shim_data.args,
      vec!["run", "--no-config", "http://localhost:4545/echo_server.ts",]
    );
  }

  #[tokio::test]
  async fn install_unstable_features() {
    let shim_data = resolve_shim_data(
      &Flags {
        unstable_config: UnstableConfig {
          features: vec!["kv".to_string(), "cron".to_string()],
          ..Default::default()
        },
        ..Default::default()
      },
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo_server.ts".to_string()],
        args: vec![],
        name: None,
        root: Some(env::temp_dir().to_string_lossy().into_owned()),
        force: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(shim_data.name, "echo_server");
    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--unstable-kv",
        "--unstable-cron",
        "--no-config",
        "http://localhost:4545/echo_server.ts",
      ]
    );
  }

  #[tokio::test]
  async fn install_inferred_name_from_parent() {
    let shim_data = resolve_shim_data(
      &Flags::default(),
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/subdir/main.ts".to_string()],
        args: vec![],
        name: None,
        root: Some(env::temp_dir().to_string_lossy().into_owned()),
        force: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(shim_data.name, "subdir");
    assert_eq!(
      shim_data.args,
      vec!["run", "--no-config", "http://localhost:4545/subdir/main.ts",]
    );
  }

  #[tokio::test]
  async fn install_inferred_name_after_redirect_for_no_path_url() {
    let _http_server_guard = test_util::http_server();
    let shim_data = resolve_shim_data(
      &Flags::default(),
      &InstallFlagsGlobal {
        module_urls: vec![
          "http://localhost:4550/?redirect_to=/subdir/redirects/a.ts"
            .to_string(),
        ],
        args: vec![],
        name: None,
        root: Some(env::temp_dir().to_string_lossy().into_owned()),
        force: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(shim_data.name, "a");
    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--no-config",
        "http://localhost:4550/?redirect_to=/subdir/redirects/a.ts",
      ]
    );
  }

  #[tokio::test]
  async fn install_custom_dir_option() {
    let shim_data = resolve_shim_data(
      &Flags::default(),
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo_server.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(env::temp_dir().to_string_lossy().into_owned()),
        force: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(shim_data.name, "echo_test");
    assert_eq!(
      shim_data.args,
      vec!["run", "--no-config", "http://localhost:4545/echo_server.ts",]
    );
  }

  #[tokio::test]
  async fn install_with_flags() {
    let shim_data = resolve_shim_data(
      &Flags {
        permissions: PermissionFlags {
          allow_net: Some(vec![]),
          allow_read: Some(vec![]),
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::None,
        log_level: Some(Level::Error),
        ..Flags::default()
      },
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo_server.ts".to_string()],
        args: vec!["--foobar".to_string()],
        name: Some("echo_test".to_string()),
        root: Some(env::temp_dir().to_string_lossy().into_owned()),
        force: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(shim_data.name, "echo_test");
    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--allow-read",
        "--allow-net",
        "--quiet",
        "--no-config",
        "http://localhost:4545/echo_server.ts",
        "--foobar",
      ]
    );
  }

  #[tokio::test]
  async fn install_prompt() {
    let shim_data = resolve_shim_data(
      &Flags {
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      },
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo_server.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(env::temp_dir().to_string_lossy().into_owned()),
        force: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--no-prompt",
        "--no-config",
        "http://localhost:4545/echo_server.ts",
      ]
    );
  }

  #[tokio::test]
  async fn install_allow_all() {
    let shim_data = resolve_shim_data(
      &Flags {
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      },
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo_server.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(env::temp_dir().to_string_lossy().into_owned()),
        force: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--allow-all",
        "--no-config",
        "http://localhost:4545/echo_server.ts",
      ]
    );
  }

  #[tokio::test]
  async fn install_npm_lockfile_default() {
    let temp_dir = canonicalize_path(&env::temp_dir()).unwrap();
    let shim_data = resolve_shim_data(
      &Flags {
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      },
      &InstallFlagsGlobal {
        module_urls: vec!["npm:cowsay".to_string()],
        args: vec![],
        name: None,
        root: Some(temp_dir.to_string_lossy().into_owned()),
        force: false,
      },
    )
    .await
    .unwrap();

    let lock_path = temp_dir.join("bin").join(".cowsay.lock.json");
    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--allow-all",
        "--no-config",
        "--lock",
        &lock_path.to_string_lossy(),
        "npm:cowsay"
      ]
    );
    assert_eq!(shim_data.extra_files, vec![(lock_path, "{}".to_string())]);
  }

  #[tokio::test]
  async fn install_npm_no_lock() {
    let shim_data = resolve_shim_data(
      &Flags {
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        no_lock: true,
        ..Flags::default()
      },
      &InstallFlagsGlobal {
        module_urls: vec!["npm:cowsay".to_string()],
        args: vec![],
        name: None,
        root: Some(env::temp_dir().to_string_lossy().into_owned()),
        force: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--allow-all",
        "--no-config",
        "--no-lock",
        "npm:cowsay"
      ]
    );
    assert_eq!(shim_data.extra_files, vec![]);
  }

  #[tokio::test]
  async fn install_local_module() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();
    let local_module = env::current_dir().unwrap().join("echo_server.ts");
    let local_module_url = Url::from_file_path(&local_module).unwrap();
    let local_module_str = local_module.to_string_lossy();

    create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec![local_module_str.to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: false,
      },
    )
    .await
    .unwrap();

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }

    assert!(file_path.exists());
    let content = fs::read_to_string(file_path).unwrap();
    assert!(content.contains(&local_module_url.to_string()));
  }

  #[tokio::test]
  async fn install_force() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();

    create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo_server.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: false,
      },
    )
    .await
    .unwrap();

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }
    assert!(file_path.exists());

    // No force. Install failed.
    let no_force_result = create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/cat.ts".to_string()], // using a different URL
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: false,
      },
    )
    .await;
    assert!(no_force_result.is_err());
    assert!(
      no_force_result
        .unwrap_err()
        .to_string()
        .contains("Existing installation found")
    );
    // Assert not modified
    let file_content = fs::read_to_string(&file_path).unwrap();
    assert!(file_content.contains("echo_server.ts"));

    // Force. Install success.
    let force_result = create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/cat.ts".to_string()], // using a different URL
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: true,
      },
    )
    .await;
    assert!(force_result.is_ok());
    // Assert modified
    let file_content_2 = fs::read_to_string(&file_path).unwrap();
    assert!(file_content_2.contains("cat.ts"));
  }

  #[tokio::test]
  async fn install_with_config() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    let config_file_path = temp_dir.path().join("test_tsconfig.json");
    let config = "{}";
    let mut config_file = File::create(&config_file_path).unwrap();
    let result = config_file.write_all(config.as_bytes());
    assert!(result.is_ok());

    let result = create_install_shim(
      &Flags {
        config_flag: ConfigFlag::Path(config_file_path.to_string()),
        ..Flags::default()
      },
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/cat.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: true,
      },
    )
    .await;
    assert!(result.is_ok());

    let config_file_name = ".echo_test.deno.json";

    let file_path = bin_dir.join(config_file_name);
    assert!(file_path.exists());
    let content = fs::read_to_string(file_path).unwrap();
    assert!(content == "{}");
  }

  // TODO: enable on Windows after fixing batch escaping
  #[cfg(not(windows))]
  #[tokio::test]
  async fn install_shell_escaping() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();

    create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo_server.ts".to_string()],
        args: vec!["\"".to_string()],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: false,
      },
    )
    .await
    .unwrap();

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }

    assert!(file_path.exists());
    let content = fs::read_to_string(file_path).unwrap();
    if cfg!(windows) {
      // TODO: see comment above this test
    } else {
      assert!(content.contains(
        r#"run --no-config 'http://localhost:4545/echo_server.ts' '"'"#
      ));
    }
  }

  #[tokio::test]
  async fn install_unicode() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();
    let unicode_dir = temp_dir.path().join("Magnús");
    std::fs::create_dir(&unicode_dir).unwrap();
    let local_module = unicode_dir.join("echo_server.ts");
    let local_module_str = local_module.to_string_lossy();
    std::fs::write(&local_module, "// Some JavaScript I guess").unwrap();

    create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec![local_module_str.to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: false,
      },
    )
    .await
    .unwrap();

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }

    // We need to actually run it to make sure the URL is interpreted correctly
    let status = Command::new(file_path)
      .env_clear()
      // use the deno binary in the target directory
      .env("PATH", test_util::target_dir())
      .env("RUST_BACKTRACE", "1")
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
    assert!(status.success());
  }

  #[tokio::test]
  async fn install_with_import_map() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    let import_map_path = temp_dir.path().join("import_map.json");
    let import_map_url = Url::from_file_path(&import_map_path).unwrap();
    let import_map = "{ \"imports\": {} }";
    let mut import_map_file = File::create(&import_map_path).unwrap();
    let result = import_map_file.write_all(import_map.as_bytes());
    assert!(result.is_ok());

    let result = create_install_shim(
      &Flags {
        import_map_path: Some(import_map_path.to_string()),
        ..Flags::default()
      },
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/cat.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: true,
      },
    )
    .await;
    assert!(result.is_ok());

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }
    assert!(file_path.exists());

    let mut expected_string = format!(
      "--import-map '{import_map_url}' --no-config 'http://localhost:4545/cat.ts'"
    );
    if cfg!(windows) {
      expected_string = format!(
        "\"--import-map\" \"{import_map_url}\" \"--no-config\" \"http://localhost:4545/cat.ts\""
      );
    }

    let content = fs::read_to_string(file_path).unwrap();
    assert!(content.contains(&expected_string));
  }

  // Regression test for https://github.com/denoland/deno/issues/10556.
  #[tokio::test]
  async fn install_file_url() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    let module_path = fs::canonicalize(testdata_path().join("cat.ts")).unwrap();
    let file_module_string =
      Url::from_file_path(module_path).unwrap().to_string();
    assert!(file_module_string.starts_with("file:///"));

    let result = create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec![file_module_string.to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: true,
      },
    )
    .await;
    assert!(result.is_ok());

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }
    assert!(file_path.exists());

    let mut expected_string =
      format!("run --no-config '{}'", &file_module_string);
    if cfg!(windows) {
      expected_string =
        format!("\"run\" \"--no-config\" \"{}\"", &file_module_string);
    }

    let content = fs::read_to_string(file_path).unwrap();
    assert!(content.contains(&expected_string));
  }

  #[tokio::test]
  async fn uninstall_basic() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();

    let mut file_path = bin_dir.join("echo_test");
    File::create(&file_path).unwrap();
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
      File::create(&file_path).unwrap();
    }

    // create extra files
    {
      let file_path = file_path.with_extension("deno.json");
      File::create(file_path).unwrap();
    }
    {
      // legacy tsconfig.json, make sure it's cleaned up for now
      let file_path = file_path.with_extension("tsconfig.json");
      File::create(file_path).unwrap();
    }
    {
      let file_path = file_path.with_extension("lock.json");
      File::create(file_path).unwrap();
    }

    uninstall(
      Default::default(),
      UninstallFlags {
        kind: UninstallKind::Global(UninstallFlagsGlobal {
          name: "echo_test".to_string(),
          root: Some(temp_dir.path().to_string()),
        }),
      },
    )
    .await
    .unwrap();

    assert!(!file_path.exists());
    assert!(!file_path.with_extension("tsconfig.json").exists());
    assert!(!file_path.with_extension("deno.json").exists());
    assert!(!file_path.with_extension("lock.json").exists());

    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
      assert!(!file_path.exists());
    }
  }

  #[test]
  fn test_remove_import_map_field_from_text() {
    assert_eq!(
      remove_import_map_field_from_text(
        r#"{
    "importMap": "./value.json"
}"#,
      )
      .unwrap(),
      "{}"
    );
  }
}
