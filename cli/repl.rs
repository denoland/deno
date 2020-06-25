// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::deno_dir::DenoDir;
use crate::op_error::OpError;
use deno_core::ErrBox;
use rustyline::Editor;
use std::fs;
use std::path::PathBuf;

pub struct Repl {
  editor: Editor<()>,
  history_file: PathBuf,
}

impl Repl {
  pub fn new(history_file: PathBuf) -> Self {
    let mut repl = Self {
      editor: Editor::<()>::new(),
      history_file,
    };

    repl.load_history();
    repl
  }

  fn load_history(&mut self) {
    debug!("Loading REPL history: {:?}", self.history_file);
    self
      .editor
      .load_history(&self.history_file.to_str().unwrap())
      .map_err(|e| {
        debug!("Unable to load history file: {:?} {}", self.history_file, e)
      })
      // ignore this error (e.g. it occurs on first load)
      .unwrap_or(())
  }

  fn save_history(&mut self) -> Result<(), ErrBox> {
    fs::create_dir_all(self.history_file.parent().unwrap())?;
    self
      .editor
      .save_history(&self.history_file.to_str().unwrap())
      .map(|_| debug!("Saved REPL history to: {:?}", self.history_file))
      .map_err(|e| {
        eprintln!("Unable to save REPL history: {:?} {}", self.history_file, e);
        ErrBox::from(e)
      })
  }

  pub fn readline(&mut self, prompt: &str) -> Result<String, OpError> {
    self
      .editor
      .readline(&prompt)
      .map(|line| {
        self.editor.add_history_entry(line.clone());
        line
      })
      .map_err(OpError::from)
    // Forward error to TS side for processing
  }
}

impl Drop for Repl {
  fn drop(&mut self) {
    self.save_history().unwrap();
  }
}

pub fn history_path(dir: &DenoDir, history_file: &str) -> PathBuf {
  let mut p: PathBuf = dir.root.clone();
  p.push(history_file);
  p
}
