// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::js::CLI_SNAPSHOT;
use crate::js::COMPILER_SNAPSHOT;
use deno_core::Snapshot;
use deno_core::StartupData;

#[cfg(feature = "no-snapshot-init")]
pub fn deno_isolate_init() -> StartupData<'static> {
  debug!("Deno isolate init without snapshots.");
  let source =
    include_str!(concat!(env!("GN_OUT_DIR"), "/gen/cli/bundle/main.js"));
  StartupData::Script(deno_core::Script {
    filename: "gen/cli/bundle/main.js",
    source,
  })
}

#[cfg(not(feature = "no-snapshot-init"))]
pub fn deno_isolate_init() -> StartupData<'static> {
  debug!("Deno isolate init with snapshots.");
  let data = CLI_SNAPSHOT;
  StartupData::Snapshot(Snapshot::Static(data))
}

#[cfg(feature = "no-snapshot-init")]
pub fn compiler_isolate_init() -> StartupData<'static> {
  debug!("Compiler isolate init without snapshots.");
  let source =
    include_str!(concat!(env!("GN_OUT_DIR"), "/gen/cli/bundle/compiler.js"));
  StartupData::Script(deno_core::Script {
    filename: "gen/cli/bundle/compiler.js",
    source,
  })
}

#[cfg(not(feature = "no-snapshot-init"))]
pub fn compiler_isolate_init() -> StartupData<'static> {
  debug!("Deno isolate init with snapshots.");
  let data = COMPILER_SNAPSHOT;
  StartupData::Snapshot(Snapshot::Static(data))
}
