// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::futures::TryFutureExt;
use deno_core::ModuleSpecifier;

use super::run::check_permission_before_script;
use super::run::maybe_npm_install;
use crate::args::Flags;
use crate::args::ServeFlags;
use crate::args::WatchFlagsWithPaths;
use crate::factory::CliFactory;
use crate::util::file_watcher::WatcherRestartMode;
use crate::worker::CliMainWorkerFactory;

pub async fn serve(
  flags: Arc<Flags>,
  serve_flags: ServeFlags,
) -> Result<i32, AnyError> {
  check_permission_before_script(&flags);

  if let Some(watch_flags) = serve_flags.watch {
    return serve_with_watch(flags, watch_flags, serve_flags.worker_count)
      .await;
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

  let worker_factory =
    Arc::new(factory.create_cli_main_worker_factory().await?);
  let hmr = serve_flags
    .watch
    .map(|watch_flags| watch_flags.hmr)
    .unwrap_or(false);
  do_serve(
    worker_factory,
    main_module.clone(),
    serve_flags.worker_count,
    hmr,
  )
  .await
}

async fn do_serve(
  worker_factory: Arc<CliMainWorkerFactory>,
  main_module: ModuleSpecifier,
  worker_count: Option<usize>,
  hmr: bool,
) -> Result<i32, AnyError> {
  let mut worker = worker_factory
    .create_main_worker(
      deno_runtime::WorkerExecutionMode::Serve {
        is_main: true,
        worker_count,
      },
      main_module.clone(),
    )
    .await?;
  let worker_count = match worker_count {
    None | Some(1) => return worker.run().await.map_err(Into::into),
    Some(c) => c,
  };

  let main = deno_core::unsync::spawn(async move { worker.run().await });

  let extra_workers = worker_count.saturating_sub(1);

  let mut channels = Vec::with_capacity(extra_workers);
  for i in 0..extra_workers {
    let worker_factory = worker_factory.clone();
    let main_module = main_module.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    channels.push(rx);
    std::thread::Builder::new()
      .name(format!("serve-worker-{i}"))
      .spawn(move || {
        deno_runtime::tokio_util::create_and_run_current_thread(async move {
          let result = run_worker(i, worker_factory, main_module, hmr).await;
          let _ = tx.send(result);
        });
      })?;
  }

  let (main_result, worker_results) = tokio::try_join!(
    main.map_err(AnyError::from),
    deno_core::futures::future::try_join_all(
      channels.into_iter().map(|r| r.map_err(AnyError::from))
    )
  )?;

  let mut exit_code = main_result?;
  for res in worker_results {
    let ret = res?;
    if ret != 0 && exit_code == 0 {
      exit_code = ret;
    }
  }
  Ok(exit_code)
}

async fn run_worker(
  worker_count: usize,
  worker_factory: Arc<CliMainWorkerFactory>,
  main_module: ModuleSpecifier,
  hmr: bool,
) -> Result<i32, AnyError> {
  let mut worker: crate::worker::CliMainWorker = worker_factory
    .create_main_worker(
      deno_runtime::WorkerExecutionMode::Serve {
        is_main: false,
        worker_count: Some(worker_count),
      },
      main_module,
    )
    .await?;
  if hmr {
    worker.run_for_watcher().await?;
    Ok(0)
  } else {
    worker.run().await.map_err(Into::into)
  }
}

async fn serve_with_watch(
  flags: Arc<Flags>,
  watch_flags: WatchFlagsWithPaths,
  worker_count: Option<usize>,
) -> Result<i32, AnyError> {
  let hmr = watch_flags.hmr;
  crate::util::file_watcher::watch_recv(
    flags,
    crate::util::file_watcher::PrintConfig::new_with_banner(
      if watch_flags.hmr { "HMR" } else { "Watcher" },
      "Process",
      !watch_flags.no_clear_screen,
    ),
    WatcherRestartMode::Automatic,
    move |flags, watcher_communicator, changed_paths| {
      watcher_communicator.show_path_changed(changed_paths.clone());
      Ok(async move {
        let factory = CliFactory::from_flags_for_watcher(
          flags,
          watcher_communicator.clone(),
        );
        let cli_options = factory.cli_options()?;
        let main_module = cli_options.resolve_main_module()?;

        maybe_npm_install(&factory).await?;

        let _ = watcher_communicator.watch_paths(cli_options.watch_paths());
        let worker_factory =
          Arc::new(factory.create_cli_main_worker_factory().await?);

        do_serve(worker_factory, main_module.clone(), worker_count, hmr)
          .await?;

        Ok(())
      })
    },
  )
  .boxed_local()
  .await?;
  Ok(0)
}
