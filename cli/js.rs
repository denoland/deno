// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use log::debug;

#[cfg(not(feature = "hmr"))]
static CLI_SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/CLI_SNAPSHOT.bin"));

pub fn deno_isolate_init() -> Option<&'static [u8]> {
  debug!("Deno isolate init with snapshots.");
  #[cfg(not(feature = "hmr"))]
  {
    Some(CLI_SNAPSHOT)
  }
  #[cfg(feature = "hmr")]
  {
    None
  }
}
