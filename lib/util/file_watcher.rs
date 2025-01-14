use std::path::PathBuf;

use deno_runtime::colors;
use parking_lot::Mutex;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::error::SendError;

#[derive(Clone, Copy, Debug)]
pub enum WatcherRestartMode {
  /// When a file path changes the process is restarted.
  Automatic,

  /// When a file path changes the caller will trigger a restart, using
  /// `WatcherInterface.restart_tx`.
  Manual,
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
