// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use log::debug;

#[cfg(not(feature = "__runtime_js_sources"))]
static CLI_SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/CLI_SNAPSHOT.bin"));

pub fn deno_isolate_init() -> Option<&'static [u8]> {
  debug!("Deno isolate init with snapshots.");
  #[cfg(not(feature = "__runtime_js_sources"))]
  {
    Some(CLI_SNAPSHOT)
  }
  #[cfg(feature = "__runtime_js_sources")]
  {
    None
  }
}
