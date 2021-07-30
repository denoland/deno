// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::ModuleSpecifier;
use lspower::lsp;

pub struct RefactorCodeActionKind {
  pub kind: lsp::CodeActionKind,
  matches_callback: Box<dyn Fn(&str) -> bool + Send + Sync>,
}

impl RefactorCodeActionKind {
  pub fn matches(&self, tag: &str) -> bool {
    (self.matches_callback)(tag)
  }
}

lazy_static::lazy_static! {
  pub static ref EXTRACT_FUNCTION: RefactorCodeActionKind = RefactorCodeActionKind {
    kind : [lsp::CodeActionKind::REFACTOR_EXTRACT.as_str(), "function"].join(".").into(),
    matches_callback: Box::new(|tag: &str| tag.starts_with("function_")),
  };

  pub static ref EXTRACT_CONSTANT: RefactorCodeActionKind = RefactorCodeActionKind {
    kind : [lsp::CodeActionKind::REFACTOR_EXTRACT.as_str(), "constant"].join(".").into(),
    matches_callback: Box::new(|tag: &str| tag.starts_with("constant_")),
  };

  pub static ref EXTRACT_TYPE: RefactorCodeActionKind = RefactorCodeActionKind {
    kind : [lsp::CodeActionKind::REFACTOR_EXTRACT.as_str(), "type"].join(".").into(),
    matches_callback: Box::new(|tag: &str| tag.starts_with("Extract to type alias")),
  };

  pub static ref EXTRACT_INTERFACE: RefactorCodeActionKind = RefactorCodeActionKind {
    kind : [lsp::CodeActionKind::REFACTOR_EXTRACT.as_str(), "interface"].join(".").into(),
    matches_callback: Box::new(|tag: &str| tag.starts_with("Extract to interface")),
  };

  pub static ref MOVE_NEWFILE: RefactorCodeActionKind = RefactorCodeActionKind {
    kind : [lsp::CodeActionKind::REFACTOR.as_str(), "move", "newFile"].join(".").into(),
    matches_callback: Box::new(|tag: &str| tag.starts_with("Move to a new file")),
  };

  pub static ref REWRITE_IMPORT: RefactorCodeActionKind = RefactorCodeActionKind {
    kind : [lsp::CodeActionKind::REFACTOR_REWRITE.as_str(), "import"].join(".").into(),
    matches_callback: Box::new(|tag: &str| tag.starts_with("Convert namespace import") || tag.starts_with("Convert named imports")),
  };

  pub static ref REWRITE_EXPORT: RefactorCodeActionKind = RefactorCodeActionKind {
    kind : [lsp::CodeActionKind::REFACTOR_REWRITE.as_str(), "export"].join(".").into(),
    matches_callback: Box::new(|tag: &str| tag.starts_with("Convert default export") || tag.starts_with("Convert named export")),
  };

  pub static ref REWRITE_ARROW_BRACES: RefactorCodeActionKind = RefactorCodeActionKind {
    kind : [lsp::CodeActionKind::REFACTOR_REWRITE.as_str(), "arrow", "braces"].join(".").into(),
    matches_callback: Box::new(|tag: &str| tag.starts_with("Add or remove braces in an arrow function")),
  };

  pub static ref REWRITE_PARAMETERS_TODESTRUCTURED: RefactorCodeActionKind = RefactorCodeActionKind {
    kind : [lsp::CodeActionKind::REFACTOR_REWRITE.as_str(), "parameters", "toDestructured"].join(".").into(),
    matches_callback: Box::new(|tag: &str| tag.starts_with("Convert parameters to destructured object")),
  };

  pub static ref REWRITE_PROPERTY_GENERATEACCESSORS: RefactorCodeActionKind = RefactorCodeActionKind {
    kind : [lsp::CodeActionKind::REFACTOR_REWRITE.as_str(), "property", "generateAccessors"].join(".").into(),
    matches_callback: Box::new(|tag: &str| tag.starts_with("Generate 'get' and 'set' accessors")),
  };

  pub static ref ALL_KNOWN_REFACTOR_ACTION_KINDS: Vec<&'static RefactorCodeActionKind> = vec![
    &EXTRACT_FUNCTION,
    &EXTRACT_CONSTANT,
    &EXTRACT_TYPE,
    &EXTRACT_INTERFACE,
    &MOVE_NEWFILE,
    &REWRITE_IMPORT,
    &REWRITE_EXPORT,
    &REWRITE_ARROW_BRACES,
    &REWRITE_PARAMETERS_TODESTRUCTURED,
    &REWRITE_PROPERTY_GENERATEACCESSORS
  ];
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefactorCodeActionData {
  pub specifier: ModuleSpecifier,
  pub range: lsp::Range,
  pub refactor_name: String,
  pub action_name: String,
}
