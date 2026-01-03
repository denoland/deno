// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Context;
use file_test_runner::NO_CAPTURE;
use file_test_runner::TestResult;
use file_test_runner::collection::CollectOptions;
use file_test_runner::collection::CollectTestsError;
use file_test_runner::collection::CollectedCategoryOrTest;
use file_test_runner::collection::CollectedTest;
use file_test_runner::collection::CollectedTestCategory;
use file_test_runner::collection::collect_tests_or_exit;
use file_test_runner::collection::strategies::FileTestMapperStrategy;
use file_test_runner::collection::strategies::TestPerDirectoryCollectionStrategy;
use serde::Deserialize;
use test_util::IS_CI;
use test_util::PathRef;
use test_util::TestContextBuilder;
use test_util::test_runner::FlakyTestTracker;
use test_util::test_runner::Parallelism;
use test_util::test_runner::run_maybe_flaky_test;
use test_util::tests_path;

const MANIFEST_FILE_NAME: &str = "__test__.jsonc";

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
  #[serde(default)]
  pub variants: BTreeMap<String, JsonMap>,
}

impl MultiTestMetaData {
  pub fn into_collected_tests(
    mut self,
    parent_test: &CollectedTest,
  ) -> Vec<CollectedCategoryOrTest<serde_json::Value>> {
    fn merge_json_value(
      multi_test_meta_data: &MultiTestMetaData,
      value: &mut JsonMap,
    ) {
      if let Some(base) = &multi_test_meta_data.base
        && !value.contains_key("base")
      {
        value.insert("base".to_string(), base.clone().into());
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
      if !multi_test_meta_data.variants.is_empty() {
        if !value.contains_key("variants") {
          value.insert("variants".to_string(), JsonMap::default().into());
        }
        let variants_obj =
          value.get_mut("variants").unwrap().as_object_mut().unwrap();
        for (key, value) in &multi_test_meta_data.variants {
          if !variants_obj.contains_key(key) {
            variants_obj.insert(key.into(), value.clone().into());
          }
        }
      }
    }

    let mut collected_tests = Vec::with_capacity(self.tests.len());
    for (name, mut json_data) in std::mem::take(&mut self.tests) {
      merge_json_value(&self, &mut json_data);
      collected_tests.push(CollectedTest {
        name: format!("{}::{}", parent_test.name, name),
        path: parent_test.path.clone(),
        line_and_column: None,
        data: serde_json::Value::Object(json_data),
      });
    }
    let mut all_tests = Vec::with_capacity(collected_tests.len());
    for test in collected_tests {
      if let Some(variants) = test
        .data
        .as_object()
        .and_then(|o| o.get("variants"))
        .and_then(|v| v.as_object())
        && !variants.is_empty()
      {
        all_tests.push(
          map_variants(&test.data, &test.path, &test.name, variants.iter())
            .unwrap(),
        );
      } else {
        all_tests.push(CollectedCategoryOrTest::Test(test));
      }
    }
    all_tests
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
  /// Whether to run this test.
  #[serde(rename = "if")]
  pub if_cond: Option<String>,
  /// Whether the temporary directory should be canonicalized.
  ///
  /// This should be used sparingly, but is sometimes necessary
  /// on the CI.
  #[serde(default)]
  pub canonicalized_temp_dir: bool,
  /// Whether the temporary directory should be symlinked to another path.
  #[serde(default)]
  pub symlinked_temp_dir: bool,
  #[serde(default)]
  pub flaky: bool,
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
  #[serde(default)]
  pub variants: BTreeMap<String, JsonMap>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SingleTestMetaData {
  #[serde(default)]
  pub base: Option<String>,
  #[serde(default)]
  pub temp_dir: bool,
  #[serde(default)]
  pub canonicalized_temp_dir: bool,
  #[serde(default)]
  pub symlinked_temp_dir: bool,
  #[serde(default)]
  pub repeat: Option<usize>,
  #[serde(flatten)]
  pub step: StepMetaData,
  #[serde(default)]
  pub ignore: bool,
  #[allow(dead_code)]
  #[serde(default)]
  pub variants: BTreeMap<String, JsonMap>,
}

impl SingleTestMetaData {
  pub fn into_multi(self) -> MultiStepMetaData {
    MultiStepMetaData {
      base: self.base,
      cwd: None,
      if_cond: self.step.if_cond.clone(),
      flaky: self.step.flaky,
      temp_dir: self.temp_dir,
      canonicalized_temp_dir: self.canonicalized_temp_dir,
      symlinked_temp_dir: self.symlinked_temp_dir,
      repeat: self.repeat,
      envs: Default::default(),
      steps: vec![self.step],
      ignore: self.ignore,
      variants: self.variants,
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
  #[serde(default)]
  pub variants: BTreeMap<String, JsonMap>,
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
    })
    .into_flat_category();

  if root_category.is_empty() {
    return; // all tests filtered out
  }

  let _http_guard = test_util::http_server();
  let parallelism = Parallelism::default();
  let flaky_test_tracker = Arc::new(FlakyTestTracker::default());
  file_test_runner::run_tests(
    &root_category,
    file_test_runner::RunOptions {
      parallelism: parallelism.max_parallelism(),
      reporter: test_util::test_runner::get_test_reporter(
        "specs",
        flaky_test_tracker.clone(),
      ),
    },
    move |test| run_test(test, &flaky_test_tracker, &parallelism),
  );
}

fn run_test(
  test: &CollectedTest<serde_json::Value>,
  flaky_test_tracker: &FlakyTestTracker,
  parallelism: &Parallelism,
) -> TestResult {
  let cwd = PathRef::new(&test.path).parent();
  let metadata_value = test.data.clone();
  let diagnostic_logger = Rc::new(RefCell::new(Vec::<u8>::new()));
  let result = TestResult::from_maybe_panic_or_result(AssertUnwindSafe(|| {
    let metadata = deserialize_value(metadata_value);
    let substs = variant_substitutions(&BTreeMap::new(), &metadata.variants);
    let if_cond = metadata
      .if_cond
      .as_deref()
      .map(|s| apply_substs(s, &substs));
    if metadata.ignore || !should_run(if_cond.as_deref()) {
      TestResult::Ignored
    } else if let Some(repeat) = metadata.repeat {
      for _ in 0..repeat {
        let result = run_test_inner(
          test,
          &metadata,
          &cwd,
          diagnostic_logger.clone(),
          flaky_test_tracker,
          parallelism,
        );
        if result.is_failed() {
          return result;
        }
      }
      TestResult::Passed { duration: None }
    } else {
      run_test_inner(
        test,
        &metadata,
        &cwd,
        diagnostic_logger.clone(),
        flaky_test_tracker,
        parallelism,
      )
    }
  }));
  match result {
    TestResult::Failed {
      duration,
      output: panic_output,
    } => {
      let mut output = diagnostic_logger.borrow().clone();
      output.push(b'\n');
      output.extend(panic_output);
      TestResult::Failed { duration, output }
    }
    TestResult::Passed { .. }
    | TestResult::Ignored
    | TestResult::SubTests { .. } => result,
  }
}

fn run_test_inner(
  test: &CollectedTest<serde_json::Value>,
  metadata: &MultiStepMetaData,
  cwd: &PathRef,
  diagnostic_logger: Rc<RefCell<Vec<u8>>>,
  flaky_test_tracker: &FlakyTestTracker,
  parallelism: &Parallelism,
) -> TestResult {
  let run_fn = || {
    let context =
      test_context_from_metadata(metadata, cwd, diagnostic_logger.clone());
    for step in metadata
      .steps
      .iter()
      .filter(|s| should_run(s.if_cond.as_deref()))
    {
      let run_func = || {
        TestResult::from_maybe_panic_or_result(AssertUnwindSafe(|| {
          run_step(step, metadata, cwd, &context);
          TestResult::Passed { duration: None }
        }))
      };
      let result = run_maybe_flaky_test(
        &test.name,
        step.flaky,
        flaky_test_tracker,
        None,
        run_func,
      );
      if result.is_failed() {
        return result;
      }
    }
    TestResult::Passed { duration: None }
  };
  run_maybe_flaky_test(
    &test.name,
    metadata.flaky || *IS_CI,
    flaky_test_tracker,
    Some(parallelism),
    run_fn,
  )
}

fn deserialize_value(metadata_value: serde_json::Value) -> MultiStepMetaData {
  let metadata_string = metadata_value.to_string();
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
  .with_context(|| format!("Failed to parse test spec: {}", metadata_string))
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

  if metadata.canonicalized_temp_dir {
    // not actually deprecated, we just want to discourage its use
    #[allow(deprecated)]
    {
      builder = builder.use_canonicalized_temp_dir();
    }
  }
  if metadata.symlinked_temp_dir {
    // not actually deprecated, we just want to discourage its use
    // because it's mostly used for testing purposes locally
    #[allow(deprecated)]
    {
      builder = builder.use_symlinked_temp_dir();
    }
    if cfg!(not(debug_assertions)) {
      // panic to prevent using this on the CI as CI already uses
      // a symlinked temp directory for every test
      panic!("Cannot use symlinkedTempDir in release mode");
    }
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

fn should_run(if_cond: Option<&str>) -> bool {
  if let Some(cond) = if_cond {
    match cond {
      "windows" => cfg!(windows),
      "unix" => cfg!(unix),
      "mac" => cfg!(target_os = "macos"),
      "linux" => cfg!(target_os = "linux"),
      "notCI" => std::env::var_os("CI").is_none(),
      "notMacIntel" => {
        cfg!(unix)
          && !(cfg!(target_os = "macos") && cfg!(target_arch = "x86_64"))
      }
      value => panic!("Unknown if condition: {}", value),
    }
  } else {
    true
  }
}

fn run_step(
  step: &StepMetaData,
  metadata: &MultiStepMetaData,
  cwd: &PathRef,
  context: &test_util::TestContext,
) {
  let substs = variant_substitutions(&step.variants, &metadata.variants);

  let command = if substs.is_empty() {
    let envs = metadata.envs.iter().chain(step.envs.iter());
    context.new_command().envs(envs)
  } else {
    let mut envs = metadata
      .envs
      .iter()
      .chain(step.envs.iter())
      .map(|(key, value)| (key.clone(), value.clone()))
      .collect::<HashMap<_, _>>();
    substitute_variants_into_envs(&substs, &mut envs);
    context.new_command().envs(envs)
  };

  let command = match &step.args {
    VecOrString::Vec(args) => {
      if substs.is_empty() {
        command.args_vec(args)
      } else {
        let mut args_replaced = args.clone();
        for arg in args {
          for (from, to) in &substs {
            let arg_replaced = arg.replace(from, to);
            if arg_replaced.is_empty() && &arg_replaced != arg {
              continue;
            }
            args_replaced.push(arg_replaced);
          }
        }

        command.args_vec(args)
      }
    }
    VecOrString::String(text) => {
      if substs.is_empty() {
        command.args(text)
      } else {
        let text = apply_substs(text, &substs);
        command.args(text.as_ref())
      }
    }
  };
  let command = match step.cwd.as_ref().or(metadata.cwd.as_ref()) {
    Some(cwd) => command.current_dir(cwd),
    None => command,
  };
  let command = match &step.command_name {
    Some(command_name) => {
      if substs.is_empty() {
        command.name(command_name)
      } else {
        let command_name = apply_substs(command_name, &substs);
        command.name(command_name.as_ref())
      }
    }
    None => command,
  };
  let command = match *NO_CAPTURE {
    // deprecated is only to prevent use, so this is fine here
    #[allow(deprecated)]
    true => command.show_output(),
    false => command,
  };
  let command = match &step.input {
    Some(input) => {
      if input.ends_with(".in") {
        let test_input_path = cwd.join(input);
        command.stdin_text(std::fs::read_to_string(test_input_path).unwrap())
      } else {
        command.stdin_text(input)
      }
    }
    None => command,
  };
  let output = command.run();

  let step_output = {
    if substs.is_empty() {
      step.output.clone()
    } else {
      let mut output = step.output.clone();
      for (from, to) in substs {
        output = output.replace(&from, &to);
      }
      output
    }
  };
  if step_output.ends_with(".out") {
    let test_output_path = cwd.join(&step_output);
    output.assert_matches_file(test_output_path);
  } else {
    assert!(
      step_output.len() <= 160,
      "The \"output\" property in your __test__.jsonc file is too long. Please extract this to an `.out` file to improve readability."
    );
    output.assert_matches_text(&step_output);
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

fn map_variants<'a, I>(
  test_data: &serde_json::Value,
  test_path: &Path,
  test_name: &str,
  variants: I,
) -> Result<CollectedCategoryOrTest<serde_json::Value>, CollectTestsError>
where
  I: IntoIterator<Item = (&'a String, &'a serde_json::Value)>,
{
  let mut children = Vec::with_capacity(2);
  for (variant_name, variant_data) in variants {
    let mut child_data = test_data.clone();
    let child_obj = child_data
      .as_object_mut()
      .unwrap()
      .get_mut("variants")
      .unwrap()
      .as_object_mut()
      .unwrap();
    child_obj.clear();
    child_obj.insert(variant_name.clone(), variant_data.clone());
    children.push(CollectedCategoryOrTest::Test(CollectedTest {
      name: format!("{}::{}", test_name, variant_name),
      path: test_path.to_path_buf(),
      line_and_column: None,
      data: child_data,
    }));
  }
  Ok(CollectedCategoryOrTest::Category(CollectedTestCategory {
    children,
    name: test_name.to_string(),
    path: test_path.to_path_buf(),
  }))
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
      children: data.into_collected_tests(&test),
      name: test.name,
      path: test.path,
    }))
  } else if let Some(variants) = metadata_value
    .as_object()
    .and_then(|o| o.get("variants"))
    .and_then(|v| v.as_object())
    && !variants.is_empty()
  {
    map_variants(&metadata_value, &test.path, &test.name, variants.iter())
  } else {
    Ok(CollectedCategoryOrTest::Test(CollectedTest {
      name: test.name,
      path: test.path,
      line_and_column: None,
      data: metadata_value,
    }))
  }
}

// in the future we could consider using https://docs.rs/aho_corasick to do multiple replacements at once
// in practice, though, i suspect the numbers here will be small enough that the naive approach is fast enough
fn variant_substitutions(
  variants: &BTreeMap<String, JsonMap>,
  multi_step_variants: &BTreeMap<String, JsonMap>,
) -> Vec<(String, String)> {
  if variants.is_empty() && multi_step_variants.is_empty() {
    return Vec::new();
  }
  let mut variant = variants.values().next().cloned().unwrap_or_default();
  let multi_step_variant = multi_step_variants
    .values()
    .next()
    .cloned()
    .unwrap_or_default();

  for (name, value) in multi_step_variant {
    if !variant.contains_key(&name) {
      variant.insert(name, value.clone());
    }
  }

  let mut pairs = variant
    .into_iter()
    .filter_map(|(name, value)| {
      value
        .as_str()
        .map(|value| (format!("${{{}}}", name), value.to_string()))
    })
    .collect::<Vec<_>>();
  pairs.sort_by(|a, b| a.0.cmp(&b.0).reverse());
  pairs
}

fn substitute_variants_into_envs(
  pairs: &Vec<(String, String)>,
  envs: &mut HashMap<String, String>,
) {
  let mut to_remove = Vec::new();
  for (key, value) in pairs {
    for (k, v) in envs.iter_mut() {
      let replaced = v.replace(key.as_str(), value);
      if replaced.is_empty() && &replaced != v {
        to_remove.push(k.clone());
        continue;
      }
      *v = replaced;
    }
  }
  for key in to_remove {
    envs.remove(&key);
  }
}

fn apply_substs<'a>(
  text: &'a str,
  substs: &'_ [(String, String)],
) -> Cow<'a, str> {
  if substs.is_empty() {
    Cow::Borrowed(text)
  } else {
    let mut text = Cow::Borrowed(text);
    for (from, to) in substs {
      text = text.replace(from, to).into();
    }
    text
  }
}
