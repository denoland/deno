// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use std::env;
use std::path::PathBuf;

fn main() {
  // To debug snapshot issues uncomment:
  // deno_typescript::trace_serializer();

  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());
  let js_dir = c.join("../js");
  let import_maps = vec![
    deno_ops_fs_bundle::get_import_map(),
    deno_bundle_util::get_import_map(),
    deno_dispatch_json_bundle::get_import_map(),
  ];
  let import_map = Some(deno_typescript::merge_import_maps(import_maps));
  dbg!(&import_map);

  let root_names = vec![js_dir.join("main.ts")];
  let bundle = o.join("CLI_SNAPSHOT.js");
  let state =
    deno_typescript::compile_bundle(&bundle, root_names, import_map.clone())
      .unwrap();
  assert!(bundle.exists());
  deno_typescript::mksnapshot_bundle(&bundle, state).unwrap();

  let root_names = vec![js_dir.join("compiler.ts")];
  let bundle = o.join("COMPILER_SNAPSHOT.js");
  let state =
    deno_typescript::compile_bundle(&bundle, root_names, import_map).unwrap();
  assert!(bundle.exists());
  deno_typescript::mksnapshot_bundle_ts(&bundle, state).unwrap();
}
