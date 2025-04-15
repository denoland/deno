// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashSet;
use std::future::Future;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use deno_config::glob::PathOrPatternSet;
use deno_core::error::AnyError;
use deno_core::error::CoreError;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_lib::util::result::any_and_jserrorbox_downcast_ref;
use deno_runtime::fmt_errors::format_js_error;
use log::info;
use notify::event::Event as NotifyEvent;
use notify::event::EventKind;
use notify::Error as NotifyError;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use tokio::select;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

use crate::args::Flags;
use crate::colors;
use crate::util::fs::canonicalize_path;

const CLEAR_SCREEN: &str = "\x1B[H\x1B[2J\x1B[3J";
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
    let error_string = match any_and_jserrorbox_downcast_ref::<CoreError>(&err)
    {
      Some(CoreError::Js(e)) => format_js_error(e),
      _ => format!("{err:?}"),
    };
    log::error!(
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
  banner: &'static str,
  /// Printing watcher status to terminal.
  job_name: &'static str,
  /// Determine whether to clear the terminal screen; applicable to TTY environments only.
  clear_screen: bool,
}

impl PrintConfig {
  /// By default `PrintConfig` uses "Watcher" as a banner name that will
  /// be printed in color. If you need to customize it, use
  /// `PrintConfig::new_with_banner` instead.
  pub fn new(job_name: &'static str, clear_screen: bool) -> Self {
    Self {
      banner: "Watcher",
      job_name,
      clear_screen,
    }
  }

  pub fn new_with_banner(
    banner: &'static str,
    job_name: &'static str,
    clear_screen: bool,
  ) -> Self {
    Self {
      banner,
      job_name,
      clear_screen,
    }
  }
}

fn create_print_after_restart_fn(clear_screen: bool) -> impl Fn() {
  move || {
    #[allow(clippy::print_stderr)]
    if clear_screen && std::io::stderr().is_terminal() {
      eprint!("{}", CLEAR_SCREEN);
    }
  }
}

#[derive(Debug)]
pub struct WatcherCommunicatorOptions {
  /// Send a list of paths that should be watched for changes.
  pub paths_to_watch_tx: tokio::sync::mpsc::UnboundedSender<Vec<PathBuf>>,
  /// Listen for a list of paths that were changed.
  pub changed_paths_rx: tokio::sync::broadcast::Receiver<Option<Vec<PathBuf>>>,
  pub changed_paths_tx: tokio::sync::broadcast::Sender<Option<Vec<PathBuf>>>,
  /// Send a message to force a restart.
  pub restart_tx: tokio::sync::mpsc::UnboundedSender<()>,
  pub restart_mode: WatcherRestartMode,
  pub banner: String,
}

/// An interface to interact with Deno's CLI file watcher.
#[derive(Debug)]
pub struct WatcherCommunicator {
  /// Send a list of paths that should be watched for changes.
  paths_to_watch_tx: tokio::sync::mpsc::UnboundedSender<Vec<PathBuf>>,
  /// Listen for a list of paths that were changed.
  changed_paths_rx: tokio::sync::broadcast::Receiver<Option<Vec<PathBuf>>>,
  changed_paths_tx: tokio::sync::broadcast::Sender<Option<Vec<PathBuf>>>,
  /// Send a message to force a restart.
  restart_tx: tokio::sync::mpsc::UnboundedSender<()>,
  restart_mode: Mutex<WatcherRestartMode>,
  banner: String,
}

impl WatcherCommunicator {
  pub fn new(options: WatcherCommunicatorOptions) -> Self {
    Self {
      paths_to_watch_tx: options.paths_to_watch_tx,
      changed_paths_rx: options.changed_paths_rx,
      changed_paths_tx: options.changed_paths_tx,
      restart_tx: options.restart_tx,
      restart_mode: Mutex::new(options.restart_mode),
      banner: options.banner,
    }
  }

  pub fn watch_paths(
    &self,
    paths: Vec<PathBuf>,
  ) -> Result<(), SendError<Vec<PathBuf>>> {
    if paths.is_empty() {
      return Ok(());
    }
    self.paths_to_watch_tx.send(paths)
  }

  pub fn force_restart(&self) -> Result<(), SendError<()>> {
    // Change back to automatic mode, so that HMR can set up watching
    // from scratch.
    *self.restart_mode.lock() = WatcherRestartMode::Automatic;
    self.restart_tx.send(())
  }

  pub async fn watch_for_changed_paths(
    &self,
  ) -> Result<Option<Vec<PathBuf>>, RecvError> {
    let mut rx = self.changed_paths_rx.resubscribe();
    rx.recv().await
  }

  pub fn change_restart_mode(&self, restart_mode: WatcherRestartMode) {
    *self.restart_mode.lock() = restart_mode;
  }

  pub fn send(
    &self,
    paths: Option<Vec<PathBuf>>,
  ) -> Result<(), SendError<Option<Vec<PathBuf>>>> {
    match *self.restart_mode.lock() {
      WatcherRestartMode::Automatic => {
        self.restart_tx.send(()).map_err(|_| SendError(None))
      }
      WatcherRestartMode::Manual => self
        .changed_paths_tx
        .send(paths)
        .map(|_| ())
        .map_err(|e| SendError(e.0)),
    }
  }

  pub fn print(&self, msg: String) {
    log::info!("{} {}", self.banner, colors::gray(msg));
  }

  pub fn show_path_changed(&self, changed_paths: Option<Vec<PathBuf>>) {
    if let Some(paths) = changed_paths {
      if !paths.is_empty() {
        self.print(format!("Restarting! File change detected: {:?}", paths[0]))
      } else {
        self.print("Restarting! File change detected.".to_string())
      }
    }
  }
}

/// Creates a file watcher.
///
/// - `operation` is the actual operation we want to run every time the watcher detects file
///   changes. For example, in the case where we would like to bundle, then `operation` would
///   have the logic for it like bundling the code.
pub async fn watch_func<O, F>(
  flags: Arc<Flags>,
  print_config: PrintConfig,
  operation: O,
) -> Result<(), AnyError>
where
  O: FnMut(
    Arc<Flags>,
    Arc<WatcherCommunicator>,
    Option<Vec<PathBuf>>,
  ) -> Result<F, AnyError>,
  F: Future<Output = Result<(), AnyError>>,
{
  let fut = watch_recv(
    flags,
    print_config,
    WatcherRestartMode::Automatic,
    operation,
  )
  .boxed_local();

  fut.await
}

#[derive(Clone, Copy, Debug)]
pub enum WatcherRestartMode {
  /// When a file path changes the process is restarted.
  Automatic,

  /// When a file path changes the caller will trigger a restart, using
  /// `WatcherInterface.restart_tx`.
  Manual,
}

/// Creates a file watcher.
///
/// - `operation` is the actual operation we want to run every time the watcher detects file
///    changes. For example, in the case where we would like to bundle, then `operation` would
///    have the logic for it like bundling the code.
pub async fn watch_recv<O, F>(
  mut flags: Arc<Flags>,
  print_config: PrintConfig,
  restart_mode: WatcherRestartMode,
  mut operation: O,
) -> Result<(), AnyError>
where
  O: FnMut(
    Arc<Flags>,
    Arc<WatcherCommunicator>,
    Option<Vec<PathBuf>>,
  ) -> Result<F, AnyError>,
  F: Future<Output = Result<(), AnyError>>,
{
  let exclude_set = flags.resolve_watch_exclude_set()?;
  let (paths_to_watch_tx, mut paths_to_watch_rx) =
    tokio::sync::mpsc::unbounded_channel();
  let (restart_tx, mut restart_rx) = tokio::sync::mpsc::unbounded_channel();
  let (changed_paths_tx, changed_paths_rx) = tokio::sync::broadcast::channel(4);
  let (watcher_sender, mut watcher_receiver) =
    DebouncedReceiver::new_with_sender();

  let PrintConfig {
    banner,
    job_name,
    clear_screen,
  } = print_config;

  let print_after_restart = create_print_after_restart_fn(clear_screen);
  let watcher_communicator =
    Arc::new(WatcherCommunicator::new(WatcherCommunicatorOptions {
      paths_to_watch_tx: paths_to_watch_tx.clone(),
      changed_paths_rx: changed_paths_rx.resubscribe(),
      changed_paths_tx,
      restart_tx: restart_tx.clone(),
      restart_mode,
      banner: colors::intense_blue(banner).to_string(),
    }));
  info!("{} {} started.", colors::intense_blue(banner), job_name);

  let changed_paths = Rc::new(RefCell::new(None));
  let changed_paths_ = changed_paths.clone();
  let watcher_ = watcher_communicator.clone();

  deno_core::unsync::spawn(async move {
    loop {
      let received_changed_paths = watcher_receiver.recv().await;
      changed_paths_
        .borrow_mut()
        .clone_from(&received_changed_paths);

      // TODO(bartlomieju): should we fail on sending changed paths?
      let _ = watcher_.send(received_changed_paths);
    }
  });

  loop {
    // We may need to give the runtime a tick to settle, as cancellations may need to propagate
    // to tasks. We choose yielding 10 times to the runtime as a decent heuristic. If watch tests
    // start to fail, this may need to be increased.
    for _ in 0..10 {
      tokio::task::yield_now().await;
    }

    let mut watcher = new_watcher(watcher_sender.clone())?;
    consume_paths_to_watch(&mut watcher, &mut paths_to_watch_rx, &exclude_set);

    let receiver_future = async {
      loop {
        let maybe_paths = paths_to_watch_rx.recv().await;
        add_paths_to_watcher(&mut watcher, &maybe_paths.unwrap(), &exclude_set);
      }
    };
    let operation_future = error_handler(operation(
      flags.clone(),
      watcher_communicator.clone(),
      changed_paths.borrow_mut().take(),
    )?);

    // don't reload dependencies after the first run
    if flags.reload {
      flags = Arc::new(Flags {
        reload: false,
        ..Arc::unwrap_or_clone(flags)
      });
    }

    select! {
      _ = receiver_future => {},
      _ = restart_rx.recv() => {
        print_after_restart();
        continue;
      },
      success = operation_future => {
        consume_paths_to_watch(&mut watcher, &mut paths_to_watch_rx, &exclude_set);
        // TODO(bartlomieju): print exit code here?
        info!(
          "{} {} {}. Restarting on file change...",
          colors::intense_blue(banner),
          job_name,
          if success {
            "finished"
          } else {
            "failed"
          }
        );
      },
    }
    let receiver_future = async {
      loop {
        let maybe_paths = paths_to_watch_rx.recv().await;
        add_paths_to_watcher(&mut watcher, &maybe_paths.unwrap(), &exclude_set);
      }
    };

    // If we got this far, it means that the `operation` has finished; let's wait
    // and see if there are any new paths to watch received or any of the already
    // watched paths has changed.
    select! {
      _ = receiver_future => {},
      _ = restart_rx.recv() => {
        print_after_restart();
        continue;
      },
    }
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

fn add_paths_to_watcher(
  watcher: &mut RecommendedWatcher,
  paths: &[PathBuf],
  paths_to_exclude: &PathOrPatternSet,
) {
  // Ignore any error e.g. `PathNotFound`
  let mut watched_paths = Vec::new();

  for path in paths {
    if paths_to_exclude.matches_path(path) {
      continue;
    }

    watched_paths.push(path.clone());
    let _ = watcher.watch(path, RecursiveMode::Recursive);
  }
  log::debug!("Watching paths: {:?}", watched_paths);
}

fn consume_paths_to_watch(
  watcher: &mut RecommendedWatcher,
  receiver: &mut UnboundedReceiver<Vec<PathBuf>>,
  exclude_set: &PathOrPatternSet,
) {
  loop {
    match receiver.try_recv() {
      Ok(paths) => {
        add_paths_to_watcher(watcher, &paths, exclude_set);
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
