// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use test_util as util;

#[macro_export]
macro_rules! itest(
($name:ident {$( $key:ident: $value:expr,)*})  => {
  #[test]
  fn $name() {
    (test_util::CheckOutputIntegrationTest {
      $(
        $key: $value,
       )*
      .. Default::default()
    }).run()
  }
}
);

#[macro_export]
macro_rules! itest_flaky(
($name:ident {$( $key:ident: $value:expr,)*})  => {
  #[flaky_test::flaky_test]
  fn $name() {
    (test_util::CheckOutputIntegrationTest {
      $(
        $key: $value,
       )*
      .. Default::default()
    }).run()
  }
}
);

// These files have `_tests.rs` suffix to make it easier to tell which file is
// the test (ex. `lint_tests.rs`) and which is the implementation (ex. `lint.rs`)
// when both are open, especially for two tabs in VS Code

#[path = "bench_tests.rs"]
mod bench;
#[path = "bundle_tests.rs"]
mod bundle;
#[path = "cache_tests.rs"]
mod cache;
#[path = "cert_tests.rs"]
mod cert;
#[path = "check_tests.rs"]
mod check;
#[path = "compile_tests.rs"]
mod compile;
#[path = "coverage_tests.rs"]
mod coverage;
#[path = "doc_tests.rs"]
mod doc;
#[path = "eval_tests.rs"]
mod eval;
#[path = "flags_tests.rs"]
mod flags;
#[path = "fmt_tests.rs"]
mod fmt;
#[path = "info_tests.rs"]
mod info;
#[path = "init_tests.rs"]
mod init;
#[path = "inspector_tests.rs"]
mod inspector;
#[path = "install_tests.rs"]
mod install;
#[path = "lint_tests.rs"]
mod lint;
#[path = "lsp_tests.rs"]
mod lsp;
#[path = "npm_tests.rs"]
mod npm;
#[path = "repl_tests.rs"]
mod repl;
#[path = "run_tests.rs"]
mod run;
#[path = "task_tests.rs"]
mod task;
#[path = "test_tests.rs"]
mod test;
#[path = "upgrade_tests.rs"]
mod upgrade;
#[path = "vendor_tests.rs"]
mod vendor;
#[path = "watcher_tests.rs"]
mod watcher;
#[path = "worker_tests.rs"]
mod worker;

#[test]
fn js_unit_tests_lint() {
  let status = util::deno_cmd()
    .arg("lint")
    .arg("--unstable")
    .arg(util::tests_path().join("unit"))
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn js_unit_tests() {
  let _g = util::http_server();

  // Note that the unit tests are not safe for concurrency and must be run with a concurrency limit
  // of one because there are some chdir tests in there.
  // TODO(caspervonb) split these tests into two groups: parallel and serial.
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("test")
    .arg("--unstable")
    .arg("--location=http://js-unit-tests/foo/bar")
    .arg("--no-prompt")
    .arg("-A")
    .arg(util::tests_path().join("unit"))
    .spawn()
    .expect("failed to spawn script");

  let status = deno.wait().expect("failed to wait for the child process");
  assert_eq!(Some(0), status.code());
  assert!(status.success());
}
