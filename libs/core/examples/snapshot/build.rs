// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::extension;
use deno_core::snapshot::CreateSnapshotOptions;
use deno_core::snapshot::create_snapshot;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
  extension!(
    runjs_extension,
    // Must specify an entrypoint so that our module gets loaded while snapshotting:
    esm_entry_point = "my:runtime",
    esm = [
      dir "src",
      "my:runtime" = "runtime.js",
    ],
  );

  let options = CreateSnapshotOptions {
    cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
    startup_snapshot: None,
    extensions: vec![runjs_extension::init()],
    with_runtime_cb: None,
    skip_op_registration: false,
    extension_transpiler: None,
  };
  let warmup_script = None;

  let snapshot =
    create_snapshot(options, warmup_script).expect("Error creating snapshot");

  // Save the snapshot for use by our source code:
  let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
  let file_path = out_dir.join("RUNJS_SNAPSHOT.bin");
  fs::write(file_path, snapshot.output).expect("Failed to write snapshot");

  // Let cargo know that builds depend on these files:
  for path in snapshot.files_loaded_during_snapshot {
    println!("cargo:rerun-if-changed={}", path.display());
  }
}
