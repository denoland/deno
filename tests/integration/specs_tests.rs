use std::collections::HashSet;

use deno_terminal::colors;
use serde::Deserialize;
use test_util::tests_path;
use test_util::PathRef;
use test_util::TestContextBuilder;

#[test]
fn test_specs() {
  let categories = filter(collect_tests());
  let total_tests = categories.iter().map(|c| c.tests.len()).sum::<usize>();
  let mut failures = Vec::new();
  for category in &categories {
    eprintln!("");
    eprintln!(
      "     {} {} tests...",
      colors::green("Running"),
      category.name
    );
    for test in &category.tests {
      eprintln!("");
      eprintln!("==== {} {} ====", colors::bold("Starting"), test.name);
      let result = std::panic::catch_unwind(|| run_test(test));
      let success = result.is_ok();
      if !success {
        failures.push(format!("{}::{}", category.name, test.name));
      }
      eprintln!("");
      eprintln!(
        "==== {} {} ====",
        if success {
          colors::green("Finished")
        } else {
          colors::red("^^FAILED^^")
        },
        test.name
      );
    }
  }

  eprintln!("");
  if !failures.is_empty() {
    eprintln!("spec failures:");
    for failure in &failures {
      eprintln!("  {}", failure);
    }
    eprintln!("");
    panic!("{} failed of {}", failures.len(), total_tests);
  }
  eprintln!("{} tests passed", total_tests);

  if total_tests == 0 {
    panic!("no tests found");
  }
}

fn run_test(test: &Test) {
  let metadata = &test.metadata;
  let mut builder = TestContextBuilder::new();
  let cwd = &test.cwd;
  builder = builder.cwd(cwd.to_string_lossy());

  if let Some(base) = &metadata.base {
    match base.as_str() {
      "npm" => {
        builder = builder.add_npm_env_vars().use_http_server();
      }
      _ => panic!("Unknown test base: {}", base),
    }
  }

  if test.metadata.temp_dir {
    builder = builder.use_temp_cwd();
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
    if step.clean {
      context.deno_dir().path().remove_dir_all();
    }

    let test_output_path = cwd.join(&step.output);
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
#[serde(deny_unknown_fields, untagged)]
enum TestMetaData {
  Multi(MultiTestMetaData),
  Single(SingleTestMetaData),
}

impl TestMetaData {
  pub fn into_multi(self) -> MultiTestMetaData {
    match self {
      TestMetaData::Multi(mut m) => {
        m.only = m.only || m.steps.iter().any(|t| t.only);
        m
      }
      TestMetaData::Single(s) => s.into_multi(),
    }
  }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct MultiTestMetaData {
  #[serde(default)]
  pub only: bool,
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
      only: self.step.only,
      base: self.base,
      temp_dir: self.temp_dir,
      steps: vec![self.step],
    }
  }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StepMetaData {
  #[serde(default)]
  pub only: bool,
  /// Whether to clean the deno_dir before running the step.
  #[serde(default)]
  pub clean: bool,
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

impl TestCategory {
  pub fn with_only(&self) -> Self {
    TestCategory {
      name: self.name.clone(),
      tests: self
        .tests
        .iter()
        .filter(|test| test.metadata.only)
        .map(|test| {
          let mut test = test.clone();
          test.metadata.steps =
            test.metadata.steps.into_iter().filter(|t| t.only).collect();
          test
        })
        .collect(),
    }
  }
}

fn filter(categories: Vec<TestCategory>) -> Vec<TestCategory> {
  let only_categories = categories
    .iter()
    .map(|c| c.with_only())
    .filter(|c| !c.tests.is_empty())
    .collect::<Vec<_>>();
  if !only_categories.is_empty() {
    if std::env::var("CI").is_ok() {
      panic!("A test had `\"only\": true` set. Please remove this before committing.");
    }
    only_categories
  } else {
    categories
  }
}

fn collect_tests() -> Vec<TestCategory> {
  let specs_dir = tests_path().join("specs");
  let mut result = Vec::new();
  for entry in specs_dir.read_dir() {
    let entry = entry.unwrap();
    if !entry.file_type().unwrap().is_dir() {
      continue;
    }

    let mut category = TestCategory {
      name: entry.file_name().to_string_lossy().to_string(),
      tests: Vec::new(),
    };

    let category_path = PathRef::new(entry.path());
    for entry in category_path.read_dir() {
      let entry = entry.unwrap();
      if !entry.file_type().unwrap().is_dir() {
        continue;
      }

      let test_dir = PathRef::new(entry.path());
      let metadata_path = test_dir.join("__test__.json");
      let metadata = metadata_path.read_json::<TestMetaData>();
      category.tests.push(Test {
        name: entry.file_name().to_string_lossy().to_string(),
        cwd: test_dir,
        metadata: metadata.into_multi(),
      });
    }
    result.push(category);
  }
  result
}
