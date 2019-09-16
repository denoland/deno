// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#![cfg(test)]

use std::process::Child;
use std::process::Command;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::thread;
use std::time;

lazy_static! {
  static ref GUARD: Mutex<()> = Mutex::new(());
}

pub struct Guard<'a> {
  g: MutexGuard<'a, ()>,
  child: Child,
}

impl<'a> Drop for Guard<'a> {
  fn drop(&mut self) {
    self.child.kill().expect("failed to kill http_server.py");
    drop(&self.g);
  }
}

// TODO(ry) Allow tests to use the http server in parallel.
pub fn run<'a>() -> Guard<'a> {
  let g = GUARD.lock().unwrap();

  let child = Command::new("python")
    .current_dir("../")
    .arg("tools/http_server.py")
    .spawn()
    .expect("failed to execute child");

  // Wait 1 second for the server to come up. TODO(ry) this is Racy.
  thread::sleep(time::Duration::from_secs(1));

  Guard { child, g }
}
