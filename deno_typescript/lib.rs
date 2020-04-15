// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#![deny(warnings)]

extern crate deno_core;
extern crate serde;
extern crate serde_json;

use deno_core::js_check;
pub use deno_core::v8_set_flags;
use deno_core::CoreOp;
use deno_core::ErrBox;
use deno_core::Isolate;
use deno_core::ZeroCopyBuf;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

static TYPESCRIPT_CODE: &str = include_str!("typescript/lib/typescript.js");
static SYSTEM_LOADER: &str = include_str!("system_loader.js");

pub fn ts_version() -> String {
  let data = include_str!("typescript/package.json");
  let pkg: serde_json::Value = serde_json::from_str(data).unwrap();
  pkg["version"].as_str().unwrap().to_string()
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
  isolate: &mut Isolate,
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
  isolate: &mut Isolate,
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
  runtime_isolate: &mut Isolate,
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
/// Isolate.
pub fn op_fetch_asset<S: ::std::hash::BuildHasher>(
  custom_assets: HashMap<String, PathBuf, S>,
) -> impl Fn(&[u8], Option<ZeroCopyBuf>) -> CoreOp {
  for (_, path) in custom_assets.iter() {
    println!("cargo:rerun-if-changed={}", path.display());
  }
  move |control: &[u8], zero_copy_buf: Option<ZeroCopyBuf>| -> CoreOp {
    assert!(zero_copy_buf.is_none()); // zero_copy_buf unused in this op.
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
