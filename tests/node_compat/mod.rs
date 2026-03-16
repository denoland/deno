// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use file_test_runner::RunOptions;
use file_test_runner::TestResult;
use file_test_runner::collection::CollectedCategoryOrTest;
use file_test_runner::collection::CollectedTest;
use file_test_runner::collection::CollectedTestCategory;
use regex::Regex;
use report::CollectedResult;
use report::ErrorInfo;
use serde::Deserialize;
use test_util as util;
use test_util::IS_CI;
use test_util::PathRef;
use test_util::test_runner::FlakyTestTracker;
use test_util::test_runner::Parallelism;
use test_util::test_runner::run_maybe_flaky_test;
use util::tests_path;

mod report;

/// Global counter for generating unique test serial IDs
static TEST_SERIAL_ID: AtomicUsize = AtomicUsize::new(0);

const RUN_ARGS: &[&str] = &[
  "run",
  "-A",
  "--quiet",
  "--unstable-unsafe-proto",
  "--unstable-bare-node-builtins",
];

const TEST_ARGS: &[&str] = &[
  "test",
  "-A",
  "--quiet",
  "--unstable-unsafe-proto",
  "--unstable-bare-node-builtins",
  "--no-check",
  "--unstable-detect-cjs",
];

/// Per-platform value: either a boolean (true = enabled, false = disabled)
/// or an object describing an expected failure.
///
/// Uses `#[serde(untagged)]` so that config.jsonc can use both
/// `"windows": false` (boolean) and `"windows": { "exitCode": 1 }` (object).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PlatformExpectation {
  Enabled(bool),
  ExpectedFailure(ExpectedFailure),
}

/// Describes an expected test failure: the exit code and/or output pattern.
#[derive(Debug, Clone, Default, Deserialize)]
struct ExpectedFailure {
  #[serde(rename = "exitCode")]
  exit_code: Option<i32>,
  output: Option<String>,
}

/// Configuration for a single test from config.jsonc
///
/// Platform fields (`windows`, `darwin`, `linux`) accept:
///   - `false` — skip the test on that OS
///   - `true` (or omit) — run normally; expected to pass
///   - `{ "exitCode": N, "output": "pattern with [WILDCARD]" }` — run the test
///     but expect it to fail with the given exit code / output
///
/// Top-level `exitCode` and `output` apply to **all** platforms unless a
/// platform-specific entry overrides them.
#[derive(Debug, Clone, Default, Deserialize)]
struct TestConfig {
  #[serde(default)]
  flaky: bool,
  /// When `true`, the test is skipped on all platforms. Requires `reason`.
  #[serde(default)]
  ignore: bool,
  windows: Option<PlatformExpectation>,
  darwin: Option<PlatformExpectation>,
  linux: Option<PlatformExpectation>,
  reason: Option<String>,
  /// Expected exit code for all platforms (overridden by per-platform config)
  #[serde(rename = "exitCode")]
  exit_code: Option<i32>,
  /// Expected output pattern (with [WILDCARD] support) for all platforms
  output: Option<String>,
}

/// The full config.json structure
#[derive(Debug, Deserialize)]
struct NodeCompatConfig {
  tests: HashMap<String, TestConfig>,
}

/// Data attached to each collected test
#[derive(Debug, Clone)]
struct NodeCompatTestData {
  test_path: String,
}

fn main() {
  let ci_hash = test_util::hash::check_ci_hash("node_compat", |hasher| {
    let tests = test_util::tests_path();
    hasher
      .hash_dir(tests.join("node_compat"))
      .hash_dir(tests.join("util"))
      .hash_file(test_util::deno_exe_path())
      .hash_file(test_util::test_server_path());
  });
  if matches!(ci_hash, test_util::hash::CiHashStatus::Skip) {
    return;
  }

  let cli_args = parse_cli_args();
  let config = load_config();
  let cli_filter = file_test_runner::collection::parse_cli_arg_filter();
  let mut category = collect_all_tests();

  if let Some(filter) = &cli_filter {
    // With a filter, run any matching test from the full suite
    // e.g. `cargo test --test node_compat -- test-assert`
    category.filter_children(filter);
  } else if !cli_args.report {
    // Without a filter, only run tests listed in config.jsonc
    let config_tests: std::collections::HashSet<&str> =
      config.tests.keys().map(|s| s.as_str()).collect();
    category.children.retain(|child| match child {
      CollectedCategoryOrTest::Test(t) => {
        config_tests.contains(t.data.test_path.as_str())
      }
      _ => true,
    });
  }

  if category.is_empty() {
    return;
  }

  // Partition into sequential and parallel tests
  let (sequential_category, parallel_category) =
    category.partition(|test| test.data.test_path.starts_with("sequential/"));

  let parallelism = Parallelism::default();
  let flaky_test_tracker = Arc::new(FlakyTestTracker::default());

  // Shared state for collecting results
  let results: Arc<Mutex<HashMap<String, CollectedResult>>> =
    Arc::new(Mutex::new(HashMap::new()));

  let reporter = test_util::test_runner::get_test_reporter(
    "node_compat",
    flaky_test_tracker.clone(),
  );

  let config = Arc::new(config);
  let all_tests_flaky = *IS_CI && !cli_args.report;

  // Run sequential tests with parallelism=1
  let summary = file_test_runner::run_tests_summary(
    &sequential_category,
    RunOptions {
      parallelism: file_test_runner::Parallelism::from_usize(1),
      reporter: reporter.clone(),
    },
    {
      let cli_args = cli_args.clone();
      let config = config.clone();
      let flaky_test_tracker = flaky_test_tracker.clone();
      let results = results.clone();
      move |test| {
        let test_config = config.tests.get(&test.data.test_path);
        run_maybe_flaky_test(
          &test.name,
          test_config.is_some_and(|c| c.flaky) || all_tests_flaky,
          &flaky_test_tracker,
          None,
          || run_test(&cli_args, test, test_config, &results),
        )
      }
    },
  );
  if !cli_args.report {
    summary.panic_on_failures();
  }

  // Run parallel tests
  let summary = file_test_runner::run_tests_summary(
    &parallel_category,
    RunOptions {
      parallelism: parallelism.max_parallelism(),
      reporter: reporter.clone(),
    },
    {
      let flaky_test_tracker = flaky_test_tracker.clone();
      let results = results.clone();
      let cli_args = cli_args.clone();
      let config = config.clone();
      move |test| {
        let test_config = config.tests.get(&test.data.test_path);
        run_maybe_flaky_test(
          &test.name,
          test_config.is_some_and(|c| c.flaky) || all_tests_flaky,
          &flaky_test_tracker,
          Some(&parallelism),
          || run_test(&cli_args, test, test_config, &results),
        )
      }
    },
  );

  if !cli_args.report {
    summary.panic_on_failures();
  } else if std::env::var("CI").is_ok() {
    report::generate_report(&results.lock().unwrap());
  }
  if let test_util::hash::CiHashStatus::RunThenCommit(pending) = ci_hash {
    pending.commit();
  }
}

#[derive(Clone)]
struct CliArgs {
  inspect_brk: bool,
  inspect_wait: bool,
  report: bool,
}

// Run with: `cargo test --test node_compat -- <filter>`
// For example: `cargo test --test node_compat -- test-assert`
// Debug with: `cargo test --test node_compat -- test-assert --inspect-brk`
fn parse_cli_args() -> CliArgs {
  let mut inspect_brk = false;
  let mut inspect_wait = false;
  let mut report = false;

  for arg in std::env::args() {
    match arg.as_str() {
      "--inspect-brk" => inspect_brk = true,
      "--inspect-wait" => inspect_wait = true,
      "--report" => report = true,
      _ => {}
    }
  }

  CliArgs {
    inspect_brk,
    inspect_wait,
    report,
  }
}

fn load_config() -> NodeCompatConfig {
  let config_path = tests_path().join("node_compat").join("config.jsonc");
  let config_content = std::fs::read_to_string(&config_path).unwrap();
  let value =
    jsonc_parser::parse_to_serde_value(&config_content, &Default::default())
      .unwrap()
      .unwrap();
  serde_json::from_value(value).unwrap()
}

// Directories that don't contain runnable tests
// from https://github.com/denoland/std/pull/2787#discussion_r1001237016
const IGNORED_TEST_DIRS: &[&str] = &[
  "addons",
  "async-hooks",
  "benchmark",
  "cctest",
  "common",
  "doctool",
  "embedding",
  "fixtures",
  "fuzzers",
  "js-native-api",
  "known_issues",
  "node-api",
  "overlapped-checker",
  "report",
  "testpy",
  "tick-processor",
  "tools",
  "v8-updates",
  "wasi",
  "wpt",
];

/// Collect all test files from the suite directory.
fn collect_all_tests() -> CollectedTestCategory<NodeCompatTestData> {
  let suite_dir = suite_test_dir();
  let mut children = Vec::new();

  // Scan all subdirectories in the test suite
  for subdir_entry in std::fs::read_dir(&suite_dir).unwrap().flatten() {
    let subdir_name = match subdir_entry.file_name().to_str() {
      Some(name) => name.to_string(),
      None => continue,
    };

    // Skip hidden directories (includes .tmp*)
    if subdir_name.starts_with('.') {
      continue;
    }

    // Skip directories that don't contain runnable tests
    if IGNORED_TEST_DIRS.contains(&subdir_name.as_str()) {
      continue;
    }

    if !subdir_entry.file_type().is_ok_and(|t| t.is_dir()) {
      continue;
    }

    // Recursively collect test files from this subdirectory
    collect_test_files_recursive(
      &subdir_entry.path(),
      &subdir_name,
      &mut children,
    );
  }

  wrap_in_category(children)
}

fn collect_test_files_recursive(
  dir: &std::path::Path,
  relative_prefix: &str,
  children: &mut Vec<CollectedCategoryOrTest<NodeCompatTestData>>,
) {
  for entry in std::fs::read_dir(dir).unwrap().flatten() {
    let file_name = match entry.file_name().to_str() {
      Some(name) => name.to_string(),
      None => continue,
    };

    // Skip hidden files/directories
    if file_name.starts_with('.') {
      continue;
    }

    let file_type = match entry.file_type() {
      Ok(ft) => ft,
      Err(_) => continue,
    };

    if file_type.is_dir() {
      // Recurse into subdirectory
      let new_prefix = format!("{}/{}", relative_prefix, file_name);
      collect_test_files_recursive(&entry.path(), &new_prefix, children);
    } else {
      // Only include test-*.{js,mjs,cjs,ts} files
      if !file_name.starts_with("test-") {
        continue;
      }
      if !file_name.ends_with(".js")
        && !file_name.ends_with(".mjs")
        && !file_name.ends_with(".cjs")
        && !file_name.ends_with(".ts")
      {
        continue;
      }

      let test_name = format!("{}/{}", relative_prefix, file_name);
      children.push(create_collected_test(&test_name));
    }
  }
}

fn suite_test_dir() -> std::path::PathBuf {
  tests_path()
    .join("node_compat/runner/suite/test")
    .to_path_buf()
}

fn create_collected_test(
  test_name: &str,
) -> CollectedCategoryOrTest<NodeCompatTestData> {
  let test_file_path = suite_test_dir().join(test_name);
  let full_name = format!("node_compat::{}", test_name.replace('/', "::"));

  CollectedCategoryOrTest::Test(CollectedTest {
    name: full_name,
    path: test_file_path,
    line_and_column: None,
    data: NodeCompatTestData {
      test_path: test_name.to_string(),
    },
  })
}

fn wrap_in_category(
  children: Vec<CollectedCategoryOrTest<NodeCompatTestData>>,
) -> CollectedTestCategory<NodeCompatTestData> {
  CollectedTestCategory {
    name: "node_compat".to_string(),
    path: tests_path().join("node_compat").to_path_buf(),
    children,
  }
}

fn platform_expectation(config: &TestConfig) -> Option<&PlatformExpectation> {
  let os = std::env::consts::OS;
  match os {
    "windows" => config.windows.as_ref(),
    "linux" => config.linux.as_ref(),
    "macos" => config.darwin.as_ref(),
    _ => None,
  }
}

fn should_ignore(config: &TestConfig) -> Option<&str> {
  if config.ignore {
    return Some(
      config
        .reason
        .as_deref()
        .expect("tests with `ignore: true` must have a `reason`"),
    );
  }
  if let Some(PlatformExpectation::Enabled(false)) =
    platform_expectation(config)
  {
    let os = std::env::consts::OS;
    return Some(config.reason.as_deref().unwrap_or(match os {
      "windows" => "disabled on windows",
      "linux" => "disabled on linux",
      "macos" => "disabled on macos",
      _ => "disabled on this platform",
    }));
  }
  None
}

/// Resolve the expected-failure expectation for the current platform.
///
/// Returns `Some(ExpectedFailure)` if the test is expected to fail
/// (either from a platform-specific object or from top-level fields),
/// or `None` if the test is expected to pass.
fn resolve_expected_failure(config: &TestConfig) -> Option<ExpectedFailure> {
  // 1. Platform-specific object takes precedence
  if let Some(PlatformExpectation::ExpectedFailure(ef)) =
    platform_expectation(config)
  {
    return Some(ef.clone());
  }

  // 2. Fall back to top-level exitCode / output
  if config.exit_code.is_some() || config.output.is_some() {
    return Some(ExpectedFailure {
      exit_code: config.exit_code,
      output: config.output.clone(),
    });
  }

  None
}

fn uses_node_test_module(source: &str) -> bool {
  source.contains("'node:test'") || source.contains("\"node:test\"")
}

fn parse_flags(source: &str) -> (Vec<String>, Vec<String>) {
  let mut v8_flags = Vec::new();
  let mut node_options = Vec::new();

  let re = Regex::new(r"^// Flags: (.+)$").unwrap();
  for line in source.lines() {
    if let Some(captures) = re.captures(line) {
      let flags_str = captures.get(1).unwrap().as_str();
      for flag in flags_str.split_whitespace() {
        match flag {
          "--expose_externalize_string" => {
            v8_flags.push("--expose-externalize-string".to_string());
          }
          "--expose-gc" => {
            v8_flags.push("--expose-gc".to_string());
          }
          "--no-concurrent-array-buffer-sweeping" => {
            v8_flags.push("--no-concurrent-array-buffer-sweeping".to_string());
          }
          "--no-warnings" => {
            node_options.push("--no-warnings".to_string());
          }
          "--pending-deprecation" => {
            node_options.push("--pending-deprecation".to_string());
          }
          "--allow-natives-syntax" => {
            v8_flags.push("--allow-natives-syntax".to_string());
          }
          _ => {}
        }
      }
      break; // Only process the first Flags: line
    }
  }

  (v8_flags, node_options)
}

fn truncate_output(output: &str, max_len: usize) -> String {
  if output.len() > max_len {
    format!("{} ...", &output[..output.floor_char_boundary(max_len)])
  } else {
    output.to_string()
  }
}

/// Common setup for running a node compat test (shared between PTY and piped paths).
struct TestSetup {
  test_suite_path: PathRef,
  uses_node_test: bool,
  args: Vec<String>,
  env_vars: HashMap<String, String>,
  timeout: Duration,
}

impl TestSetup {
  fn new(cli_args: &CliArgs, data: &NodeCompatTestData) -> Self {
    let test_suite_path = tests_path().join("node_compat/runner/suite");
    let test_path = format!("test/{}", data.test_path);
    let full_test_path = test_suite_path.join(&test_path);

    let source = full_test_path.read_to_string();
    let uses_node_test = uses_node_test_module(&source);
    let (v8_flags, node_options) = parse_flags(&source);

    let mut args: Vec<String> = if uses_node_test {
      TEST_ARGS.iter().map(|s| s.to_string()).collect()
    } else {
      RUN_ARGS.iter().map(|s| s.to_string()).collect()
    };

    if !v8_flags.is_empty() {
      args.push(format!("--v8-flags={}", v8_flags.join(",")));
    }
    if cli_args.inspect_brk {
      args.push("--inspect-brk".to_string());
    }
    if cli_args.inspect_wait {
      args.push("--inspect-wait".to_string());
    }
    args.push(test_path.clone());

    let serial_id = TEST_SERIAL_ID.fetch_add(1, Ordering::SeqCst);

    let mut env_vars = HashMap::new();
    env_vars.insert("NODE_TEST_KNOWN_GLOBALS".to_string(), "0".to_string());
    env_vars.insert("NODE_SKIP_FLAG_CHECK".to_string(), "1".to_string());
    env_vars.insert("NODE_OPTIONS".to_string(), node_options.join(" "));
    env_vars.insert("NO_COLOR".to_string(), "1".to_string());
    env_vars.insert("TEST_SERIAL_ID".to_string(), serial_id.to_string());

    let timeout = Duration::from_millis(if cfg!(target_os = "macos") {
      20_000
    } else {
      10_000
    });

    TestSetup {
      test_suite_path,
      uses_node_test,
      args,
      env_vars,
      timeout,
    }
  }

  fn run_piped(&self) -> (bool, String, Option<i32>) {
    let mut cmd = util::deno_cmd().disable_diagnostic_logging();
    cmd = cmd.current_dir(&self.test_suite_path);
    for arg in &self.args {
      cmd = cmd.arg(arg);
    }
    for (key, value) in &self.env_vars {
      cmd = cmd.env(key, value);
    }

    let child = cmd.piped_output().spawn().unwrap();
    match child.wait_with_output_and_timeout(self.timeout) {
      Ok(output) => {
        let success = output.status.success();
        let exit_code = output.status.code();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let output_str = format!("{}\n{}", stdout, stderr);
        (success, output_str, exit_code)
      }
      Err(_) => {
        let output_str =
          format!("Test timed out after {}ms", self.timeout.as_millis());
        (false, output_str, None)
      }
    }
  }

  fn run_pty(&self) -> (bool, String, Option<i32>) {
    let deno_exe = util::deno_exe_path();
    let args: Vec<&str> = self.args.iter().map(|s| s.as_str()).collect();

    // PTY needs the full environment (inherit + our overrides)
    let mut env_vars = self.env_vars.clone();
    for (key, value) in std::env::vars() {
      env_vars.entry(key).or_insert(value);
    }

    let pty_output = util::pty::run_in_pty(
      deno_exe.as_path(),
      &args,
      self.test_suite_path.as_path(),
      Some(env_vars),
      self.timeout,
    );

    let output_text = String::from_utf8_lossy(&pty_output.output).to_string();
    let success = pty_output.exit_code == Some(0);
    (success, output_text, pty_output.exit_code)
  }

  fn debugging_command_text(&self) -> String {
    let node_options = self
      .env_vars
      .get("NODE_OPTIONS")
      .map(|s| s.as_str())
      .unwrap_or("");
    let args_str = self
      .args
      .iter()
      .map(|a| {
        if a.contains(' ') {
          format!("'{}'", a)
        } else {
          a.clone()
        }
      })
      .collect::<Vec<_>>()
      .join(" ");
    format!(
      "Command: {}",
      deno_terminal::colors::gray(format!(
        "NODE_TEST_KNOWN_GLOBALS=0 NODE_SKIP_FLAG_CHECK=1 NODE_OPTIONS='{}' deno {}",
        node_options.replace("'", "\\'"),
        args_str,
      ))
    )
  }
}

fn make_collected_result(
  success: bool,
  output_text: &str,
  uses_node_test: bool,
  timeout: Duration,
  timed_out: bool,
  exit_code: Option<i32>,
) -> CollectedResult {
  if success {
    CollectedResult {
      passed: Some(true),
      error: None,
      uses_node_test,
      ignore_reason: None,
    }
  } else {
    CollectedResult {
      passed: Some(false),
      error: Some(ErrorInfo {
        code: exit_code,
        stderr: if output_text.is_empty() {
          None
        } else {
          Some(truncate_output(output_text, 2000))
        },
        timeout: if timed_out {
          Some(timeout.as_millis() as u64)
        } else {
          None
        },
        message: None,
      }),
      uses_node_test,
      ignore_reason: None,
    }
  }
}

fn run_test(
  cli_args: &CliArgs,
  test: &CollectedTest<NodeCompatTestData>,
  test_config: Option<&TestConfig>,
  results: &Arc<Mutex<HashMap<String, CollectedResult>>>,
) -> TestResult {
  let data = &test.data;

  // Check platform-specific ignores
  if let Some(reason) = test_config.and_then(|c| should_ignore(c)) {
    results.lock().unwrap().insert(
      data.test_path.clone(),
      CollectedResult {
        passed: None,
        error: None,
        uses_node_test: false,
        ignore_reason: Some(reason.to_string()),
      },
    );
    return TestResult::Ignored;
  }

  // Skip pseudo-tty tests when PTY is not supported (e.g., Windows CI)
  let is_pseudo_tty_test = data.test_path.starts_with("pseudo-tty/");
  if is_pseudo_tty_test && !util::pty::Pty::is_supported() {
    results.lock().unwrap().insert(
      data.test_path.clone(),
      CollectedResult {
        passed: None,
        error: None,
        uses_node_test: false,
        ignore_reason: Some("PTY not supported on this platform".to_string()),
      },
    );
    return TestResult::Ignored;
  }

  let setup = TestSetup::new(cli_args, data);

  let (success, output_text, exit_code) = if is_pseudo_tty_test {
    setup.run_pty()
  } else {
    setup.run_piped()
  };

  let debugging_command_text = setup.debugging_command_text();

  // Check for expected failure configuration
  if let Some(ef) = test_config.and_then(resolve_expected_failure) {
    return handle_expected_failure(
      &ef,
      success,
      exit_code,
      setup.uses_node_test,
      &output_text,
      &debugging_command_text,
      &data.test_path,
      results,
    );
  }

  let collected = make_collected_result(
    success,
    &output_text,
    setup.uses_node_test,
    setup.timeout,
    !success && output_text.starts_with("Test timed out"),
    exit_code,
  );

  results
    .lock()
    .unwrap()
    .insert(data.test_path.clone(), collected);

  if success {
    if *file_test_runner::NO_CAPTURE {
      test_util::eprintln!("{}", debugging_command_text);
    }
    TestResult::Passed { duration: None }
  } else {
    TestResult::Failed {
      duration: None,
      output: format!("{}\n{}", output_text, debugging_command_text)
        .into_bytes(),
    }
  }
}

/// Handle a test that has an expected-failure configuration.
///
/// Returns `Passed` when the test fails in the expected way (matching exit code
/// and/or output pattern), and `Failed` otherwise.
#[allow(clippy::too_many_arguments)]
fn handle_expected_failure(
  ef: &ExpectedFailure,
  success: bool,
  actual_exit_code: Option<i32>,
  uses_node_test: bool,
  output_text: &str,
  debugging_command_text: &str,
  test_path: &str,
  results: &Arc<Mutex<HashMap<String, CollectedResult>>>,
) -> TestResult {
  if success {
    // Test passed but was expected to fail
    results.lock().unwrap().insert(
      test_path.to_string(),
      CollectedResult {
        passed: Some(false),
        error: Some(ErrorInfo {
          code: actual_exit_code,
          stderr: None,
          timeout: None,
          message: Some("expected test to fail but it passed".to_string()),
        }),
        uses_node_test,
        ignore_reason: None,
      },
    );
    return TestResult::Failed {
      duration: None,
      output: format!(
        "Test was expected to fail but passed\n{}",
        debugging_command_text
      )
      .into_bytes(),
    };
  }

  // When exitCode/output are omitted, any value is accepted.
  let exit_code_matches =
    ef.exit_code.is_none_or(|ec| actual_exit_code == Some(ec));
  let output_matches = ef.output.as_ref().is_none_or(|pattern| {
    util::wildcard_match_detailed(pattern, output_text).is_success()
  });

  if exit_code_matches && output_matches {
    // Failed as expected — treat as a pass
    results.lock().unwrap().insert(
      test_path.to_string(),
      CollectedResult {
        passed: Some(true),
        error: None,
        uses_node_test,
        ignore_reason: None,
      },
    );
    if *file_test_runner::NO_CAPTURE {
      test_util::eprintln!("{}", debugging_command_text);
    }
    return TestResult::Passed { duration: None };
  }

  // Failed, but not in the expected way
  let mut mismatch_details = String::new();
  if !exit_code_matches {
    mismatch_details.push_str(&format!(
      "Exit code mismatch: expected {:?}, got {:?}\n",
      ef.exit_code, actual_exit_code
    ));
  }
  if !output_matches {
    let match_result = ef
      .output
      .as_ref()
      .map(|pattern| util::wildcard_match_detailed(pattern, output_text));
    if let Some(util::WildcardMatchResult::Fail(detail)) = match_result {
      mismatch_details
        .push_str(&format!("Output mismatch detail:\n{}\n", detail));
    } else {
      mismatch_details.push_str("Output did not match expected pattern\n");
    }
  }

  results.lock().unwrap().insert(
    test_path.to_string(),
    CollectedResult {
      passed: Some(false),
      error: Some(ErrorInfo {
        code: actual_exit_code,
        stderr: Some(truncate_output(output_text, 2000)),
        timeout: None,
        message: Some(mismatch_details.clone()),
      }),
      uses_node_test,
      ignore_reason: None,
    },
  );
  TestResult::Failed {
    duration: None,
    output: format!(
      "Test failed but not in the expected way:\n{}\nActual output:\n{}\n{}",
      mismatch_details, output_text, debugging_command_text
    )
    .into_bytes(),
  }
}
