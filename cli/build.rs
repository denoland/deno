// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Run "cargo build -vv" if you want to see gn output.
mod gn {
  include!("../tools/gn.rs");
}

fn main() {
  let build = gn::Build::setup();
  // When RLS is running "cargo check" to analyze the source code, we're not
  // trying to build a working executable, rather we're just compiling all
  // rust code.
  if !build.check_only {
    build.run("cli:deno_deps");
  }
}
