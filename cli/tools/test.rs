// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::fs_util::{collect_files, get_extension, is_supported_ext};
use crate::tools::fmt::run_parallelized;
use deno_core::error::AnyError;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

pub struct TestReporter {
  failed: usize,
  filtered: usize,
  ignored: usize,
  measured: usize,
  passed: usize,
}

pub struct TestPlan {
  pub count: usize,
}

pub struct TestResult {
  pub name: String,
  pub ignore: bool,
  pub error: Option<String>,
}

pub struct TestSummary {
  pub failed: usize,
  pub filtered: usize,
  pub ignored: usize,
  pub measured: usize,
  pub passed: usize,
}

impl TestReporter {
  fn new() -> TestReporter {
    TestReporter {
      failed: 0,
      filtered: 0,
      ignored: 0,
      measured: 0,
      passed: 0,
    }
  }

  pub fn visit_plan(&mut self, plan: TestPlan) {
    println!("running {} tests", plan.count);
  }

  pub fn visit_result(&mut self, result: TestResult) {
    print!("test {} ... ", result.name);

    if let Some(error) = result.error {
      println!("{}", colors::red("FAILED"));
    } else if result.ignore {
      println!("{}", colors::yellow("ignored"));
    } else {
      println!("{}", colors::green("ok"));
    }
  }

  pub fn visit_summary(&mut self, summary: TestSummary) {
    self.passed += summary.passed;
    self.failed += summary.failed;
    self.filtered += summary.filtered;
    self.ignored += summary.ignored;
    self.measured += summary.measured;
  }

  pub fn close(&mut self) {
    print!("\ntest result: ");

    let success = self.failed == 0;
    if success {
      print!("{}", colors::green("ok"));
    } else {
      print!("{}", colors::red("FAILED"));
    }

    println!(
      ". {} passed; {} failed; {} ignored; {} measured; {} filtered out\n\n",
      self.passed, self.failed, self.ignored, self.measured, self.filtered
    );
  }
}

fn is_supported_test(p: &Path) -> bool {
  use std::path::Component;
  if let Some(Component::Normal(basename_os_str)) = p.components().next_back() {
    let basename = basename_os_str.to_string_lossy();
    basename.ends_with("_test.ts")
      || basename.ends_with("_test.tsx")
      || basename.ends_with("_test.js")
      || basename.ends_with("_test.mjs")
      || basename.ends_with("_test.jsx")
      || basename.ends_with(".test.ts")
      || basename.ends_with(".test.tsx")
      || basename.ends_with(".test.js")
      || basename.ends_with(".test.mjs")
      || basename.ends_with(".test.jsx")
      || basename == "test.ts"
      || basename == "test.tsx"
      || basename == "test.js"
      || basename == "test.mjs"
      || basename == "test.jsx"
  } else {
    false
  }
}

pub async fn test_files<F>(
  args: Vec<PathBuf>,
  ignore: Vec<PathBuf>,
  no_run: bool,
  fail_fast: bool,
  allow_none: bool,
  test_file: F,
) -> Result<(), AnyError>
where
  F: FnOnce(PathBuf, Arc<Mutex<TestReporter>>) -> Result<(), AnyError>
    + Send
    + 'static
    + Clone,
{
  let target_files = collect_files(&args, &ignore, is_supported_test)?;
  if target_files.is_empty() {
    println!("No matching test modules found");
    if !allow_none {
      std::process::exit(1);
    }
    return Ok(());
  }

  let reporter_lock = Arc::new(Mutex::new(TestReporter::new()));

  run_parallelized(target_files, {
    let reporter_lock = reporter_lock.clone();

    move |file_path| {
      test_file(file_path.clone(), reporter_lock.clone())?;
      Ok(())
    }
  })
  .await?;

  reporter_lock.lock().unwrap().close();

  Ok(())
}
