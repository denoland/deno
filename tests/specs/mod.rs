// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;

use deno_core::anyhow::Context;
use deno_core::serde_json;
use deno_terminal::colors;
use serde::Deserialize;
use test_util::tests_path;
use test_util::PathRef;
use test_util::TestContextBuilder;

pub fn main() {
  let maybe_filter = parse_cli_arg_filter();
  let categories = filter(collect_tests(), maybe_filter.as_deref());
  let total_tests = categories.iter().map(|c| c.tests.len()).sum::<usize>();
  let mut failures = Vec::new();
  let _http_guard = test_util::http_server();
  // todo(dsherret): the output should be changed to be terse
  // when it passes, but verbose on failure
  for category in &categories {
    eprintln!();
    eprintln!("     {} {}", colors::green_bold("Running"), category.name);
    for test in &category.tests {
      eprintln!();
      eprintln!("==== Starting {} ====", test.name);
      let result = std::panic::catch_unwind(|| run_test(test));
      let success = result.is_ok();
      if !success {
        failures.push(&test.name);
      }
      eprintln!(
        "==== {} {} ====",
        if success {
          "Finished".to_string()
        } else {
          colors::red("^^FAILED^^").to_string()
        },
        test.name
      );
    }
  }

  eprintln!();
  if !failures.is_empty() {
    eprintln!("spec failures:");
    for failure in &failures {
      eprintln!("  {}", failure);
    }
    eprintln!();
    panic!("{} failed of {}", failures.len(), total_tests);
  }
  eprintln!("{} tests passed", total_tests);
}

fn parse_cli_arg_filter() -> Option<String> {
  let args: Vec<String> = std::env::args().collect();
  let maybe_filter =
    args.get(1).filter(|s| !s.starts_with('-') && !s.is_empty());
  maybe_filter.cloned()
}

fn run_test(test: &Test) {
  let metadata = &test.metadata;
  let mut builder = TestContextBuilder::new();
  let cwd = &test.cwd;

  if test.metadata.temp_dir {
    builder = builder.use_temp_cwd();
  } else {
    builder = builder.cwd(cwd.to_string_lossy());
  }

  if let Some(base) = &metadata.base {
    match base.as_str() {
      "npm" => {
        builder = builder.add_npm_env_vars();
      }
      "jsr" => {
        builder = builder.add_jsr_env_vars();
      }
      _ => panic!("Unknown test base: {}", base),
    }
  }

  let context = builder.build();

  if test.metadata.temp_dir {
    // copy all the files in the cwd to a temp directory
    // excluding the metadata and assertion files
    let temp_dir = context.temp_dir().path();
    let assertion_paths = test.resolve_test_and_assertion_files();
    cwd.copy_to_recursive_with_exclusions(temp_dir, &assertion_paths);
  }

  for step in &metadata.steps {
    if step.clean_deno_dir {
      context.deno_dir().path().remove_dir_all();
    }

    let test_output_path = cwd.join(&step.output);
    if !test_output_path.to_string_lossy().ends_with(".out") {
      panic!(
        "Use the .out extension for output files (invalid: {})",
        test_output_path
      );
    }
    let expected_output = test_output_path.read_to_string();
    let command = context.new_command();
    let command = match &step.args {
      VecOrString::Vec(args) => command.args_vec(args),
      VecOrString::String(text) => command.args(text),
    };
    let output = command.run();
    output.assert_matches_text(expected_output);
    output.assert_exit_code(step.exit_code);
  }
}

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
  pub args: VecOrString,
  pub output: String,
  #[serde(default)]
  pub exit_code: i32,
}

#[derive(Clone)]
struct Test {
  pub name: String,
  pub cwd: PathRef,
  pub metadata: MultiTestMetaData,
}

impl Test {
  pub fn resolve_test_and_assertion_files(&self) -> HashSet<PathRef> {
    let mut result = HashSet::with_capacity(self.metadata.steps.len() + 1);
    result.insert(self.cwd.join("__test__.json"));
    result.extend(
      self
        .metadata
        .steps
        .iter()
        .map(|step| self.cwd.join(&step.output)),
    );
    result
  }
}

struct TestCategory {
  pub name: String,
  pub tests: Vec<Test>,
}

fn filter(
  categories: Vec<TestCategory>,
  maybe_filter: Option<&str>,
) -> Vec<TestCategory> {
  if categories.iter().all(|c| c.tests.is_empty()) {
    panic!("no tests found");
  }
  match maybe_filter {
    Some(filter) => categories
      .into_iter()
      .map(|mut c| {
        c.tests.retain(|t| t.name.contains(filter));
        c
      })
      .collect(),
    None => categories,
  }
}

fn collect_tests() -> Vec<TestCategory> {
  let specs_dir = tests_path().join("specs");
  let mut result = Vec::new();
  for entry in specs_dir.read_dir() {
    let entry = entry.unwrap();
    let file_type = entry
      .file_type()
      .context(entry.path().to_string_lossy().to_string())
      .unwrap();
    if !file_type.is_dir() {
      continue;
    }

    let mut category = TestCategory {
      name: format!("specs::{}", entry.file_name().to_string_lossy()),
      tests: Vec::new(),
    };

    let category_path = PathRef::new(entry.path());
    for entry in category_path.read_dir() {
      let entry = entry.unwrap();
      let file_type = entry
        .file_type()
        .context(entry.path().to_string_lossy().to_string())
        .unwrap();
      if !file_type.is_dir() {
        continue;
      }

      let test_dir = PathRef::new(entry.path());
      let metadata_path = test_dir.join("__test__.json");
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
      category.tests.push(Test {
        name: format!(
          "{}::{}",
          category.name,
          entry.file_name().to_string_lossy()
        ),
        cwd: test_dir,
        metadata,
      });
    }
    result.push(category);
  }
  result
}
