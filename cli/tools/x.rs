use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_lib::worker::LibWorkerFactoryRoots;
use deno_runtime::UnconfiguredRuntime;
use deno_runtime::deno_permissions::PathQueryDescriptor;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageReq;

use crate::args::Flags;
use crate::args::XFlags;
use crate::args::XFlagsKind;
use crate::factory::CliFactory;
use crate::node::CliNodeResolver;
use crate::npm::CliNpmResolver;
use node_resolver::BinValue;

fn resolve_local_bins(
  node_resolver: &CliNodeResolver,
  npm_resolver: &CliNpmResolver,
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
        let bins = node_resolver
          .resolve_npm_binary_commands_for_package(&package_folder)?;
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

  if npm {
    crate::tools::run::set_npm_user_agent();
  }

  crate::tools::run::maybe_npm_install(&factory).await?;

  let worker_factory = factory
    .create_cli_main_worker_factory_with_roots(roots)
    .await?;
  let mut worker = worker_factory
    .create_main_worker_with_unconfigured_runtime(
      deno_runtime::WorkerExecutionMode::Run,
      main_module.clone(),
      preload_modules,
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

  let bins = resolve_local_bins(&node_resolver, &npm_resolver)?;
  let Some(bin_value) = bins.get(command) else {
    return Ok(None);
  };
  match bin_value {
    BinValue::JsFile(path_buf) => {
      let path = deno_path_util::url_from_file_path(path_buf.as_ref())?;
      let unconfigured_runtime = unconfigured_runtime.take();
      return run_js_file(&factory, roots, unconfigured_runtime, &path, true)
        .await
        .map(Some);
    }
    BinValue::Executable(path_buf) => {
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
      return Ok(Some(status.code().unwrap_or(1)));
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

fn create_temp_node_modules_parent_dir(
  prefix: Option<&str>,
  package_req: &PackageReq,
) -> Result<XTempDir, AnyError> {
  let mut package_req_folder = String::from(prefix.unwrap_or(""));
  package_req_folder.push_str(&package_req.to_string());
  let temp_dir = std::env::temp_dir()
    .join("deno_x_nm")
    .join(package_req_folder);
  if temp_dir.exists() {
    let canonicalized_temp_dir = temp_dir
      .canonicalize()
      .ok()
      .map(deno_path_util::strip_unc_prefix);
    let temp_dir = canonicalized_temp_dir.unwrap_or_else(|| temp_dir);
    return Ok(XTempDir::Existing(temp_dir));
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
  let temp_dir = canonicalized_temp_dir.unwrap_or_else(|| temp_dir);
  Ok(XTempDir::New(temp_dir))
}

pub async fn run(
  flags: Arc<Flags>,
  x_flags: XFlags,
  mut unconfigured_runtime: Option<UnconfiguredRuntime>,
  roots: LibWorkerFactoryRoots,
) -> Result<i32, AnyError> {
  let command = match x_flags.kind {
    XFlagsKind::InstallAlias => {
      return Ok(0);
    }
    XFlagsKind::Command(command) => command,
    XFlagsKind::Print => {
      let factory = CliFactory::from_flags(flags.clone());
      let npm_resolver = factory.npm_resolver().await?;
      let node_resolver = factory.node_resolver().await?;
      let bins = resolve_local_bins(&node_resolver, &npm_resolver)?;
      println!("Available commands:");
      for command in bins.keys() {
        println!("  {}", command);
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
    &node_resolver,
    &npm_resolver,
    &command,
  )
  .await?;
  if let Some(exit_code) = result {
    return Ok(exit_code);
  }

  let cli_options = factory.cli_options()?;
  let cwd = cli_options.initial_cwd();

  let is_file_like = command.starts_with('.')
    || command.starts_with('/')
    || command.starts_with('~')
    || command.starts_with('\\')
    || Path::new(&command).extension().is_some();

  let thing_to_run = if is_file_like {
    let url = deno_path_util::resolve_url_or_path(&command, cwd)?;
    ReqRefOrUrl::Url(url)
  } else {
    match deno_core::url::Url::parse(&command) {
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
        let new_command = format!("npm:{}", command);
        let req_ref = NpmPackageReqReference::from_str(&new_command)?;
        ReqRefOrUrl::Npm(req_ref)
      }
      Err(e) => {
        return Err(e.into());
      }
    }
  };

  match thing_to_run {
    ReqRefOrUrl::Npm(npm_package_req_reference) => {
      let (new_flags, new_factory) =
        autoinstall_package(ReqRef::Npm(&npm_package_req_reference), &flags)
          .await?;
      let new_node_resolver = new_factory.node_resolver().await?;
      let new_npm_resolver = new_factory.npm_resolver().await?;

      let bin_name = npm_package_req_reference
        .sub_path()
        .unwrap_or_else(|| &npm_package_req_reference.req().name);

      let res = maybe_run_local_npm_bin(
        &new_factory,
        &new_flags,
        roots.clone(),
        &mut unconfigured_runtime,
        new_node_resolver,
        new_npm_resolver,
        bin_name,
      )
      .await?;
      if let Some(exit_code) = res {
        return Ok(exit_code);
      } else {
        let bins = resolve_local_bins(&new_node_resolver, &new_npm_resolver)?;
        return Err(anyhow::anyhow!(
          "Unable to choose binary for {}\n  Available bins:\n{}",
          command,
          bins
            .keys()
            .map(|k| format!("    {}", k))
            .collect::<Vec<_>>()
            .join("\n")
        ));
      }
    }
    ReqRefOrUrl::Jsr(jsr_package_req_reference) => {
      let (_new_flags, new_factory) =
        autoinstall_package(ReqRef::Jsr(&jsr_package_req_reference), &flags)
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

async fn autoinstall_package(
  req_ref: ReqRef<'_>,
  old_flags: &Flags,
) -> Result<(Arc<Flags>, CliFactory), AnyError> {
  fn make_new_flags(old_flags: &Flags, temp_dir: &PathBuf) -> Arc<Flags> {
    let mut new_flags = (*old_flags).clone();
    new_flags.node_modules_dir =
      Some(deno_config::deno_json::NodeModulesDirMode::Manual);
    let temp_node_modules = temp_dir.join("node_modules");
    new_flags.internal.root_node_modules_dir_override = Some(temp_node_modules);
    new_flags.config_flag = crate::args::ConfigFlag::Path(
      temp_dir.join("deno.json").to_string_lossy().into_owned(),
    );

    let new_flags = Arc::new(new_flags);
    new_flags
  }
  let temp_dir =
    create_temp_node_modules_parent_dir(Some(req_ref.prefix()), req_ref.req())?;

  let new_flags = make_new_flags(old_flags, &temp_dir.path());
  let new_factory = CliFactory::from_flags(new_flags.clone());

  match temp_dir {
    XTempDir::Existing(_) => Ok((new_flags, new_factory)),

    XTempDir::New(temp_dir) => {
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
              "{{ \"nodeModulesDir\": \"auto\", \"imports\": {{ \"{}\": \"{}\" }} }}",
              req_ref.req().name,
              format_args!("jsr:{}", req_ref.req())
            ),
          )?;
        }
      }

      crate::tools::pm::cache_top_level_deps(&new_factory, None).await?;
      Ok((new_flags, new_factory))
    }
  }
}

#[derive(Debug, Clone, Copy)]
enum ReqRef<'a> {
  Npm(&'a NpmPackageReqReference),
  Jsr(&'a JsrPackageReqReference),
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
      ReqRef::Npm(_) => "npm",
      ReqRef::Jsr(_) => "jsr",
    }
  }
}

enum ReqRefOrUrl {
  Npm(NpmPackageReqReference),
  Jsr(JsrPackageReqReference),
  Url(deno_core::url::Url),
}
