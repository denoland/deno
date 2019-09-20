// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use std::env;
use std::path::PathBuf;

fn main() {
  // To debug snapshot issues uncomment:
  // deno_typescript::trace_serializer();

  println!(
    "cargo:rustc-env=TS_VERSION={}",
    deno_typescript::ts_version()
  );

  let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
  let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());

  let root_names = vec![c.join("main.ts")];
  let bundle = o.join("CLI_SNAPSHOT.js");
  let state = deno_typescript::compile_bundle(&bundle, root_names).unwrap();
  assert!(bundle.exists());
  deno_typescript::mksnapshot_bundle(&bundle, state).unwrap();

  let root_names = vec![c.join("compiler.ts")];
  let bundle = o.join("COMPILER_SNAPSHOT.js");
  let state = deno_typescript::compile_bundle(&bundle, root_names).unwrap();
  assert!(bundle.exists());
  deno_typescript::mksnapshot_bundle_ts(&bundle, state).unwrap();
}
