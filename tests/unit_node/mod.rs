// Copyright 2018-2025 the Deno authors. MIT license.

use std::num::NonZeroUsize;
use std::sync::Arc;

use file_test_runner::RunOptions;
use file_test_runner::TestResult;
use file_test_runner::collection::CollectOptions;
use file_test_runner::collection::CollectedTest;
use file_test_runner::collection::collect_tests_or_exit;
use file_test_runner::collection::strategies::TestPerFileCollectionStrategy;
use test_util as util;
use test_util::test_runner::FlakyTestTracker;
use test_util::test_runner::Parallelism;
use test_util::test_runner::flaky_test_ci;
use test_util::tests_path;
use util::deno_config_path;
use util::env_vars_for_npm_tests;

fn main() {
  let category = collect_tests_or_exit(CollectOptions {
    base: tests_path().join("unit_node").to_path_buf(),
    strategy: Box::new(TestPerFileCollectionStrategy {
      file_pattern: Some(".*_test\\.ts$".to_string()),
    }),
    filter_override: None,
  });
  if category.is_empty() {
    return;
  }
  let parallelism = Parallelism::default();
  let flaky_test_tracker = Arc::new(FlakyTestTracker::default());
  let _g = util::http_server();
  // Run the crypto category tests separately without concurrency because they run in Deno with --parallel
  let (crypto_category, category) =
    category.partition(|test| test.name.contains("::crypto::"));
  let reporter = test_util::test_runner::get_test_reporter(
    "unit_node",
    flaky_test_tracker.clone(),
  );
  file_test_runner::run_tests(
    &category,
    RunOptions {
      parallelism: parallelism.max_parallelism(),
      reporter: reporter.clone(),
    },
    {
      let flaky_test_tracker = flaky_test_tracker.clone();
      move |test| {
        flaky_test_ci(
          &test.name,
          &flaky_test_tracker,
          Some(&parallelism),
          || run_test(test),
        )
      }
    },
  );
  file_test_runner::run_tests(
    &crypto_category,
    RunOptions {
      parallelism: NonZeroUsize::new(1).unwrap(),
      reporter: reporter.clone(),
    },
    move |test| {
      flaky_test_ci(&test.name, &flaky_test_tracker, None, || run_test(test))
    },
  );
}

fn run_test(test: &CollectedTest) -> TestResult {
  let mut deno = util::deno_cmd()
    .disable_diagnostic_logging()
    .current_dir(util::root_path())
    .arg("test")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-lock")
    .arg("--unstable-net")
    .arg("-A");

  // Some tests require the root CA cert file to be loaded.
  if test.name.ends_with("::http2_test")
    || test.name.ends_with("::http_test")
    || test.name.ends_with("::https_test")
  {
    deno = deno.arg("--cert=./tests/testdata/tls/RootCA.pem");
  }

  // Parallel tests for crypto
  if test.name.contains("::crypto::") {
    deno = deno.arg("--parallel");
  }

  deno
    .arg(test.path.clone())
    .envs(env_vars_for_npm_tests())
    .piped_output()
    .spawn()
    .expect("failed to spawn script")
    .wait_to_test_result(&test.name)
}
