// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::cache::ParsedSourceCache;
use crate::graph_util::ModuleGraphContainer;
use crate::module_loader::CjsResolutionStore;

use deno_core::parking_lot::Mutex;
use deno_core::ModuleSpecifier;

use std::path::PathBuf;
use std::sync::Arc;

// todo: remove this.

pub struct FileWatcher {
  cli_options: Arc<CliOptions>,
  cjs_resolutions: Arc<CjsResolutionStore>,
  graph_container: Arc<ModuleGraphContainer>,
  maybe_reporter: Option<FileWatcherReporter>,
  parsed_source_cache: Arc<ParsedSourceCache>,
}

impl FileWatcher {
  pub fn new(
    cli_options: Arc<CliOptions>,
    cjs_resolutions: Arc<CjsResolutionStore>,
    graph_container: Arc<ModuleGraphContainer>,
    maybe_reporter: Option<FileWatcherReporter>,
    parsed_source_cache: Arc<ParsedSourceCache>,
  ) -> Self {
    Self {
      cli_options,
      cjs_resolutions,
      parsed_source_cache,
      graph_container,
      maybe_reporter,
    }
  }
  /// Reset all runtime state to its default. This should be used on file
  /// watcher restarts.
  pub fn reset(&self) {
    self.cjs_resolutions.clear();
    self.parsed_source_cache.clear();
    self.graph_container.clear();

    self.init_watcher();
  }

  // Add invariant files like the import map and explicit watch flag list to
  // the watcher. Dedup for build_for_file_watcher and reset_for_file_watcher.
  pub fn init_watcher(&self) {
    let files_to_watch_sender = match &self.maybe_reporter {
      Some(reporter) => &reporter.sender,
      None => return,
    };
    if let Some(watch_paths) = self.cli_options.watch_paths() {
      files_to_watch_sender.send(watch_paths.clone()).unwrap();
    }
    if let Ok(Some(import_map_path)) = self
      .cli_options
      .resolve_import_map_specifier()
      .map(|ms| ms.and_then(|ref s| s.to_file_path().ok()))
    {
      files_to_watch_sender.send(vec![import_map_path]).unwrap();
    }
  }
}

#[derive(Clone, Debug)]
pub struct FileWatcherReporter {
  sender: tokio::sync::mpsc::UnboundedSender<Vec<PathBuf>>,
  file_paths: Arc<Mutex<Vec<PathBuf>>>,
}

impl FileWatcherReporter {
  pub fn new(sender: tokio::sync::mpsc::UnboundedSender<Vec<PathBuf>>) -> Self {
    Self {
      sender,
      file_paths: Default::default(),
    }
  }
}

impl deno_graph::source::Reporter for FileWatcherReporter {
  fn on_load(
    &self,
    specifier: &ModuleSpecifier,
    modules_done: usize,
    modules_total: usize,
  ) {
    let mut file_paths = self.file_paths.lock();
    if specifier.scheme() == "file" {
      file_paths.push(specifier.to_file_path().unwrap());
    }

    if modules_done == modules_total {
      self.sender.send(file_paths.drain(..).collect()).unwrap();
    }
  }
}
