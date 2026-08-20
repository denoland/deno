// Copyright 2018-2026 the Deno authors. MIT license.

// The logic of this module is heavily influenced by
// https://github.com/microsoft/vscode/blob/main/extensions/typescript-language-features/src/languageFeatures/refactor.ts

use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use lsp_types::Uri;
use once_cell::sync::Lazy;
use tower_lsp::lsp_types as lsp;

pub struct RefactorCodeActionKind {
  pub kind: lsp::CodeActionKind,
}

impl RefactorCodeActionKind {}

pub static EXTRACT_FUNCTION: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR_EXTRACT.as_str(), "function"]
      .join(".")
      .into(),
  });

pub static EXTRACT_CONSTANT: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR_EXTRACT.as_str(), "constant"]
      .join(".")
      .into(),
  });

pub static EXTRACT_TYPE: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR_EXTRACT.as_str(), "type"]
      .join(".")
      .into(),
  });

pub static EXTRACT_INTERFACE: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR_EXTRACT.as_str(), "interface"]
      .join(".")
      .into(),
  });

pub static MOVE_NEWFILE: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR.as_str(), "move", "newFile"]
      .join(".")
      .into(),
  });

pub static REWRITE_IMPORT: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR_REWRITE.as_str(), "import"]
      .join(".")
      .into(),
  });

pub static REWRITE_EXPORT: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR_REWRITE.as_str(), "export"]
      .join(".")
      .into(),
  });

pub static REWRITE_ARROW_BRACES: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [
      lsp::CodeActionKind::REFACTOR_REWRITE.as_str(),
      "arrow",
      "braces",
    ]
    .join(".")
    .into(),
  });

pub static REWRITE_PARAMETERS_TO_DESTRUCTURED: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [
      lsp::CodeActionKind::REFACTOR_REWRITE.as_str(),
      "parameters",
      "toDestructured",
    ]
    .join(".")
    .into(),
  });

pub static REWRITE_PROPERTY_GENERATEACCESSORS: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [
      lsp::CodeActionKind::REFACTOR_REWRITE.as_str(),
      "property",
      "generateAccessors",
    ]
    .join(".")
    .into(),
  });

pub static INFER_FUNCTION_RETURN_TYPE: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [
      lsp::CodeActionKind::REFACTOR_REWRITE.as_str(),
      "function",
      "returnType",
    ]
    .join(".")
    .into(),
  });

pub static ALL_KNOWN_REFACTOR_ACTION_KINDS: Lazy<
  Vec<&'static RefactorCodeActionKind>,
> = Lazy::new(|| {
  vec![
    &EXTRACT_FUNCTION,
    &EXTRACT_CONSTANT,
    &EXTRACT_TYPE,
    &EXTRACT_INTERFACE,
    &MOVE_NEWFILE,
    &REWRITE_IMPORT,
    &REWRITE_EXPORT,
    &REWRITE_ARROW_BRACES,
    &REWRITE_PARAMETERS_TO_DESTRUCTURED,
    &REWRITE_PROPERTY_GENERATEACCESSORS,
    &INFER_FUNCTION_RETURN_TYPE,
  ]
});

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefactorCodeActionData {
  pub uri: Uri,
  pub range: lsp::Range,
  pub refactor_name: String,
  pub action_name: String,
}
