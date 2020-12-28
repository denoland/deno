// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

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
use tokio::time::{interval, Interval};

const DEBOUNCE_INTERVAL_MS: Duration = Duration::from_millis(200);

// TODO(bartlomieju): rename
type WatchFuture = Pin<Box<dyn Future<Output = Result<(), AnyError>>>>;

struct Debounce {
  interval: Interval,
  event_detected: Arc<AtomicBool>,
}

impl Debounce {
  fn new() -> Self {
    Self {
      interval: interval(DEBOUNCE_INTERVAL_MS),
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
      let _ = inner.interval.poll_tick(cx);
      Poll::Pending
    }
  }
}

async fn error_handler(watch_future: WatchFuture) {
  let result = watch_future.await;
  if let Err(err) = result {
    let msg = format!("{}: {}", colors::red_bold("error"), err.to_string(),);
    eprintln!("{}", msg);
  }
}

pub async fn watch_func<F>(
  paths: &[PathBuf],
  closure: F,
) -> Result<(), AnyError>
where
  F: Fn() -> WatchFuture,
{
  let mut debounce = Debounce::new();
  // This binding is required for the watcher to work properly without being dropped.
  let _watcher = new_watcher(paths, &debounce)?;

  loop {
    let func = error_handler(closure());
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
    }
    if !is_file_changed {
      info!(
        "{} Process terminated! Restarting on file change...",
        colors::intense_blue("Watcher"),
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
