// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#![deny(warnings)]

extern crate deno_core;
extern crate serde;
extern crate serde_json;

mod ops;
use deno_core::js_check;
pub use deno_core::v8_set_flags;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use deno_core::Op;
use deno_core::StartupData;
use deno_core::ZeroCopyBuf;
pub use ops::EmitResult;
use ops::WrittenFile;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

static TYPESCRIPT_CODE: &str = include_str!("typescript/lib/typescript.js");
static COMPILER_CODE: &str = include_str!("compiler_main.js");
static SYSTEM_LOADER: &str = include_str!("system_loader.js");

pub fn ts_version() -> String {
  let data = include_str!("typescript/package.json");
  let pkg: serde_json::Value = serde_json::from_str(data).unwrap();
  pkg["version"].as_str().unwrap().to_string()
}

type ExternCrateModules = HashMap<String, String>;

#[derive(Debug)]
pub struct TSState {
  bundle: bool,
  exit_code: i32,
  emit_result: Option<EmitResult>,
  /// A list of files emitted by typescript. WrittenFile is tuple of the form
  /// (url, corresponding_module, source_code)
  written_files: Vec<WrittenFile>,
  extern_crate_modules: ExternCrateModules,
}

fn compiler_op<D>(
  ts_state: Arc<Mutex<TSState>>,
  dispatcher: D,
) -> impl Fn(&mut CoreIsolateState, &[u8], &mut [ZeroCopyBuf]) -> Op
where
  D: Fn(&mut TSState, &[u8]) -> Op,
{
  move |_state: &mut CoreIsolateState,
        control: &[u8],
        zero_copy_bufs: &mut [ZeroCopyBuf]|
        -> Op {
    assert!(zero_copy_bufs.is_empty()); // zero_copy_bufs unused in compiler.
    let mut s = ts_state.lock().unwrap();
    dispatcher(&mut s, control)
  }
}

pub struct TSIsolate {
  isolate: CoreIsolate,
  state: Arc<Mutex<TSState>>,
}

impl TSIsolate {
  fn new(
    bundle: bool,
    maybe_extern_crate_modules: Option<ExternCrateModules>,
  ) -> TSIsolate {
    let mut isolate = CoreIsolate::new(StartupData::None, false);
    js_check(isolate.execute("assets/typescript.js", TYPESCRIPT_CODE));
    js_check(isolate.execute("compiler_main.js", COMPILER_CODE));

    let extern_crate_modules = maybe_extern_crate_modules.unwrap_or_default();

    let state = Arc::new(Mutex::new(TSState {
      bundle,
      exit_code: 0,
      emit_result: None,
      written_files: Vec::new(),
      extern_crate_modules,
    }));

    isolate.register_op(
      "op_load_module",
      compiler_op(state.clone(), ops::json_op(ops::op_load_module)),
    );
    isolate.register_op(
      "op_exit2",
      compiler_op(state.clone(), ops::json_op(ops::op_exit2)),
    );
    isolate.register_op(
      "op_write_file",
      compiler_op(state.clone(), ops::json_op(ops::op_write_file)),
    );
    isolate.register_op(
      "op_resolve_module_names",
      compiler_op(state.clone(), ops::json_op(ops::op_resolve_module_names)),
    );
    isolate.register_op(
      "op_set_emit_result",
      compiler_op(state.clone(), ops::json_op(ops::op_set_emit_result)),
    );

    TSIsolate { isolate, state }
  }

  // TODO(ry) Instead of Result<Arc<Mutex<TSState>>, ErrBox>, return something
  // like Result<TSState, ErrBox>. I think it would be nicer if this function
  // consumes TSIsolate.
  /// Compiles each module to ESM. Doesn't write any files to disk.
  /// Passes all output via state.
  fn compile(
    mut self,
    config_json: &serde_json::Value,
    root_names: Vec<String>,
  ) -> Result<Arc<Mutex<TSState>>, ErrBox> {
    let root_names_json = serde_json::json!(root_names).to_string();
    let source =
      &format!("main({:?}, {})", config_json.to_string(), root_names_json);
    self.isolate.execute("<anon>", source)?;
    Ok(self.state)
  }
}

/// Compile provided roots into a single JS bundle.
///
/// This function writes compiled bundle to disk at provided path.
///
/// Source map file and type declaration file are emitted
/// alongside the bundle.
///
/// To instantiate bundle use returned `module_name`.
pub fn compile_bundle(
  bundle_filename: &Path,
  root_names: Vec<PathBuf>,
  extern_crate_modules: Option<ExternCrateModules>,
) -> Result<String, ErrBox> {
  let ts_isolate = TSIsolate::new(true, extern_crate_modules);

  let config_json = serde_json::json!({
    "compilerOptions": {
      "declaration": true,
      // Emit the source alongside the sourcemaps within a single file;
      // requires --inlineSourceMap or --sourceMap to be set.
      // "inlineSources": true,
      "lib": ["esnext"],
      "listEmittedFiles": true,
      "listFiles": true,
      "module": "system",
      "outFile": bundle_filename,
      "removeComments": true,
      "sourceMap": true,
      "strict": true,
      "target": "esnext",
      "typeRoots" : ["$typeRoots$"],
    },
  });

  let root_names_str: Vec<String> = root_names
    .iter()
    .map(|p| {
      if !p.exists() {
        panic!("File not found {}", p.display());
      }

      let module_specifier =
        ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
      module_specifier.as_str().to_string()
    })
    .collect();

  // TODO lift js_check to caller?
  let locked_state = js_check(ts_isolate.compile(&config_json, root_names_str));
  let state = locked_state.lock().unwrap();
  // Assuming that TypeScript has emitted the main file last.
  let main = state.written_files.last().unwrap();
  let module_name = main.module_name.clone();
  Ok(module_name)
}

#[allow(dead_code)]
fn print_source_code(code: &str) {
  let mut i = 1;
  for line in code.lines() {
    println!("{:3}  {}", i, line);
    i += 1;
  }
}

/// Create a V8 snapshot.
pub fn mksnapshot_bundle(
  isolate: &mut CoreIsolate,
  snapshot_filename: &Path,
  bundle_filename: &Path,
  main_module_name: &str,
) -> Result<(), ErrBox> {
  js_check(isolate.execute("system_loader.js", SYSTEM_LOADER));
  let source_code_vec = std::fs::read(bundle_filename).unwrap();
  let bundle_source_code = std::str::from_utf8(&source_code_vec).unwrap();
  js_check(
    isolate.execute(&bundle_filename.to_string_lossy(), bundle_source_code),
  );
  let script = &format!("__instantiate(\"{}\");", main_module_name);
  js_check(isolate.execute("anon", script));
  write_snapshot(isolate, snapshot_filename)?;
  Ok(())
}

/// Create a V8 snapshot. This differs from mksnapshot_bundle in that is also
/// runs typescript.js
pub fn mksnapshot_bundle_ts(
  isolate: &mut CoreIsolate,
  snapshot_filename: &Path,
  bundle_filename: &Path,
  main_module_name: &str,
) -> Result<(), ErrBox> {
  js_check(isolate.execute("typescript.js", TYPESCRIPT_CODE));
  mksnapshot_bundle(
    isolate,
    snapshot_filename,
    bundle_filename,
    main_module_name,
  )
}

fn write_snapshot(
  runtime_isolate: &mut CoreIsolate,
  snapshot_filename: &Path,
) -> Result<(), ErrBox> {
  println!("Creating snapshot...");
  let snapshot = runtime_isolate.snapshot();
  let snapshot_slice: &[u8] = &*snapshot;
  println!("Snapshot size: {}", snapshot_slice.len());
  fs::write(&snapshot_filename, snapshot_slice)?;
  println!("Snapshot written to: {} ", snapshot_filename.display());
  Ok(())
}

pub fn get_asset(name: &str) -> Option<&'static str> {
  macro_rules! inc {
    ($e:expr) => {
      Some(include_str!(concat!("typescript/lib/", $e)))
    };
  }
  match name {
    "system_loader.js" => Some(include_str!("system_loader.js")),
    "bootstrap.ts" => Some("console.log(\"hello deno\");"),
    "typescript.d.ts" => inc!("typescript.d.ts"),
    "lib.dom.d.ts" => inc!("lib.dom.d.ts"),
    "lib.dom.iterable.d.ts" => inc!("lib.dom.iterable.d.ts"),
    "lib.es5.d.ts" => inc!("lib.es5.d.ts"),
    "lib.es6.d.ts" => inc!("lib.es6.d.ts"),
    "lib.esnext.d.ts" => inc!("lib.esnext.d.ts"),
    "lib.es2020.d.ts" => inc!("lib.es2020.d.ts"),
    "lib.es2020.full.d.ts" => inc!("lib.es2020.full.d.ts"),
    "lib.es2019.d.ts" => inc!("lib.es2019.d.ts"),
    "lib.es2019.full.d.ts" => inc!("lib.es2019.full.d.ts"),
    "lib.es2018.d.ts" => inc!("lib.es2018.d.ts"),
    "lib.es2018.full.d.ts" => inc!("lib.es2018.full.d.ts"),
    "lib.es2017.d.ts" => inc!("lib.es2017.d.ts"),
    "lib.es2017.full.d.ts" => inc!("lib.es2017.full.d.ts"),
    "lib.es2016.d.ts" => inc!("lib.es2016.d.ts"),
    "lib.es2016.full.d.ts" => inc!("lib.es2016.full.d.ts"),
    "lib.es2015.d.ts" => inc!("lib.es2015.d.ts"),
    "lib.es2015.collection.d.ts" => inc!("lib.es2015.collection.d.ts"),
    "lib.es2015.core.d.ts" => inc!("lib.es2015.core.d.ts"),
    "lib.es2015.generator.d.ts" => inc!("lib.es2015.generator.d.ts"),
    "lib.es2015.iterable.d.ts" => inc!("lib.es2015.iterable.d.ts"),
    "lib.es2015.promise.d.ts" => inc!("lib.es2015.promise.d.ts"),
    "lib.es2015.proxy.d.ts" => inc!("lib.es2015.proxy.d.ts"),
    "lib.es2015.reflect.d.ts" => inc!("lib.es2015.reflect.d.ts"),
    "lib.es2015.symbol.d.ts" => inc!("lib.es2015.symbol.d.ts"),
    "lib.es2015.symbol.wellknown.d.ts" => {
      inc!("lib.es2015.symbol.wellknown.d.ts")
    }
    "lib.es2016.array.include.d.ts" => inc!("lib.es2016.array.include.d.ts"),
    "lib.es2017.intl.d.ts" => inc!("lib.es2017.intl.d.ts"),
    "lib.es2017.object.d.ts" => inc!("lib.es2017.object.d.ts"),
    "lib.es2017.sharedmemory.d.ts" => inc!("lib.es2017.sharedmemory.d.ts"),
    "lib.es2017.string.d.ts" => inc!("lib.es2017.string.d.ts"),
    "lib.es2017.typedarrays.d.ts" => inc!("lib.es2017.typedarrays.d.ts"),
    "lib.es2018.asyncgenerator.d.ts" => inc!("lib.es2018.asyncgenerator.d.ts"),
    "lib.es2018.asynciterable.d.ts" => inc!("lib.es2018.asynciterable.d.ts"),
    "lib.es2018.intl.d.ts" => inc!("lib.es2018.intl.d.ts"),
    "lib.es2018.promise.d.ts" => inc!("lib.es2018.promise.d.ts"),
    "lib.es2018.regexp.d.ts" => inc!("lib.es2018.regexp.d.ts"),
    "lib.es2019.array.d.ts" => inc!("lib.es2019.array.d.ts"),
    "lib.es2019.object.d.ts" => inc!("lib.es2019.object.d.ts"),
    "lib.es2019.string.d.ts" => inc!("lib.es2019.string.d.ts"),
    "lib.es2019.symbol.d.ts" => inc!("lib.es2019.symbol.d.ts"),
    "lib.es2020.bigint.d.ts" => inc!("lib.es2020.bigint.d.ts"),
    "lib.es2020.promise.d.ts" => inc!("lib.es2020.promise.d.ts"),
    "lib.es2020.string.d.ts" => inc!("lib.es2020.string.d.ts"),
    "lib.es2020.symbol.wellknown.d.ts" => {
      inc!("lib.es2020.symbol.wellknown.d.ts")
    }
    "lib.esnext.array.d.ts" => inc!("lib.esnext.array.d.ts"),
    "lib.esnext.asynciterable.d.ts" => inc!("lib.esnext.asynciterable.d.ts"),
    "lib.esnext.bigint.d.ts" => inc!("lib.esnext.bigint.d.ts"),
    "lib.esnext.intl.d.ts" => inc!("lib.esnext.intl.d.ts"),
    "lib.esnext.promise.d.ts" => inc!("lib.esnext.promise.d.ts"),
    "lib.esnext.string.d.ts" => inc!("lib.esnext.string.d.ts"),
    "lib.esnext.symbol.d.ts" => inc!("lib.esnext.symbol.d.ts"),
    "lib.scripthost.d.ts" => inc!("lib.scripthost.d.ts"),
    "lib.webworker.d.ts" => inc!("lib.webworker.d.ts"),
    "lib.webworker.importscripts.d.ts" => {
      inc!("lib.webworker.importscripts.d.ts")
    }
    _ => None,
  }
}

/// Sets the --trace-serializer V8 flag for debugging snapshots.
pub fn trace_serializer() {
  let dummy = "foo".to_string();
  let r = deno_core::v8_set_flags(vec![
    dummy.clone(),
    "--trace-serializer".to_string(),
  ]);
  assert_eq!(r, vec![dummy]);
}

/// Warning: Returns a non-JSON op dispatcher. Must be manually attached to
/// CoreIsolate.
pub fn op_fetch_asset<S: ::std::hash::BuildHasher>(
  custom_assets: HashMap<String, PathBuf, S>,
) -> impl Fn(&mut CoreIsolateState, &[u8], &mut [ZeroCopyBuf]) -> Op {
  for (_, path) in custom_assets.iter() {
    println!("cargo:rerun-if-changed={}", path.display());
  }
  move |_state: &mut CoreIsolateState,
        control: &[u8],
        zero_copy_bufs: &mut [ZeroCopyBuf]|
        -> Op {
    assert!(zero_copy_bufs.is_empty()); // zero_copy_bufs unused in this op.
    let name = std::str::from_utf8(control).unwrap();

    let asset_code = if let Some(source_code) = get_asset(name) {
      source_code.to_string()
    } else if let Some(asset_path) = custom_assets.get(name) {
      let source_code_vec =
        std::fs::read(&asset_path).expect("Asset not found");
      let source_code = std::str::from_utf8(&source_code_vec).unwrap();
      source_code.to_string()
    } else {
      panic!("fetch_asset bad asset {}", name)
    };

    let vec = asset_code.into_bytes();
    deno_core::Op::Sync(vec.into_boxed_slice())
  }
}
