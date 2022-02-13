// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::fs_util::canonicalize_path;

use deno_core::error::AnyError;
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
use tokio::time::sleep;

const CLEAR_SCREEN: &str = "\x1B[2J\x1B[1;1H";
const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(200);

struct DebouncedReceiver {
  receiver: mpsc::UnboundedReceiver<Vec<PathBuf>>,
}

impl DebouncedReceiver {
  fn new_with_sender() -> (Arc<mpsc::UnboundedSender<Vec<PathBuf>>>, Self) {
    let (sender, receiver) = mpsc::unbounded_channel();
    (Arc::new(sender), Self { receiver })
  }

  async fn recv(&mut self) -> Option<Vec<PathBuf>> {
    let mut received_items = self
      .receiver
      .recv()
      .await?
      .into_iter()
      .collect::<HashSet<_>>(); // prevent duplicates
    loop {
      tokio::select! {
        items = self.receiver.recv() => {
          received_items.extend(items?);
        }
        _ = sleep(DEBOUNCE_INTERVAL) => {
          return Some(received_items.into_iter().collect());
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
    let msg = format!("{}: {}", colors::red_bold("error"), err);
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

  let print_after_restart = || {
    if clear_screen {
      eprint!("{}", CLEAR_SCREEN);
    }
    info!(
      "{} File change detected! Restarting!",
      colors::intense_blue("Watcher"),
    );
  };

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

  if clear_screen {
    eprint!("{}", CLEAR_SCREEN);
  }

  info!("{} {} started.", colors::intense_blue("Watcher"), job_name,);

  loop {
    let watcher = new_watcher(&paths_to_watch, sender.clone())?;

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

fn new_watcher(
  paths: &[PathBuf],
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

  log::debug!("Watching paths: {:?}", paths);
  for path in paths {
    // Ignore any error e.g. `PathNotFound`
    let _ = watcher.watch(path, RecursiveMode::Recursive);
  }

  Ok(watcher)
}
