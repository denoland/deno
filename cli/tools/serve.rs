use std::sync::Arc;

use deno_core::error::AnyError;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;

use super::run::check_permission_before_script;
use super::run::maybe_npm_install;
use crate::args::Flags;
use crate::args::WatchFlagsWithPaths;
use crate::factory::CliFactory;

pub async fn serve(
  flags: Arc<Flags>,
  watch: Option<WatchFlagsWithPaths>,
) -> Result<i32, AnyError> {
  check_permission_before_script(&flags);

  if let Some(_watch_flags) = watch {
    todo!();
  }

  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let deno_dir = factory.deno_dir()?;
  let http_client = factory.http_client_provider();

  // Run a background task that checks for available upgrades or output
  // if an earlier run of this background task found a new version of Deno.
  #[cfg(feature = "upgrade")]
  super::upgrade::check_for_upgrades(
    http_client.clone(),
    deno_dir.upgrade_check_file_path(),
  );

  let main_module = cli_options.resolve_main_module()?;

  maybe_npm_install(&factory).await?;

  let permissions = PermissionsContainer::new(Permissions::from_options(
    &cli_options.permissions_options()?,
  )?);
  let worker_factory = factory.create_cli_main_worker_factory().await?;
  let mut worker = worker_factory
    .create_main_worker(
      deno_runtime::WorkerExecutionMode::Serve,
      main_module.clone(),
      permissions.clone(),
    )
    .await?;

  let main = deno_core::unsync::spawn(async move { worker.run().await });
  let n = std::thread::available_parallelism()?.get();
  for _ in 0..n {
    let worker_factory = worker_factory.clone();
    let main_module = main_module.clone();
    let permissions = permissions.clone();

    std::thread::spawn(move || {
      deno_runtime::tokio_util::create_and_run_current_thread(async move {
        let mut worker = worker_factory
          .create_main_worker(
            deno_runtime::WorkerExecutionMode::Serve,
            main_module,
            permissions,
          )
          .await?;
        worker.run().await?;
        Ok::<_, AnyError>(())
      })
      .unwrap();
    });
  }
  Ok(main.await??)
}
