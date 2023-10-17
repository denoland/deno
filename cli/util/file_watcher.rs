// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::Flags;
use crate::colors;
use crate::util::fs::canonicalize_path;

use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::Future;
use deno_runtime::fmt_errors::format_js_error;
use log::info;
use notify::event::Event as NotifyEvent;
use notify::event::EventKind;
use notify::Error as NotifyError;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

const CLEAR_SCREEN: &str = "\x1B[2J\x1B[1;1H";
const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(200);

struct DebouncedReceiver {
  // The `recv()` call could be used in a tokio `select!` macro,
  // and so we store this state on the struct to ensure we don't
  // lose items if a `recv()` never completes
  received_items: HashSet<PathBuf>,
  receiver: UnboundedReceiver<Vec<PathBuf>>,
}

impl DebouncedReceiver {
  fn new_with_sender() -> (Arc<mpsc::UnboundedSender<Vec<PathBuf>>>, Self) {
    let (sender, receiver) = mpsc::unbounded_channel();
    (
      Arc::new(sender),
      Self {
        receiver,
        received_items: HashSet::new(),
      },
    )
  }

  async fn recv(&mut self) -> Option<Vec<PathBuf>> {
    if self.received_items.is_empty() {
      self
        .received_items
        .extend(self.receiver.recv().await?.into_iter());
    }

    loop {
      select! {
        items = self.receiver.recv() => {
          self.received_items.extend(items?);
        }
        _ = sleep(DEBOUNCE_INTERVAL) => {
          return Some(self.received_items.drain().collect());
        }
      }
    }
  }
}

async fn error_handler<F>(watch_future: F) -> bool
where
  F: Future<Output = Result<(), AnyError>>,
{
  let result = watch_future.await;
  if let Err(err) = result {
    let error_string = match err.downcast_ref::<JsError>() {
      Some(e) => format_js_error(e),
      None => format!("{err:?}"),
    };
    eprintln!(
      "{}: {}",
      colors::red_bold("error"),
      error_string.trim_start_matches("error: ")
    );
    false
  } else {
    true
  }
}

pub struct PrintConfig {
  /// printing watcher status to terminal.
  pub job_name: String,
  /// determine whether to clear the terminal screen; applicable to TTY environments only.
  pub clear_screen: bool,
}

fn create_print_after_restart_fn(clear_screen: bool) -> impl Fn() {
  move || {
    if clear_screen && std::io::stderr().is_terminal() {
      eprint!("{CLEAR_SCREEN}");
    }
    info!(
      "{} File change detected! Restarting!",
      colors::intense_blue("Watcher"),
    );
  }
}

// TODO(bartlomieju): this is a poor name; change it
pub struct WatcherInterface {
  pub paths_to_watch_sender: tokio::sync::mpsc::UnboundedSender<Vec<PathBuf>>,
  pub changed_paths_receiver: tokio::sync::broadcast::Receiver<Vec<PathBuf>>,
  pub restart_sender: tokio::sync::mpsc::UnboundedSender<()>,
}

impl Clone for WatcherInterface {
  fn clone(&self) -> Self {
    Self {
      paths_to_watch_sender: self.paths_to_watch_sender.clone(),
      changed_paths_receiver: self.changed_paths_receiver.resubscribe(),
      restart_sender: self.restart_sender.clone(),
    }
  }
}

/// Creates a file watcher.
///
/// - `operation` is the actual operation we want to run every time the watcher detects file
/// changes. For example, in the case where we would like to bundle, then `operation` would
/// have the logic for it like bundling the code.
pub async fn watch_func<O, F>(
  mut flags: Flags,
  print_config: PrintConfig,
  mut operation: O,
) -> Result<(), AnyError>
where
  O:
    FnMut(Flags, WatcherInterface, Option<Vec<PathBuf>>) -> Result<F, AnyError>,
  F: Future<Output = Result<(), AnyError>>,
{
  let (paths_to_watch_sender, mut paths_to_watch_receiver) =
    tokio::sync::mpsc::unbounded_channel();
  let (watcher_restart_sender, mut watcher_restart_receiver) =
    tokio::sync::mpsc::unbounded_channel();
  // TODO(bartlomieju): currently unused, unify with `watch_recv`.
  let (_changed_paths_sender, changed_paths_receiver) =
    tokio::sync::broadcast::channel(4);
  let (watcher_sender, mut watcher_receiver) =
    DebouncedReceiver::new_with_sender();

  let PrintConfig {
    job_name,
    clear_screen,
  } = print_config;

  let print_after_restart = create_print_after_restart_fn(clear_screen);
  let watcher_interface = WatcherInterface {
    paths_to_watch_sender: paths_to_watch_sender.clone(),
    changed_paths_receiver: changed_paths_receiver.resubscribe(),
    restart_sender: watcher_restart_sender.clone(),
  };
  info!("{} {} started.", colors::intense_blue("Watcher"), job_name,);

  let mut changed_paths = None;
  loop {
    // We may need to give the runtime a tick to settle, as cancellations may need to propagate
    // to tasks. We choose yielding 10 times to the runtime as a decent heuristic. If watch tests
    // start to fail, this may need to be increased.
    for _ in 0..10 {
      tokio::task::yield_now().await;
    }

    let mut watcher = new_watcher(watcher_sender.clone())?;
    consume_paths_to_watch(&mut watcher, &mut paths_to_watch_receiver);

    let receiver_future = async {
      loop {
        let maybe_paths = paths_to_watch_receiver.recv().await;
        add_paths_to_watcher(&mut watcher, &maybe_paths.unwrap());
      }
    };
    let operation_future = error_handler(operation(
      flags.clone(),
      watcher_interface.clone(),
      changed_paths.take(),
    )?);

    // don't reload dependencies after the first run
    flags.reload = false;

    select! {
      _ = receiver_future => {},
      _ = watcher_restart_receiver.recv() => {
        print_after_restart();
        continue;
      },
      received_changed_paths = watcher_receiver.recv() => {
        print_after_restart();
        changed_paths = received_changed_paths;
        continue;
      },
      success = operation_future => {
        consume_paths_to_watch(&mut watcher, &mut paths_to_watch_receiver);
        // TODO(bartlomieju): print exit code here?
        info!(
          "{} {} {}. Restarting on file change...",
          colors::intense_blue("Watcher"),
          job_name,
          if success {
            "finished"
          } else {
            "failed"
          }
        );
      },
    };

    let receiver_future = async {
      loop {
        let maybe_paths = paths_to_watch_receiver.recv().await;
        add_paths_to_watcher(&mut watcher, &maybe_paths.unwrap());
      }
    };
    select! {
      _ = receiver_future => {},
      received_changed_paths = watcher_receiver.recv() => {
        print_after_restart();
        changed_paths = received_changed_paths;
        continue;
      },
    };
  }
}

/// Creates a file watcher.
///
/// - `operation` is the actual operation we want to run and notify every time
/// the watcher detects file changes.
pub async fn watch_recv<O, F>(
  flags: Flags,
  print_config: PrintConfig,
  mut operation: O,
) -> Result<i32, AnyError>
where
  O: FnMut(Flags, WatcherInterface) -> Result<F, AnyError>,
  F: Future<Output = Result<(), AnyError>>,
{
  let (paths_to_watch_sender, mut paths_to_watch_receiver) =
    tokio::sync::mpsc::unbounded_channel();
  let (watcher_restart_sender, mut watcher_restart_receiver) =
    tokio::sync::mpsc::unbounded_channel();
  let (changed_paths_sender, changed_paths_receiver) =
    tokio::sync::broadcast::channel(4);
  let (watcher_sender, mut watcher_receiver) =
    DebouncedReceiver::new_with_sender();

  let PrintConfig { job_name, .. } = print_config;
  let watcher_interface = WatcherInterface {
    paths_to_watch_sender: paths_to_watch_sender.clone(),
    changed_paths_receiver: changed_paths_receiver.resubscribe(),
    restart_sender: watcher_restart_sender.clone(),
  };
  info!("{} {} started.", colors::intense_blue("HMR"), job_name,);

  loop {
    // We may need to give the runtime a tick to settle, as cancellations may need to propagate
    // to tasks. We choose yielding 10 times to the runtime as a decent heuristic. If watch tests
    // start to fail, this may need to be increased.
    for _ in 0..10 {
      tokio::task::yield_now().await;
    }

    let mut watcher = new_watcher(watcher_sender.clone())?;
    consume_paths_to_watch(&mut watcher, &mut paths_to_watch_receiver);

    let receiver_future = async {
      loop {
        let maybe_paths = paths_to_watch_receiver.recv().await;
        add_paths_to_watcher(&mut watcher, &maybe_paths.unwrap());
      }
    };
    let operation_future =
      error_handler(operation(flags.clone(), watcher_interface.clone())?);

    // TODO(bartlomieju): this is almost identical with `watch_func`, besides
    // `received_changed_paths` arm - figure out how to remove it.
    select! {
      _ = receiver_future => {},
      received_changed_paths = watcher_receiver.recv() => {
        // eprintln!("changed paths for watcher {:#?}", received_changed_paths);
        if let Some(changed_paths) = received_changed_paths {
          changed_paths_sender.send(changed_paths)?;
        }
      },
      _ = watcher_restart_receiver.recv() => {
        continue;
      },
      success = operation_future => {
        consume_paths_to_watch(&mut watcher, &mut paths_to_watch_receiver);
        // TODO(bartlomieju): print exit code here?
        info!(
          "{} {} {}. Restarting on file change...",
          colors::intense_blue("Watcher"),
          job_name,
          if success {
            "finished"
          } else {
            "failed"
          }
        );
      },
    };

    let receiver_future = async {
      loop {
        let maybe_paths = paths_to_watch_receiver.recv().await;
        add_paths_to_watcher(&mut watcher, &maybe_paths.unwrap());
      }
    };
    select! {
      _ = receiver_future => {},
      _received_changed_paths = watcher_receiver.recv() => {
        // print_after_restart();
        // changed_paths = received_changed_paths;
        continue;
      },
    };
  }
}

fn new_watcher(
  sender: Arc<mpsc::UnboundedSender<Vec<PathBuf>>>,
) -> Result<RecommendedWatcher, AnyError> {
  Ok(Watcher::new(
    move |res: Result<NotifyEvent, NotifyError>| {
      let Ok(event) = res else {
        return;
      };

      if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
      ) {
        return;
      }

      let paths = event
        .paths
        .iter()
        .filter_map(|path| canonicalize_path(path).ok())
        .collect();
      sender.send(paths).unwrap();
    },
    Default::default(),
  )?)
}

fn add_paths_to_watcher(watcher: &mut RecommendedWatcher, paths: &[PathBuf]) {
  // Ignore any error e.g. `PathNotFound`
  for path in paths {
    let _ = watcher.watch(path, RecursiveMode::Recursive);
  }
  log::debug!("Watching paths: {:?}", paths);
}

fn consume_paths_to_watch(
  watcher: &mut RecommendedWatcher,
  receiver: &mut UnboundedReceiver<Vec<PathBuf>>,
) {
  loop {
    match receiver.try_recv() {
      Ok(paths) => {
        add_paths_to_watcher(watcher, &paths);
      }
      Err(e) => match e {
        mpsc::error::TryRecvError::Empty => {
          break;
        }
        // there must be at least one receiver alive
        _ => unreachable!(),
      },
    }
  }
}
