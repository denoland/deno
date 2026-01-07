// Copyright 2018-2025 the Deno authors. MIT license.

use std::num::NonZeroUsize;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;

use file_test_runner::RunOptions;
use file_test_runner::TestResult;
use file_test_runner::collection::CollectedTest;
use file_test_runner::collection::CollectedTestCategory;
use test_util::TestMacroCase;
use test_util::test_runner::FlakyTestTracker;
use test_util::test_runner::Parallelism;
use test_util::test_runner::run_maybe_flaky_test;

// These files have `_tests.rs` suffix to make it easier to tell which file is
// the test (ex. `lint_tests.rs`) and which is the implementation (ex. `lint.rs`)
// when both are open, especially for two tabs in VS Code

#[path = "bench_tests.rs"]
mod bench;
#[path = "cache_tests.rs"]
mod cache;
#[path = "check_tests.rs"]
mod check;
#[path = "compile_tests.rs"]
mod compile;
#[path = "coverage_tests.rs"]
mod coverage;
#[path = "eval_tests.rs"]
mod eval;
#[path = "flags_tests.rs"]
mod flags;
#[path = "fmt_tests.rs"]
mod fmt;
#[path = "init_tests.rs"]
mod init;
#[path = "inspector_tests.rs"]
mod inspector;
#[path = "install_tests.rs"]
mod install;
#[path = "jsr_tests.rs"]
mod jsr;
#[path = "jupyter_tests.rs"]
mod jupyter;
#[path = "lsp_tests.rs"]
mod lsp;
#[path = "npm_tests.rs"]
mod npm;
#[path = "pm_tests.rs"]
mod pm;
#[path = "publish_tests.rs"]
mod publish;

#[path = "repl_tests.rs"]
mod repl;
#[path = "run_tests.rs"]
mod run;
#[path = "serve_tests.rs"]
mod serve;
#[path = "shared_library_tests.rs"]
mod shared_library_tests;
#[path = "task_tests.rs"]
mod task;
#[path = "test_tests.rs"]
mod test;
#[path = "upgrade_tests.rs"]
mod upgrade;
#[path = "watcher_tests.rs"]
mod watcher;

pub fn main() {
  let mut main_category: CollectedTestCategory<&'static TestMacroCase> =
    CollectedTestCategory {
      name: module_path!().to_string(),
      path: PathBuf::from(file!()),
      children: Default::default(),
    };
  test_util::collect_and_filter_tests(&mut main_category);
  if main_category.is_empty() {
    return; // no tests to run for the filter
  }

  let run_test = move |test: &CollectedTest<&'static TestMacroCase>,
                       flaky_test_tracker: &FlakyTestTracker,
                       parallelism: Option<&Parallelism>| {
    if test.data.ignore {
      return TestResult::Ignored;
    }
    let run_test = || {
      let _test_timeout_holder = test.data.timeout.map(|timeout_secs| {
        test_util::test_runner::with_timeout(
          test.name.clone(),
          std::time::Duration::from_secs(timeout_secs as u64),
        )
      });
      let (mut captured_output, result) =
        test_util::print::with_captured_output(|| {
          TestResult::from_maybe_panic_or_result(AssertUnwindSafe(|| {
            (test.data.func)();
            TestResult::Passed { duration: None }
          }))
        });
      match result {
        TestResult::Passed { .. } | TestResult::Ignored => result,
        TestResult::Failed { output, duration } => {
          if !captured_output.is_empty() {
            captured_output.push(b'\n');
          }
          captured_output.extend_from_slice(&output);
          TestResult::Failed {
            duration,
            output: captured_output,
          }
        }
        // no support for sub tests
        TestResult::SubTests { .. } => unreachable!(),
      }
    };
    run_maybe_flaky_test(
      &test.name,
      test.data.flaky || *test_util::IS_CI,
      flaky_test_tracker,
      parallelism,
      run_test,
    )
  };

  let (watcher_tests, main_tests) =
    main_category.partition(|t| t.name.contains("::watcher::"));

  // watcher tests are really flaky, so run them sequentially
  let flaky_test_tracker = Arc::new(FlakyTestTracker::default());
  let reporter = test_util::test_runner::get_test_reporter(
    "integration",
    flaky_test_tracker.clone(),
  );
  file_test_runner::run_tests(
    &watcher_tests,
    RunOptions {
      parallelism: NonZeroUsize::new(1).unwrap(),
      reporter: reporter.clone(),
    },
    {
      let flaky_test_tracker = flaky_test_tracker.clone();
      move |test| run_test(test, &flaky_test_tracker, None)
    },
  );
  let parallelism = Parallelism::default();
  file_test_runner::run_tests(
    &main_tests,
    RunOptions {
      parallelism: parallelism.max_parallelism(),
      reporter: reporter.clone(),
    },
    move |test| run_test(test, &flaky_test_tracker, Some(&parallelism)),
  );
}
