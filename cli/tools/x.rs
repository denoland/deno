// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_cache_dir::file_fetcher::CacheSetting;
use deno_core::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_lib::worker::LibWorkerFactoryRoots;
use deno_npm_installer::PackagesAllowedScripts;
use deno_runtime::UnconfiguredRuntime;
use deno_runtime::deno_permissions::PathQueryDescriptor;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageReq;
use node_resolver::BinValue;

use crate::args::DenoXShimName;
use crate::args::Flags;
use crate::args::XFlags;
use crate::args::XFlagsKind;
use crate::factory::CliFactory;
use crate::node::CliNodeResolver;
use crate::npm::CliManagedNpmResolver;
use crate::npm::CliNpmResolver;
use crate::tools::pm::CacheTopLevelDepsOptions;
use crate::util::console::ConfirmOptions;
use crate::util::console::confirm;
use crate::util::draw_thread::DrawThread;

async fn resolve_local_bins(
  node_resolver: &CliNodeResolver,
  npm_resolver: &CliNpmResolver,
  factory: &CliFactory,
) -> Result<BTreeMap<String, BinValue>, AnyError> {
  match &npm_resolver {
    deno_resolver::npm::NpmResolver::Byonm(npm_resolver) => {
      let node_modules_dir = npm_resolver.root_node_modules_path().unwrap();
      let bin_dir = node_modules_dir.join(".bin");
      Ok(node_resolver.resolve_npm_commands_from_bin_dir(&bin_dir))
    }
    deno_resolver::npm::NpmResolver::Managed(npm_resolver) => {
      let mut all_bins = BTreeMap::new();
      for id in npm_resolver.resolution().top_level_packages() {
        let package_folder =
          npm_resolver.resolve_pkg_folder_from_pkg_id(&id)?;
        let bins = match node_resolver
          .resolve_npm_binary_commands_for_package(&package_folder)
        {
          Ok(bins) => bins,
          Err(_) => {
            crate::tools::pm::cache_top_level_deps(
              factory,
              None,
              CacheTopLevelDepsOptions {
                lockfile_only: false,
              },
            )
            .await?;
            node_resolver
              .resolve_npm_binary_commands_for_package(&package_folder)?
          }
        };
        for (command, bin_value) in bins {
          all_bins.insert(command.clone(), bin_value.clone());
        }
      }
      Ok(all_bins)
    }
  }
}

async fn run_js_file(
  factory: &CliFactory,
  roots: LibWorkerFactoryRoots,
  unconfigured_runtime: Option<UnconfiguredRuntime>,
  main_module: &deno_core::url::Url,
  npm: bool,
) -> Result<i32, AnyError> {
  let cli_options = factory.cli_options()?;
  let preload_modules = cli_options.preload_modules()?;
  let require_modules = cli_options.require_modules()?;

  if npm {
    crate::tools::run::set_npm_user_agent();
  }

  crate::tools::run::maybe_npm_install(factory).await?;

  let worker_factory = factory
    .create_cli_main_worker_factory_with_roots(roots)
    .await?;
  let mut worker = worker_factory
    .create_main_worker_with_unconfigured_runtime(
      deno_runtime::WorkerExecutionMode::Run,
      main_module.clone(),
      preload_modules,
      require_modules,
      unconfigured_runtime,
    )
    .await
    .inspect_err(|e| deno_telemetry::report_event("boot_failure", e))?;

  let exit_code = worker
    .run()
    .await
    .inspect_err(|e| deno_telemetry::report_event("uncaught_exception", e))?;
  Ok(exit_code)
}

async fn maybe_run_local_npm_bin(
  factory: &CliFactory,
  flags: &Flags,
  roots: LibWorkerFactoryRoots,
  unconfigured_runtime: &mut Option<UnconfiguredRuntime>,
  node_resolver: &CliNodeResolver,
  npm_resolver: &CliNpmResolver,
  command: &str,
) -> Result<Option<i32>, AnyError> {
  let permissions = factory.root_permissions_container()?;

  let mut bins =
    resolve_local_bins(node_resolver, npm_resolver, factory).await?;
  let bin_value = if let Some(bin_value) = bins.remove(command) {
    bin_value
  } else if let Some(bin_value) = {
    let command = if command.starts_with("@") && command.contains("/") {
      command.split("/").last().unwrap()
    } else {
      command
    };
    bins.remove(command)
  } {
    bin_value
  } else {
    return Ok(None);
  };

  match bin_value {
    BinValue::JsFile(path_buf) => {
      let path = deno_path_util::url_from_file_path(path_buf.as_ref())?;
      let unconfigured_runtime = unconfigured_runtime.take();
      return run_js_file(factory, roots, unconfigured_runtime, &path, true)
        .await
        .map(Some);
    }
    BinValue::Executable(mut path_buf) => {
      if cfg!(windows) && path_buf.extension().is_none() {
        // prefer cmd shim over sh
        path_buf.set_extension("cmd");
        if !path_buf.exists() {
          //  just fall back to original path
          path_buf.set_extension("");
        }
      }
      permissions.check_run(
        &deno_runtime::deno_permissions::RunQueryDescriptor::Path(
          PathQueryDescriptor::new(
            &factory.sys(),
            std::borrow::Cow::Borrowed(path_buf.as_ref()),
          )?,
        ),
        "entrypoint",
      )?;
      let mut child = std::process::Command::new(path_buf)
        .args(&flags.argv)
        .spawn()
        .context("Failed to spawn command")?;
      let status = child.wait()?;
      Ok(Some(status.code().unwrap_or(1)))
    }
  }
}

enum XTempDir {
  Existing(PathBuf),
  New(PathBuf),
}
impl XTempDir {
  fn path(&self) -> &PathBuf {
    match self {
      XTempDir::Existing(path) => path,
      XTempDir::New(path) => path,
    }
  }
}

fn create_package_temp_dir(
  prefix: Option<&str>,
  package_req: &PackageReq,
  reload: bool,
  deno_dir: &Path,
) -> Result<XTempDir, AnyError> {
  let mut package_req_folder = String::from(prefix.unwrap_or(""));
  package_req_folder.push_str(
    &package_req
      .to_string()
      .replace("/", "__")
      .replace(">", "gt")
      .replace("<", "lt"),
  );
  let temp_dir = deno_dir.join("deno_x_cache").join(package_req_folder);
  if temp_dir.exists() {
    if reload || !temp_dir.join("deno.lock").exists() {
      std::fs::remove_dir_all(&temp_dir)?;
    } else {
      let canonicalized_temp_dir = temp_dir
        .canonicalize()
        .ok()
        .map(deno_path_util::strip_unc_prefix);
      let temp_dir = canonicalized_temp_dir.unwrap_or(temp_dir);
      return Ok(XTempDir::Existing(temp_dir));
    }
  }
  std::fs::create_dir_all(&temp_dir)?;
  let package_json_path = temp_dir.join("package.json");
  std::fs::write(&package_json_path, "{}")?;
  let deno_json_path = temp_dir.join("deno.json");
  std::fs::write(&deno_json_path, r#"{"nodeModulesDir": "auto"}"#)?;

  let canonicalized_temp_dir = temp_dir
    .canonicalize()
    .ok()
    .map(deno_path_util::strip_unc_prefix);
  let temp_dir = canonicalized_temp_dir.unwrap_or(temp_dir);
  Ok(XTempDir::New(temp_dir))
}

fn write_shim(
  out_dir: &Path,
  shim_name: DenoXShimName,
) -> Result<(), AnyError> {
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let out_path = out_dir.join(shim_name.name());
    if let DenoXShimName::Other(_) = shim_name {
      std::fs::write(
        &out_path,
        r##"#!/bin/sh
SCRIPT_DIR="$(dirname -- "$(readlink -f -- "$0")")"
exec "$SCRIPT_DIR/deno" x "$@"
"##
          .as_bytes(),
      )?;
      let mut permissions = std::fs::metadata(&out_path)?.permissions();
      permissions.set_mode(0o755);
      std::fs::set_permissions(&out_path, permissions)?;
    } else {
      match std::os::unix::fs::symlink("./deno", &out_path) {
        Ok(_) => {}
        Err(e) => match e.kind() {
          std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(&out_path)?;
            std::os::unix::fs::symlink("./deno", &out_path)?;
          }
          _ => return Err(e.into()),
        },
      }
    }
  }

  #[cfg(windows)]
  {
    let out_path = out_dir.join(format!("{}.cmd", shim_name.name()));
    std::fs::write(
      out_path,
      r##"@echo off
"%~dp0deno.exe" x %*
exit /b %ERRORLEVEL%
"##,
    )?;
  }

  Ok(())
}

pub async fn run(
  flags: Arc<Flags>,
  x_flags: XFlags,
  mut unconfigured_runtime: Option<UnconfiguredRuntime>,
  roots: LibWorkerFactoryRoots,
) -> Result<i32, AnyError> {
  let command_flags = match x_flags.kind {
    XFlagsKind::InstallAlias(shim_name) => {
      let exe = std::env::current_exe()?;
      let out_dir = exe.parent().unwrap();
      write_shim(out_dir, shim_name)?;
      return Ok(0);
    }
    XFlagsKind::Command(command) => command,
    XFlagsKind::Print => {
      let factory = CliFactory::from_flags(flags.clone());
      let npm_resolver = factory.npm_resolver().await?;
      let node_resolver = factory.node_resolver().await?;
      let bins =
        resolve_local_bins(node_resolver, npm_resolver, &factory).await?;
      if bins.is_empty() {
        log::info!("No local commands found");
        return Ok(0);
      }
      log::info!("Available (local) commands:");
      for command in bins.keys() {
        log::info!("  {}", command);
      }
      return Ok(0);
    }
  };
  let factory = CliFactory::from_flags(flags.clone());
  let npm_resolver = factory.npm_resolver().await?;
  let node_resolver = factory.node_resolver().await?;
  let result = maybe_run_local_npm_bin(
    &factory,
    &flags,
    roots.clone(),
    &mut unconfigured_runtime,
    node_resolver,
    npm_resolver,
    &command_flags.command,
  )
  .await?;
  if let Some(exit_code) = result {
    return Ok(exit_code);
  }

  let cli_options = factory.cli_options()?;

  let is_file_like = command_flags.command.starts_with('.')
    || command_flags.command.starts_with('/')
    || command_flags.command.starts_with('~')
    || command_flags.command.starts_with('\\')
    || Path::new(&command_flags.command).extension().is_some();
  if is_file_like && Path::new(&command_flags.command).is_file() {
    return Err(anyhow::anyhow!(
      "Use 'deno run' to run a local file directly, 'deno x' is intended for running commands from packages."
    ));
  }

  let thing_to_run = match deno_core::url::Url::parse(&command_flags.command) {
    Ok(url) => {
      if url.scheme() == "npm" {
        let req_ref = NpmPackageReqReference::from_specifier(&url)?;
        ReqRefOrUrl::Npm(req_ref)
      } else if url.scheme() == "jsr" {
        let req_ref = JsrPackageReqReference::from_specifier(&url)?;
        ReqRefOrUrl::Jsr(req_ref)
      } else {
        ReqRefOrUrl::Url(url)
      }
    }
    Err(deno_core::url::ParseError::RelativeUrlWithoutBase) => {
      let new_command = format!("npm:{}", command_flags.command);
      let req_ref = NpmPackageReqReference::from_str(&new_command)?;
      ReqRefOrUrl::Npm(req_ref)
    }
    Err(e) => {
      return Err(e.into());
    }
  };

  let cache_setting = cli_options.cache_setting();
  let reload = matches!(cache_setting, CacheSetting::ReloadAll);
  match thing_to_run {
    ReqRefOrUrl::Npm(npm_package_req_reference) => {
      let (managed_flags, managed_factory) = autoinstall_package(
        ReqRef::Npm(&npm_package_req_reference),
        &flags,
        reload,
        command_flags.yes,
        &factory.deno_dir()?.root,
      )
      .await?;
      let mut runner_flags = (*managed_flags).clone();
      runner_flags.node_modules_dir =
        Some(deno_config::deno_json::NodeModulesDirMode::Manual);
      let runner_flags = Arc::new(runner_flags);
      let runner_factory = CliFactory::from_flags(runner_flags.clone());
      let runner_node_resolver = runner_factory.node_resolver().await?;
      let runner_npm_resolver = runner_factory.npm_resolver().await?;

      let bin_name =
        if let Some(sub_path) = npm_package_req_reference.sub_path() {
          sub_path
        } else {
          npm_package_req_reference.req().name.as_str()
        };

      let res = maybe_run_local_npm_bin(
        &runner_factory,
        &runner_flags,
        roots.clone(),
        &mut unconfigured_runtime,
        runner_node_resolver,
        runner_npm_resolver,
        bin_name,
      )
      .await?;
      if let Some(exit_code) = res {
        Ok(exit_code)
      } else {
        let managed_npm_resolver =
          managed_factory.npm_resolver().await?.as_managed().unwrap();
        let bin_commands = bin_commands_for_package(
          runner_node_resolver,
          managed_npm_resolver,
          npm_package_req_reference.req(),
        )?;
        let fallback_name = if bin_commands.len() == 1 {
          Some(bin_commands.keys().next().unwrap())
        } else {
          None
        };

        if let Some(fallback_name) = fallback_name
          && let Some(exit_code) = maybe_run_local_npm_bin(
            &runner_factory,
            &runner_flags,
            roots.clone(),
            &mut unconfigured_runtime,
            runner_node_resolver,
            runner_npm_resolver,
            fallback_name.as_ref(),
          )
          .await?
        {
          return Ok(exit_code);
        }

        Err(anyhow::anyhow!(
          "Unable to choose binary for {}\n  Available bins:\n{}",
          command_flags.command,
          bin_commands
            .keys()
            .map(|k| format!("    {}", k))
            .collect::<Vec<_>>()
            .join("\n")
        ))
      }
    }
    ReqRefOrUrl::Jsr(jsr_package_req_reference) => {
      let (_new_flags, new_factory) = autoinstall_package(
        ReqRef::Jsr(&jsr_package_req_reference),
        &flags,
        reload,
        command_flags.yes,
        &factory.deno_dir()?.root,
      )
      .await?;

      let url =
        deno_core::url::Url::parse(&jsr_package_req_reference.to_string())?;
      run_js_file(&new_factory, roots, None, &url, false).await
    }
    ReqRefOrUrl::Url(url) => {
      let mut new_flags = (*flags).clone();
      new_flags.node_modules_dir =
        Some(deno_config::deno_json::NodeModulesDirMode::None);
      new_flags.internal.lockfile_skip_write = true;

      let new_flags = Arc::new(new_flags);
      let new_factory = CliFactory::from_flags(new_flags.clone());
      run_js_file(&new_factory, roots, None, &url, false).await
    }
  }
}

fn bin_commands_for_package(
  node_resolver: &CliNodeResolver,
  managed_npm_resolver: &CliManagedNpmResolver,
  package_req: &PackageReq,
) -> Result<BTreeMap<String, BinValue>, AnyError> {
  let pkg_id =
    managed_npm_resolver.resolve_pkg_id_from_deno_module_req(package_req)?;
  let package_folder =
    managed_npm_resolver.resolve_pkg_folder_from_pkg_id(&pkg_id)?;
  node_resolver
    .resolve_npm_binary_commands_for_package(&package_folder)
    .map_err(Into::into)
}

async fn autoinstall_package(
  req_ref: ReqRef<'_>,
  old_flags: &Flags,
  reload: bool,
  yes: bool,
  deno_dir: &Path,
) -> Result<(Arc<Flags>, CliFactory), AnyError> {
  fn make_new_flags(old_flags: &Flags, temp_dir: &Path) -> Arc<Flags> {
    let mut new_flags = (*old_flags).clone();
    new_flags.node_modules_dir =
      Some(deno_config::deno_json::NodeModulesDirMode::Auto);
    let temp_node_modules = temp_dir.join("node_modules");
    new_flags.internal.root_node_modules_dir_override = Some(temp_node_modules);
    new_flags.config_flag = crate::args::ConfigFlag::Path(
      temp_dir.join("deno.json").to_string_lossy().into_owned(),
    );
    new_flags.allow_scripts = PackagesAllowedScripts::All;

    log::debug!("new_flags: {:?}", new_flags);

    Arc::new(new_flags)
  }
  let temp_dir = create_package_temp_dir(
    Some(req_ref.prefix()),
    req_ref.req(),
    reload,
    deno_dir,
  )?;

  let new_flags = make_new_flags(old_flags, temp_dir.path());
  let new_factory = CliFactory::from_flags(new_flags.clone());

  match temp_dir {
    XTempDir::Existing(_) => Ok((new_flags, new_factory)),

    XTempDir::New(temp_dir) => {
      let confirmed = yes
        || match confirm(ConfirmOptions {
          default: true,
          message: format!("Install {}?", req_ref),
        }) {
          Some(true) => true,
          Some(false) => false,
          None if !DrawThread::is_supported() => {
            log::warn!(
              "Unable to prompt, installing {} without confirmation",
              req_ref.req()
            );
            true
          }
          None => false,
        };
      if !confirmed {
        return Err(anyhow::anyhow!("Installation rejected"));
      }
      match req_ref {
        ReqRef::Npm(req_ref) => {
          let pkg_json = temp_dir.join("package.json");
          std::fs::write(
            &pkg_json,
            format!(
              "{{\"dependencies\": {{\"{}\": \"{}\"}} }}",
              req_ref.req().name,
              req_ref.req().version_req
            ),
          )?;
        }
        ReqRef::Jsr(req_ref) => {
          let deno_json = temp_dir.join("deno.json");
          std::fs::write(
            &deno_json,
            format!(
              "{{ \"nodeModulesDir\": \"manual\", \"imports\": {{ \"{}\": \"{}\" }} }}",
              req_ref.req().name,
              format_args!("jsr:{}", req_ref.req())
            ),
          )?;
        }
      }

      crate::tools::pm::cache_top_level_deps(
        &new_factory,
        None,
        CacheTopLevelDepsOptions {
          lockfile_only: false,
        },
      )
      .await?;

      if let Some(lockfile) = new_factory.maybe_lockfile().await? {
        lockfile.write_if_changed()?;
      }
      Ok((new_flags, new_factory))
    }
  }
}

#[derive(Debug, Clone, Copy)]
enum ReqRef<'a> {
  Npm(&'a NpmPackageReqReference),
  Jsr(&'a JsrPackageReqReference),
}
impl<'a> std::fmt::Display for ReqRef<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ReqRef::Npm(req) => write!(f, "{}", req),
      ReqRef::Jsr(req) => write!(f, "{}", req),
    }
  }
}
impl<'a> ReqRef<'a> {
  fn req(&self) -> &PackageReq {
    match self {
      ReqRef::Npm(req) => req.req(),
      ReqRef::Jsr(req) => req.req(),
    }
  }

  fn prefix(&self) -> &str {
    match self {
      ReqRef::Npm(_) => "npm-",
      ReqRef::Jsr(_) => "jsr-",
    }
  }
}

enum ReqRefOrUrl {
  Npm(NpmPackageReqReference),
  Jsr(JsrPackageReqReference),
  Url(deno_core::url::Url),
}
