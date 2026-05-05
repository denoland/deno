// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Arc;

use dashmap::DashSet;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_npm_installer::lifecycle_scripts::LifecycleScriptsWarning;
use deno_resolver::workspace::WorkspaceResolver;

pub use self::bin_name_resolver::BinNameResolver;
use crate::args::Flags;
use crate::args::InstallEntrypointsFlags;
use crate::args::InstallFlags;
use crate::args::InstallFlagsLocal;
use crate::factory::CliFactory;
use crate::graph_container::CollectSpecifiersOptions;
use crate::graph_container::ModuleGraphContainer;
use crate::npm::CliNpmResolver;
use crate::sys::CliSys;
use crate::util::display;

mod bin_name_resolver;
mod global;
mod local;
mod npm_compat;

pub use global::uninstall;
use local::CategorizedInstalledDeps;
use local::categorize_installed_npm_deps;

#[derive(Default)]
pub struct InstallStats {
  pub resolved_jsr: DashSet<String>,
  pub downloaded_jsr: DashSet<String>,
  pub reused_jsr: DashSet<String>,
  pub resolved_npm: DashSet<String>,
  pub downloaded_npm: DashSet<String>,
  pub intialized_npm: DashSet<String>,
  pub reused_npm: crate::util::sync::RelaxedAtomicCounter,
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

pub async fn install_from_entrypoints(
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
    &[],
  );
  Ok(())
}

pub async fn install_command(
  flags: Arc<Flags>,
  install_flags: InstallFlags,
) -> Result<(), AnyError> {
  match install_flags {
    InstallFlags::Global(global_flags) => {
      Box::pin(global::install_global(flags, global_flags)).await
    }
    InstallFlags::Local(local_flags, _) => {
      if let InstallFlagsLocal::Add(add_flags) = &local_flags {
        local::check_if_installs_a_single_package_globally(Some(add_flags))?;
      }
      Box::pin(local::install_local(flags, local_flags)).await
    }
  }
}

pub fn print_install_report(
  sys: &dyn sys_traits::boxed::FsOpenBoxed,
  elapsed: std::time::Duration,
  install_reporter: &InstallReporter,
  workspace: &WorkspaceResolver<CliSys>,
  npm_resolver: &CliNpmResolver,
  installed_jsr_compat: &[npm_compat::InstalledJsrPackage],
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

  let has_deps = !installed_normal_deps.is_empty()
    || !rep.stats.downloaded_jsr.is_empty()
    || !installed_jsr_compat.is_empty();

  if has_deps {
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
    // JSR packages installed via npm.jsr.io for stock TS compatibility
    for pkg in installed_jsr_compat {
      // Convert @jsr/std__assert back to @std/assert for display
      let display_name = pkg
        .name
        .strip_prefix("@jsr/")
        .map(|n| {
          if let Some((scope, name)) = n.split_once("__") {
            format!("@{scope}/{name}")
          } else {
            n.to_string()
          }
        })
        .unwrap_or_else(|| pkg.name.clone());
      log::info!(
        "{} {}{} {}",
        deno_terminal::colors::green("+"),
        deno_terminal::colors::gray("jsr:"),
        display_name,
        deno_terminal::colors::gray(&pkg.version)
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
