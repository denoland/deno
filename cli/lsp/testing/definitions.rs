// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use super::lsp_custom;
use super::lsp_custom::TestData;

use crate::lsp::client::TestingNotification;
use crate::lsp::logging::lsp_warn;
use crate::lsp::urls::url_to_uri;
use crate::tools::test::TestDescription;
use crate::tools::test::TestStepDescription;
use crate::util::checksum;

use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use lsp::Range;
use std::collections::HashMap;
use std::collections::HashSet;
use tower_lsp::lsp_types as lsp;

#[derive(Debug, Clone, PartialEq)]
pub struct TestDefinition {
  pub id: String,
  pub name: String,
  pub range: Option<Range>,
  pub is_dynamic: bool,
  pub parent_id: Option<String>,
  pub step_ids: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestModule {
  pub specifier: ModuleSpecifier,
  pub defs: HashMap<String, TestDefinition>,
}

impl TestModule {
  pub fn new(specifier: ModuleSpecifier) -> Self {
    Self {
      specifier,
      defs: Default::default(),
    }
  }

  /// Returns `(id, is_newly_registered)`.
  pub fn register(
    &mut self,
    name: String,
    range: Option<Range>,
    is_dynamic: bool,
    parent_id: Option<String>,
  ) -> (String, bool) {
    let mut id_components = Vec::with_capacity(7);
    id_components.push(name.as_bytes());
    let mut current_parent_id = &parent_id;
    while let Some(parent_id) = current_parent_id {
      let parent = match self.defs.get(parent_id) {
        Some(d) => d,
        None => {
          lsp_warn!("Internal Error: parent_id \"{}\" of test \"{}\" was not registered.", parent_id, &name);
          id_components.push("<unknown>".as_bytes());
          break;
        }
      };
      id_components.push(parent.name.as_bytes());
      current_parent_id = &parent.parent_id;
    }
    id_components.push(self.specifier.as_str().as_bytes());
    id_components.reverse();
    let id = checksum::gen(&id_components);
    if self.defs.contains_key(&id) {
      return (id, false);
    }
    if let Some(parent_id) = &parent_id {
      let parent = self.defs.get_mut(parent_id).unwrap();
      parent.step_ids.insert(id.clone());
    }
    self.defs.insert(
      id.clone(),
      TestDefinition {
        id: id.clone(),
        name,
        range,
        is_dynamic,
        parent_id,
        step_ids: Default::default(),
      },
    );
    (id, true)
  }

  /// Returns `(id, was_newly_registered)`.
  pub fn register_dynamic(&mut self, desc: &TestDescription) -> (String, bool) {
    self.register(desc.name.clone(), None, true, None)
  }

  /// Returns `(id, was_newly_registered)`.
  pub fn register_step_dynamic(
    &mut self,
    desc: &TestStepDescription,
    parent_static_id: &str,
  ) -> (String, bool) {
    self.register(
      desc.name.clone(),
      None,
      true,
      Some(parent_static_id.to_string()),
    )
  }

  pub fn get(&self, id: &str) -> Option<&TestDefinition> {
    self.defs.get(id)
  }

  pub fn get_test_data(&self, id: &str) -> TestData {
    fn get_test_data_inner(tm: &TestModule, id: &str) -> TestData {
      let def = tm.defs.get(id).unwrap();
      TestData {
        id: def.id.clone(),
        label: def.name.clone(),
        steps: def
          .step_ids
          .iter()
          .map(|id| get_test_data_inner(tm, id))
          .collect(),
        range: def.range,
      }
    }
    let def = self.defs.get(id).unwrap();
    let mut current_data = get_test_data_inner(self, &def.id);
    let mut current_parent_id = &def.parent_id;
    while let Some(parent_id) = current_parent_id {
      let parent = self.defs.get(parent_id).unwrap();
      current_data = TestData {
        id: parent.id.clone(),
        label: parent.name.clone(),
        steps: vec![current_data],
        range: None,
      };
      current_parent_id = &parent.parent_id;
    }
    current_data
  }

  /// Return the test definitions as a testing module notification.
  pub fn as_replace_notification(
    &self,
    maybe_root_uri: Option<&ModuleSpecifier>,
  ) -> Result<TestingNotification, AnyError> {
    let label = self.label(maybe_root_uri);
    Ok(TestingNotification::Module(
      lsp_custom::TestModuleNotificationParams {
        text_document: lsp::TextDocumentIdentifier {
          uri: url_to_uri(&self.specifier)?,
        },
        kind: lsp_custom::TestModuleNotificationKind::Replace,
        label,
        tests: self
          .defs
          .iter()
          .filter(|(_, def)| def.parent_id.is_none())
          .map(|(id, _)| self.get_test_data(id))
          .collect(),
      },
    ))
  }

  pub fn label(&self, maybe_root_uri: Option<&ModuleSpecifier>) -> String {
    if let Some(root) = maybe_root_uri {
      self.specifier.as_str().replace(root.as_str(), "")
    } else {
      self
        .specifier
        .path_segments()
        .and_then(|s| s.last().map(|s| s.to_string()))
        .unwrap_or_else(|| "<unknown>".to_string())
    }
  }

  pub fn is_empty(&self) -> bool {
    self.defs.is_empty()
  }
}
