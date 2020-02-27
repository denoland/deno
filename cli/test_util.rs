// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// TODO(ry) Make this file test-only. Somehow it's very difficult to export
// methods to tests/integration_tests.rs if this is enabled...
// #![cfg(test)]

use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

lazy_static! {
  static ref SERVER: Mutex<Option<Child>> = Mutex::new(None);
  static ref SERVER_COUNT: AtomicUsize = AtomicUsize::new(0);
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

pub struct HttpServerGuard {}

impl Drop for HttpServerGuard {
  fn drop(&mut self) {
    let count = SERVER_COUNT.fetch_sub(1, Ordering::SeqCst);
    // If no more tests hold guard we can kill the server

    if count == 1 {
      kill_http_server();
    }
  }
}

fn kill_http_server() {
  let mut server_guard = SERVER.lock().unwrap();
  let mut child = server_guard
    .take()
    .expect("Trying to kill server but already killed");
  match child.try_wait() {
    Ok(None) => {
      child.kill().expect("failed to kill http_server.py");
    }
    Ok(Some(status)) => panic!("http_server.py exited unexpectedly {}", status),
    Err(e) => panic!("http_server.py error: {}", e),
  }
  drop(server_guard);
}

pub fn http_server() -> HttpServerGuard {
  SERVER_COUNT.fetch_add(1, Ordering::SeqCst);
  {
    let mut server_guard = SERVER.lock().unwrap();
    if server_guard.is_none() {
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
            server_guard.replace(child);
            break;
          }
        } else {
          panic!(maybe_line.unwrap_err());
        }
      }
    }
  }
  HttpServerGuard {}
}
