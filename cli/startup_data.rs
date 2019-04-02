// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use deno::{Script, StartupData};

pub fn deno_isolate_init() -> StartupData<'static> {
  if cfg!(feature = "no-snapshot-init") {
    debug!("Deno isolate init without snapshots.");
    #[cfg(not(feature = "check-only"))]
    let source_bytes =
      include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/cli/bundle/main.js"));
    #[cfg(feature = "check-only")]
    let source_bytes = vec![];

    StartupData::Script(Script {
      filename: "gen/cli/bundle/main.js".to_string(),
      source: std::str::from_utf8(&source_bytes[..]).unwrap().to_string(),
    })
  } else {
    debug!("Deno isolate init with snapshots.");
    #[cfg(not(any(feature = "check-only", feature = "no-snapshot-init")))]
    let data =
      include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/cli/snapshot_deno.bin"));
    #[cfg(any(feature = "check-only", feature = "no-snapshot-init"))]
    let data = vec![];

    StartupData::Snapshot(data)
  }
}

pub fn compiler_isolate_init() -> StartupData<'static> {
  if cfg!(feature = "no-snapshot-init") {
    debug!("Compiler isolate init without snapshots.");
    #[cfg(not(feature = "check-only"))]
    let source_bytes = include_bytes!(concat!(
      env!("GN_OUT_DIR"),
      "/gen/cli/bundle/compiler.js"
    ));
    #[cfg(feature = "check-only")]
    let source_bytes = vec![];

    StartupData::Script(Script {
      filename: "gen/cli/bundle/compiler.js".to_string(),
      source: std::str::from_utf8(&source_bytes[..]).unwrap().to_string(),
    })
  } else {
    debug!("Deno isolate init with snapshots.");
    #[cfg(not(any(feature = "check-only", feature = "no-snapshot-init")))]
    let data = include_bytes!(concat!(
      env!("GN_OUT_DIR"),
      "/gen/cli/snapshot_compiler.bin"
    ));
    #[cfg(any(feature = "check-only", feature = "no-snapshot-init"))]
    let data = vec![];

    StartupData::Snapshot(data)
  }
}
