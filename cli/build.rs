// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use deno_core::js_check;
use deno_core::CoreOp;
use deno_core::ErrBox;
use deno_core::Isolate;
use deno_core::Op;
use deno_core::PinnedBuf;
use deno_core::StartupData;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize)]
struct FetchAssetArgs {
  name: String,
}

fn op_fetch_asset(
  custom_assets: Vec<(String, PathBuf)>,
  control: &[u8],
) -> CoreOp {
  let result = serde_json::from_slice(control)
    .map_err(ErrBox::from)
    .and_then(move |args| inner_fetch_asset(custom_assets, args));

  let response = match result {
    Ok(v) => json!({ "ok": v }),
    Err(err) => json!({ "err": err.to_string() }),
  };

  let x = serde_json::to_string(&response).unwrap();
  let vec = x.into_bytes();
  Op::Sync(vec.into_boxed_slice())
}

fn inner_fetch_asset(
  custom_assets: Vec<(String, PathBuf)>,
  v: Value,
) -> Result<Value, ErrBox> {
  let args: FetchAssetArgs = serde_json::from_value(v)?;

  if let Some(source_code) = deno_typescript::get_asset(&args.name) {
    return Ok(json!(source_code));
  }

  for (asset_name, asset_path) in custom_assets {
    if asset_name == args.name {
      let source_code_vec = std::fs::read(&asset_path)?;
      let source_code = std::str::from_utf8(&source_code_vec)?;
      return Ok(json!(source_code));
    }
  }

  panic!("op_fetch_asset bad asset {}", args.name)
}

fn make_op_fetch_asset<D>(
  custom_assets: Vec<(String, PathBuf)>,
  dispatcher: D,
) -> impl Fn(&[u8], Option<PinnedBuf>) -> CoreOp
where
  D: Fn(Vec<(String, PathBuf)>, &[u8]) -> CoreOp,
{
  move |control: &[u8], zero_copy_buf: Option<PinnedBuf>| -> CoreOp {
    assert!(zero_copy_buf.is_none()); // zero_copy_buf unused in this op.
    dispatcher(custom_assets.clone(), control)
  }
}

fn create_main_snapshot() {
  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  let root_names = vec![c.join("js/main.ts")];
  let bundle_path = o.join("CLI_SNAPSHOT.js");
  let snapshot_path = o.join("CLI_SNAPSHOT.bin");

  let compiled_files =
    deno_typescript::compile_bundle(&bundle_path, root_names)
      .expect("Bundle compilation failed");
  // bundle, source map and type declarations
  assert_eq!(compiled_files.len(), 3);

  let mut bundle_file = None;
  for compiled_file in compiled_files {
    std::fs::write(&compiled_file.file_name, &compiled_file.source_code)
      .expect("Failed to write bundle file to disk");
    if compiled_file.url.ends_with(".js") {
      bundle_file = Some(compiled_file)
    }
  }
  assert!(bundle_path.exists());
  assert!(bundle_file.is_some());
  let bundle_file = bundle_file.unwrap();

  let runtime_isolate = &mut Isolate::new(StartupData::None, true);
  let source_code_vec = std::fs::read(&bundle_path).unwrap();
  let source_code = std::str::from_utf8(&source_code_vec).unwrap();

  js_check(
    runtime_isolate.execute("bundle_loader.js", deno_typescript::BUNDLE_LOADER),
  );
  js_check(
    runtime_isolate.execute(&bundle_path.to_string_lossy(), &source_code),
  );

  let script = &format!("instantiate('{}')", bundle_file.module_name);
  js_check(runtime_isolate.execute("anon", script));

  println!("Creating main snapshot...");
  let snapshot = runtime_isolate
    .snapshot()
    .expect("Failed to take V8 snapshot");
  let snapshot_slice: &[u8] = &*snapshot;
  println!("Snapshot size: {}", snapshot_slice.len());
  fs::write(&snapshot_path, snapshot_slice)
    .expect("Failed to write snapshot file");
  println!("Snapshot written to: {} ", snapshot_path.display());
}

fn create_compiler_snapshot() {
  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  let root_names = vec![c.join("js/compiler.ts")];
  let bundle_path = o.join("COMPILER_SNAPSHOT.js");
  let snapshot_path = o.join("COMPILER_SNAPSHOT.bin");
  let custom_libs = vec![(
    "lib.deno_runtime.d.ts".to_string(),
    c.join("js/lib.deno_runtime.d.ts"),
  )];

  let compiled_files =
    deno_typescript::compile_bundle(&bundle_path, root_names)
      .expect("Bundle compilation failed");
  // bundle, source map and type declarations
  assert_eq!(compiled_files.len(), 3);

  let mut bundle_file = None;
  for compiled_file in compiled_files {
    std::fs::write(&compiled_file.file_name, &compiled_file.source_code)
      .expect("Failed to write bundle file to disk");
    if compiled_file.url.ends_with(".js") {
      bundle_file = Some(compiled_file)
    }
  }
  assert!(bundle_path.exists());
  assert!(bundle_file.is_some());
  let bundle_file = bundle_file.unwrap();

  let runtime_isolate = &mut Isolate::new(StartupData::None, true);
  runtime_isolate.register_op(
    "fetch_asset",
    make_op_fetch_asset(custom_libs, op_fetch_asset),
  );
  let source_code_vec = std::fs::read(&bundle_path).unwrap();
  let source_code = std::str::from_utf8(&source_code_vec).unwrap();

  js_check(
    runtime_isolate.execute("bundle_loader.js", deno_typescript::BUNDLE_LOADER),
  );
  js_check(
    runtime_isolate.execute("typescript.js", deno_typescript::TYPESCRIPT_CODE),
  );
  js_check(
    runtime_isolate.execute(&bundle_path.to_string_lossy(), &source_code),
  );

  let script = &format!("instantiate('{}')", bundle_file.module_name);
  js_check(runtime_isolate.execute("anon", script));

  println!("Creating main snapshot...");
  let snapshot = runtime_isolate
    .snapshot()
    .expect("Failed to take V8 snapshot");
  let snapshot_slice: &[u8] = &*snapshot;
  println!("Snapshot size: {}", snapshot_slice.len());
  fs::write(&snapshot_path, snapshot_slice)
    .expect("Failed to write snapshot file");
  println!("Snapshot written to: {} ", snapshot_path.display());
}

fn main() {
  // To debug snapshot issues uncomment:
  // deno_typescript::trace_serializer();

  println!(
    "cargo:rustc-env=TS_VERSION={}",
    deno_typescript::ts_version()
  );

  create_main_snapshot();
  create_compiler_snapshot();
}
