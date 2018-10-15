// Copyright 2018 the Deno authors. All rights reserved. MIT license.
extern crate rustyline;
use rustyline::Editor;

use std::sync::Arc;
use std::error::Error;
//use futures::Future;
use msg::ErrorKind;


use isolate;
use errors::DenoResult;
use errors::new as deno_error;

pub fn readline(_state: &Arc<isolate::IsolateState>, prompt: &String) -> DenoResult<String> {
  // FIXME
  // let mut maybe_editor = state.repl.lock().unwrap();
  // if maybe_editor.is_none() {
  //   println!("{}", "Creating new Editor<()>");
  //   *maybe_editor = Some(start_repl()); // will this assign within IsolateState?
  // }
  let maybe_editor = Some(start_repl());
  maybe_editor
    .unwrap()
    .readline(prompt)
    .map_err(|err| deno_error(ErrorKind::Other, err.description().to_string()))
}

// FIXME can we call save_history when this is dropped / upon exit?
// rl.save_history("history.txt").unwrap();
fn start_repl() -> Editor<()> {
    let mut editor = Editor::<()>::new();
    if editor.load_history("history.txt").is_err() {
        eprintln!("No previous history.");
    }
    editor
}
