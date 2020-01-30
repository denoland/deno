// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use deno_core::CoreOp;
use deno_core::Isolate;
use deno_core::Op;
use deno_core::StartupData;
use deno_core::ZeroCopyBuf;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

fn op_fetch_asset(
  custom_assets: HashMap<String, PathBuf>,
) -> impl Fn(&[u8], Option<ZeroCopyBuf>) -> CoreOp {
  move |control: &[u8], zero_copy_buf: Option<ZeroCopyBuf>| -> CoreOp {
    assert!(zero_copy_buf.is_none()); // zero_copy_buf unused in this op.
    let custom_assets = custom_assets.clone();
    let name = std::str::from_utf8(control).unwrap();

    let asset_code = if let Some(source_code) = deno_typescript::get_asset(name)
    {
      source_code.to_string()
    } else if let Some(asset_path) = custom_assets.get(name) {
      let source_code_vec =
        std::fs::read(&asset_path).expect("Asset not found");
      let source_code = std::str::from_utf8(&source_code_vec).unwrap();
      source_code.to_string()
    } else {
      panic!("op_fetch_asset bad asset {}", name)
    };

    let vec = asset_code.into_bytes();
    Op::Sync(vec.into_boxed_slice())
  }
}

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

  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  // Main snapshot
  let root_names = vec![c.join("js/main.ts")];
  let bundle_path = o.join("CLI_SNAPSHOT.js");
  let snapshot_path = o.join("CLI_SNAPSHOT.bin");

  let main_module_name =
    deno_typescript::compile_bundle(&bundle_path, root_names)
      .expect("Bundle compilation failed");
  assert!(bundle_path.exists());

  let runtime_isolate = &mut Isolate::new(StartupData::None, true);

  deno_typescript::mksnapshot_bundle(
    runtime_isolate,
    &snapshot_path,
    &bundle_path,
    &main_module_name,
  )
  .expect("Failed to create snapshot");

  // Compiler snapshot
  let root_names = vec![c.join("js/compiler.ts")];
  let bundle_path = o.join("COMPILER_SNAPSHOT.js");
  let snapshot_path = o.join("COMPILER_SNAPSHOT.bin");
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

  let main_module_name =
    deno_typescript::compile_bundle(&bundle_path, root_names)
      .expect("Bundle compilation failed");
  assert!(bundle_path.exists());

  let runtime_isolate = &mut Isolate::new(StartupData::None, true);
  runtime_isolate.register_op("fetch_asset", op_fetch_asset(custom_libs));

  deno_typescript::mksnapshot_bundle_ts(
    runtime_isolate,
    &snapshot_path,
    &bundle_path,
    &main_module_name,
  )
  .expect("Failed to create snapshot");
}
