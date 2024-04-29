// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::panic::AssertUnwindSafe;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::serde_json;
use file_test_runner::collection::collect_tests_or_exit;
use file_test_runner::collection::strategies::TestPerDirectoryCollectionStrategy;
use file_test_runner::collection::CollectOptions;
use file_test_runner::collection::CollectedTest;
use serde::Deserialize;
use test_util::tests_path;
use test_util::PathRef;
use test_util::TestContextBuilder;

const MANIFEST_FILE_NAME: &str = "__test__.jsonc";

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum VecOrString {
  Vec(Vec<String>),
  String(String),
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct MultiTestMetaData {
  /// Whether to copy all the non-assertion files in the current
  /// test directory to a temporary directory before running the
  /// steps.
  #[serde(default)]
  pub temp_dir: bool,
  /// The base environment to use for the test.
  #[serde(default)]
  pub base: Option<String>,
  #[serde(default)]
  pub envs: HashMap<String, String>,
  pub steps: Vec<StepMetaData>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SingleTestMetaData {
  #[serde(default)]
  pub base: Option<String>,
  #[serde(default)]
  pub temp_dir: bool,
  #[serde(flatten)]
  pub step: StepMetaData,
}

impl SingleTestMetaData {
  pub fn into_multi(self) -> MultiTestMetaData {
    MultiTestMetaData {
      base: self.base,
      temp_dir: self.temp_dir,
      envs: Default::default(),
      steps: vec![self.step],
    }
  }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StepMetaData {
  /// Whether to clean the deno_dir before running the step.
  #[serde(default)]
  pub clean_deno_dir: bool,
  /// If the test should be retried multiple times on failure.
  #[serde(default)]
  pub flaky: bool,
  pub args: VecOrString,
  pub cwd: Option<String>,
  #[serde(rename = "if")]
  pub if_cond: Option<String>,
  pub command_name: Option<String>,
  #[serde(default)]
  pub envs: HashMap<String, String>,
  pub output: String,
  #[serde(default)]
  pub exit_code: i32,
}

pub fn main() {
  let root_category = collect_tests_or_exit(CollectOptions {
    base: tests_path().join("specs").to_path_buf(),
    strategy: Box::new(TestPerDirectoryCollectionStrategy {
      file_name: MANIFEST_FILE_NAME.to_string(),
    }),
    filter_override: None,
  });

  if root_category.is_empty() {
    return; // all tests filtered out
  }

  let _http_guard = test_util::http_server();
  file_test_runner::run_tests(
    &root_category,
    file_test_runner::RunOptions { parallel: true },
    Arc::new(|test| {
      let diagnostic_logger = Rc::new(RefCell::new(Vec::<u8>::new()));
      let result = file_test_runner::TestResult::from_maybe_panic(
        AssertUnwindSafe(|| run_test(test, diagnostic_logger.clone())),
      );
      match result {
        file_test_runner::TestResult::Passed
        | file_test_runner::TestResult::Ignored => result,
        file_test_runner::TestResult::Failed {
          output: panic_output,
        } => {
          let mut output = diagnostic_logger.borrow().clone();
          output.push(b'\n');
          output.extend(panic_output);
          file_test_runner::TestResult::Failed { output }
        }
        file_test_runner::TestResult::Steps(_) => unreachable!(),
      }
    }),
  );
}

fn run_test(test: &CollectedTest, diagnostic_logger: Rc<RefCell<Vec<u8>>>) {
  let metadata_path = PathRef::new(&test.path);
  let metadata_value = metadata_path.read_jsonc_value();
  // checking for "steps" leads to a more targeted error message
  // instead of when deserializing an untagged enum
  let metadata = if metadata_value
    .as_object()
    .and_then(|o| o.get("steps"))
    .is_some()
  {
    serde_json::from_value::<MultiTestMetaData>(metadata_value)
  } else {
    serde_json::from_value::<SingleTestMetaData>(metadata_value)
      .map(|s| s.into_multi())
  }
  .with_context(|| format!("Failed to parse {}", metadata_path))
  .unwrap();

  let mut builder = TestContextBuilder::new();
  builder = builder.logging_capture(diagnostic_logger);
  let cwd = PathRef::new(test.path.parent().unwrap());

  if metadata.temp_dir {
    builder = builder.use_temp_cwd();
  } else {
    builder = builder.cwd(cwd.to_string_lossy());
  }

  match &metadata.base {
    // todo(dsherret): add bases in the future as needed
    Some(base) => panic!("Unknown test base: {}", base),
    None => {
      // by default add all these
      builder = builder
        .add_jsr_env_vars()
        .add_npm_env_vars()
        .add_compile_env_vars();
    }
  }

  let context = builder.build();

  if metadata.temp_dir {
    // copy all the files in the cwd to a temp directory
    // excluding the metadata and assertion files
    let temp_dir = context.temp_dir().path();
    let assertion_paths = resolve_test_and_assertion_files(&cwd, &metadata);
    cwd.copy_to_recursive_with_exclusions(temp_dir, &assertion_paths);
  }

  for step in metadata.steps.iter().filter(|s| should_run_step(s)) {
    let run_func = || run_step(step, &metadata, &cwd, &context);
    if step.flaky {
      run_flaky(run_func);
    } else {
      run_func();
    }
  }
}

fn should_run_step(step: &StepMetaData) -> bool {
  if let Some(cond) = &step.if_cond {
    match cond.as_str() {
      "windows" => cfg!(windows),
      "unix" => cfg!(unix),
      "mac" => cfg!(target_os = "macos"),
      "linux" => cfg!(target_os = "linux"),
      value => panic!("Unknown if condition: {}", value),
    }
  } else {
    true
  }
}

fn run_flaky(action: impl Fn()) {
  for _ in 0..2 {
    let result = std::panic::catch_unwind(AssertUnwindSafe(&action));
    if result.is_ok() {
      return;
    }
  }

  // surface error on third try
  action();
}

fn run_step(
  step: &StepMetaData,
  metadata: &MultiTestMetaData,
  cwd: &PathRef,
  context: &test_util::TestContext,
) {
  if step.clean_deno_dir {
    context.deno_dir().path().remove_dir_all();
  }

  let command = context
    .new_command()
    .envs(metadata.envs.iter().chain(step.envs.iter()));
  let command = match &step.args {
    VecOrString::Vec(args) => command.args_vec(args),
    VecOrString::String(text) => command.args(text),
  };
  let command = match &step.cwd {
    Some(cwd) => command.current_dir(cwd),
    None => command,
  };
  let command = match &step.command_name {
    Some(command_name) => command.name(command_name),
    None => command,
  };
  let output = command.run();
  if step.output.ends_with(".out") {
    let test_output_path = cwd.join(&step.output);
    output.assert_matches_file(test_output_path);
  } else {
    output.assert_matches_text(&step.output);
  }
  output.assert_exit_code(step.exit_code);
}

fn resolve_test_and_assertion_files(
  dir: &PathRef,
  metadata: &MultiTestMetaData,
) -> HashSet<PathRef> {
  let mut result = HashSet::with_capacity(metadata.steps.len() + 1);
  result.insert(dir.join(MANIFEST_FILE_NAME));
  result.extend(metadata.steps.iter().map(|step| dir.join(&step.output)));
  result
}
