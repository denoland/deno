// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use deno_core::deno_buf;
use deno_core::{Script, StartupData};

pub fn deno_isolate_init() -> StartupData {
  if cfg!(feature = "no-snapshot-init") {
    debug!("Deno isolate init without snapshots.");
    #[cfg(not(feature = "check-only"))]
    let source_bytes =
      include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/bundle/main.js"));
    #[cfg(feature = "check-only")]
    let source_bytes = vec![];

    StartupData::Script(Script {
      filename: "gen/bundle/main.js".to_string(),
      source: std::str::from_utf8(source_bytes).unwrap().to_string(),
    })
  } else {
    debug!("Deno isolate init with snapshots.");
    #[cfg(not(any(feature = "check-only", feature = "no-snapshot-init")))]
    let data =
      include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/snapshot_deno.bin"));
    #[cfg(any(feature = "check-only", feature = "no-snapshot-init"))]
    let data = vec![];

    unsafe {
      StartupData::Snapshot(deno_buf::from_raw_parts(data.as_ptr(), data.len()))
    }
  }
}

pub fn compiler_isolate_init() -> StartupData {
  if cfg!(feature = "no-snapshot-init") {
    debug!("Deno isolate init without snapshots.");
    #[cfg(not(feature = "check-only"))]
    let source_bytes =
      include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/bundle/compiler.js"));
    #[cfg(feature = "check-only")]
    let source_bytes = vec![];

    StartupData::Script(Script {
      filename: "gen/bundle/compiler.js".to_string(),
      source: std::str::from_utf8(source_bytes).unwrap().to_string(),
    })
  } else {
    debug!("Deno isolate init with snapshots.");
    #[cfg(not(any(feature = "check-only", feature = "no-snapshot-init")))]
    let data =
      include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/snapshot_compiler.bin"));
    #[cfg(any(feature = "check-only", feature = "no-snapshot-init"))]
    let data = vec![];

    unsafe {
      StartupData::Snapshot(deno_buf::from_raw_parts(data.as_ptr(), data.len()))
    }
  }
}
