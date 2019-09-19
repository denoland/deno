// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#[macro_use]
extern crate lazy_static;
extern crate tempfile;
mod util;
use util::*;

// TODO(#2933): Rewrite these tests in rust.
// TODO(ry) These tests can't run in parallel.
#[test]
fn tty_tests() {
  let g = http_server();
  run_python_script("tools/complex_permissions_test.py");
  run_python_script("tools/permission_prompt_test.py");
  // TODO(ry) is_tty_test is not passing on travis when run with "cargo test"
  // run_python_script("tools/is_tty_test.py");
  drop(g);
}
