// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use deno_core::Isolate;
use deno_core::StartupData;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

fn main() {
  // Don't build V8 if "cargo doc" is being run. This is to support docs.rs.
  if env::var_os("RUSTDOCFLAGS").is_some() {
    return;
  }

  // To debug snapshot issues uncomment:
  // deno_typescript::trace_serializer();

  println!(
    "cargo:rustc-env=TS_VERSION={}",
    deno_typescript::ts_version()
  );

  // The generation of snapshots is slow and often unnecessary. Until we figure
  // out how to speed it up, or avoid it when unnecessary, this env var provides
  // an escape hatch for the impatient hacker in need of faster incremental
  // builds.
  // USE WITH EXTREME CAUTION
  if env::var_os("NO_BUILD_SNAPSHOTS").is_some() {
    println!("NO_BUILD_SNAPSHOTS is set, skipping snapshot building.");
    return;
  }

  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  // Main snapshot
  let bundle_path = PathBuf::from("rt.js");
  let snapshot_path = o.join("CLI_SNAPSHOT.bin");

  assert!(bundle_path.exists());

  let runtime_isolate = &mut Isolate::new(StartupData::None, true);

  deno_typescript::mksnapshot_bundle(
    runtime_isolate,
    &snapshot_path,
    &bundle_path,
    "cli/js/main.ts",
  )
  .expect("Failed to create snapshot");

  // Compiler snapshot
  let bundle_path = PathBuf::from("tsrt.js");
  let snapshot_path = o.join("COMPILER_SNAPSHOT.bin");

  assert!(bundle_path.exists());

  let runtime_isolate = &mut Isolate::new(StartupData::None, true);

  let mut custom_libs: HashMap<String, PathBuf> = HashMap::new();
  custom_libs.insert(
    "lib.deno.window.d.ts".to_string(),
    c.join("js/lib.deno.window.d.ts"),
  );
  custom_libs.insert(
    "lib.deno.worker.d.ts".to_string(),
    c.join("js/lib.deno.worker.d.ts"),
  );
  custom_libs.insert(
    "lib.deno.shared_globals.d.ts".to_string(),
    c.join("js/lib.deno.shared_globals.d.ts"),
  );
  custom_libs.insert(
    "lib.deno.ns.d.ts".to_string(),
    c.join("js/lib.deno.ns.d.ts"),
  );
  runtime_isolate.register_op(
    "op_fetch_asset",
    deno_typescript::op_fetch_asset(custom_libs),
  );

  deno_typescript::mksnapshot_bundle_ts(
    runtime_isolate,
    &snapshot_path,
    &bundle_path,
    "cli/js/compiler.ts",
  )
  .expect("Failed to create snapshot");
}
