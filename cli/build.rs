// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Run "cargo build -vv" if you want to see gn output.
include!("../tools/build_common.rs");

fn main() {
  // TODO(ry) When running "cargo check" only build "msg_rs"
  let (_, ninja_env) = setup();
  cargo_gn::build("cli:deno_deps", ninja_env);
}
