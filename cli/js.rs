// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::Snapshot;
use log::debug;

static CLI_SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/CLI_SNAPSHOT.bin"));

pub fn deno_isolate_init() -> Snapshot {
  debug!("Deno isolate init with snapshots.");
  Snapshot::Static(CLI_SNAPSHOT)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn runtime_snapshot() {
    let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
      startup_snapshot: Some(deno_isolate_init()),
      ..Default::default()
    });
    js_runtime
      .execute_script_static(
        "<anon>",
        r#"
      if (!(bootstrap.mainRuntime && bootstrap.workerRuntime)) {
        throw Error("bad");
      }
      console.log("we have console.log!!!");
    "#,
      )
      .unwrap();
  }
}
