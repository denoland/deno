// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::panic::AssertUnwindSafe;
use std::rc::Rc;

use deno_core::anyhow::Context;
use deno_core::serde_json;
use file_test_runner::collection::collect_tests_or_exit;
use file_test_runner::collection::strategies::FileTestMapperStrategy;
use file_test_runner::collection::strategies::TestPerDirectoryCollectionStrategy;
use file_test_runner::collection::CollectOptions;
use file_test_runner::collection::CollectTestsError;
use file_test_runner::collection::CollectedCategoryOrTest;
use file_test_runner::collection::CollectedTest;
use file_test_runner::collection::CollectedTestCategory;
use file_test_runner::SubTestResult;
use file_test_runner::TestResult;
use once_cell::sync::Lazy;
use serde::Deserialize;
use test_util::tests_path;
use test_util::PathRef;
use test_util::TestContextBuilder;

const MANIFEST_FILE_NAME: &str = "__test__.jsonc";

static NO_CAPTURE: Lazy<bool> =
  Lazy::new(|| std::env::args().any(|arg| arg == "--nocapture"));

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum VecOrString {
  Vec(Vec<String>),
  String(String),
}

type JsonMap = serde_json::Map<String, serde_json::Value>;

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
  #[serde(default)]
  pub cwd: Option<String>,
  #[serde(default)]
  pub tests: BTreeMap<String, JsonMap>,
  #[serde(default)]
  pub ignore: bool,
}

impl MultiTestMetaData {
  pub fn into_collected_tests(
    mut self,
    parent_test: &CollectedTest,
  ) -> Vec<CollectedTest<serde_json::Value>> {
    fn merge_json_value(
      multi_test_meta_data: &MultiTestMetaData,
      value: &mut JsonMap,
    ) {
      if let Some(base) = &multi_test_meta_data.base {
        if !value.contains_key("base") {
          value.insert("base".to_string(), base.clone().into());
        }
      }
      if multi_test_meta_data.temp_dir && !value.contains_key("tempDir") {
        value.insert("tempDir".to_string(), true.into());
      }
      if multi_test_meta_data.cwd.is_some() && !value.contains_key("cwd") {
        value
          .insert("cwd".to_string(), multi_test_meta_data.cwd.clone().into());
      }
      if !multi_test_meta_data.envs.is_empty() {
        if !value.contains_key("envs") {
          value.insert("envs".to_string(), JsonMap::default().into());
        }
        let envs_obj = value.get_mut("envs").unwrap().as_object_mut().unwrap();
        for (key, value) in &multi_test_meta_data.envs {
          if !envs_obj.contains_key(key) {
            envs_obj.insert(key.into(), value.clone().into());
          }
        }
      }
      if multi_test_meta_data.ignore && !value.contains_key("ignore") {
        value.insert("ignore".to_string(), true.into());
      }
    }

    let mut collected_tests = Vec::with_capacity(self.tests.len());
    for (name, mut json_data) in std::mem::take(&mut self.tests) {
      merge_json_value(&self, &mut json_data);
      collected_tests.push(CollectedTest {
        name: format!("{}::{}", parent_test.name, name),
        path: parent_test.path.clone(),
        data: serde_json::Value::Object(json_data),
      });
    }

    collected_tests
  }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct MultiStepMetaData {
  /// Whether to copy all the non-assertion files in the current
  /// test directory to a temporary directory before running the
  /// steps.
  #[serde(default)]
  pub temp_dir: bool,
  /// The base environment to use for the test.
  #[serde(default)]
  pub base: Option<String>,
  #[serde(default)]
  pub cwd: Option<String>,
  #[serde(default)]
  pub envs: HashMap<String, String>,
  #[serde(default)]
  pub repeat: Option<usize>,
  #[serde(default)]
  pub steps: Vec<StepMetaData>,
  #[serde(default)]
  pub ignore: bool,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SingleTestMetaData {
  #[serde(default)]
  pub base: Option<String>,
  #[serde(default)]
  pub temp_dir: bool,
  #[serde(default)]
  pub repeat: Option<usize>,
  #[serde(flatten)]
  pub step: StepMetaData,
  #[serde(default)]
  pub ignore: bool,
}

impl SingleTestMetaData {
  pub fn into_multi(self) -> MultiStepMetaData {
    MultiStepMetaData {
      base: self.base,
      cwd: None,
      temp_dir: self.temp_dir,
      repeat: self.repeat,
      envs: Default::default(),
      steps: vec![self.step],
      ignore: self.ignore,
    }
  }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StepMetaData {
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
  pub input: Option<String>,
  pub output: String,
  #[serde(default)]
  pub exit_code: i32,
}

pub fn main() {
  let root_category =
    collect_tests_or_exit::<serde_json::Value>(CollectOptions {
      base: tests_path().join("specs").to_path_buf(),
      strategy: Box::new(FileTestMapperStrategy {
        base_strategy: TestPerDirectoryCollectionStrategy {
          file_name: MANIFEST_FILE_NAME.to_string(),
        },
        map: map_test_within_file,
      }),
      filter_override: None,
    });

  if root_category.is_empty() {
    return; // all tests filtered out
  }

  let _http_guard = test_util::http_server();
  file_test_runner::run_tests(
    &root_category,
    file_test_runner::RunOptions {
      parallel: !*NO_CAPTURE,
    },
    run_test,
  );
}

/// Maps a __test__.jsonc file to a category of tests if it contains a "test" object.
fn map_test_within_file(
  test: CollectedTest,
) -> Result<CollectedCategoryOrTest<serde_json::Value>, CollectTestsError> {
  let test_path = PathRef::new(&test.path);
  let metadata_value = test_path.read_jsonc_value();
  if metadata_value
    .as_object()
    .map(|o| o.contains_key("tests"))
    .unwrap_or(false)
  {
    let data: MultiTestMetaData = serde_json::from_value(metadata_value)
      .with_context(|| format!("Failed deserializing {}", test_path))
      .map_err(CollectTestsError::Other)?;
    Ok(CollectedCategoryOrTest::Category(CollectedTestCategory {
      children: data
        .into_collected_tests(&test)
        .into_iter()
        .map(CollectedCategoryOrTest::Test)
        .collect(),
      name: test.name,
      path: test.path,
    }))
  } else {
    Ok(CollectedCategoryOrTest::Test(CollectedTest {
      name: test.name,
      path: test.path,
      data: metadata_value,
    }))
  }
}

fn run_test(test: &CollectedTest<serde_json::Value>) -> TestResult {
  let cwd = PathRef::new(&test.path).parent();
  let metadata_value = test.data.clone();
  let diagnostic_logger = Rc::new(RefCell::new(Vec::<u8>::new()));
  let result = TestResult::from_maybe_panic_or_result(AssertUnwindSafe(|| {
    let metadata = deserialize_value(metadata_value);
    if metadata.ignore {
      TestResult::Ignored
    } else if let Some(repeat) = metadata.repeat {
      TestResult::SubTests(
        (0..repeat)
          .map(|i| {
            let diagnostic_logger = diagnostic_logger.clone();
            SubTestResult {
              name: format!("run {}", i + 1),
              result: TestResult::from_maybe_panic(AssertUnwindSafe(|| {
                run_test_inner(&metadata, &cwd, diagnostic_logger);
              })),
            }
          })
          .collect(),
      )
    } else {
      run_test_inner(&metadata, &cwd, diagnostic_logger.clone());
      TestResult::Passed
    }
  }));
  match result {
    TestResult::Failed {
      output: panic_output,
    } => {
      let mut output = diagnostic_logger.borrow().clone();
      output.push(b'\n');
      output.extend(panic_output);
      TestResult::Failed { output }
    }
    TestResult::Passed | TestResult::Ignored | TestResult::SubTests(_) => {
      result
    }
  }
}

fn run_test_inner(
  metadata: &MultiStepMetaData,
  cwd: &PathRef,
  diagnostic_logger: Rc<RefCell<Vec<u8>>>,
) {
  let context = test_context_from_metadata(metadata, cwd, diagnostic_logger);
  for step in metadata.steps.iter().filter(|s| should_run_step(s)) {
    let run_func = || run_step(step, metadata, cwd, &context);
    if step.flaky {
      run_flaky(run_func);
    } else {
      run_func();
    }
  }
}

fn deserialize_value(metadata_value: serde_json::Value) -> MultiStepMetaData {
  // checking for "steps" leads to a more targeted error message
  // instead of when deserializing an untagged enum
  if metadata_value
    .as_object()
    .map(|o| o.contains_key("steps"))
    .unwrap_or(false)
  {
    serde_json::from_value::<MultiStepMetaData>(metadata_value)
  } else {
    serde_json::from_value::<SingleTestMetaData>(metadata_value)
      .map(|s| s.into_multi())
  }
  .context("Failed to parse test spec")
  .unwrap()
}

fn test_context_from_metadata(
  metadata: &MultiStepMetaData,
  cwd: &PathRef,
  diagnostic_logger: Rc<RefCell<Vec<u8>>>,
) -> test_util::TestContext {
  let mut builder = TestContextBuilder::new();
  builder = builder.logging_capture(diagnostic_logger);

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
    let assertion_paths = resolve_test_and_assertion_files(cwd, metadata);
    cwd.copy_to_recursive_with_exclusions(temp_dir, &assertion_paths);
  }
  context
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
  metadata: &MultiStepMetaData,
  cwd: &PathRef,
  context: &test_util::TestContext,
) {
  let command = context
    .new_command()
    .envs(metadata.envs.iter().chain(step.envs.iter()));
  let command = match &step.args {
    VecOrString::Vec(args) => command.args_vec(args),
    VecOrString::String(text) => command.args(text),
  };
  let command = match step.cwd.as_ref().or(metadata.cwd.as_ref()) {
    Some(cwd) => command.current_dir(cwd),
    None => command,
  };
  let command = match &step.command_name {
    Some(command_name) => command.name(command_name),
    None => command,
  };
  let command = match *NO_CAPTURE {
    // deprecated is only to prevent use, so this is fine here
    #[allow(deprecated)]
    true => command.show_output(),
    false => command,
  };
  let command = match &step.input {
    Some(input) => command.stdin_text(input),
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
  metadata: &MultiStepMetaData,
) -> HashSet<PathRef> {
  let mut result = HashSet::with_capacity(metadata.steps.len() + 1);
  result.insert(dir.join(MANIFEST_FILE_NAME));
  result.extend(metadata.steps.iter().map(|step| dir.join(&step.output)));
  result
}
