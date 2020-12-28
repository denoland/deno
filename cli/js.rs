// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::Snapshot;

pub const TS_VERSION: &str = env!("TS_VERSION");

pub static CLI_SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/CLI_SNAPSHOT.bin"));
pub static COMPILER_SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/COMPILER_SNAPSHOT.bin"));
pub static DENO_NS_LIB: &str = include_str!("dts/lib.deno.ns.d.ts");
pub static DENO_WEB_LIB: &str = include_str!(env!("DENO_WEB_LIB_PATH"));
pub static DENO_FETCH_LIB: &str = include_str!(env!("DENO_FETCH_LIB_PATH"));
pub static SHARED_GLOBALS_LIB: &str =
  include_str!("dts/lib.deno.shared_globals.d.ts");
pub static WINDOW_LIB: &str = include_str!("dts/lib.deno.window.d.ts");
pub static UNSTABLE_NS_LIB: &str = include_str!("dts/lib.deno.unstable.d.ts");

pub fn deno_isolate_init() -> Snapshot {
  debug!("Deno isolate init with snapshots.");
  let data = CLI_SNAPSHOT;
  Snapshot::Static(data)
}

pub fn compiler_isolate_init() -> Snapshot {
  debug!("Deno compiler isolate init with snapshots.");
  let data = COMPILER_SNAPSHOT;
  Snapshot::Static(data)
}

#[test]
fn cli_snapshot() {
  let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
    startup_snapshot: Some(deno_isolate_init()),
    ..Default::default()
  });
  js_runtime
    .execute(
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

#[test]
fn compiler_snapshot() {
  let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
    startup_snapshot: Some(compiler_isolate_init()),
    ..Default::default()
  });
  js_runtime
    .execute(
      "<anon>",
      r#"
    if (!(startup)) {
        throw Error("bad");
      }
      console.log(`ts version: ${ts.version}`);
    "#,
    )
    .unwrap();
}
