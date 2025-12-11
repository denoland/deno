// Copyright 2018-2025 the Deno authors. MIT license.

use std::num::NonZeroUsize;
use std::str::FromStr;
use std::sync::Arc;

use deno_core::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::futures::TryFutureExt;
use deno_lib::worker::LibWorkerFactoryRoots;
use deno_runtime::UnconfiguredRuntime;

use super::run::check_permission_before_script;
use super::run::maybe_npm_install;
use crate::args::Flags;
use crate::args::ServeFlags;
use crate::args::WatchFlagsWithPaths;
use crate::args::WorkspaceMainModuleResolver;
use crate::args::parallelism_count;
use crate::factory::CliFactory;
use crate::util::file_watcher::WatcherRestartMode;
use crate::worker::CliMainWorkerFactory;

pub async fn serve(
  flags: Arc<Flags>,
  serve_flags: ServeFlags,
  unconfigured_runtime: Option<UnconfiguredRuntime>,
  roots: LibWorkerFactoryRoots,
) -> Result<i32, AnyError> {
  check_permission_before_script(&flags);

  if let Some(watch_flags) = serve_flags.watch {
    return serve_with_watch(
      flags,
      watch_flags,
      parallelism_count(serve_flags.parallel),
    )
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

  let workspace_resolver = factory.workspace_resolver().await?.clone();
  let node_resolver = factory.node_resolver().await?.clone();

  let main_module = cli_options.resolve_main_module_with_resolver(Some(
    &WorkspaceMainModuleResolver::new(workspace_resolver, node_resolver),
  ))?;

  maybe_npm_install(&factory).await?;

  let worker_factory = Arc::new(
    factory
      .create_cli_main_worker_factory_with_roots(roots)
      .await?,
  );

  if serve_flags.open_site {
    let url = resolve_serve_url(serve_flags.host, serve_flags.port);
    let _ = open::that_detached(url);
  }

  let hmr = serve_flags
    .watch
    .map(|watch_flags| watch_flags.hmr)
    .unwrap_or(false);
  do_serve(
    worker_factory,
    main_module.clone(),
    parallelism_count(serve_flags.parallel),
    hmr,
    unconfigured_runtime,
  )
  .await
}

async fn do_serve(
  worker_factory: Arc<CliMainWorkerFactory>,
  main_module: ModuleSpecifier,
  parallelism_count: NonZeroUsize,
  hmr: bool,
  unconfigured_runtime: Option<UnconfiguredRuntime>,
) -> Result<i32, AnyError> {
  let worker_count = parallelism_count.get() - 1;
  let mut worker = worker_factory
    .create_main_worker_with_unconfigured_runtime(
      deno_runtime::WorkerExecutionMode::ServeMain { worker_count },
      main_module.clone(),
      // TODO(bartlomieju):
      vec![],
      vec![],
      unconfigured_runtime,
    )
    .await?;
  let worker_count = match worker_count {
    0 => return worker.run().await.map_err(Into::into),
    c => c,
  };

  let main = deno_core::unsync::spawn(async move { worker.run().await });

  let mut channels = Vec::with_capacity(worker_count);
  for i in 0..worker_count {
    let worker_factory = worker_factory.clone();
    let main_module = main_module.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    channels.push(rx);
    std::thread::Builder::new()
      .name(format!("serve-worker-{}", i + 1))
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
  worker_index: usize,
  worker_factory: Arc<CliMainWorkerFactory>,
  main_module: ModuleSpecifier,
  hmr: bool,
) -> Result<i32, AnyError> {
  let mut worker: crate::worker::CliMainWorker = worker_factory
    .create_main_worker(
      deno_runtime::WorkerExecutionMode::ServeWorker { worker_index },
      main_module,
      // TODO(bartlomieju):
      vec![],
      vec![],
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
  parallelism_count: NonZeroUsize,
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

        do_serve(
          worker_factory,
          main_module.clone(),
          parallelism_count,
          hmr,
          None,
        )
        .await?;

        Ok(())
      })
    },
  )
  .boxed_local()
  .await?;
  Ok(0)
}

fn resolve_serve_url(host: String, port: u16) -> String {
  let host = if matches!(host.as_str(), "0.0.0.0" | "::") {
    "127.0.0.1".to_string()
  } else if std::net::Ipv6Addr::from_str(&host).is_ok() {
    format!("[{}]", host)
  } else {
    host
  };
  if port == 80 {
    format!("http://{host}/")
  } else {
    format!("http://{host}:{port}/")
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_resolve_serve_url() {
    assert_eq!(
      resolve_serve_url("localhost".to_string(), 80),
      "http://localhost/"
    );
    assert_eq!(
      resolve_serve_url("0.0.0.0".to_string(), 80),
      "http://127.0.0.1/"
    );
    assert_eq!(resolve_serve_url("::".to_string(), 80), "http://127.0.0.1/");
    assert_eq!(
      resolve_serve_url("::".to_string(), 90),
      "http://127.0.0.1:90/"
    );
  }
}
