// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::fmt_errors::format_js_error;
use crate::fs_util::canonicalize_path;

use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::Future;
use log::info;
use notify::event::Event as NotifyEvent;
use notify::event::EventKind;
use notify::Config;
use notify::Error as NotifyError;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use std::collections::HashSet;
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

async fn error_handler<F>(watch_future: F)
where
  F: Future<Output = Result<(), AnyError>>,
{
  let result = watch_future.await;
  if let Err(err) = result {
    let error_string = match err.downcast_ref::<JsError>() {
      Some(e) => format_js_error(e),
      None => format!("{:?}", err),
    };
    eprintln!(
      "{}: {}",
      colors::red_bold("error"),
      error_string.trim_start_matches("error: ")
    );
  }
}

pub enum ResolutionResult<T> {
  Restart {
    paths_to_watch: Vec<PathBuf>,
    result: Result<T, AnyError>,
  },
  Ignore,
}

async fn next_restart<R, T, F>(
  resolver: &mut R,
  debounced_receiver: &mut DebouncedReceiver,
) -> (Vec<PathBuf>, Result<T, AnyError>)
where
  R: FnMut(Option<Vec<PathBuf>>) -> F,
  F: Future<Output = ResolutionResult<T>>,
{
  loop {
    let changed = debounced_receiver.recv().await;
    match resolver(changed).await {
      ResolutionResult::Ignore => {
        log::debug!("File change ignored")
      }
      ResolutionResult::Restart {
        paths_to_watch,
        result,
      } => {
        return (paths_to_watch, result);
      }
    }
  }
}

pub struct PrintConfig {
  /// printing watcher status to terminal.
  pub job_name: String,
  /// determine whether to clear the terminal screen
  pub clear_screen: bool,
}

fn create_print_after_restart_fn(clear_screen: bool) -> impl Fn() {
  move || {
    if clear_screen {
      eprint!("{}", CLEAR_SCREEN);
    }
    info!(
      "{} File change detected! Restarting!",
      colors::intense_blue("Watcher"),
    );
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
pub async fn watch_func<R, O, T, F1, F2>(
  mut resolver: R,
  mut operation: O,
  print_config: PrintConfig,
) -> Result<(), AnyError>
where
  R: FnMut(Option<Vec<PathBuf>>) -> F1,
  O: FnMut(T) -> F2,
  F1: Future<Output = ResolutionResult<T>>,
  F2: Future<Output = Result<(), AnyError>>,
{
  let (sender, mut receiver) = DebouncedReceiver::new_with_sender();

  let PrintConfig {
    job_name,
    clear_screen,
  } = print_config;

  // Store previous data. If module resolution fails at some point, the watcher will try to
  // continue watching files using these data.
  let mut paths_to_watch;
  let mut resolution_result;

  let print_after_restart = create_print_after_restart_fn(clear_screen);

  match resolver(None).await {
    ResolutionResult::Ignore => {
      // The only situation where it makes sense to ignore the initial 'change'
      // is if the command isn't supposed to do anything until something changes,
      // e.g. a variant of `deno test` which doesn't run the entire test suite to start with,
      // but instead does nothing until you make a change.
      //
      // In that case, this is probably the correct output.
      info!(
        "{} Waiting for file changes...",
        colors::intense_blue("Watcher"),
      );

      let (paths, result) = next_restart(&mut resolver, &mut receiver).await;
      paths_to_watch = paths;
      resolution_result = result;

      print_after_restart();
    }
    ResolutionResult::Restart {
      paths_to_watch: paths,
      result,
    } => {
      paths_to_watch = paths;
      resolution_result = result;
    }
  };

  info!("{} {} started.", colors::intense_blue("Watcher"), job_name,);

  loop {
    let mut watcher = new_watcher(sender.clone())?;
    add_paths_to_watcher(&mut watcher, &paths_to_watch);

    match resolution_result {
      Ok(operation_arg) => {
        let fut = error_handler(operation(operation_arg));
        select! {
          (paths, result) = next_restart(&mut resolver, &mut receiver) => {
            if result.is_ok() {
              paths_to_watch = paths;
            }
            resolution_result = result;

            print_after_restart();
            continue;
          },
          _ = fut => {},
        };

        info!(
          "{} {} finished. Restarting on file change...",
          colors::intense_blue("Watcher"),
          job_name,
        );
      }
      Err(error) => {
        eprintln!("{}: {}", colors::red_bold("error"), error);
        info!(
          "{} {} failed. Restarting on file change...",
          colors::intense_blue("Watcher"),
          job_name,
        );
      }
    }

    let (paths, result) = next_restart(&mut resolver, &mut receiver).await;
    if result.is_ok() {
      paths_to_watch = paths;
    }
    resolution_result = result;

    print_after_restart();

    drop(watcher);
  }
}

/// Creates a file watcher.
///
/// - `operation` is the actual operation we want to run every time the watcher detects file
/// changes. For example, in the case where we would like to bundle, then `operation` would
/// have the logic for it like bundling the code.
pub async fn watch_func2<T: Clone, O, F>(
  mut paths_to_watch_receiver: UnboundedReceiver<Vec<PathBuf>>,
  mut operation: O,
  operation_args: T,
  print_config: PrintConfig,
) -> Result<(), AnyError>
where
  O: FnMut(T) -> Result<F, AnyError>,
  F: Future<Output = Result<(), AnyError>>,
{
  let (watcher_sender, mut watcher_receiver) =
    DebouncedReceiver::new_with_sender();

  let PrintConfig {
    job_name,
    clear_screen,
  } = print_config;

  let print_after_restart = create_print_after_restart_fn(clear_screen);

  info!("{} {} started.", colors::intense_blue("Watcher"), job_name,);

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

  loop {
    let mut watcher = new_watcher(watcher_sender.clone())?;
    consume_paths_to_watch(&mut watcher, &mut paths_to_watch_receiver);

    let receiver_future = async {
      loop {
        let maybe_paths = paths_to_watch_receiver.recv().await;
        add_paths_to_watcher(&mut watcher, &maybe_paths.unwrap());
      }
    };
    let operation_future = error_handler(operation(operation_args.clone())?);

    select! {
      _ = receiver_future => {},
      _ = watcher_receiver.recv() => {
        print_after_restart();
        continue;
      },
      _ = operation_future => {
        // TODO(bartlomieju): print exit code here?
        info!(
          "{} {} finished. Restarting on file change...",
          colors::intense_blue("Watcher"),
          job_name,
        );
        consume_paths_to_watch(&mut watcher, &mut paths_to_watch_receiver);
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
      _ = watcher_receiver.recv() => {
        print_after_restart();
        continue;
      },
    };
  }
}

fn new_watcher(
  sender: Arc<mpsc::UnboundedSender<Vec<PathBuf>>>,
) -> Result<RecommendedWatcher, AnyError> {
  let mut watcher: RecommendedWatcher =
    Watcher::new(move |res: Result<NotifyEvent, NotifyError>| {
      if let Ok(event) = res {
        if matches!(
          event.kind,
          EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
        ) {
          let paths = event
            .paths
            .iter()
            .filter_map(|path| canonicalize_path(path).ok())
            .collect();
          sender.send(paths).unwrap();
        }
      }
    })?;

  watcher.configure(Config::PreciseEvents(true)).unwrap();

  Ok(watcher)
}

fn add_paths_to_watcher(watcher: &mut RecommendedWatcher, paths: &[PathBuf]) {
  // Ignore any error e.g. `PathNotFound`
  for path in paths {
    let _ = watcher.watch(path, RecursiveMode::Recursive);
  }
  log::debug!("Watching paths: {:?}", paths);
}
