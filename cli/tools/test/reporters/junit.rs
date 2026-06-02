// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::VecDeque;
use std::io::Write;

use console_static_text::ansi::strip_ansi_codes;
use deno_core::anyhow::Context;

use super::fmt::to_relative_path_or_remote_url;
use super::*;

pub struct JunitTestReporter {
  cwd: Url,
  output_path: String,
  // Stores TestCases (i.e. Tests) by the Test ID
  cases: IndexMap<usize, JunitTestCase>,
  // Stores nodes representing test cases in such a way that can be traversed
  // from child to parent to build the full test name that reflects the test
  // hierarchy.
  test_name_tree: TestNameTree,
  failure_format_options: TestFailureFormatOptions,
}

impl JunitTestReporter {
  pub fn new(
    cwd: Url,
    output_path: String,
    failure_format_options: TestFailureFormatOptions,
  ) -> Self {
    Self {
      cwd,
      output_path,
      cases: IndexMap::new(),
      test_name_tree: TestNameTree::new(),
      failure_format_options,
    }
  }

  fn convert_status(
    status: &TestResult,
    failure_format_options: &TestFailureFormatOptions,
  ) -> JunitTestCaseStatus {
    match status {
      TestResult::Ok => JunitTestCaseStatus::success(),
      TestResult::Ignored => JunitTestCaseStatus::skipped(),
      TestResult::Failed(failure) => {
        let message = if failure_format_options.strip_ascii_color {
          strip_ansi_codes(&failure.overview()).to_string()
        } else {
          failure.overview()
        };
        JunitTestCaseStatus::NonSuccess {
          kind: JunitNonSuccessKind::Failure,
          message: Some(message),
          ty: None,
          description: Some(
            failure.format(failure_format_options).into_owned(),
          ),
        }
      }
      TestResult::Cancelled => JunitTestCaseStatus::NonSuccess {
        kind: JunitNonSuccessKind::Error,
        message: Some("Cancelled".to_string()),
        ty: None,
        description: None,
      },
    }
  }

  fn convert_step_status(
    status: &TestStepResult,
    failure_format_options: &TestFailureFormatOptions,
  ) -> JunitTestCaseStatus {
    match status {
      TestStepResult::Ok => JunitTestCaseStatus::success(),
      TestStepResult::Ignored => JunitTestCaseStatus::skipped(),
      TestStepResult::Failed(failure) => {
        let message = if failure_format_options.strip_ascii_color {
          strip_ansi_codes(&failure.overview()).to_string()
        } else {
          failure.overview()
        };
        JunitTestCaseStatus::NonSuccess {
          kind: JunitNonSuccessKind::Failure,
          message: Some(message),
          ty: None,
          description: Some(
            failure.format(failure_format_options).into_owned(),
          ),
        }
      }
    }
  }
}

impl TestReporter for JunitTestReporter {
  fn report_register(&mut self, description: &TestDescription) {
    let mut case = JunitTestCase::new(
      description.name.clone(),
      JunitTestCaseStatus::skipped(),
    );
    case.classname = Some(to_relative_path_or_remote_url(
      &self.cwd,
      &description.location.file_name,
    ));
    case.extra.insert(
      String::from("line"),
      description.location.line_number.to_string(),
    );
    case.extra.insert(
      String::from("col"),
      description.location.column_number.to_string(),
    );
    self.cases.insert(description.id, case);

    self.test_name_tree.add_node(description.clone().into());
  }

  fn report_plan(&mut self, _plan: &TestPlan) {}

  fn report_slow(&mut self, _description: &TestDescription, _elapsed: u64) {}
  fn report_wait(&mut self, _description: &TestDescription) {}

  fn report_output(&mut self, _metadata: &OutputMetadata, _output: &[u8]) {
    /*
     TODO(skycoop): Right now we don't include stdout/stderr in the report.
     `_metadata` now identifies the test/step that produced each chunk of
     output (via `test_id`/`step_ids`), so this could be wired up to attach
     captured output to the corresponding `<testcase>`. This is a nice to have
     feature, so we can come back to it later.
    */
  }

  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    elapsed: u64,
  ) {
    if let Some(case) = self.cases.get_mut(&description.id) {
      case.status = Self::convert_status(result, &self.failure_format_options);
      case.set_time(Duration::from_millis(elapsed));
    }
  }

  fn report_uncaught_error(&mut self, _origin: &str, _error: Box<JsError>) {}

  fn report_step_register(&mut self, description: &TestStepDescription) {
    self.test_name_tree.add_node(description.clone().into());
    let test_case_name =
      self.test_name_tree.construct_full_test_name(description.id);

    let mut case =
      JunitTestCase::new(test_case_name, JunitTestCaseStatus::skipped());
    case.classname = Some(to_relative_path_or_remote_url(
      &self.cwd,
      &description.location.file_name,
    ));
    case.extra.insert(
      String::from("line"),
      description.location.line_number.to_string(),
    );
    case.extra.insert(
      String::from("col"),
      description.location.column_number.to_string(),
    );
    self.cases.insert(description.id, case);
  }

  fn report_step_wait(&mut self, _description: &TestStepDescription) {}

  fn report_step_result(
    &mut self,
    description: &TestStepDescription,
    result: &TestStepResult,
    elapsed: u64,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    if let Some(case) = self.cases.get_mut(&description.id) {
      case.status =
        Self::convert_step_status(result, &self.failure_format_options);
      case.set_time(Duration::from_millis(elapsed));
    }
  }

  fn report_summary(
    &mut self,
    _elapsed: &Duration,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
  }

  fn report_sigint(
    &mut self,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    for id in tests_pending {
      if let Some(description) = tests.get(id) {
        self.report_result(description, &TestResult::Cancelled, 0)
      }
    }
  }

  fn report_exit(
    &mut self,
    _exit_code: i32,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    for id in tests_pending {
      if let Some(description) = tests.get(id) {
        self.report_result(description, &TestResult::Cancelled, 0)
      }
    }
  }

  fn report_isolate_exit(&mut self, _origin: &str, _exit_code: i32) {
    // JUnit reporters are file-oriented; we surface the isolate exit via the
    // overall test run failure status rather than emitting a synthetic case.
  }

  fn report_completed(&mut self) {
    // TODO(mmastrac): This reporter does not handle stdout/stderr yet, and when we do, we may need to redirect
    // pre-and-post-test output somewhere.
  }

  fn flush_report(
    &mut self,
    elapsed: &Duration,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) -> anyhow::Result<()> {
    let mut suites: IndexMap<String, JunitTestSuite> = IndexMap::new();
    for (id, case) in &self.cases {
      let abs_filename = match (tests.get(id), test_steps.get(id)) {
        (Some(test), _) => &test.location.file_name,
        (_, Some(step)) => &step.location.file_name,
        (None, None) => {
          unreachable!("Unknown test ID '{id}' provided");
        }
      };

      let filename = to_relative_path_or_remote_url(&self.cwd, abs_filename);

      suites
        .entry(filename.clone())
        .and_modify(|s| {
          s.add_test_case(case.clone());
        })
        .or_insert_with(|| {
          let mut suite = JunitTestSuite::new(filename);
          suite.add_test_case(case.clone());
          suite
        });
    }

    let report = JunitReport::new("deno test", *elapsed, suites.into_values());

    if self.output_path == "-" {
      report
        .serialize(std::io::stdout())
        .with_context(|| "Failed to write JUnit report to stdout")?;
    } else {
      let file = crate::util::fs::create_file(Path::new(&self.output_path))
        .context("Failed to open JUnit report file.")?;
      report.serialize(file).with_context(|| {
        format!("Failed to write JUnit report to {}", self.output_path)
      })?;
    }

    Ok(())
  }
}

struct JunitReport {
  name: String,
  time: Duration,
  tests: usize,
  failures: usize,
  errors: usize,
  test_suites: Vec<JunitTestSuite>,
}

impl JunitReport {
  fn new(
    name: impl Into<String>,
    time: Duration,
    test_suites: impl IntoIterator<Item = JunitTestSuite>,
  ) -> Self {
    let test_suites = test_suites.into_iter().collect::<Vec<_>>();
    Self {
      name: name.into(),
      time,
      tests: test_suites.iter().map(|suite| suite.tests).sum(),
      failures: test_suites.iter().map(|suite| suite.failures).sum(),
      errors: test_suites.iter().map(|suite| suite.errors).sum(),
      test_suites,
    }
  }

  fn serialize(&self, mut writer: impl Write) -> std::io::Result<()> {
    writeln!(writer, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    write!(writer, "<testsuites")?;
    write_attr(&mut writer, "name", &self.name)?;
    write_attr(&mut writer, "tests", &self.tests.to_string())?;
    write_attr(&mut writer, "failures", &self.failures.to_string())?;
    write_attr(&mut writer, "errors", &self.errors.to_string())?;
    write_attr(&mut writer, "time", &format_time(self.time))?;
    writeln!(writer, ">")?;
    for test_suite in &self.test_suites {
      test_suite.serialize(&mut writer)?;
    }
    writeln!(writer, "</testsuites>")
  }
}

struct JunitTestSuite {
  name: String,
  tests: usize,
  disabled: usize,
  errors: usize,
  failures: usize,
  test_cases: Vec<JunitTestCase>,
}

impl JunitTestSuite {
  fn new(name: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      tests: 0,
      disabled: 0,
      errors: 0,
      failures: 0,
      test_cases: vec![],
    }
  }

  fn add_test_case(&mut self, test_case: JunitTestCase) {
    self.tests += 1;
    match &test_case.status {
      JunitTestCaseStatus::Success => {}
      JunitTestCaseStatus::NonSuccess { kind, .. } => match kind {
        JunitNonSuccessKind::Failure => self.failures += 1,
        JunitNonSuccessKind::Error => self.errors += 1,
      },
      JunitTestCaseStatus::Skipped { .. } => self.disabled += 1,
    }
    self.test_cases.push(test_case);
  }

  fn serialize(&self, mut writer: impl Write) -> std::io::Result<()> {
    write!(writer, "    <testsuite")?;
    write_attr(&mut writer, "name", &self.name)?;
    write_attr(&mut writer, "tests", &self.tests.to_string())?;
    write_attr(&mut writer, "disabled", &self.disabled.to_string())?;
    write_attr(&mut writer, "errors", &self.errors.to_string())?;
    write_attr(&mut writer, "failures", &self.failures.to_string())?;
    writeln!(writer, ">")?;
    for test_case in &self.test_cases {
      test_case.serialize(&mut writer)?;
    }
    writeln!(writer, "    </testsuite>")
  }
}

#[derive(Clone)]
struct JunitTestCase {
  name: String,
  classname: Option<String>,
  time: Option<Duration>,
  status: JunitTestCaseStatus,
  extra: IndexMap<String, String>,
}

impl JunitTestCase {
  fn new(name: impl Into<String>, status: JunitTestCaseStatus) -> Self {
    Self {
      name: name.into(),
      classname: None,
      time: None,
      status,
      extra: IndexMap::new(),
    }
  }

  fn set_time(&mut self, time: Duration) {
    self.time = Some(time);
  }

  fn serialize(&self, mut writer: impl Write) -> std::io::Result<()> {
    write!(writer, "        <testcase")?;
    write_attr(&mut writer, "name", &self.name)?;
    if let Some(classname) = &self.classname {
      write_attr(&mut writer, "classname", classname)?;
    }
    if let Some(time) = self.time {
      write_attr(&mut writer, "time", &format_time(time))?;
    }
    for (key, value) in &self.extra {
      write_attr(&mut writer, key, value)?;
    }
    writeln!(writer, ">")?;
    self.status.serialize(&mut writer)?;
    writeln!(writer, "        </testcase>")
  }
}

#[derive(Clone)]
enum JunitTestCaseStatus {
  Success,
  NonSuccess {
    kind: JunitNonSuccessKind,
    message: Option<String>,
    ty: Option<String>,
    description: Option<String>,
  },
  Skipped {
    message: Option<String>,
    ty: Option<String>,
    description: Option<String>,
  },
}

impl JunitTestCaseStatus {
  fn success() -> Self {
    Self::Success
  }

  fn skipped() -> Self {
    Self::Skipped {
      message: None,
      ty: None,
      description: None,
    }
  }

  fn serialize(&self, mut writer: impl Write) -> std::io::Result<()> {
    match self {
      Self::Success => Ok(()),
      Self::NonSuccess {
        kind,
        message,
        ty,
        description,
      } => serialize_status(
        &mut writer,
        kind.tag_name(),
        message.as_deref(),
        ty.as_deref(),
        description.as_deref(),
      ),
      Self::Skipped {
        message,
        ty,
        description,
      } => serialize_status(
        &mut writer,
        "skipped",
        message.as_deref(),
        ty.as_deref(),
        description.as_deref(),
      ),
    }
  }
}

#[derive(Clone)]
enum JunitNonSuccessKind {
  Failure,
  Error,
}

impl JunitNonSuccessKind {
  fn tag_name(&self) -> &'static str {
    match self {
      Self::Failure => "failure",
      Self::Error => "error",
    }
  }
}

fn serialize_status(
  mut writer: impl Write,
  tag_name: &str,
  message: Option<&str>,
  ty: Option<&str>,
  description: Option<&str>,
) -> std::io::Result<()> {
  write!(writer, "            <{tag_name}")?;
  if let Some(message) = message {
    write_attr(&mut writer, "message", message)?;
  }
  if let Some(ty) = ty {
    write_attr(&mut writer, "type", ty)?;
  }
  if let Some(description) = description {
    write!(writer, ">")?;
    write_escaped(&mut writer, description)?;
    writeln!(writer, "</{tag_name}>")
  } else {
    writeln!(writer, "/>")
  }
}

fn write_attr(
  mut writer: impl Write,
  name: &str,
  value: &str,
) -> std::io::Result<()> {
  write!(writer, " {name}=\"")?;
  write_escaped(&mut writer, value)?;
  write!(writer, "\"")
}

fn write_escaped(mut writer: impl Write, value: &str) -> std::io::Result<()> {
  for ch in value.chars() {
    match ch {
      '<' => write!(writer, "&lt;")?,
      '>' => write!(writer, "&gt;")?,
      '&' => write!(writer, "&amp;")?,
      '\'' => write!(writer, "&apos;")?,
      '"' => write!(writer, "&quot;")?,
      _ => write!(writer, "{ch}")?,
    }
  }
  Ok(())
}

fn format_time(time: Duration) -> String {
  format!("{:.3}", time.as_secs_f64())
}

#[derive(Debug, Default)]
struct TestNameTree(IndexMap<usize, TestNameTreeNode>);

impl TestNameTree {
  fn new() -> Self {
    // Pre-allocate some space to avoid excessive reallocations.
    Self(IndexMap::with_capacity(256))
  }

  fn add_node(&mut self, node: TestNameTreeNode) {
    self.0.insert(node.id, node);
  }

  /// Constructs the full test name by traversing the tree from the specified
  /// node as a child to its parent nodes.
  /// If the provided ID is not found in the tree, or the tree is broken (e.g.
  /// a child node refers to a parent node that doesn't exist), this method
  /// just panics.
  fn construct_full_test_name(&self, id: usize) -> String {
    let mut current_id = Some(id);
    let mut name_pieces = VecDeque::new();

    while let Some(id) = current_id {
      let Some(node) = self.0.get(&id) else {
        // The ID specified as a parent node by the child node should exist in
        // the tree, but it doesn't. In this case we give up constructing the
        // full test name.
        unreachable!("Unregistered test ID '{id}' provided");
      };

      name_pieces.push_front(node.test_name.as_str());
      current_id = node.parent_id;
    }

    if name_pieces.is_empty() {
      unreachable!("Unregistered test ID '{id}' provided");
    }

    let v: Vec<_> = name_pieces.into();
    v.join(" > ")
  }
}

#[derive(Debug)]
struct TestNameTreeNode {
  id: usize,
  parent_id: Option<usize>,
  test_name: String,
}

impl From<TestDescription> for TestNameTreeNode {
  fn from(description: TestDescription) -> Self {
    Self {
      id: description.id,
      parent_id: None,
      test_name: description.name,
    }
  }
}

impl From<TestStepDescription> for TestNameTreeNode {
  fn from(description: TestStepDescription) -> Self {
    Self {
      id: description.id,
      parent_id: Some(description.parent_id),
      test_name: description.name,
    }
  }
}

#[cfg(test)]
mod tests {
  use deno_core::error::JsStackFrame;

  use super::*;

  #[test]
  fn construct_full_test_name_one_node() {
    let mut tree = TestNameTree::new();
    tree.add_node(TestNameTreeNode {
      id: 0,
      parent_id: None,
      test_name: "root".to_string(),
    });

    assert_eq!(tree.construct_full_test_name(0), "root".to_string());
  }

  #[test]
  fn construct_full_test_name_two_level_hierarchy() {
    let mut tree = TestNameTree::new();
    tree.add_node(TestNameTreeNode {
      id: 0,
      parent_id: None,
      test_name: "root".to_string(),
    });
    tree.add_node(TestNameTreeNode {
      id: 1,
      parent_id: Some(0),
      test_name: "child".to_string(),
    });

    assert_eq!(tree.construct_full_test_name(0), "root".to_string());
    assert_eq!(tree.construct_full_test_name(1), "root > child".to_string());
  }

  #[test]
  fn construct_full_test_name_three_level_hierarchy() {
    let mut tree = TestNameTree::new();
    tree.add_node(TestNameTreeNode {
      id: 0,
      parent_id: None,
      test_name: "root".to_string(),
    });
    tree.add_node(TestNameTreeNode {
      id: 1,
      parent_id: Some(0),
      test_name: "child".to_string(),
    });
    tree.add_node(TestNameTreeNode {
      id: 2,
      parent_id: Some(1),
      test_name: "grandchild".to_string(),
    });

    assert_eq!(tree.construct_full_test_name(0), "root".to_string());
    assert_eq!(tree.construct_full_test_name(1), "root > child".to_string());
    assert_eq!(
      tree.construct_full_test_name(2),
      "root > child > grandchild".to_string()
    );
  }

  #[test]
  fn construct_full_test_name_one_root_two_chains() {
    //     0
    //    / \
    //   1  2
    //  / \
    // 3  4
    let mut tree = TestNameTree::new();
    tree.add_node(TestNameTreeNode {
      id: 0,
      parent_id: None,
      test_name: "root".to_string(),
    });
    tree.add_node(TestNameTreeNode {
      id: 1,
      parent_id: Some(0),
      test_name: "child 1".to_string(),
    });
    tree.add_node(TestNameTreeNode {
      id: 2,
      parent_id: Some(0),
      test_name: "child 2".to_string(),
    });
    tree.add_node(TestNameTreeNode {
      id: 3,
      parent_id: Some(1),
      test_name: "grandchild 1".to_string(),
    });
    tree.add_node(TestNameTreeNode {
      id: 4,
      parent_id: Some(1),
      test_name: "grandchild 2".to_string(),
    });

    assert_eq!(tree.construct_full_test_name(0), "root".to_string());
    assert_eq!(
      tree.construct_full_test_name(1),
      "root > child 1".to_string(),
    );
    assert_eq!(
      tree.construct_full_test_name(2),
      "root > child 2".to_string(),
    );
    assert_eq!(
      tree.construct_full_test_name(3),
      "root > child 1 > grandchild 1".to_string(),
    );
    assert_eq!(
      tree.construct_full_test_name(4),
      "root > child 1 > grandchild 2".to_string(),
    );
  }

  #[test]
  fn escapes_short_failure_message() {
    let jserror = JsError {
      exception_message: "Uncaught Error: \x1b[31mtest error\x1b[0m"
        .to_string(),
      frames: vec![JsStackFrame::from_location(
        Some("File name".to_string()),
        Some(10),
        Some(15),
      )],
      name: Some("Error".to_string()),
      message: Some("test error".to_string()),
      source_line: Some("&quot;source \x1b[32mline\x1b[0m&quot;".to_string()),
      source_line_frame_index: Some(0),
      stack: None,
      cause: None,
      aggregated: None,
      additional_properties: vec![],
    };

    let step_result =
      TestStepResult::Failed(TestFailure::JsError(Box::new(jserror)));
    let step = JunitTestReporter::convert_step_status(
      &step_result,
      &TestFailureFormatOptions {
        strip_ascii_color: true,
        hide_stacktraces: false,
        ..Default::default()
      },
    );
    if let JunitTestCaseStatus::NonSuccess {
      description,
      message,
      ..
    } = step
    {
      assert!(!description.unwrap().contains("\x1b"));
      assert!(!message.unwrap().contains("\x1b"));
    } else {
      panic!("Expected NonSuccess status");
    }
  }
}
