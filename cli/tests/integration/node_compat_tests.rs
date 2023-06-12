// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use test_util as util;

#[test]
fn node_compat_tests() {
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("test")
    .arg("--unstable")
    .arg("-A")
    .arg(util::tests_path().join("node_compat"))
    .spawn()
    .expect("failed to spawn script");

  let status = deno.wait().expect("failed to wait for the child process");
  assert_eq!(Some(0), status.code());
  assert!(status.success());
}
