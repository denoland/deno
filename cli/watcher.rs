// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::parking_lot::Mutex;
use deno_core::ModuleSpecifier;

use std::path::PathBuf;
use std::sync::Arc;

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

  pub fn as_reporter(&self) -> &dyn deno_graph::source::Reporter {
    self
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
