// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::js_check;
use deno_core::CoreIsolate;
use deno_core::StartupData;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
extern crate winapi;
#[cfg(target_os = "windows")]
extern crate winres;

fn create_snapshot(
  mut isolate: CoreIsolate,
  snapshot_path: &Path,
  files: Vec<String>,
) {
  for file in files {
    println!("cargo:rerun-if-changed={}", file);
    js_check(isolate.execute(&file, &std::fs::read_to_string(&file).unwrap()));
  }

  let snapshot = isolate.snapshot();
  let snapshot_slice: &[u8] = &*snapshot;
  println!("Snapshot size: {}", snapshot_slice.len());
  std::fs::write(&snapshot_path, snapshot_slice).unwrap();
  println!("Snapshot written to: {} ", snapshot_path.display());
}

fn create_runtime_snapshot(snapshot_path: &Path, files: Vec<String>) {
  let runtime_isolate = CoreIsolate::new(StartupData::None, true);
  create_snapshot(runtime_isolate, snapshot_path, files);
}

fn create_compiler_snapshot(
  snapshot_path: &Path,
  files: Vec<String>,
  cwd: &Path,
) {
  let mut runtime_isolate = CoreIsolate::new(StartupData::None, true);
  let mut custom_libs: HashMap<String, PathBuf> = HashMap::new();
  custom_libs.insert(
    "lib.deno.window.d.ts".to_string(),
    cwd.join("js2/lib.deno.window.d.ts"),
  );
  custom_libs.insert(
    "lib.deno.worker.d.ts".to_string(),
    cwd.join("js2/lib.deno.worker.d.ts"),
  );
  custom_libs.insert(
    "lib.deno.shared_globals.d.ts".to_string(),
    cwd.join("js2/lib.deno.shared_globals.d.ts"),
  );
  custom_libs.insert(
    "lib.deno.ns.d.ts".to_string(),
    cwd.join("js2/lib.deno.ns.d.ts"),
  );
  custom_libs.insert(
    "lib.deno.unstable.d.ts".to_string(),
    cwd.join("js2/lib.deno.unstable.d.ts"),
  );
  runtime_isolate.register_op(
    "op_fetch_asset",
    deno_typescript::op_fetch_asset(custom_libs),
  );

  js_check(
    runtime_isolate.execute("typescript.js", deno_typescript::TYPESCRIPT_CODE),
  );

  create_snapshot(runtime_isolate, snapshot_path, files);
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

  println!(
    "cargo:rustc-env=TARGET={}",
    std::env::var("TARGET").unwrap()
  );

  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  // Main snapshot
  let runtime_snapshot_path = o.join("CLI_SNAPSHOT.bin");
  let compiler_snapshot_path = o.join("COMPILER_SNAPSHOT.bin");

  let mut js_files = std::fs::read_dir("js2/")
    .unwrap()
    .map(|dir_entry| {
      let file = dir_entry.unwrap();
      file.path().to_string_lossy().to_string()
    })
    .filter(|filename| filename.ends_with(".js"))
    .collect::<Vec<String>>();

  js_files.sort();

  let runtime_files = js_files
    .clone()
    .into_iter()
    .filter(|filepath| !filepath.ends_with("compiler.js"))
    .collect::<Vec<String>>();
  create_runtime_snapshot(&runtime_snapshot_path, runtime_files);
  create_compiler_snapshot(&compiler_snapshot_path, js_files, &c);

  set_binary_metadata();
}

#[cfg(target_os = "windows")]
fn set_binary_metadata() {
  let mut res = winres::WindowsResource::new();
  res.set_icon("deno.ico");
  res.set_language(winapi::um::winnt::MAKELANGID(
    winapi::um::winnt::LANG_ENGLISH,
    winapi::um::winnt::SUBLANG_ENGLISH_US,
  ));
  res.compile().unwrap();
}

#[cfg(not(target_os = "windows"))]
fn set_binary_metadata() {}
