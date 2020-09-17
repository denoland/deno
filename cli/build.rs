// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

mod op_fetch_asset;

use deno_core::js_check;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::path::PathBuf;

fn create_snapshot(
  mut isolate: JsRuntime,
  snapshot_path: &Path,
  files: Vec<PathBuf>,
) {
  deno_web::init(&mut isolate);
  deno_fetch::init(&mut isolate);
  // TODO(nayeemrmn): https://github.com/rust-lang/cargo/issues/3946 to get the
  // workspace root.
  let display_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
  for file in files {
    println!("cargo:rerun-if-changed={}", file.display());
    let display_path = file.strip_prefix(display_root).unwrap();
    let display_path_str = display_path.display().to_string();
    js_check(isolate.execute(
      &("deno:".to_string() + &display_path_str.replace('\\', "/")),
      &std::fs::read_to_string(&file).unwrap(),
    ));
  }

  let snapshot = isolate.snapshot();
  let snapshot_slice: &[u8] = &*snapshot;
  println!("Snapshot size: {}", snapshot_slice.len());
  std::fs::write(&snapshot_path, snapshot_slice).unwrap();
  println!("Snapshot written to: {} ", snapshot_path.display());
}

fn create_runtime_snapshot(snapshot_path: &Path, files: Vec<PathBuf>) {
  let isolate = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    ..Default::default()
  });
  create_snapshot(isolate, snapshot_path, files);
}

fn create_compiler_snapshot(
  snapshot_path: &Path,
  files: Vec<PathBuf>,
  cwd: &Path,
) {
  let mut custom_libs: HashMap<String, PathBuf> = HashMap::new();
  custom_libs
    .insert("lib.deno.web.d.ts".to_string(), deno_web::get_declaration());
  custom_libs.insert(
    "lib.deno.fetch.d.ts".to_string(),
    deno_fetch::get_declaration(),
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

  let mut isolate = JsRuntime::new(RuntimeOptions {
    will_snapshot: true,
    ..Default::default()
  });
  isolate.register_op(
    "op_fetch_asset",
    op_fetch_asset::op_fetch_asset(custom_libs),
  );
  create_snapshot(isolate, snapshot_path, files);
}

fn ts_version() -> String {
  std::fs::read_to_string("tsc/00_typescript.js")
    .unwrap()
    .lines()
    .find(|l| l.contains("ts.version = "))
    .expect(
      "Failed to find the pattern `ts.version = ` in typescript source code",
    )
    .chars()
    .skip_while(|c| !char::is_numeric(*c))
    .take_while(|c| *c != '"')
    .collect::<String>()
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
    deno_web::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_FETCH_LIB_PATH={}",
    deno_fetch::get_declaration().display()
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

  let js_files = get_js_files("rt");
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
