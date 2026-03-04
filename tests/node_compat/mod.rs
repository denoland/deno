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
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use test_util as util;
use test_util::IS_CI;
use test_util::test_runner::FlakyTestTracker;
use test_util::test_runner::Parallelism;
use test_util::test_runner::run_maybe_flaky_test;
use util::tests_path;

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

/// Report structures for generating report.json
#[derive(Debug, Serialize)]
struct TestReport {
  date: String,
  #[serde(rename = "denoVersion")]
  deno_version: String,
  os: String,
  arch: String,
  #[serde(rename = "nodeVersion")]
  node_version: String,
  #[serde(rename = "runId")]
  run_id: Option<String>,
  total: usize,
  pass: usize,
  ignore: usize,
  results: HashMap<String, TestResultEntry>,
}

// Result entry: [pass: bool | "IGNORE", error: Option<ErrorInfo>, info: ResultInfo]
type TestResultEntry = (Value, Option<ErrorInfo>, ResultInfo);

#[derive(Debug, Serialize, Clone)]
struct ErrorInfo {
  #[serde(skip_serializing_if = "Option::is_none")]
  code: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  stderr: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  timeout: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  message: Option<String>,
}

#[derive(Debug, Default, Serialize, Clone)]
struct ResultInfo {
  #[serde(rename = "usesNodeTest", skip_serializing_if = "Option::is_none")]
  uses_node_test: Option<u8>,
  #[serde(rename = "ignoreReason", skip_serializing_if = "Option::is_none")]
  ignore_reason: Option<String>,
}

/// Collected test result for report generation
#[derive(Debug, Clone)]
struct CollectedResult {
  passed: Option<bool>, // None means ignored
  error: Option<ErrorInfo>,
  uses_node_test: bool,
  ignore_reason: Option<String>,
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
  let mut category = if cli_args.report {
    collect_all_tests()
  } else if let Some(filter) = cli_args.filter.as_ref() {
    let mut category = collect_all_tests();
    category.filter_children(filter);
    category
  } else {
    collect_tests_from_config(&config)
  };

  // Apply CLI filter if provided
  if let Some(filter) = file_test_runner::collection::parse_cli_arg_filter() {
    category.filter_children(&filter);
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
    generate_report(&results.lock().unwrap());
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
  filter: Option<String>,
}

// You need to run with `--test node_compat` for this to work.
// For example: `cargo test --test node_compat <test-file-name> -- --inspect-brk`
fn parse_cli_args() -> CliArgs {
  let mut inspect_brk = false;
  let mut inspect_wait = false;
  let mut report = false;
  let mut filter = None;

  let mut has_filter = false;
  for arg in std::env::args() {
    if has_filter {
      filter = Some(arg.as_str().to_string());
      has_filter = false;
    }

    match arg.as_str() {
      "--inspect-brk" => inspect_brk = true,
      "--inspect-wait" => inspect_wait = true,
      "--report" => report = true,
      "--filter" => {
        has_filter = true;
      }
      _ => {}
    }
  }

  CliArgs {
    inspect_brk,
    inspect_wait,
    report,
    filter,
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

fn collect_tests_from_config(
  config: &NodeCompatConfig,
) -> CollectedTestCategory<NodeCompatTestData> {
  let children = config
    .tests
    .keys()
    .map(|test_name| create_collected_test(test_name))
    .collect();

  wrap_in_category(children)
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
    format!("{} ...", &output[..max_len])
  } else {
    output.to_string()
  }
}

enum TestOutput {
  Completed(std::process::Output),
  TimedOut,
}

fn wait_with_timeout(
  child: test_util::DenoChild,
  timeout: Duration,
) -> TestOutput {
  match child.wait_with_output_and_timeout(timeout) {
    Ok(output) => TestOutput::Completed(output),
    Err(_) => TestOutput::TimedOut,
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

  let test_suite_path = tests_path().join("node_compat/runner/suite");
  let test_path = format!("test/{}", data.test_path);
  let full_test_path = test_suite_path.join(&test_path);

  // Read source to extract flags and detect node:test usage
  let source = std::fs::read_to_string(&full_test_path).unwrap_or_default();
  let uses_node_test = uses_node_test_module(&source);
  let (v8_flags, node_options) = parse_flags(&source);

  // Build command
  let mut cmd = util::deno_cmd().disable_diagnostic_logging();
  cmd = cmd.current_dir(&test_suite_path);

  // Choose deno test vs deno run
  if uses_node_test {
    for arg in TEST_ARGS {
      cmd = cmd.arg(arg);
    }
  } else {
    for arg in RUN_ARGS {
      cmd = cmd.arg(arg);
    }
  }

  // Add V8 flags
  if !v8_flags.is_empty() {
    cmd = cmd.arg(format!("--v8-flags={}", v8_flags.join(",")));
  }

  if cli_args.inspect_brk {
    cmd = cmd.arg("--inspect-brk");
  }
  if cli_args.inspect_wait {
    cmd = cmd.arg("--inspect-wait");
  }

  // Add test file
  cmd = cmd.arg(&test_path);

  // Generate unique serial ID for this test (used for temp directory isolation)
  let serial_id = TEST_SERIAL_ID.fetch_add(1, Ordering::SeqCst);

  // Set environment variables
  cmd = cmd
    .env("NODE_TEST_KNOWN_GLOBALS", "0")
    .env("NODE_SKIP_FLAG_CHECK", "1")
    .env("NODE_OPTIONS", node_options.join(" "))
    .env("NO_COLOR", "1")
    .env("TEST_SERIAL_ID", serial_id.to_string());

  let debugging_command_text = format!(
    "Command: {}",
    deno_terminal::colors::gray(format!(
      "NODE_TEST_KNOWN_GLOBALS=0 NODE_SKIP_FLAG_CHECK=1 NODE_OPTIONS='{}' {}",
      node_options.join(" ").replace("'", "\\'"),
      cmd.build_command_text_for_debugging()
    ))
  );

  let timeout = Duration::from_millis(if cfg!(target_os = "macos") {
    20_000
  } else {
    10_000
  });

  // Format v8_flags for reuse in both PTY and non-PTY paths
  let v8_flags_arg = if !v8_flags.is_empty() {
    Some(format!("--v8-flags={}", v8_flags.join(",")))
  } else {
    None
  };

  let (success, actual_exit_code, collected, output_for_error) =
    if is_pseudo_tty_test {
      // Run in PTY for pseudo-tty tests (PTY support was already verified above)
      let deno_exe = util::deno_exe_path();
      let mut args: Vec<&str> = if uses_node_test {
        TEST_ARGS.to_vec()
      } else {
        RUN_ARGS.to_vec()
      };

      // Add V8 flags
      if let Some(ref flags) = v8_flags_arg {
        args.push(flags);
      }

      // Add inspect flags
      if cli_args.inspect_brk {
        args.push("--inspect-brk");
      }
      if cli_args.inspect_wait {
        args.push("--inspect-wait");
      }

      args.push(&test_path);

      let mut env_vars = std::collections::HashMap::new();
      env_vars.insert("NODE_TEST_KNOWN_GLOBALS".to_string(), "0".to_string());
      env_vars.insert("NODE_SKIP_FLAG_CHECK".to_string(), "1".to_string());
      env_vars.insert("NODE_OPTIONS".to_string(), node_options.join(" "));
      env_vars.insert("NO_COLOR".to_string(), "1".to_string());
      env_vars.insert("TEST_SERIAL_ID".to_string(), serial_id.to_string());
      // Inherit current environment
      for (key, value) in std::env::vars() {
        env_vars.entry(key).or_insert(value);
      }

      let pty_output = util::pty::run_in_pty(
        deno_exe.as_path(),
        &args,
        test_suite_path.as_path(),
        Some(env_vars),
        timeout,
      );

      let output_text = String::from_utf8_lossy(&pty_output.output).to_string();
      let exit_code = pty_output.exit_code;
      let success = exit_code == Some(0);

      let collected = if success {
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
            stderr: Some(truncate_output(&output_text, 2000)),
            timeout: if exit_code.is_none() {
              Some(timeout.as_millis() as u64)
            } else {
              None
            },
            message: None,
          }),
          uses_node_test,
          ignore_reason: None,
        }
      };

      (success, exit_code, collected, output_text)
    } else {
      // Run normally with piped output
      let child = cmd.piped_output().spawn().unwrap();
      let test_output = wait_with_timeout(child, timeout);

      match test_output {
        TestOutput::Completed(output) => {
          let success = output.status.success();
          let exit_code = output.status.code();
          let stderr = String::from_utf8_lossy(&output.stderr);
          let stdout = String::from_utf8_lossy(&output.stdout);
          let output_text = if uses_node_test {
            stdout.to_string()
          } else {
            stderr.to_string()
          };

          let collected = if success {
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
                stderr: Some(truncate_output(&output_text, 2000)),
                timeout: None,
                message: None,
              }),
              uses_node_test,
              ignore_reason: None,
            }
          };
          let output_str = format!("{}\n{}", stdout, stderr);
          (success, exit_code, collected, output_str)
        }
        TestOutput::TimedOut => {
          let collected = CollectedResult {
            passed: Some(false),
            error: Some(ErrorInfo {
              code: None,
              stderr: None,
              timeout: Some(timeout.as_millis() as u64),
              message: None,
            }),
            uses_node_test,
            ignore_reason: None,
          };
          let output_str =
            format!("Test timed out after {}ms", timeout.as_millis());
          (false, None, collected, output_str)
        }
      }
    };

  // Check for expected failure configuration
  let expected_failure = test_config.and_then(resolve_expected_failure);

  if let Some(ef) = expected_failure {
    // Test has an expected-failure spec for this platform
    if success {
      // Test passed but was expected to fail
      results.lock().unwrap().insert(
        data.test_path.clone(),
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

    let exit_code_matches =
      ef.exit_code.map_or(true, |ec| actual_exit_code == Some(ec));
    let output_matches = ef.output.as_ref().map_or(true, |pattern| {
      util::wildcard_match_detailed(pattern, &output_for_error).is_success()
    });

    if exit_code_matches && output_matches {
      // Failed as expected — treat as a pass
      results.lock().unwrap().insert(
        data.test_path.clone(),
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
      let match_result = ef.output.as_ref().map(|pattern| {
        util::wildcard_match_detailed(pattern, &output_for_error)
      });
      if let Some(util::WildcardMatchResult::Fail(detail)) = match_result {
        mismatch_details
          .push_str(&format!("Output mismatch detail:\n{}\n", detail));
      } else {
        mismatch_details.push_str("Output did not match expected pattern\n");
      }
    }

    results.lock().unwrap().insert(
      data.test_path.clone(),
      CollectedResult {
        passed: Some(false),
        error: Some(ErrorInfo {
          code: actual_exit_code,
          stderr: Some(truncate_output(&output_for_error, 2000)),
          timeout: None,
          message: Some(mismatch_details.clone()),
        }),
        uses_node_test,
        ignore_reason: None,
      },
    );
    return TestResult::Failed {
      duration: None,
      output: format!(
        "Test failed but not in the expected way:\n{}\nActual output:\n{}\n{}",
        mismatch_details, output_for_error, debugging_command_text
      )
      .into_bytes(),
    };
  }

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
      output: format!("{}\n{}", output_for_error, debugging_command_text)
        .into_bytes(),
    }
  }
}

fn generate_report(results: &HashMap<String, CollectedResult>) {
  let node_version = read_node_version();
  let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
  let deno_version = get_deno_version();
  let os = std::env::consts::OS.to_string();
  let arch = std::env::consts::ARCH.to_string();
  let run_id = std::env::var("GITHUB_RUN_ID").ok();

  let mut report_results: HashMap<String, TestResultEntry> = HashMap::new();
  let mut pass_count = 0;
  let mut ignore_count = 0;

  for (test_path, result) in results {
    let entry = match result.passed {
      Some(true) => {
        pass_count += 1;
        let info = ResultInfo {
          uses_node_test: if result.uses_node_test { Some(1) } else { None },
          ignore_reason: None,
        };
        (Value::Bool(true), None, info)
      }
      Some(false) => {
        let info = ResultInfo {
          uses_node_test: if result.uses_node_test { Some(1) } else { None },
          ignore_reason: None,
        };
        (Value::Bool(false), result.error.clone(), info)
      }
      None => {
        ignore_count += 1;
        let info = ResultInfo {
          uses_node_test: None,
          ignore_reason: result.ignore_reason.clone(),
        };
        (Value::String("IGNORE".to_string()), None, info)
      }
    };
    report_results.insert(test_path.clone(), entry);
  }

  let total = results.len() - ignore_count;
  let report = TestReport {
    date,
    deno_version,
    os,
    arch,
    node_version,
    run_id,
    total,
    pass: pass_count,
    ignore: ignore_count,
    results: report_results,
  };

  let report_path = tests_path().join("node_compat").join("report.json");
  let json = serde_json::to_string(&report).unwrap();
  std::fs::write(&report_path, json).unwrap();
}

fn get_deno_version() -> String {
  // Run `deno -v` to get the actual version
  let output = std::process::Command::new(util::deno_exe_path().as_path())
    .arg("-v")
    .output()
    .ok()
    .unwrap();

  let stdout = String::from_utf8_lossy(&output.stdout);
  // Parse: "deno 2.x.x (...)"
  let line = stdout.lines().next().unwrap();
  let version = line.strip_prefix("deno ").unwrap();
  let version = version.split_whitespace().next().unwrap();
  version.to_string()
}

fn read_node_version() -> String {
  // Read from tests/node_compat/runner/suite/node_version.ts
  let version_file =
    tests_path().join("node_compat/runner/suite/node_version.ts");
  let content = std::fs::read_to_string(&version_file).unwrap_or_default();

  // Parse: export const version = "24.2.0";
  let re = Regex::new(r#"export const version = "([^"]+)"#).unwrap();
  let captures = re.captures(&content).unwrap();
  captures.get(1).unwrap().as_str().to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_deserialize_legacy_bool_platform() {
    let json = r#"{ "windows": false, "reason": "disabled on windows" }"#;
    let config: TestConfig = serde_json::from_str(json).unwrap();
    assert!(matches!(
      config.windows,
      Some(PlatformExpectation::Enabled(false))
    ));
    assert!(config.darwin.is_none());
  }

  #[test]
  fn test_deserialize_platform_expected_failure() {
    let json =
      r#"{ "windows": { "exitCode": 1, "output": "boom [WILDCARD]" } }"#;
    let config: TestConfig = serde_json::from_str(json).unwrap();
    match &config.windows {
      Some(PlatformExpectation::ExpectedFailure(ef)) => {
        assert_eq!(ef.exit_code, Some(1));
        assert_eq!(ef.output.as_deref(), Some("boom [WILDCARD]"));
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_deserialize_top_level_expected_failure() {
    let json = r#"{ "exitCode": 2, "output": "error [WILDCARD]" }"#;
    let config: TestConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.exit_code, Some(2));
    assert_eq!(config.output.as_deref(), Some("error [WILDCARD]"));
    assert!(config.windows.is_none());
  }

  #[test]
  fn test_deserialize_mixed_platforms() {
    let json = r#"{
      "linux": true,
      "darwin": true,
      "windows": { "exitCode": 1, "output": "Failed to create file at [WILDCARD]" }
    }"#;
    let config: TestConfig = serde_json::from_str(json).unwrap();
    assert!(matches!(
      config.linux,
      Some(PlatformExpectation::Enabled(true))
    ));
    assert!(matches!(
      config.darwin,
      Some(PlatformExpectation::Enabled(true))
    ));
    match &config.windows {
      Some(PlatformExpectation::ExpectedFailure(ef)) => {
        assert_eq!(ef.exit_code, Some(1));
        assert!(ef.output.as_deref().unwrap().contains("[WILDCARD]"));
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_deserialize_empty_config() {
    let json = r#"{}"#;
    let config: TestConfig = serde_json::from_str(json).unwrap();
    assert!(config.windows.is_none());
    assert!(config.darwin.is_none());
    assert!(config.linux.is_none());
    assert!(!config.flaky);
    assert!(config.exit_code.is_none());
    assert!(config.output.is_none());
  }

  #[test]
  fn test_should_ignore_disabled_platform() {
    let config = TestConfig {
      windows: Some(PlatformExpectation::Enabled(false)),
      reason: Some("broken on windows".to_string()),
      ..Default::default()
    };
    let os = std::env::consts::OS;
    if os == "windows" {
      assert!(should_ignore(&config).is_some());
    } else {
      assert!(should_ignore(&config).is_none());
    }
  }

  #[test]
  fn test_should_ignore_expected_failure_is_not_ignored() {
    // A platform with an ExpectedFailure should NOT be ignored - it should run
    let config = TestConfig {
      windows: Some(PlatformExpectation::ExpectedFailure(ExpectedFailure {
        exit_code: Some(1),
        output: None,
      })),
      ..Default::default()
    };
    // Even on Windows, should_ignore should return None because this is
    // an expected-failure, not a disabled test
    assert!(should_ignore(&config).is_none());
  }

  #[test]
  fn test_resolve_expected_failure_platform_specific() {
    let config = TestConfig {
      windows: Some(PlatformExpectation::ExpectedFailure(ExpectedFailure {
        exit_code: Some(1),
        output: Some("error [WILDCARD]".to_string()),
      })),
      ..Default::default()
    };
    let ef = resolve_expected_failure(&config);
    let os = std::env::consts::OS;
    if os == "windows" {
      let ef = ef.unwrap();
      assert_eq!(ef.exit_code, Some(1));
      assert_eq!(ef.output.as_deref(), Some("error [WILDCARD]"));
    } else {
      assert!(ef.is_none());
    }
  }

  #[test]
  fn test_resolve_expected_failure_top_level() {
    let config = TestConfig {
      exit_code: Some(2),
      output: Some("fail".to_string()),
      ..Default::default()
    };
    let ef = resolve_expected_failure(&config).unwrap();
    assert_eq!(ef.exit_code, Some(2));
    assert_eq!(ef.output.as_deref(), Some("fail"));
  }

  #[test]
  fn test_resolve_expected_failure_platform_overrides_top_level() {
    // On the current OS, platform config should override top-level
    let os = std::env::consts::OS;
    let platform_ef = ExpectedFailure {
      exit_code: Some(42),
      output: Some("platform specific".to_string()),
    };
    let mut config = TestConfig {
      exit_code: Some(1),
      output: Some("global".to_string()),
      ..Default::default()
    };
    match os {
      "windows" => {
        config.windows = Some(PlatformExpectation::ExpectedFailure(platform_ef))
      }
      "linux" => {
        config.linux = Some(PlatformExpectation::ExpectedFailure(platform_ef))
      }
      "macos" => {
        config.darwin = Some(PlatformExpectation::ExpectedFailure(platform_ef))
      }
      _ => {}
    }

    let ef = resolve_expected_failure(&config).unwrap();
    assert_eq!(ef.exit_code, Some(42));
    assert_eq!(ef.output.as_deref(), Some("platform specific"));
  }

  #[test]
  fn test_resolve_expected_failure_platform_true_uses_top_level() {
    // If platform is simply `true`, fall back to top-level
    let os = std::env::consts::OS;
    let mut config = TestConfig {
      exit_code: Some(1),
      output: Some("global".to_string()),
      ..Default::default()
    };
    match os {
      "windows" => config.windows = Some(PlatformExpectation::Enabled(true)),
      "linux" => config.linux = Some(PlatformExpectation::Enabled(true)),
      "macos" => config.darwin = Some(PlatformExpectation::Enabled(true)),
      _ => {}
    }

    let ef = resolve_expected_failure(&config).unwrap();
    assert_eq!(ef.exit_code, Some(1));
    assert_eq!(ef.output.as_deref(), Some("global"));
  }

  #[test]
  fn test_resolve_no_expected_failure() {
    let config = TestConfig::default();
    assert!(resolve_expected_failure(&config).is_none());
  }

  #[test]
  fn test_full_config_deserialization() {
    let json = r#"{
      "tests": {
        "test-pass.js": {},
        "test-disabled.js": {
          "windows": false,
          "reason": "broken"
        },
        "test-flaky.js": {
          "flaky": true
        },
        "test-expected-fail-all.js": {
          "exitCode": 1,
          "output": "error [WILDCARD]"
        },
        "test-expected-fail-windows.js": {
          "windows": {
            "exitCode": 1,
            "output": "Failed to create file at [WILDCARD]"
          }
        },
        "test-mixed.js": {
          "linux": true,
          "darwin": true,
          "windows": {
            "exitCode": 1,
            "output": "Failed [WILDCARD]"
          },
          "flaky": true
        }
      }
    }"#;
    let config: NodeCompatConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.tests.len(), 6);

    // Empty config
    let tc = &config.tests["test-pass.js"];
    assert!(tc.windows.is_none());

    // Disabled
    let tc = &config.tests["test-disabled.js"];
    assert!(matches!(
      tc.windows,
      Some(PlatformExpectation::Enabled(false))
    ));

    // Top-level expected failure
    let tc = &config.tests["test-expected-fail-all.js"];
    assert_eq!(tc.exit_code, Some(1));

    // Platform-specific expected failure
    let tc = &config.tests["test-expected-fail-windows.js"];
    match &tc.windows {
      Some(PlatformExpectation::ExpectedFailure(ef)) => {
        assert_eq!(ef.exit_code, Some(1));
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_expected_failure_exit_code_only() {
    let json = r#"{ "windows": { "exitCode": 3 } }"#;
    let config: TestConfig = serde_json::from_str(json).unwrap();
    match &config.windows {
      Some(PlatformExpectation::ExpectedFailure(ef)) => {
        assert_eq!(ef.exit_code, Some(3));
        assert!(ef.output.is_none());
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_expected_failure_output_only() {
    let json = r#"{ "exitCode": null, "output": "some error" }"#;
    let config: TestConfig = serde_json::from_str(json).unwrap();
    assert!(config.exit_code.is_none());
    assert_eq!(config.output.as_deref(), Some("some error"));
    let ef = resolve_expected_failure(&config).unwrap();
    assert!(ef.exit_code.is_none());
    assert_eq!(ef.output.as_deref(), Some("some error"));
  }
}
