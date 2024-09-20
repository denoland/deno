// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use test_util as util;
use util::deno_config_path;

#[test]
fn node_compat_tests() {
  let _server = util::http_server();

  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .envs(util::env_vars_for_npm_tests())
    .arg("test")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-lock")
    .arg("-A")
    .arg(util::tests_path().join("node_compat/test.ts"))
    .spawn()
    .expect("failed to spawn script");

  let status = deno.wait().expect("failed to wait for the child process");
  assert_eq!(Some(0), status.code());
  assert!(status.success());
}
