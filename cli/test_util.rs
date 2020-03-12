// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// TODO(ry) Make this file test-only. Somehow it's very difficult to export
// methods to tests/integration_tests.rs if this is enabled...
// #![cfg(test)]

use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::sync::Mutex;
use std::sync::MutexGuard;

lazy_static! {
  static ref GUARD: Mutex<()> = Mutex::new(());
}

pub fn root_path() -> PathBuf {
  PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
}

pub fn target_dir() -> PathBuf {
  let current_exe = std::env::current_exe().unwrap();
  let target_dir = current_exe.parent().unwrap().parent().unwrap();
  println!("target_dir {}", target_dir.display());
  target_dir.into()
}

pub fn deno_exe_path() -> PathBuf {
  // Something like /Users/rld/src/deno/target/debug/deps/deno
  let mut p = target_dir().join("deno");
  if cfg!(windows) {
    p.set_extension("exe");
  }
  p
}

pub struct HttpServerGuard<'a> {
  #[allow(dead_code)]
  g: MutexGuard<'a, ()>,
  child: Child,
}

impl<'a> Drop for HttpServerGuard<'a> {
  fn drop(&mut self) {
    match self.child.try_wait() {
      Ok(None) => {
        self.child.kill().expect("failed to kill http_server.py");
      }
      Ok(Some(status)) => {
        panic!("http_server.py exited unexpectedly {}", status)
      }
      Err(e) => panic!("http_server.py err {}", e),
    }
  }
}

/// Starts tools/http_server.py when the returned guard is dropped, the server
/// will be killed.
pub fn http_server<'a>() -> HttpServerGuard<'a> {
  // TODO(bartlomieju) Allow tests to use the http server in parallel.
  let g = GUARD.lock().unwrap();

  println!("tools/http_server.py starting...");
  let mut child = Command::new("python")
    .current_dir(root_path())
    .args(&["-u", "tools/http_server.py"])
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");

  let stdout = child.stdout.as_mut().unwrap();
  use std::io::{BufRead, BufReader};
  let lines = BufReader::new(stdout).lines();
  // Wait for "ready" on stdout. See tools/http_server.py
  for maybe_line in lines {
    if let Ok(line) = maybe_line {
      if line.starts_with("ready") {
        break;
      }
    } else {
      panic!(maybe_line.unwrap_err());
    }
  }

  HttpServerGuard { child, g }
}
