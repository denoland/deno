// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::lsp_custom;

use crate::checksum;
use crate::lsp::client::TestingNotification;

use deno_ast::swc::common::Span;
use deno_ast::SourceTextInfo;
use deno_core::ModuleSpecifier;
use lspower::lsp;
use std::collections::HashMap;

fn span_to_range(
  span: &Span,
  source_text_info: &SourceTextInfo,
) -> Option<lsp::Range> {
  let start = source_text_info.line_and_column_index(span.lo);
  let end = source_text_info.line_and_column_index(span.hi);
  Some(lsp::Range {
    start: lsp::Position {
      line: start.line_index as u32,
      character: start.column_index as u32,
    },
    end: lsp::Position {
      line: end.line_index as u32,
      character: end.column_index as u32,
    },
  })
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestDefinition {
  pub id: String,
  pub level: usize,
  pub name: String,
  pub span: Span,
  pub steps: Option<Vec<TestDefinition>>,
}

impl TestDefinition {
  pub fn new(
    specifier: &ModuleSpecifier,
    name: String,
    span: Span,
    steps: Option<Vec<TestDefinition>>,
  ) -> Self {
    let id = checksum::gen(&[specifier.as_str().as_bytes(), name.as_bytes()]);
    Self {
      id,
      level: 0,
      name,
      span,
      steps,
    }
  }

  pub fn new_step(
    name: String,
    span: Span,
    parent: String,
    level: usize,
    steps: Option<Vec<TestDefinition>>,
  ) -> Self {
    let id = checksum::gen(&[
      parent.as_bytes(),
      &level.to_be_bytes(),
      name.as_bytes(),
    ]);
    Self {
      id,
      level,
      name,
      span,
      steps,
    }
  }

  fn as_test_data(
    &self,
    source_text_info: &SourceTextInfo,
  ) -> lsp_custom::TestData {
    lsp_custom::TestData {
      id: self.id.clone(),
      label: self.name.clone(),
      steps: self.steps.as_ref().map(|steps| {
        steps
          .iter()
          .map(|step| step.as_test_data(source_text_info))
          .collect()
      }),
      range: span_to_range(&self.span, source_text_info),
    }
  }

  fn find_step(&self, name: &str, level: usize) -> Option<&TestDefinition> {
    if let Some(steps) = &self.steps {
      for step in steps {
        if step.name == name && step.level == level {
          return Some(step);
        } else if let Some(step) = step.find_step(name, level) {
          return Some(step);
        }
      }
    }
    None
  }
}

#[derive(Debug, Clone)]
pub struct TestDefinitions {
  /// definitions of tests and their steps which were statically discovered from
  /// the source document.
  pub discovered: Vec<TestDefinition>,
  /// Tests and steps which the test runner notified us of, which were
  /// dynamically added
  pub injected: Vec<lsp_custom::TestData>,
  /// The version of the document that the discovered tests relate to.
  pub script_version: String,
}

impl TestDefinitions {
  /// Return the test definitions as a testing module notification.
  pub fn as_notification(
    &self,
    specifier: &ModuleSpecifier,
    maybe_root: Option<&ModuleSpecifier>,
    source_text_info: &SourceTextInfo,
  ) -> TestingNotification {
    let label = if let Some(root) = maybe_root {
      specifier.as_str().replace(root.as_str(), "")
    } else {
      specifier
        .path_segments()
        .and_then(|s| s.last().map(|s| s.to_string()))
        .unwrap_or_else(|| "<unknown>".to_string())
    };
    let mut tests_map: HashMap<String, lsp_custom::TestData> = self
      .injected
      .iter()
      .map(|td| (td.id.clone(), td.clone()))
      .collect();
    tests_map.extend(self.discovered.iter().map(|td| {
      let test_data = td.as_test_data(source_text_info);
      (test_data.id.clone(), test_data)
    }));
    TestingNotification::Module(lsp_custom::TestModuleNotificationParams {
      text_document: lsp::TextDocumentIdentifier {
        uri: specifier.clone(),
      },
      kind: lsp_custom::TestModuleNotificationKind::Replace,
      label,
      tests: tests_map.into_values().collect(),
    })
  }

  /// Return a test definition identified by the test ID.
  pub fn get_by_id<S: AsRef<str>>(&self, id: S) -> Option<&TestDefinition> {
    self
      .discovered
      .iter()
      .find(|td| td.id.as_str() == id.as_ref())
  }

  /// Return a test definition by the test name.
  pub fn get_by_name(&self, name: &str) -> Option<&TestDefinition> {
    self.discovered.iter().find(|td| td.name.as_str() == name)
  }

  pub fn get_step_by_name(
    &self,
    test_name: &str,
    level: usize,
    name: &str,
  ) -> Option<&TestDefinition> {
    self
      .get_by_name(test_name)
      .and_then(|td| td.find_step(name, level))
  }
}
