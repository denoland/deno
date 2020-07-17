// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![allow(unused)]

use deno_core::include_crate_modules;
use deno_core::js_check;
use deno_core::CoreIsolate;
use deno_core::StartupData;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
extern crate winapi;
#[cfg(target_os = "windows")]
extern crate winres;

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

  let extern_crate_modules = include_crate_modules![deno_core];

  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  // Main snapshot
  let snapshot_path = o.join("CLI_SNAPSHOT.bin");
  let compiler_snapshot_path = o.join("COMPILER_SNAPSHOT.bin");

  let mut runtime_isolate = CoreIsolate::new(StartupData::None, true);

  let mut sorted_files = std::fs::read_dir("js2/")
    .unwrap()
    .map(|dir_entry| {
      let file = dir_entry.unwrap();
      let filename = file.path().to_string_lossy().to_string();
      filename
    })
    .filter(|filename| {
      filename.ends_with(".js") && !filename.ends_with("compiler.js")
    })
    .collect::<Vec<String>>();

  sorted_files.sort();

  eprintln!("sorted files {:#?}", sorted_files);
  for file in sorted_files {
    println!("cargo:rerun-if-changed={}", file);
    js_check(
      runtime_isolate.execute(&file, &std::fs::read_to_string(&file).unwrap()),
    );
  }

  let snapshot = runtime_isolate.snapshot();
  let snapshot_slice: &[u8] = &*snapshot;
  println!("Snapshot size: {}", snapshot_slice.len());
  std::fs::write(&snapshot_path, snapshot_slice).unwrap();
  println!("Snapshot written to: {} ", snapshot_path.display());

  let mut runtime_isolate = CoreIsolate::new(StartupData::None, true);

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
  custom_libs.insert(
    "lib.deno.unstable.d.ts".to_string(),
    c.join("js/lib.deno.unstable.d.ts"),
  );
  runtime_isolate.register_op(
    "op_fetch_asset",
    deno_typescript::op_fetch_asset(custom_libs),
  );

  js_check(
    runtime_isolate.execute("typescript.js", deno_typescript::TYPESCRIPT_CODE),
  );

  let mut sorted_files = std::fs::read_dir("js2/")
    .unwrap()
    .map(|dir_entry| {
      let file = dir_entry.unwrap();
      let filename = file.path().to_string_lossy().to_string();
      filename
    })
    .filter(|filename| filename.ends_with(".js"))
    .collect::<Vec<String>>();

  sorted_files.sort();

  eprintln!("sorted files {:#?}", sorted_files);
  for file in sorted_files {
    println!("cargo:rerun-if-changed={}", file);
    js_check(
      runtime_isolate.execute(&file, &std::fs::read_to_string(&file).unwrap()),
    );
  }

  let snapshot = runtime_isolate.snapshot();
  let snapshot_slice: &[u8] = &*snapshot;
  println!("Snapshot size: {}", snapshot_slice.len());
  std::fs::write(&compiler_snapshot_path, snapshot_slice).unwrap();
  println!("Snapshot written to: {} ", compiler_snapshot_path.display());
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
