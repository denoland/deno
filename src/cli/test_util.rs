// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// TODO(ry) Make this file test-only. Somehow it's very difficult to export
// methods to tests/integration_tests.rs and tests/tty_tests.rs if this
// is enabled...
// #![cfg(test)]

use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
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
  // TODO(ry) Allow tests to use the http server in parallel.
  let g = GUARD.lock().unwrap();

  println!("tools/http_server.py starting...");
  let child = Command::new("python")
    .current_dir(root_path())
    .arg("tools/http_server.py")
    .spawn()
    .expect("failed to execute child");

  // Wait 1 second for the server to come up. TODO(ry) this is Racy.
  std::thread::sleep(std::time::Duration::from_secs(2));

  println!("tools/http_server.py ready");

  HttpServerGuard { child, g }
}
