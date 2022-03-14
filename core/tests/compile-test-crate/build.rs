use deno_core::{JsRuntime, RuntimeOptions};
use std::path::PathBuf;

fn main() {
  if std::env::var_os("CARGO_FEATURE_SNAPSHOT").is_some() {
    let snapshot_path = {
      let mut path = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
      path.push("SNAPSHOT.bin");
      path
    };

    let mut js_runtime = JsRuntime::new(RuntimeOptions {
      will_snapshot: true,
      ..Default::default()
    });

    std::fs::write(snapshot_path, js_runtime.snapshot()).unwrap();
  }
}
