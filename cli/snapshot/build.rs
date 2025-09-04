// Copyright 2018-2025 the Deno authors. MIT license.

#[cfg(not(feature = "disable"))]
mod shared;

fn main() {
  #[cfg(not(feature = "disable"))]
  {
    let o = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let cli_snapshot_path = o.join("CLI_SNAPSHOT.bin");
    create_cli_snapshot(cli_snapshot_path);
  }
}

#[cfg(not(feature = "disable"))]
fn create_cli_snapshot(snapshot_path: std::path::PathBuf) {
  use deno_runtime::ops::bootstrap::SnapshotOptions;

  let snapshot_options = SnapshotOptions {
    ts_version: shared::TS_VERSION.to_string(),
    v8_version: deno_runtime::deno_core::v8::VERSION_STRING,
    target: std::env::var("TARGET").unwrap(),
  };

  deno_runtime::snapshot::create_runtime_snapshot(
    snapshot_path,
    snapshot_options,
    vec![],
  );
}
