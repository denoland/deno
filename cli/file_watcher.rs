// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use core::task::{Context, Poll};
use deno_core::error::AnyError;
use deno_core::futures::stream::{Stream, StreamExt};
use deno_core::futures::{Future, FutureExt};
use notify::event::Event as NotifyEvent;
use notify::event::EventKind;
use notify::Config;
use notify::Error as NotifyError;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::time::{delay_for, Delay};

const DEBOUNCE_INTERVAL_MS: Duration = Duration::from_millis(200);

// TODO(bartlomieju): rename
type WatchFuture<T> = Pin<Box<dyn Future<Output = Result<T, AnyError>>>>;

struct Debounce {
  delay: Delay,
  event_detected: Arc<AtomicBool>,
}

impl Debounce {
  fn new() -> Self {
    Self {
      delay: delay_for(DEBOUNCE_INTERVAL_MS),
      event_detected: Arc::new(AtomicBool::new(false)),
    }
  }
}

impl Stream for Debounce {
  type Item = ();

  /// Note that this never returns `Poll::Ready(None)`, which means that file watcher will be alive
  /// until the Deno process is terminated.
  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Option<Self::Item>> {
    let inner = self.get_mut();
    if inner.event_detected.load(Ordering::Relaxed) {
      inner.event_detected.store(false, Ordering::Relaxed);
      Poll::Ready(Some(()))
    } else {
      match inner.delay.poll_unpin(cx) {
        Poll::Ready(_) => {
          inner.delay = delay_for(DEBOUNCE_INTERVAL_MS);
          Poll::Pending
        }
        Poll::Pending => Poll::Pending,
      }
    }
  }
}

async fn error_handler(watch_future: WatchFuture<()>) {
  let result = watch_future.await;
  if let Err(err) = result {
    let msg = format!("{}: {}", colors::red_bold("error"), err.to_string(),);
    eprintln!("{}", msg);
  }
}

/// This function adds watcher functionality to subcommands like `fmt` or `lint`.
/// The difference from [`watch_func_with_module_resolution`] is that this doesn't depend on
/// [`ModuleGraph`] stuff.
///
/// - `target_resolver` is used for resolving file paths to be watched at every restarting of the watcher. The
/// return value of this closure will then be passed to `operation` as an argument.
///
/// - `operation` is the actual operation we want to run every time the watcher detects file
/// changes. For example, in the case where we would like to apply `fmt`, then `operation` would
/// have the logic for it like calling `format_source_files`.
///
/// - `job_name` is just used for printing watcher status to terminal.
///
/// [`ModuleGraph`]: crate::module_graph::Graph
pub async fn watch_func<F, G>(
  target_resolver: F,
  operation: G,
  job_name: &str,
) -> Result<(), AnyError>
where
  F: Fn() -> Result<Vec<PathBuf>, AnyError>,
  G: Fn(Vec<PathBuf>) -> WatchFuture<()>,
{
  let mut debounce = Debounce::new();

  loop {
    let paths = target_resolver()?;
    let _watcher = new_watcher(&paths, &debounce)?;
    let func = error_handler(operation(paths));
    let mut is_file_changed = false;
    select! {
      _ = debounce.next() => {
        is_file_changed = true;
        info!(
          "{} File change detected! Restarting!",
          colors::intense_blue("Watcher"),
        );
      },
      _ = func => {},
    };

    if !is_file_changed {
      info!(
        "{} {} finished! Restarting on file change...",
        colors::intense_blue("Watcher"),
        job_name,
      );
      debounce.next().await;
      info!(
        "{} File change detected! Restarting!",
        colors::intense_blue("Watcher"),
      );
    }
  }
}

/// This function adds watcher functionality to subcommands like `run` or `bundle`.
/// The difference from [`watch_func`] is that this does depend on [`ModuleGraph`] stuff.
///
/// - `module_resolver` is used for both resolving file paths to be watched at every restarting of the watcher and buidling [`ModuleGraph`] or [`ModuleSpecifier`] which will then be passed to `operation`.
///
/// - `operation` is the actual operation we want to run every time the watcher detects file
/// changes. For example, in the case where we would like to bundle, then `operation` would
/// have the logic for it like doing bundle with the help of [`ModuleGraph`].
///
/// - `job_name` is just used for printing watcher status to terminal.
///
/// [`ModuleGraph`]: crate::module_graph::Graph
/// [`ModuleSpecifier`]: deno_core::ModuleSpecifier
pub async fn watch_func_with_module_resolution<F, G, T>(
  module_resolver: F,
  operation: G,
  job_name: &str,
) -> Result<(), AnyError>
where
  F: Fn() -> WatchFuture<(Vec<PathBuf>, T)>,
  G: Fn(T) -> WatchFuture<()>,
{
  let mut debounce = Debounce::new();

  loop {
    let (paths, module) = module_resolver().await?;
    let _watcher = new_watcher(&paths, &debounce)?;
    let func = error_handler(operation(module));
    let mut is_file_changed = false;
    select! {
      _ = debounce.next() => {
        is_file_changed = true;
        info!(
          "{} File change detected! Restarting!",
          colors::intense_blue("Watcher"),
        );
      },
      _ = func => {},
    };

    if !is_file_changed {
      info!(
        "{} {} finished! Restarting on file change...",
        colors::intense_blue("Watcher"),
        job_name,
      );
      debounce.next().await;
      info!(
        "{} File change detected! Restarting!",
        colors::intense_blue("Watcher"),
      );
    }
  }
}

fn new_watcher(
  paths: &[PathBuf],
  debounce: &Debounce,
) -> Result<RecommendedWatcher, AnyError> {
  let event_detected = Arc::clone(&debounce.event_detected);

  let mut watcher: RecommendedWatcher = Watcher::new_immediate(
    move |res: Result<NotifyEvent, NotifyError>| {
      if let Ok(event) = res {
        if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_))
        {
          event_detected.store(true, Ordering::Relaxed);
        }
      }
    },
  )?;

  watcher.configure(Config::PreciseEvents(true)).unwrap();

  for path in paths {
    watcher.watch(path, RecursiveMode::NonRecursive)?;
  }

  Ok(watcher)
}
