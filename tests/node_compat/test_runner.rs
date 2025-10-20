// Copyright 2018-2025 the Deno authors. MIT license.

use test_util as util;
use util::deno_config_path;

#[test]
#[allow(clippy::print_stderr)]
fn node_compat_tests() {
  // Skip Node.js compatibility tests in CI on PRs unless ci-node-test label is present
  if std::env::var("CI_SKIP_NODE_TEST").unwrap_or_default() == "true" {
    eprintln!(
      "Skipping Node.js compatibility tests (CI_SKIP_NODE_TEST is set)"
    );
    return;
  }

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
