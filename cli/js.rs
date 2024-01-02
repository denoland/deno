// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::Snapshot;
use log::debug;

#[cfg(not(feature = "__runtime_js_sources"))]
static CLI_SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/CLI_SNAPSHOT.bin"));

pub fn deno_isolate_init() -> Option<Snapshot> {
  debug!("Deno isolate init with snapshots.");
  #[cfg(not(feature = "__runtime_js_sources"))]
  {
    Some(Snapshot::Static(CLI_SNAPSHOT))
  }
  #[cfg(feature = "__runtime_js_sources")]
  {
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn runtime_snapshot() {
    let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
      startup_snapshot: deno_isolate_init(),
      ..Default::default()
    });
    js_runtime
      .execute_script_static(
        "<anon>",
        r#"
      if (!(bootstrap.mainRuntime && bootstrap.workerRuntime)) {
        throw Error("bad");
      }
    "#,
      )
      .unwrap();
  }
}
