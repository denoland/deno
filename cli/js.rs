pub const TS_VERSION: &str = env!("TS_VERSION");

pub static CLI_SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/CLI_SNAPSHOT.bin"));
pub static CLI_SNAPSHOT_MAP: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/CLI_SNAPSHOT.js.map"));
#[allow(dead_code)]
pub static CLI_SNAPSHOT_DTS: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/CLI_SNAPSHOT.d.ts"));

pub static COMPILER_SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/COMPILER_SNAPSHOT.bin"));
pub static COMPILER_SNAPSHOT_MAP: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/COMPILER_SNAPSHOT.js.map"));
#[allow(dead_code)]
pub static COMPILER_SNAPSHOT_DTS: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/COMPILER_SNAPSHOT.d.ts"));

pub static DENO_NS_LIB: &str = include_str!("js/lib.deno.ns.d.ts");
pub static SHARED_GLOBALS_LIB: &str =
  include_str!("js/lib.deno.shared_globals.d.ts");
pub static WINDOW_LIB: &str = include_str!("js/lib.deno.window.d.ts");
pub static UNSTABLE_NS_LIB: &str = include_str!("js/lib.deno.unstable.d.ts");

#[test]
fn cli_snapshot() {
  let mut isolate = deno_core::CoreIsolate::new(
    deno_core::StartupData::Snapshot(deno_core::Snapshot::Static(CLI_SNAPSHOT)),
    false,
  );
  deno_core::js_check(isolate.execute(
    "<anon>",
    r#"
      if (!(bootstrap.mainRuntime && bootstrap.workerRuntime)) {
        throw Error("bad");
      }
      console.log("we have console.log!!!");
    "#,
  ));
}

#[test]
fn compiler_snapshot() {
  let mut isolate = deno_core::CoreIsolate::new(
    deno_core::StartupData::Snapshot(deno_core::Snapshot::Static(
      COMPILER_SNAPSHOT,
    )),
    false,
  );
  deno_core::js_check(isolate.execute(
    "<anon>",
    r#"
    if (!(bootstrap.tsCompilerRuntime)) {
        throw Error("bad");
      }
      console.log(`ts version: ${ts.version}`);
    "#,
  ));
}
