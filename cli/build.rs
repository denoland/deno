// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
mod op_fetch_asset;

use deno_core::js_check;
use deno_core::CoreIsolate;
use deno_core::StartupData;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::path::PathBuf;

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
  let web_scripts = deno_web::get_scripts();
  custom_libs.insert(
    "lib.deno.web.d.ts".to_string(),
    PathBuf::from(web_scripts.declaration),
  );
  custom_libs.insert(
    "lib.deno.window.d.ts".to_string(),
    cwd.join("dts/lib.deno.window.d.ts"),
  );
  custom_libs.insert(
    "lib.deno.worker.d.ts".to_string(),
    cwd.join("dts/lib.deno.worker.d.ts"),
  );
  custom_libs.insert(
    "lib.deno.shared_globals.d.ts".to_string(),
    cwd.join("dts/lib.deno.shared_globals.d.ts"),
  );
  custom_libs.insert(
    "lib.deno.ns.d.ts".to_string(),
    cwd.join("dts/lib.deno.ns.d.ts"),
  );
  custom_libs.insert(
    "lib.deno.unstable.d.ts".to_string(),
    cwd.join("dts/lib.deno.unstable.d.ts"),
  );
  runtime_isolate.register_op(
    "op_fetch_asset",
    op_fetch_asset::op_fetch_asset(custom_libs),
  );
  create_snapshot(runtime_isolate, snapshot_path, files);
}

fn ts_version() -> String {
  // TODO(ry) This should be automatically extracted from typescript.js
  "3.9.7".to_string()
}

fn main() {
  // Don't build V8 if "cargo doc" is being run. This is to support docs.rs.
  if env::var_os("RUSTDOCFLAGS").is_some() {
    return;
  }

  // To debug snapshot issues uncomment:
  // op_fetch_asset::trace_serializer();

  println!("cargo:rustc-env=TS_VERSION={}", ts_version());
  println!(
    "cargo:rustc-env=DENO_WEB_LIB_PATH={}",
    deno_web::get_scripts().declaration
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

  let js_files = get_js_files_for_rt();
  create_runtime_snapshot(&runtime_snapshot_path, js_files);

  let js_files = get_js_files("tsc");
  create_compiler_snapshot(&compiler_snapshot_path, js_files, &c);

  #[cfg(target_os = "windows")]
  {
    let mut res = winres::WindowsResource::new();
    res.set_icon("deno.ico");
    res.set_language(winapi::um::winnt::MAKELANGID(
      winapi::um::winnt::LANG_ENGLISH,
      winapi::um::winnt::SUBLANG_ENGLISH_US,
    ));
    res.compile().unwrap();
  }
}

fn get_js_files(d: &str) -> Vec<String> {
  let mut js_files = std::fs::read_dir(d)
    .unwrap()
    .map(|dir_entry| {
      let file = dir_entry.unwrap();
      file.path().to_string_lossy().to_string()
    })
    .filter(|filename| filename.ends_with(".js"))
    .collect::<Vec<String>>();
  js_files.sort();
  js_files
}

fn get_js_files_for_rt() -> Vec<String> {
  let web_scripts = deno_web::get_scripts();

  let f = vec![
    "rt/00_bootstrap_namespace.js",
    &web_scripts.dom_exception,
    "rt/01_build.js",
    "rt/01_colors.js",
    "rt/01_errors.js",
    &web_scripts.event,
    "rt/01_internals.js",
    "rt/01_version.js",
    "rt/01_web_util.js",
    "rt/02_abort_signal.js",
    "rt/02_console.js",
    "rt/03_dom_iterable.js",
    "rt/06_util.js",
    &web_scripts.text_encoding,
    "rt/10_dispatch_json.js",
    "rt/10_dispatch_minimal.js",
    "rt/11_crypto.js",
    "rt/11_resources.js",
    "rt/11_streams.js",
    "rt/11_timers.js",
    "rt/11_url.js",
    "rt/11_workers.js",
    "rt/12_io.js",
    "rt/13_buffer.js",
    "rt/20_blob.js",
    "rt/20_headers.js",
    "rt/20_streams_queuing_strategy.js",
    "rt/21_dom_file.js",
    "rt/21_filereader.js",
    "rt/22_form_data.js",
    "rt/23_multipart.js",
    "rt/24_body.js",
    "rt/25_request.js",
    "rt/26_fetch.js",
    "rt/30_files.js",
    "rt/30_fs.js",
    "rt/30_metrics.js",
    "rt/30_net.js",
    "rt/30_os.js",
    "rt/40_compiler_api.js",
    "rt/40_diagnostics.js",
    "rt/40_error_stack.js",
    "rt/40_fs_events.js",
    "rt/40_net_unstable.js",
    "rt/40_performance.js",
    "rt/40_permissions.js",
    "rt/40_plugins.js",
    "rt/40_process.js",
    "rt/40_read_file.js",
    "rt/40_repl.js",
    "rt/40_signals.js",
    "rt/40_testing.js",
    "rt/40_tls.js",
    "rt/40_tty.js",
    "rt/40_write_file.js",
    "rt/90_deno_ns.js",
    "rt/99_main.js",
  ];
  f.iter().map(|p| p.to_string()).collect()
}
