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

/// Configuration for a single test from config.json
#[derive(Debug, Clone, Default, Deserialize)]
struct TestConfig {
  #[serde(default)]
  flaky: bool,
  windows: Option<bool>,
  darwin: Option<bool>,
  linux: Option<bool>,
  reason: Option<String>,
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
  let cli_args = parse_cli_args();
  let config = load_config();
  let mut category = if cli_args.report {
    collect_all_tests()
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
          test_config.is_some_and(|c| c.flaky),
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
          test_config.is_some_and(|c| c.flaky),
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
}

#[derive(Clone)]
struct CliArgs {
  inspect_brk: bool,
  inspect_wait: bool,
  report: bool,
}

// You need to run with `--test node_compat` for this to work.
// For example: `cargo test --test node_compat <test-file-name> -- --inspect-brk`
fn parse_cli_args() -> CliArgs {
  let mut inspect_brk = false;
  let mut inspect_wait = false;
  let mut node_compat_report = false;
  for arg in std::env::args() {
    match arg.as_str() {
      "--inspect-brk" => inspect_brk = true,
      "--inspect-wait" => inspect_wait = true,
      "--report" => node_compat_report = true,
      _ => {}
    }
  }

  CliArgs {
    inspect_brk,
    inspect_wait,
    report: node_compat_report,
  }
}

fn load_config() -> NodeCompatConfig {
  let config_path = tests_path().join("node_compat").join("config.json");
  let config_content =
    std::fs::read_to_string(&config_path).expect("Failed to read config.json");
  serde_json::from_str(&config_content).expect("Failed to parse config.json")
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

/// Collect all test files from the suite directory.
fn collect_all_tests() -> CollectedTestCategory<NodeCompatTestData> {
  let suite_dir = suite_test_dir();
  let mut children = Vec::new();

  for subdir in ["parallel", "sequential"] {
    let dir_path = suite_dir.join(subdir);
    let entries = match std::fs::read_dir(&dir_path) {
      Ok(entries) => entries,
      Err(_) => continue,
    };

    for entry in entries.flatten() {
      let path = entry.path();
      if !path.is_file() {
        continue;
      }

      let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name,
        None => continue,
      };

      // Only include test-*.js and test-*.mjs files
      if !file_name.starts_with("test-") {
        continue;
      }
      if !file_name.ends_with(".js") && !file_name.ends_with(".mjs") {
        continue;
      }

      let test_name = format!("{}/{}", subdir, file_name);
      children.push(create_collected_test(&test_name));
    }
  }

  wrap_in_category(children)
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

fn should_ignore(config: &TestConfig) -> Option<&str> {
  let os = std::env::consts::OS;
  match os {
    "windows" if config.windows == Some(false) => {
      Some(config.reason.as_deref().unwrap_or("disabled on windows"))
    }
    "linux" if config.linux == Some(false) => {
      Some(config.reason.as_deref().unwrap_or("disabled on linux"))
    }
    "macos" if config.darwin == Some(false) => {
      Some(config.reason.as_deref().unwrap_or("disabled on macos"))
    }
    _ => None,
  }
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
  use std::sync::mpsc;
  use std::thread;

  let (tx, rx) = mpsc::channel();

  // Spawn thread to wait for child
  thread::spawn(move || {
    let result = child.wait_with_output();
    let _ = tx.send(result);
  });

  match rx.recv_timeout(timeout) {
    Ok(Ok(output)) => TestOutput::Completed(output),
    Ok(Err(_)) => TestOutput::TimedOut, // IO error treated as timeout
    Err(mpsc::RecvTimeoutError::Timeout) => TestOutput::TimedOut,
    Err(mpsc::RecvTimeoutError::Disconnected) => TestOutput::TimedOut,
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
  let child = cmd.piped_output().spawn().unwrap();
  let test_output = wait_with_timeout(child, timeout);

  let (success, collected, output_for_error) = match test_output {
    TestOutput::Completed(output) => {
      let success = output.status.success();
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
            code: output.status.code(),
            stderr: Some(truncate_output(&output_text, 2000)),
            timeout: None,
            message: None,
          }),
          uses_node_test,
          ignore_reason: None,
        }
      };
      let output_str = format!("{}\n{}", stdout, stderr);
      (success, collected, output_str)
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
      (false, collected, output_str)
    }
  };

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
