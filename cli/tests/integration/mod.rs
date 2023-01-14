// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

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
#[path = "js_unit_tests.rs"]
mod js_unit_tests;
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
