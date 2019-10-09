// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// TODO(ry) Current std tests are run in .github/workflows/build.yml but ideally
// they would be called as part of "cargo test". "deno test" is too slow to do
// this desierable thing: https://github.com/denoland/deno/issues/3088
/*
#[macro_use]
extern crate lazy_static;
extern crate tempfile;
mod util;
use util::*;

#[test]
fn std_tests() {
  let mut deno = deno_cmd()
    .current_dir(root_path())
    .arg("test")
    .arg("-A")
    .arg("std")
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert_eq!(Some(0), status.code());
  assert!(status.success());
}
*/
