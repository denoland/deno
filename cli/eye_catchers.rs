// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_runtime::colors;
use log::info;

pub fn check(message: &str) {
  info!("{} {}", colors::green("Check"), message);
}

pub fn watcher(message: &str) {
  info!("{} {}", colors::intense_blue("Watcher"), message);
}

pub fn emit(message: &str) {
  info!("{} {}", colors::green("Emit"), message);
}

pub fn bundle(message: &str) {
  info!("{} {}", colors::green("Bundle"), message);
}
