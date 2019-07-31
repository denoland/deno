// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Run "cargo build -vv" if you want to see gn output.
include!("../tools/build_common.rs");
fn main() {
  let (gn_out_path, ninja_env) = setup();
  cargo_gn::build("core:deno_core_deps", ninja_env);

  let d = gn_out_path.join("obj/core/libdeno/");
  println!("cargo:rustc-link-search=native={}", d.display());
}
