// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::js::CLI_SNAPSHOT;
use crate::js::COMPILER_SNAPSHOT;
use deno_core::Snapshot;
use deno_core::StartupData;

pub fn deno_isolate_init() -> StartupData<'static> {
  debug!("Deno isolate init with snapshots.");
  let data = CLI_SNAPSHOT;
  StartupData::Snapshot(Snapshot::Static(data))
}

pub fn compiler_isolate_init() -> StartupData<'static> {
  debug!("Deno isolate init with snapshots.");
  let data = COMPILER_SNAPSHOT;
  StartupData::Snapshot(Snapshot::Static(data))
}
