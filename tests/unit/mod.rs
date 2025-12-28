// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use file_test_runner::RunOptions;
use file_test_runner::TestResult;
use file_test_runner::collection::CollectOptions;
use file_test_runner::collection::CollectedTest;
use file_test_runner::collection::collect_tests_or_exit;
use file_test_runner::collection::strategies::TestPerFileCollectionStrategy;
use test_util as util;
use test_util::TestContextBuilder;
use test_util::test_runner::FlakyTestTracker;
use test_util::test_runner::Parallelism;
use test_util::test_runner::flaky_test_ci;
use test_util::tests_path;

fn main() {
  let category = collect_tests_or_exit(CollectOptions {
    base: tests_path().join("unit").to_path_buf(),
    strategy: Box::new(TestPerFileCollectionStrategy {
      file_pattern: Some(".*_test\\.ts$".to_string()),
    }),
    filter_override: None,
  });
  if category.is_empty() {
    return; // no tests to run for the filter
  }
  let parallelism = Parallelism::default();
  let flaky_test_tracker = Arc::new(FlakyTestTracker::default());
  let _g = util::http_server();
  file_test_runner::run_tests(
    &category,
    RunOptions {
      parallelism: parallelism.max_parallelism(),
      reporter: test_util::test_runner::get_test_reporter(
        "unit",
        flaky_test_tracker.clone(),
      ),
    },
    move |test| {
      flaky_test_ci(&test.name, &flaky_test_tracker, Some(&parallelism), || {
        run_test(test)
      })
    },
  )
}

fn run_test(test: &CollectedTest) -> TestResult {
  let mut deno = if test.name.ends_with("::bundle_test") {
    TestContextBuilder::new()
      .add_npm_env_vars()
      .use_http_server()
      .build()
      .new_command()
  } else {
    util::deno_cmd()
  };

  deno = deno
    .disable_diagnostic_logging()
    .current_dir(util::root_path())
    .arg("test")
    .arg("--config")
    .arg(util::deno_config_path())
    .arg("--no-lock")
    // TODO(bartlomieju): would be better if we could apply this unstable
    // flag to particular files, but there's many of them that rely on unstable
    // net APIs (`reusePort` in `listen` and `listenTls`; `listenDatagram`)
    .arg("--unstable-net")
    .arg("--unstable-vsock")
    .arg("--location=http://127.0.0.1:4545/")
    .arg("--no-prompt");

  if test.name.ends_with("::bundle_test") {
    deno = deno.arg("--unstable-bundle");
  }

  if test.name.ends_with("::cron_test") {
    deno = deno.arg("--unstable-cron");
  }

  if test.name.contains("::kv_") {
    deno = deno.arg("--unstable-kv");
  }

  if test.name.ends_with("::worker_permissions_test")
    || test.name.ends_with("::worker_test")
  {
    deno = deno.arg("--unstable-worker-options");
  }

  // Some tests require the root CA cert file to be loaded.
  if test.name.ends_with("::websocket_test") {
    deno = deno.arg(format!(
      "--cert={}",
      util::testdata_path()
        .join("tls")
        .join("RootCA.pem")
        .to_string_lossy()
    ));
  };

  if test.name.ends_with("::tls_sni_test") {
    // TODO(lucacasonato): fix the SNI in the certs so that this is not needed
    deno = deno.arg("--unsafely-ignore-certificate-errors");
  }

  let mut deno = deno.arg("-A").arg(test.path.clone());

  // update the snapshots if when `UPDATE=1`
  if std::env::var_os("UPDATE") == Some("1".into()) {
    deno = deno.arg("--").arg("--update");
  }

  deno
    .piped_output()
    .spawn()
    .expect("failed to spawn script")
    .wait_to_test_result(&test.name)
}
