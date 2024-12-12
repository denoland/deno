// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_runtime::deno_napi;
use std::env;

fn main() {
  // Skip building from docs.rs.
  if env::var_os("DOCS_RS").is_some() {
    return;
  }

  deno_napi::print_linker_flags("deno");

  if cfg!(windows) {
    // these dls load slowly, so delay loading them
    let dlls = [
      // webgpu
      "d3dcompiler_47",
      "OPENGL32",
      // network related functions
      "iphlpapi",
    ];
    for dll in dlls {
      println!("cargo:rustc-link-arg-bin=deno=/delayload:{dll}.dll");
    }
    // enable delay loading
    println!("cargo:rustc-link-arg-bin=deno=delayimp.lib");
  }
}
