// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::VecDeque;
use std::path::PathBuf;

use super::*;

pub struct JunitTestReporter {
  path: String,
  // Stores TestCases (i.e. Tests) by the Test ID
  cases: IndexMap<usize, quick_junit::TestCase>,
  // Stores nodes representing test cases in such a way that can be traversed
  // from child to parent to build the full test name that reflects the test
  // hierarchy.
  test_name_tree: TestNameTree,
}

impl JunitTestReporter {
  pub fn new(path: String) -> Self {
    Self {
      path,
      cases: IndexMap::new(),
      test_name_tree: TestNameTree::new(),
    }
  }

  fn convert_status(status: &TestResult) -> quick_junit::TestCaseStatus {
    match status {
      TestResult::Ok => quick_junit::TestCaseStatus::success(),
      TestResult::Ignored => quick_junit::TestCaseStatus::skipped(),
      TestResult::Failed(failure) => quick_junit::TestCaseStatus::NonSuccess {
        kind: quick_junit::NonSuccessKind::Failure,
        message: Some(failure.to_string()),
        ty: None,
        description: None,
        reruns: vec![],
      },
      TestResult::Cancelled => quick_junit::TestCaseStatus::NonSuccess {
        kind: quick_junit::NonSuccessKind::Error,
        message: Some("Cancelled".to_string()),
        ty: None,
        description: None,
        reruns: vec![],
      },
    }
  }
}

impl TestReporter for JunitTestReporter {
  fn report_register(&mut self, description: &TestDescription) {
    let mut case = quick_junit::TestCase::new(
      description.name.clone(),
      quick_junit::TestCaseStatus::skipped(),
    );
    let file_name = description.location.file_name.clone();
    let file_name = file_name.strip_prefix("file://").unwrap_or(&file_name);
    case
      .extra
      .insert(String::from("filename"), String::from(file_name));
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

  fn report_wait(&mut self, _description: &TestDescription) {}

  fn report_output(&mut self, _output: &[u8]) {
    /*
     TODO(skycoop): Right now I can't include stdout/stderr in the report because
     we have a global pair of output streams that don't differentiate between the
     output of different tests. This is a nice to have feature, so we can come
     back to it later
    */
  }

  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    elapsed: u64,
  ) {
    if let Some(case) = self.cases.get_mut(&description.id) {
      case.status = Self::convert_status(result);
      case.set_time(Duration::from_millis(elapsed));
    }
  }

  fn report_uncaught_error(&mut self, _origin: &str, _error: Box<JsError>) {}

  fn report_step_register(&mut self, description: &TestStepDescription) {
    self.test_name_tree.add_node(description.clone().into());
  }

  fn report_step_wait(&mut self, _description: &TestStepDescription) {}

  fn report_step_result(
    &mut self,
    description: &TestStepDescription,
    result: &TestStepResult,
    _elapsed: u64,
    _tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    let status = match result {
      TestStepResult::Ok => "passed",
      TestStepResult::Ignored => "skipped",
      TestStepResult::Failed(_) => "failure",
    };

    let root_id: usize;
    let mut name = String::new();
    {
      let mut ancestors = vec![];
      let mut current_desc = description;
      loop {
        if let Some(d) = test_steps.get(&current_desc.parent_id) {
          ancestors.push(&d.name);
          current_desc = d;
        } else {
          root_id = current_desc.parent_id;
          break;
        }
      }
      ancestors.reverse();
      for n in ancestors {
        name.push_str(n);
        name.push_str(" ... ");
      }
      name.push_str(&description.name);
    }

    if let Some(case) = self.cases.get_mut(&root_id) {
      case.add_property(quick_junit::Property::new(
        format!("step[{}]", status),
        name,
      ));
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

  fn report_completed(&mut self) {
    // TODO(mmastrac): This reporter does not handle stdout/stderr yet, and when we do, we may need to redirect
    // pre-and-post-test output somewhere.
  }

  fn flush_report(
    &mut self,
    elapsed: &Duration,
    tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) -> anyhow::Result<()> {
    let mut suites: IndexMap<String, quick_junit::TestSuite> = IndexMap::new();
    for (id, case) in &self.cases {
      if let Some(test) = tests.get(id) {
        suites
          .entry(test.location.file_name.clone())
          .and_modify(|s| {
            s.add_test_case(case.clone());
          })
          .or_insert_with(|| {
            let mut suite =
              quick_junit::TestSuite::new(test.location.file_name.clone());
            suite.add_test_case(case.clone());
            suite
          });
      }
    }

    let mut report = quick_junit::Report::new("deno test");
    report
      .set_time(*elapsed)
      .add_test_suites(suites.into_values());

    if self.path == "-" {
      report
        .serialize(std::io::stdout())
        .with_context(|| "Failed to write JUnit report to stdout")?;
    } else {
      let file = crate::util::fs::create_file(&PathBuf::from(&self.path))
        .context("Failed to open JUnit report file.")?;
      report.serialize(file).with_context(|| {
        format!("Failed to write JUnit report to {}", self.path)
      })?;
    }

    Ok(())
  }
}

#[derive(Debug, Default)]
struct TestNameTree(IndexMap<usize, TestNameTreeNode>);

impl TestNameTree {
  fn new() -> Self {
    Self(IndexMap::new())
  }

  fn add_node(&mut self, node: TestNameTreeNode) {
    self.0.insert(node.id, node);
  }

  fn construct_full_test_name(&self, id: usize) -> Option<String> {
    let mut current_id = Some(id);
    let mut name_pieces = VecDeque::new();

    loop {
      let Some(id) = current_id else {
        break;
      };

      let Some(node) = self.0.get(&id) else {
        // The ID specified as a parent node by the child node should exist in
        // the tree, but it doesn't. In this case we give up constructing the
        // full test name, returning None.
        return None;
      };

      name_pieces.push_front(node.test_name.as_str());
      current_id = node.parent_id;
    }

    if name_pieces.is_empty() {
      return None;
    }

    let v: Vec<_> = name_pieces.into();
    Some(v.join(" > "))
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
  use super::*;

  #[test]
  fn construct_full_test_name_no_node() {
    let mut tree = TestNameTree::new();

    assert!(tree.construct_full_test_name(0).is_none());
  }

  #[test]
  fn construct_full_test_name_one_node() {
    let mut tree = TestNameTree::new();
    tree.add_node(TestNameTreeNode {
      id: 0,
      parent_id: None,
      test_name: "root".to_string(),
    });

    assert_eq!(tree.construct_full_test_name(0), Some("root".to_string()));
    assert!(tree.construct_full_test_name(1).is_none());
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

    assert_eq!(tree.construct_full_test_name(0), Some("root".to_string()));
    assert_eq!(
      tree.construct_full_test_name(1),
      Some("root > child".to_string()),
    );
    assert!(tree.construct_full_test_name(2).is_none());
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

    assert_eq!(tree.construct_full_test_name(0), Some("root".to_string()));
    assert_eq!(
      tree.construct_full_test_name(1),
      Some("root > child".to_string()),
    );
    assert_eq!(
      tree.construct_full_test_name(2),
      Some("root > child > grandchild".to_string()),
    );
    assert!(tree.construct_full_test_name(3).is_none());
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

    assert_eq!(tree.construct_full_test_name(0), Some("root".to_string()));
    assert_eq!(
      tree.construct_full_test_name(1),
      Some("root > child 1".to_string()),
    );
    assert_eq!(
      tree.construct_full_test_name(2),
      Some("root > child 2".to_string()),
    );
    assert_eq!(
      tree.construct_full_test_name(3),
      Some("root > child 1 > grandchild 1".to_string()),
    );
    assert_eq!(
      tree.construct_full_test_name(4),
      Some("root > child 1 > grandchild 2".to_string()),
    );
    assert!(tree.construct_full_test_name(5).is_none());
  }
}
