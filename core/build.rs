// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Run "cargo build -vv" if you want to see gn output.
mod gn {
  include!("../gn.rs");
}

fn main() {
  let build = gn::Build::setup();

  println!(
    "cargo:rustc-link-search=native={}/obj/core/libdeno",
    build.gn_out_dir
  );
  if cfg!(target_os = "windows") {
    println!("cargo:rustc-link-lib=static=libdeno");
  } else {
    println!("cargo:rustc-link-lib=static=deno");
  }

  // Link the system libraries that libdeno and V8 depend on.
  if cfg!(any(target_os = "macos", target_os = "freebsd")) {
    println!("cargo:rustc-link-lib=dylib=c++");
  } else if cfg!(target_os = "windows") {
    for lib in vec!["dbghelp", "shlwapi", "winmm", "ws2_32"] {
      println!("cargo:rustc-link-lib={}", lib);
    }
  }

  build.run("core:deno_core_deps");
}
