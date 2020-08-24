// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Note: this module is used both in build.rs and main.rs.

pub use deno_core::v8_set_flags;
use deno_core::CoreIsolateState;
use deno_core::Op;
use deno_core::ZeroCopyBuf;
use std::collections::HashMap;
use std::path::PathBuf;

fn get_asset(name: &str) -> Option<&'static str> {
  macro_rules! inc {
    ($e:expr) => {
      Some(include_str!(concat!("dts/", $e)))
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

/// Warning: Returns a non-JSON op dispatcher. Must be manually attached to
/// CoreIsolate.
pub fn op_fetch_asset<S: ::std::hash::BuildHasher>(
  custom_assets: HashMap<String, PathBuf, S>,
) -> impl Fn(&mut deno_core::CoreIsolateState, &mut [ZeroCopyBuf]) -> Op {
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
