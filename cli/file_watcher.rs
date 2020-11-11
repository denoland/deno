// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use core::task::{Context, Poll};
use deno_core::error::AnyError;
use deno_core::futures::stream::{Stream, StreamExt};
use deno_core::futures::Future;
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
      match Pin::new(&mut inner.delay).poll(cx) {
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
