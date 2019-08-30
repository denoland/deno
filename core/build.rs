// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Run "cargo build -vv" if you want to see gn output.
include!("../tools/build_common.rs");
fn main() {
  if !is_rls_build() {
    let (gn_out_path, ninja_env) = setup();
    cargo_gn::build("core:deno_core_deps", ninja_env);
    println!(
      "cargo:rustc-link-search=native={}",
      gn_out_path.join("obj/core/libdeno/").to_str().unwrap()
    );
  } else {
    // Enable the 'check-only' feature, which enables some workarounds in the
    // rust source code to compile successfully without a bundle and snapshot.
    println!("cargo:rustc-cfg=feature=\"check-only\"");
  }
}
