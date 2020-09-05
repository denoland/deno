// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

mod op_fetch_asset;

use deno_core::js_check;
use deno_core::BufVec;
use deno_core::CoreIsolate;
use deno_core::Op;
use deno_core::OpId;
use deno_core::OpRegistry;
use deno_core::OpRouter;
use deno_core::OpTable;
use deno_core::StartupData;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryInto;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Default)]
struct BasicState {
  op_table: RefCell<OpTable<Self>>,
}

impl BasicState {
  pub fn new() -> Rc<Self> {
    Default::default()
  }
}

impl OpRegistry for BasicState {
  fn get_op_catalog(self: Rc<Self>) -> HashMap<String, OpId> {
    self.op_table.borrow().get_op_catalog()
  }

  fn register_op<F>(&self, name: &str, op_fn: F) -> OpId
  where
    F: Fn(Rc<Self>, BufVec) -> Op + 'static,
  {
    let mut op_table = self.op_table.borrow_mut();
    let (op_id, removed_op_fn) =
      op_table.insert_full(name.to_owned(), Rc::new(op_fn));
    assert!(removed_op_fn.is_none());
    op_id.try_into().unwrap()
  }
}

impl OpRouter for BasicState {
  fn route_op(self: Rc<Self>, op_id: OpId, bufs: BufVec) -> Op {
    let index = op_id.try_into().unwrap();
    let op_fn = self
      .op_table
      .borrow()
      .get_index(index)
      .map(|(_, op_fn)| op_fn.clone())
      .unwrap();
    (op_fn)(self, bufs)
  }
}

fn create_snapshot(
  mut isolate: CoreIsolate,
  snapshot_path: &Path,
  files: Vec<String>,
) {
  deno_web::init(&mut isolate);
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
  let state = BasicState::new();
  let isolate = CoreIsolate::new(state, StartupData::None, true);
  create_snapshot(isolate, snapshot_path, files);
}

fn create_compiler_snapshot(
  snapshot_path: &Path,
  files: Vec<String>,
  cwd: &Path,
) {
  let mut custom_libs: HashMap<String, PathBuf> = HashMap::new();
  custom_libs
    .insert("lib.deno.web.d.ts".to_string(), deno_web::get_declaration());
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

  let state = BasicState::new();
  state.register_op(
    "op_fetch_asset",
    op_fetch_asset::op_fetch_asset(custom_libs),
  );

  let isolate = CoreIsolate::new(state, StartupData::None, true);
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
