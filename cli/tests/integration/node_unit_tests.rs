// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use test_util as util;

#[test]
fn node_unit_tests() {
  let _g = util::http_server();

  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("test")
    .arg("--unstable")
    // TODO(kt3k): This option is required to pass tls_test.ts,
    // but this shouldn't be necessary. tls.connect currently doesn't
    // pass hostname option correctly and it causes cert errors.
    .arg("--unsafely-ignore-certificate-errors")
    .arg("-A")
    .arg(util::tests_path().join("unit_node"))
    .spawn()
    .expect("failed to spawn script");

  let status = deno.wait().expect("failed to wait for the child process");
  assert_eq!(Some(0), status.code());
  assert!(status.success());
}
