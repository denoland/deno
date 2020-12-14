// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::build_util::create_snapshot;
use deno_core::build_util::get_js_files;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use std::env;
use std::path::PathBuf;

fn main() {
  // Skip building from docs.rs.
  if env::var_os("DOCS_RS").is_some() {
    return;
  }

  // To debug snapshot issues uncomment:
  // op_fetch_asset::trace_serializer();

  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
  println!("cargo:rustc-env=PROFILE={}", env::var("PROFILE").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  // Main snapshot
  let runtime_snapshot_path = o.join("CLI_SNAPSHOT.bin");

  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let js_files = get_js_files(&c.join("rt"));

  let mut js_runtime = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    ..Default::default()
  });
  deno_web::init(&mut js_runtime);
  deno_fetch::init(&mut js_runtime);
  deno_crypto::init(&mut js_runtime);
  create_snapshot(js_runtime, &runtime_snapshot_path, js_files);
}
