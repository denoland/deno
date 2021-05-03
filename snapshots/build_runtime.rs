// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use std::env;
use std::path::Path;
use std::path::PathBuf;

// extensions/
use deno_runtime::deno_console;
use deno_runtime::deno_crypto;
use deno_runtime::deno_fetch;
use deno_runtime::deno_file;
use deno_runtime::deno_timers;
use deno_runtime::deno_url;
use deno_runtime::deno_web;
use deno_runtime::deno_webgpu;
use deno_runtime::deno_webidl;
use deno_runtime::deno_websocket;

// TODO(bartlomieju): this module contains a lot of duplicated
// logic with `cli/build.rs`, factor out to `deno_core`.
fn create_snapshot(
  mut js_runtime: JsRuntime,
  snapshot_path: &Path,
  js_deps: Vec<PathBuf>,
) {
  // TODO(nayeemrmn): https://github.com/rust-lang/cargo/issues/3946 to get the
  // workspace root.
  for file in js_deps {
    println!("cargo:rerun-if-changed={}", file.display());
  }

  let snapshot = js_runtime.snapshot();
  let snapshot_slice: &[u8] = &*snapshot;
  println!("Snapshot size: {}", snapshot_slice.len());
  std::fs::write(&snapshot_path, snapshot_slice).unwrap();
  println!("Snapshot written to: {} ", snapshot_path.display());
}

fn create_runtime_snapshot(snapshot_path: &Path) {
  let extensions: Vec<Extension> = vec![
    // Web extensions
    deno_webidl::init(),
    deno_console::init(),
    deno_url::init(),
    deno_web::init(),
    deno_file::init(Default::default(), Default::default()),
    deno_fetch::init::<deno_fetch::NoFetchPermissions>("".to_owned(), None),
    deno_websocket::init::<deno_websocket::NoWebSocketPermissions>(
      "".to_owned(),
      None,
    ),
    deno_crypto::init(None),
    deno_webgpu::init(false),
    deno_timers::init::<deno_timers::NoTimersPermission>(),
    // Runtime JS
    deno_runtime::js::init(),
  ];
  let js_deps = get_js_deps(&extensions);

  let js_runtime = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    extensions,
    ..Default::default()
  });

  create_snapshot(js_runtime, snapshot_path, js_deps);
}

pub fn main() {
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

  create_runtime_snapshot(&runtime_snapshot_path);
}

fn get_js_deps(extensions: &[Extension]) -> Vec<PathBuf> {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let root = manifest_dir.join("..").canonicalize().unwrap();

  let mut js_files = extensions
    .iter()
    .map(|e| e.init_js())
    .flatten()
    .map(|(path, _src)| {
      let path = path.strip_prefix("deno:").unwrap();
      root.join(path).canonicalize().unwrap()
    })
    .collect::<Vec<PathBuf>>();
  js_files.sort();
  js_files
}
