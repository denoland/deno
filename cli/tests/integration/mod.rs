// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

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

// These files have `_tests.rs` suffix to make it easier to tell which file is
// the test (ex. `lint_tests.rs`) and which is the implementation (ex. `lint.rs`)
// when both are open, especially for two tabs in VS Code

#[path = "bundle_tests.rs"]
mod bundle;
#[path = "cache_tests.rs"]
mod cache;
#[path = "compile_tests.rs"]
mod compile;
#[path = "coverage_tests.rs"]
mod coverage;
#[path = "doc_tests.rs"]
mod doc;
#[path = "eval_tests.rs"]
mod eval;
#[path = "fmt_tests.rs"]
mod fmt;
#[path = "general_tests.rs"]
mod general;
#[path = "info_tests.rs"]
mod info;
#[path = "inspector_tests.rs"]
mod inspector;
#[path = "install_tests.rs"]
mod install;
#[path = "lint_tests.rs"]
mod lint;
#[path = "repl_tests.rs"]
mod repl;
#[path = "run_tests.rs"]
mod run;
#[path = "test_tests.rs"]
mod test;
#[path = "upgrade_tests.rs"]
mod upgrade;
#[path = "watcher_tests.rs"]
mod watcher;
#[path = "worker_tests.rs"]
mod worker;
