// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;

use deno_runtime::ops::bootstrap::SnapshotOptions;

fn main() {
  let cache_dir = get_cache_dir();
  std::fs::create_dir_all(&cache_dir).unwrap();

  let snapshot_path = cache_dir.join("CLI_SNAPSHOT.bin");

  let snapshot_options = SnapshotOptions {
    ts_version: deno_snapshots::TS_VERSION.to_string(),
    v8_version: deno_runtime::deno_core::v8::VERSION_STRING,
    target: env!("TARGET").to_string(),
  };

  deno_runtime::snapshot::create_runtime_snapshot(
    snapshot_path,
    snapshot_options,
    vec![],
  );

  eprintln!(
    "Snapshot generated successfully in {}",
    cache_dir.display()
  );
}

fn get_cache_dir() -> PathBuf {
  if let Ok(dir) = std::env::var("SNAPSHOT_CACHE_DIR") {
    return PathBuf::from(dir);
  }
  let exe_path = std::env::current_exe().unwrap();
  // exe is at <target_dir>/<profile>/snapshot_generator
  let profile_dir = exe_path.parent().unwrap();
  profile_dir.join("build").join(".deno_snapshot_cache")
}
