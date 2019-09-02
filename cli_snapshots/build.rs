// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use std::env;
use std::path::PathBuf;

fn main() {
  // To debug snapshot issues uncomment:
  // deno_typescript::trace_serializer();

  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());
  let js_dir = c.join("../js");

  let root_names = vec![js_dir.join("main.ts")];
  let bundle = o.join("CLI_SNAPSHOT.js");
  let state = deno_typescript::compile_bundle(&bundle, root_names).unwrap();
  assert!(bundle.exists());
  deno_typescript::mksnapshot_bundle(&bundle, state).unwrap();

  let root_names = vec![js_dir.join("compiler.ts")];
  let bundle = o.join("COMPILER_SNAPSHOT.js");
  let state = deno_typescript::compile_bundle(&bundle, root_names).unwrap();
  assert!(bundle.exists());
  deno_typescript::mksnapshot_bundle_ts(&bundle, state).unwrap();
}
