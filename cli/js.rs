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

static DENO_RUNTIME: &str = include_str!("js/lib.deno_runtime.d.ts");

/// Same as deno_typescript::get_asset but also has lib.deno_runtime.d.ts
pub fn get_asset(name: &str) -> Option<&'static str> {
  match name {
    "lib.deno_runtime.d.ts" => Some(DENO_RUNTIME),
    _ => deno_typescript::get_asset(name),
  }
}

#[test]
fn cli_snapshot() {
  let mut isolate = deno_core::Isolate::new(
    deno_core::StartupData::Snapshot(CLI_SNAPSHOT),
    false,
  );
  deno_core::js_check(isolate.execute(
    "<anon>",
    r#"
      if (!window) {
        throw Error("bad");
      }
      console.log("we have console.log!!!");
    "#,
  ));
}

#[test]
fn compiler_snapshot() {
  let mut isolate = deno_core::Isolate::new(
    deno_core::StartupData::Snapshot(COMPILER_SNAPSHOT),
    false,
  );
  deno_core::js_check(isolate.execute(
    "<anon>",
    r#"
      if (!bootstrapTsCompiler) {
        throw Error("bad");
      }
      console.log(`ts version: ${ts.version}`);
    "#,
  ));
}
