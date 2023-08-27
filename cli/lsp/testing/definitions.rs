// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::lsp_custom;

use crate::lsp::analysis::source_range_to_lsp_range;
use crate::lsp::client::TestingNotification;
use crate::util::checksum;

use deno_ast::SourceRange;
use deno_ast::SourceTextInfo;
use deno_core::ModuleSpecifier;
use std::collections::HashMap;
use tower_lsp::lsp_types as lsp;

#[derive(Debug, Clone, PartialEq)]
pub struct TestDefinition {
  pub id: String,
  pub name: String,
  pub range: SourceRange,
  pub steps: Vec<TestDefinition>,
}

impl TestDefinition {
  pub fn new(
    specifier: &ModuleSpecifier,
    name: String,
    range: SourceRange,
    mut steps: Vec<TestDefinition>,
  ) -> Self {
    let mut id_components = Vec::with_capacity(7);
    id_components.push(specifier.as_str().as_bytes());
    id_components.push(name.as_bytes());
    let id = checksum::gen(&id_components);
    Self::fix_ids(&mut steps, &mut id_components);
    Self {
      id,
      name,
      range,
      steps,
    }
  }

  fn fix_ids<'a>(
    steps: &'a mut Vec<TestDefinition>,
    id_components: &mut Vec<&'a [u8]>,
  ) {
    for step in steps {
      id_components.push(step.name.as_bytes());
      step.id = checksum::gen(id_components);
      Self::fix_ids(&mut step.steps, id_components);
      id_components.pop();
    }
  }

  pub fn new_step(
    name: String,
    range: SourceRange,
    steps: Vec<TestDefinition>,
  ) -> Self {
    Self {
      // ID will be fixed later when the entire ancestry is available.
      id: "".to_string(),
      name,
      range,
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
      steps: self
        .steps
        .iter()
        .map(|step| step.as_test_data(source_text_info))
        .collect(),
      range: Some(source_range_to_lsp_range(&self.range, source_text_info)),
    }
  }

  fn contains_id<S: AsRef<str>>(&self, id: S) -> bool {
    let id = id.as_ref();
    self.id == id || self.steps.iter().any(|td| td.contains_id(id))
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

impl Default for TestDefinitions {
  fn default() -> Self {
    TestDefinitions {
      script_version: "1".to_string(),
      discovered: vec![],
      injected: vec![],
    }
  }
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

  /// Register a dynamically-detected test. Returns false if a test with the
  /// same static id was already registered statically or dynamically. Otherwise
  /// returns true.
  pub fn inject(&mut self, data: lsp_custom::TestData) -> bool {
    if self.discovered.iter().any(|td| td.contains_id(&data.id))
      || self.injected.iter().any(|td| td.id == data.id)
    {
      return false;
    }
    self.injected.push(data);
    true
  }

  /// Return a test definition identified by the test ID.
  pub fn get_by_id<S: AsRef<str>>(&self, id: S) -> Option<&TestDefinition> {
    self
      .discovered
      .iter()
      .find(|td| td.id.as_str() == id.as_ref())
  }

  pub fn is_empty(&self) -> bool {
    self.discovered.is_empty() && self.injected.is_empty()
  }
}
