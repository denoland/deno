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
use std::mem;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{mpsc, mpsc::Receiver};
use std::time::{Duration, Instant};
use tokio::select;

const DEBOUNCE_TIMEOUT_MS: Duration = Duration::from_millis(200);

// TODO(bartlomieju): rename
type WatchFuture = Pin<Box<dyn Future<Output = Result<(), AnyError>>>>;

struct Debounce {
  rx: Receiver<Result<NotifyEvent, AnyError>>,
  debounce_time: Duration,
  start_time: Option<Instant>,
  last_event: Option<NotifyEvent>,
}

impl Debounce {
  fn new(
    rx: Receiver<Result<NotifyEvent, AnyError>>,
    debounce_time: Duration,
  ) -> Self {
    Self {
      rx,
      debounce_time,
      start_time: None,
      last_event: None,
    }
  }
}

impl Stream for Debounce {
  type Item = NotifyEvent;

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Option<Self::Item>> {
    let inner = self.get_mut();
    if let Ok(Ok(event)) = inner.rx.try_recv() {
      inner.start_time = Some(Instant::now());
      inner.last_event = Some(event);
    }

    match &inner.last_event {
      Some(_)
        if inner.start_time.map_or(false, |start_time| {
          start_time.elapsed() >= inner.debounce_time
        }) =>
      {
        inner.start_time = None;
        let event = mem::take(&mut inner.last_event);
        Poll::Ready(event)
      }
      _ => {
        cx.waker().wake_by_ref();
        Poll::Pending
      }
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
  let (_watcher, receiver) = new_watcher(paths)?;
  let mut debounce = Debounce::new(receiver, DEBOUNCE_TIMEOUT_MS);
  loop {
    let func = error_handler(closure());
    let mut is_file_changed = false;
    select! {
      _ = wait_for_file_change(&mut debounce) => {
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
      wait_for_file_change(&mut debounce).await?;
      info!(
        "{} File change detected! Restarting!",
        colors::intense_blue("Watcher"),
      );
    }
  }
}

async fn wait_for_file_change(debounce: &mut Debounce) -> Result<(), AnyError> {
  while let Some(event) = debounce.next().await {
    match event.kind {
      EventKind::Create(_) => break,
      EventKind::Modify(_) => break,
      EventKind::Remove(_) => break,
      _ => continue,
    }
  }
  Ok(())
}

fn new_watcher(
  paths: &[PathBuf],
) -> Result<
  (RecommendedWatcher, Receiver<Result<NotifyEvent, AnyError>>),
  AnyError,
> {
  let (sender, receiver) = mpsc::channel::<Result<NotifyEvent, AnyError>>();

  let mut watcher: RecommendedWatcher =
    Watcher::new_immediate(move |res: Result<NotifyEvent, NotifyError>| {
      let res2 = res.map_err(AnyError::from);
      // Ignore result, if send failed it means that watcher was already closed,
      // but not all messages have been flushed.
      let _ = sender.send(res2);
    })?;

  watcher.configure(Config::PreciseEvents(true)).unwrap();

  for path in paths {
    watcher.watch(path, RecursiveMode::NonRecursive)?;
  }

  Ok((watcher, receiver))
}
