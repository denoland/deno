// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use deno_core::error::AnyError;
use deno_core::futures::ready;
use deno_core::futures::stream::{Stream, StreamExt};
use deno_core::futures::Future;
use notify::event::Event as NotifyEvent;
use notify::event::EventKind;
use notify::Config;
use notify::Error as NotifyError;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use pin_project::pin_project;
use std::collections::HashSet;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;
use tokio::pin;
use tokio::select;
use tokio::time::sleep;
use tokio::time::Instant;
use tokio::time::Sleep;

const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(200);

type FileWatcherFuture<T> = Pin<Box<dyn Future<Output = T>>>;

#[pin_project(project = DebounceProjection)]
struct Debounce {
  #[pin]
  timer: Sleep,
  changed_paths: Arc<Mutex<HashSet<PathBuf>>>,
}

impl Debounce {
  fn new() -> Self {
    Self {
      timer: sleep(DEBOUNCE_INTERVAL),
      changed_paths: Arc::new(Mutex::new(HashSet::new())),
    }
  }
}

impl Stream for Debounce {
  type Item = Vec<PathBuf>;

  /// Note that this never returns `Poll::Ready(None)`, which means that the
  /// file watcher will be alive until the Deno process is terminated.
  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Option<Self::Item>> {
    let mut changed_paths = self.changed_paths.lock().unwrap();
    if changed_paths.len() > 0 {
      Poll::Ready(Some(changed_paths.drain().collect()))
    } else {
      drop(changed_paths);
      let mut timer = self.project().timer;
      if let Poll::Ready(_) = timer.as_mut().poll(cx) {
        timer.reset(Instant::now() + DEBOUNCE_INTERVAL);
      }
      Poll::Pending
    }
  }
}

async fn error_handler(watch_future: FileWatcherFuture<Result<(), AnyError>>) {
  let result = watch_future.await;
  if let Err(err) = result {
    let msg = format!("{}: {}", colors::red_bold("error"), err.to_string(),);
    eprintln!("{}", msg);
  }
}

pub enum ResolutionResult<T> {
  Restart {
    paths_to_watch: Vec<PathBuf>,
    result: Result<T, AnyError>,
  },
  Ignore,
}

async fn next_restart<F, T: Clone>(
  resolver: &mut F,
  debounce: &mut Pin<&mut Debounce>,
  initial: bool,
) -> (Vec<PathBuf>, Result<T, AnyError>)
where
  F: FnMut(Option<Vec<PathBuf>>) -> FileWatcherFuture<ResolutionResult<T>>,
{
  let mut changed = if initial { None } else { debounce.next().await };
  loop {
    let initial = changed.is_none();
    match resolver(changed).await {
      ResolutionResult::Ignore => {
        debug!("File change ignored")
      }
      ResolutionResult::Restart {
        paths_to_watch,
        result,
      } => {
        if !initial {
          info!(
            "{} File change detected! Restarting!",
            colors::intense_blue("Watcher"),
          );
        }
        break (paths_to_watch, result);
      }
    }
    changed = debounce.next().await;
  }
}

/// Creates a file watcher, which will call `resolver` with every file change.
///
/// - `resolver` is used for resolving file paths to be watched at every restarting
/// of the watcher, and can also return a value to be passed to `operation`.
/// It returns a [`ResolutionResult`], which can either instruct the watcher to restart or ignore the change.
/// This always contains paths to watch;
///
/// - `operation` is the actual operation we want to run every time the watcher detects file
/// changes. For example, in the case where we would like to bundle, then `operation` would
/// have the logic for it like bundling the code.
///
/// - `job_name` is just used for printing watcher status to terminal.
pub async fn watch_func<F, G, T>(
  mut resolver: F,
  mut operation: G,
  job_name: &str,
) -> Result<(), AnyError>
where
  F: FnMut(Option<Vec<PathBuf>>) -> FileWatcherFuture<ResolutionResult<T>>,
  G: FnMut(T) -> FileWatcherFuture<Result<(), AnyError>>,
  T: Clone,
{
  let debounce = Debounce::new();
  pin!(debounce);

  // Store previous data. If module resolution fails at some point, the watcher will try to
  // continue watching files using these data.
  let mut paths_to_watch;
  let mut resolution_result;

  let (paths, result) = next_restart(&mut resolver, &mut debounce, true).await;
  paths_to_watch = paths;
  resolution_result = Some(result);

  loop {
    let watcher = new_watcher(&paths_to_watch, &debounce)?;

    if let Some(result) = resolution_result.take() {
      match result {
        Ok(operation_arg) => {
          let fut = error_handler(operation(operation_arg));
          select! {
            (paths, result) = next_restart(&mut resolver, &mut debounce, false) => {
              paths_to_watch = paths;
              resolution_result = Some(result);
              continue;
            },
            _ = fut => {},
          };

          info!(
            "{} {} finished! Restarting on file change...",
            colors::intense_blue("Watcher"),
            job_name,
          );
        }
        Err(error) => {
          eprintln!("{}: {}", colors::red_bold("error"), error);
          info!(
            "{} {} failed! Restarting on file change...",
            colors::intense_blue("Watcher"),
            job_name,
          );
        }
      }
    }

    let (paths, result) =
      next_restart(&mut resolver, &mut debounce, false).await;
    paths_to_watch = paths;
    resolution_result = Some(result);

    drop(watcher);
  }
}

fn new_watcher(
  paths: &[PathBuf],
  debounce: &Debounce,
) -> Result<RecommendedWatcher, AnyError> {
  let changed_paths = Arc::clone(&debounce.changed_paths);

  let mut watcher: RecommendedWatcher =
    Watcher::new_immediate(move |res: Result<NotifyEvent, NotifyError>| {
      if let Ok(event) = res {
        if matches!(
          event.kind,
          EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
        ) {
          let mut changed_paths = changed_paths.lock().unwrap();
          changed_paths.extend(event.paths);
        }
      }
    })?;

  watcher.configure(Config::PreciseEvents(true)).unwrap();

  for path in paths {
    // Ignore any error e.g. `PathNotFound`
    let _ = watcher.watch(path, RecursiveMode::Recursive);
  }

  Ok(watcher)
}
