use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_lib::worker::LibWorkerFactoryRoots;
use deno_runtime::UnconfiguredRuntime;
use deno_runtime::deno_permissions::PathQueryDescriptor;

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
  path: &Path,
  npm: bool,
) -> Result<i32, AnyError> {
  let cli_options = factory.cli_options()?;
  let preload_modules = cli_options.preload_modules()?;
  let main_module = deno_path_util::url_from_file_path(path)?;

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
  unconfigured_runtime: Option<UnconfiguredRuntime>,
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
      return run_js_file(
        &factory,
        roots,
        unconfigured_runtime,
        &path_buf,
        true,
      )
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

pub async fn run(
  flags: Arc<Flags>,
  x_flags: XFlags,
  unconfigured_runtime: Option<UnconfiguredRuntime>,
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
    roots,
    unconfigured_runtime,
    &node_resolver,
    &npm_resolver,
    &command,
  )
  .await?;
  if let Some(exit_code) = result {
    return Ok(exit_code);
  }

  // if command.starts_with(pat)

  Ok(0)
}
