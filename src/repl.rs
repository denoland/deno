// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use rustyline;

use rustyline::error::ReadlineError::{Eof, Interrupted};

use crate::msg::ErrorKind;
use std::error::Error;

use crate::deno_dir::DenoDir;
use crate::errors::new as deno_error;
use crate::errors::DenoResult;
use std::path::PathBuf;
use std::process::exit;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

#[cfg(not(windows))]
use rustyline::Editor;

// Work around the issue that on Windows, `struct Editor` does not implement the
// `Send` trait, because it embeds a windows HANDLE which is a type alias for
// *mut c_void. This value isn't actually a pointer and there's nothing that
// can be mutated through it, so hack around it. TODO: a prettier solution.
#[cfg(windows)]
use std::ops::{Deref, DerefMut};

#[cfg(windows)]
struct Editor<T: rustyline::Helper> {
  inner: rustyline::Editor<T>,
}

#[cfg(windows)]
unsafe impl<T: rustyline::Helper> Send for Editor<T> {}

#[cfg(windows)]
impl<T: rustyline::Helper> Editor<T> {
  pub fn new() -> Editor<T> {
    Editor {
      inner: rustyline::Editor::<T>::new(),
    }
  }
}

#[cfg(windows)]
impl<T: rustyline::Helper> Deref for Editor<T> {
  type Target = rustyline::Editor<T>;

  fn deref(&self) -> &rustyline::Editor<T> {
    &self.inner
  }
}

#[cfg(windows)]
impl<T: rustyline::Helper> DerefMut for Editor<T> {
  fn deref_mut(&mut self) -> &mut rustyline::Editor<T> {
    &mut self.inner
  }
}

pub struct Repl {
  editor: Arc<Mutex<Editor<()>>>,
  pub tx: mpsc::Sender<DenoResult<String>>,
  pub rx: Arc<Mutex<mpsc::Receiver<DenoResult<String>>>>,
  pub prompt_tx: mpsc::Sender<String>,
  history_file: PathBuf,
}

fn save_history(
  editor: Arc<Mutex<Editor<()>>>,
  history_file: &PathBuf,
) -> DenoResult<()> {
  editor
    .lock()
    .unwrap()
    .save_history(history_file.to_str().unwrap())
    .map(|_| debug!("Saved REPL history to: {:?}", history_file))
    .map_err(|e| {
      eprintln!("Unable to save REPL history: {:?} {}", history_file, e);
      deno_error(ErrorKind::Other, e.description().to_string())
    })
}

impl Repl {
  pub fn new(history_file: PathBuf) -> Self {
    let (tx, rx) = mpsc::channel::<DenoResult<String>>();
    let (prompt_tx, prompt_rx) = mpsc::channel::<String>();
    let history_file_copy = history_file.clone();

    let mut repl = Self {
      editor: Arc::new(Mutex::new(Editor::<()>::new())),
      tx,
      rx: Arc::new(Mutex::new(rx)),
      prompt_tx,
      history_file,
    };

    repl.load_history();

    // Since Rustyline is not providing async read
    // dump the real loop to another thread...
    let tx = repl.tx.clone();
    let editor = repl.editor.clone();
    let _ = thread::spawn(move || loop {
      // Use to set prompt
      // and blocking wait for user read request
      let prompt = prompt_rx.recv().unwrap();
      let maybe_line = editor.lock().unwrap().readline(&prompt);
      // Handle errors
      // Guarantee that except for Interrupted,
      // we will always send something
      if maybe_line.is_err() {
        match maybe_line.unwrap_err() {
          Interrupted | Eof => {
            let _ = save_history(editor.clone(), &history_file_copy);
            exit(1);
          }
          e => {
            // Send the error to the channel
            let _ = tx.send(Err(deno_error(
              ErrorKind::Other,
              e.description().to_string(),
            )));
          }
        }
      } else {
        // Okay, send the string to the channel
        let line = maybe_line.unwrap();
        editor.lock().unwrap().add_history_entry(line.as_ref());
        let _ = tx.send(Ok(line));
      }
    });

    repl
  }

  fn load_history(&mut self) {
    debug!("Loading REPL history: {:?}", self.history_file);
    self
      .editor
      .lock()
      .unwrap()
      .load_history(&self.history_file.to_str().unwrap())
      .map_err(|e| debug!("Unable to load history file: {:?} {}", self.history_file, e))
      // ignore this error (e.g. it occurs on first load)
      .unwrap_or(())
  }

  fn save_history(&mut self) -> DenoResult<()> {
    save_history(self.editor.clone(), &self.history_file)
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
