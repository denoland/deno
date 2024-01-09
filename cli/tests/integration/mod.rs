// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#[macro_export]
macro_rules! itest(
($name:ident {$( $key:ident: $value:expr,)*})  => {
  #[test]
  fn $name() {
    let test = test_util::CheckOutputIntegrationTest {
      $(
        $key: $value,
       )*
      .. Default::default()
    };
    let output = test.output();
    output.assert_exit_code(test.exit_code);
    if !test.output.is_empty() {
      assert!(test.output_str.is_none());
      output.assert_matches_file(test.output);
    } else {
      output.assert_matches_text(test.output_str.unwrap_or(""));
    }
  }
}
);

#[macro_export]
macro_rules! itest_flaky(
($name:ident {$( $key:ident: $value:expr,)*})  => {
  #[flaky_test::flaky_test]
  fn $name() {
    let test = test_util::CheckOutputIntegrationTest {
      $(
        $key: $value,
       )*
      .. Default::default()
    };
    let output = test.output();
    output.assert_exit_code(test.exit_code);
    if !test.output.is_empty() {
      assert!(test.output_str.is_none());
      output.assert_matches_file(test.output);
    } else {
      output.assert_matches_text(test.output_str.unwrap_or(""));
    }
  }
}
);

#[macro_export]
macro_rules! context(
({$( $key:ident: $value:expr,)*})  => {
  test_util::TestContext::create(test_util::TestContextOptions {
    $(
      $key: $value,
      )*
    .. Default::default()
  })
}
);

#[macro_export]
macro_rules! itest_steps(
($name:ident {$( $key:ident: $value:expr,)*})  => {
  #[test]
  fn $name() {
    (test_util::CheckOutputIntegrationTestSteps {
      $(
        $key: $value,
       )*
      .. Default::default()
    }).run()
  }
}
);

#[macro_export]
macro_rules! command_step(
({$( $key:ident: $value:expr,)*})  => {
  test_util::CheckOutputIntegrationTestCommandStep {
    $(
      $key: $value,
      )*
    .. Default::default()
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
#[path = "js_unit_tests.rs"]
mod js_unit_tests;
#[path = "jsr_tests.rs"]
mod jsr;
#[path = "jupyter_tests.rs"]
mod jupyter;
#[path = "lint_tests.rs"]
mod lint;
#[path = "lsp_tests.rs"]
mod lsp;
#[path = "node_compat_tests.rs"]
mod node_compat_tests;
#[path = "node_unit_tests.rs"]
mod node_unit_tests;
#[path = "npm_tests.rs"]
mod npm;
#[path = "publish_tests.rs"]
mod publish;

#[path = "repl_tests.rs"]
mod repl;
#[path = "run_tests.rs"]
mod run;
#[path = "shared_library_tests.rs"]
mod shared_library_tests;
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
