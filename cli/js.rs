// Copyright 2018-2025 the Deno authors. MIT license.

use log::debug;

pub fn deno_isolate_init() -> Option<&'static [u8]> {
  debug!("Deno isolate init with snapshots.");
  deno_snapshots::CLI_SNAPSHOT
}
