// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Run "cargo build -vv" if you want to see gn output.
mod gn {
  include!("../tools/gn.rs");
}

fn main() {
  let build = gn::Build::setup();

  println!(
    "cargo:rustc-link-search=native={}/obj/core/libdeno",
    build.gn_out_dir
  );

  build.run("core:deno_core_deps");
}
