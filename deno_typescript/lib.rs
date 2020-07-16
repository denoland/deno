// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#![deny(warnings)]

extern crate bytecount;
extern crate deno_core;
extern crate regex;
extern crate ring;
extern crate serde;
extern crate serde_json;
extern crate sourcemap;

mod bundle;
mod ops;
pub mod source_map;

use deno_core::js_check;
pub use deno_core::v8_set_flags;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use deno_core::Op;
use deno_core::OpDispatcher;
use deno_core::StartupData;
use deno_core::ZeroCopyBuf;
pub use ops::EmitResult;
use ops::WrittenFile;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::result;
use std::sync::Arc;
use std::sync::Mutex;

type Result<V> = result::Result<V, ErrBox>;

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
  exit_code: i32,
  emit_result: Option<EmitResult>,
  written_files: Vec<WrittenFile>,
  extern_crate_modules: ExternCrateModules,
}

fn compiler_op<D>(
  ts_state: Arc<Mutex<TSState>>,
  dispatcher: D,
) -> impl OpDispatcher
where
  D: Fn(&mut TSState, &[u8]) -> Op,
{
  move |_state: &mut CoreIsolateState,
        zero_copy_bufs: &mut [ZeroCopyBuf]|
        -> Op {
    assert_eq!(zero_copy_bufs.len(), 1, "Invalid number of arguments");
    let mut s = ts_state.lock().unwrap();
    dispatcher(&mut s, &zero_copy_bufs[0])
  }
}

pub struct TSIsolate {
  isolate: CoreIsolate,
  state: Arc<Mutex<TSState>>,
}

impl TSIsolate {
  fn new(maybe_extern_crate_modules: Option<ExternCrateModules>) -> TSIsolate {
    let mut isolate = CoreIsolate::new(StartupData::None, false);
    js_check(isolate.execute("assets/typescript.js", TYPESCRIPT_CODE));
    js_check(isolate.execute("compiler_main.js", COMPILER_CODE));

    let extern_crate_modules = maybe_extern_crate_modules.unwrap_or_default();

    let state = Arc::new(Mutex::new(TSState {
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
      "op_read_file",
      compiler_op(state.clone(), ops::json_op(ops::op_read_file)),
    );
    isolate.register_op(
      "op_write_file",
      compiler_op(state.clone(), ops::json_op(ops::op_write_file)),
    );
    isolate.register_op(
      "op_create_hash",
      compiler_op(state.clone(), ops::json_op(ops::op_create_hash)),
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

  // TODO(ry) Instead of Result<Arc<Mutex<TSState>>>, return something
  // like Result<TSState>. I think it would be nicer if this function
  // consumes TSIsolate.
  /// Compiles each module to ESM. Doesn't write any files to disk.
  /// Passes all output via state.
  fn compile(
    mut self,
    config_json: &serde_json::Value,
    root_names: Vec<String>,
  ) -> Result<Arc<Mutex<TSState>>> {
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
  root_name: &Path,
  bundle_filename: &Path,
  maybe_extern_crate_modules: Option<ExternCrateModules>,
  maybe_build_info: Option<PathBuf>,
  maybe_cache: Option<PathBuf>,
) -> Result<String> {
  let ts_isolate = TSIsolate::new(maybe_extern_crate_modules);

  let config_json = if let Some(build_info) = maybe_build_info {
    serde_json::json!({
      "compilerOptions": {
        // In order to help ensure there are no type directed emits in the code
        // which interferes with transpiling only, the setting
        // `"importsNotUsedAsValues"` set to `"error"` will help ensure that items
        // that are written as `import type` are caught and are treated as errors.
        "importsNotUsedAsValues": "error",
        "incremental": true,
        "lib": ["esnext"],
        "listEmittedFiles": true,
        "module": "system",
        "removeComments": true,
        "sourceMap": true,
        "sourceRoot": "$deno$",
        "strict": true,
        "target": "esnext",
        "tsBuildInfoFile": build_info,
      }
    })
  } else {
    serde_json::json!({
      "compilerOptions": {
        // In order to help ensure there are no type directed emits in the code
        // which interferes with transpiling only, the setting
        // `"importsNotUsedAsValues"` set to `"error"` will help ensure that items
        // that are written as `import type` are caught and are treated as errors.
        "importsNotUsedAsValues": "error",
        "lib": ["esnext"],
        "listEmittedFiles": true,
        "module": "system",
        "removeComments": true,
        "sourceMap": true,
        "sourceRoot": "$deno$",
        "strict": true,
        "target": "esnext",
      }
    })
  };

  let root_module_specifier =
    ModuleSpecifier::resolve_url_or_path(&root_name.to_string_lossy()).unwrap();
  let root_names_str = vec![root_module_specifier.as_str().to_string()];

  // TODO lift js_check to caller?
  let locked_state = js_check(ts_isolate.compile(&config_json, root_names_str));
  let state = locked_state.lock().unwrap();
  let main_module_name = state.emit_result.clone().unwrap().root_specifier;

  let mut out_bundle =
    bundle::Bundle::new(bundle_filename.to_owned(), maybe_cache);
  out_bundle.insert_written(state.written_files.clone());
  out_bundle.write_bundle().expect("could not write bundle");

  Ok(main_module_name)
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
) -> Result<()> {
  js_check(isolate.execute("system_loader.js", SYSTEM_LOADER));
  let source_code_vec = std::fs::read(bundle_filename).unwrap();
  let bundle_source_code = std::str::from_utf8(&source_code_vec).unwrap();
  js_check(
    isolate.execute(&bundle_filename.to_string_lossy(), bundle_source_code),
  );
  let script = &format!("__instantiate(\"{}\", false);", main_module_name);
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
) -> Result<()> {
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
) -> Result<()> {
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
    "system_loader_es5.js" => Some(include_str!("system_loader_es5.js")),
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
) -> impl OpDispatcher {
  for (_, path) in custom_assets.iter() {
    println!("cargo:rerun-if-changed={}", path.display());
  }
  move |_state: &mut CoreIsolateState,
        zero_copy_bufs: &mut [ZeroCopyBuf]|
        -> Op {
    assert_eq!(zero_copy_bufs.len(), 1, "Invalid number of arguments");
    let name = std::str::from_utf8(&zero_copy_bufs[0]).unwrap();

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

#[cfg(test)]
mod tests {
  use super::*;
  use std::env;
  use tempfile::TempDir;

  #[test]
  fn test_compile_bundle() {
    let temp_dir = TempDir::new().unwrap();
    let o = temp_dir.path().to_owned();
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root_name = c.join("tests/main.ts");
    let bundle_path = o.join("TEST_BUNDLE.js");
    let module_name =
      compile_bundle(&root_name, &bundle_path, None, None, None).unwrap();
    assert_eq!(module_name, "internal:///main.ts");
    let mut bundle_map_path = bundle_path.clone();
    bundle_map_path.set_extension("js.map");
    assert!(bundle_path.is_file());
    assert!(bundle_map_path.is_file());
    let bundle_str =
      std::fs::read_to_string(bundle_path).expect("could not read bundle");
    assert!(bundle_str.starts_with("System.register("));
    let bundle_map_str = std::fs::read_to_string(bundle_map_path)
      .expect("could not read bundle map");
    assert!(bundle_map_str.starts_with("{\"version\":3,"));
  }

  #[test]
  fn test_compile_bundle_incremental() {
    let temp_dir = TempDir::new().unwrap();
    let o = temp_dir.path().to_owned();
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root_name = c.join("tests/main.ts");
    let maybe_build_info = Some(c.join("tests/main.tsbuildinfo"));
    let maybe_cache = Some(c.join("tests/main.cache"));
    let bundle_path = o.join("TEST_BUNDLE.js");
    let module_name = compile_bundle(
      &root_name,
      &bundle_path,
      None,
      maybe_build_info,
      maybe_cache,
    )
    .unwrap();
    assert_eq!(module_name, "internal:///main.ts");
    let mut bundle_map_path = bundle_path.clone();
    bundle_map_path.set_extension("js.map");
    assert!(bundle_path.is_file());
    assert!(bundle_map_path.is_file());
    let bundle_str =
      std::fs::read_to_string(bundle_path).expect("could not read bundle");
    assert!(bundle_str.starts_with("System.register("));
    let bundle_map_str = std::fs::read_to_string(bundle_map_path)
      .expect("could not read bundle map");
    assert!(bundle_map_str.starts_with("{\"version\":3,"));
  }

  #[test]
  fn test_mksnapshot_bundle() {
    let temp_dir = TempDir::new().unwrap();
    let o = temp_dir.path().to_owned();
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let bundle_filename = c.join("tests/TEST_BUNDLE.js");
    let snapshot_filename = o.join("CLI_SNAPSHOT.bin");
    let mut isolate = CoreIsolate::new(StartupData::None, true);
    mksnapshot_bundle(
      &mut isolate,
      &snapshot_filename,
      &bundle_filename,
      "internal:///deno_typescript/tests/main.ts",
    )
    .expect("failed to make snapshot");
    assert!(snapshot_filename.is_file());
  }
}
