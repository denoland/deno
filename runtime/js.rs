// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use deno_core::Snapshot;
use log::debug;

#[cfg(not(feature = "dont_create_runtime_snapshot"))]
static RUNTIME_SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/RUNTIME_SNAPSHOT.bin"));

pub fn deno_isolate_init() -> Option<Snapshot> {
  debug!("Deno isolate init with snapshots.");
  #[cfg(not(feature = "dont_create_runtime_snapshot"))]
  {
    Some(Snapshot::Static(RUNTIME_SNAPSHOT))
  }
  #[cfg(feature = "dont_create_runtime_snapshot")]
  {
    None
  }
}

#[cfg(not(feature = "include_js_files_for_snapshotting"))]
pub static SOURCE_CODE_FOR_99_MAIN_JS: &str = include_str!("js/99_main.js");

#[cfg(feature = "include_js_files_for_snapshotting")]
pub static PATH_FOR_99_MAIN_JS: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/js/99_main.js");
