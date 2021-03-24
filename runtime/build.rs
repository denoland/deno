// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use std::env;
use std::path::Path;
use std::path::PathBuf;

// TODO(bartlomieju): this module contains a lot of duplicated
// logic with `cli/build.rs`, factor out to `deno_core`.
fn create_snapshot(
  mut js_runtime: JsRuntime,
  snapshot_path: &Path,
  files: Vec<PathBuf>,
) {
  // Init internal modules JS
  js_runtime.init_mod_js().unwrap();

  // TODO(nayeemrmn): https://github.com/rust-lang/cargo/issues/3946 to get the
  // workspace root.
  let display_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
  for file in files {
    println!("cargo:rerun-if-changed={}", file.display());
    let display_path = file.strip_prefix(display_root).unwrap();
    let display_path_str = display_path.display().to_string();
    js_runtime
      .execute(
        &("deno:".to_string() + &display_path_str.replace('\\', "/")),
        &std::fs::read_to_string(&file).unwrap(),
      )
      .unwrap();
  }

  let snapshot = js_runtime.snapshot();
  let snapshot_slice: &[u8] = &*snapshot;
  println!("Snapshot size: {}", snapshot_slice.len());
  std::fs::write(&snapshot_path, snapshot_slice).unwrap();
  println!("Snapshot written to: {} ", snapshot_path.display());
}

fn create_runtime_snapshot(snapshot_path: &Path, files: Vec<PathBuf>) {
  let extensions: Vec<Extension> = vec![
    deno_webidl::init(),
    deno_console::init(),
    deno_url::init(),
    deno_web::init(),
    deno_fetch::init::<deno_fetch::NoFetchPermissions>("".to_owned(), None),
    deno_websocket::init::<deno_websocket::NoWebSocketPermissions>(
      "".to_owned(),
      None,
    ),
    deno_crypto::init(None),
    deno_webgpu::init(false),
  ];

  let js_runtime = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    extensions,
    ..Default::default()
  });
  create_snapshot(js_runtime, snapshot_path, files);
}

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

  let js_files = get_js_files("js");
  create_runtime_snapshot(&runtime_snapshot_path, js_files);
}

fn get_js_files(d: &str) -> Vec<PathBuf> {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let mut js_files = std::fs::read_dir(d)
    .unwrap()
    .map(|dir_entry| {
      let file = dir_entry.unwrap();
      manifest_dir.join(file.path())
    })
    .filter(|path| path.extension().unwrap_or_default() == "js")
    .collect::<Vec<PathBuf>>();
  js_files.sort();
  js_files
}
