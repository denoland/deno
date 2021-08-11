// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::env;
use std::path::Path;
use std::path::PathBuf;

// This is a shim that allows to generate documentation on docs.rs
#[cfg(not(feature = "docsrs"))]
mod not_docs {
  use super::*;
  use deno_core::Extension;
  use deno_core::JsRuntime;
  use deno_core::RuntimeOptions;

  // TODO(bartlomieju): this module contains a lot of duplicated
  // logic with `cli/build.rs`, factor out to `deno_core`.
  fn create_snapshot(
    mut js_runtime: JsRuntime,
    snapshot_path: &Path,
    files: Vec<PathBuf>,
  ) {
    // TODO(nayeemrmn): https://github.com/rust-lang/cargo/issues/3946 to get the
    // workspace root.
    let display_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for file in files {
      println!("cargo:rerun-if-changed={}", file.display());
      let display_path = file.strip_prefix(display_root).unwrap();
      let display_path_str = display_path.display().to_string();
      js_runtime
        .execute_script(
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
      deno_tls::init(),
      deno_web::init(deno_web::BlobStore::default(), Default::default()),
      deno_fetch::init::<deno_fetch::NoFetchPermissions>(
        "".to_owned(),
        None,
        None,
        None,
        None,
      ),
      deno_websocket::init::<deno_websocket::NoWebSocketPermissions>(
        "".to_owned(),
        None,
        None,
      ),
      deno_webstorage::init(None),
      deno_crypto::init(None),
      deno_webgpu::init(false),
      deno_timers::init::<deno_timers::NoTimersPermission>(),
      deno_broadcast_channel::init(
        deno_broadcast_channel::InMemoryBroadcastChannel::default(),
        false, // No --unstable.
      ),
      deno_ffi::init::<deno_ffi::NoFfiPermissions>(false),
      deno_net::init::<deno_net::NoNetPermissions>(
        None, false, // No --unstable.
        None,
      ),
      deno_http::init(),
    ];

    let js_runtime = JsRuntime::new(RuntimeOptions {
      will_snapshot: true,
      extensions,
      ..Default::default()
    });
    create_snapshot(js_runtime, snapshot_path, files);
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

  pub fn build_snapshot(runtime_snapshot_path: PathBuf) {
    let js_files = get_js_files("js");
    create_runtime_snapshot(&runtime_snapshot_path, js_files);
  }
}

fn main() {
  // To debug snapshot issues uncomment:
  // op_fetch_asset::trace_serializer();

  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
  println!("cargo:rustc-env=PROFILE={}", env::var("PROFILE").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  // Main snapshot
  let runtime_snapshot_path = o.join("CLI_SNAPSHOT.bin");

  // If we're building on docs.rs we just create
  // and empty snapshot file and return, because `rusty_v8`
  // doesn't actually compile on docs.rs
  if env::var_os("DOCS_RS").is_some() {
    let snapshot_slice = &[];
    std::fs::write(&runtime_snapshot_path, snapshot_slice).unwrap();
    return;
  }

  #[cfg(not(feature = "docsrs"))]
  not_docs::build_snapshot(runtime_snapshot_path)
}
