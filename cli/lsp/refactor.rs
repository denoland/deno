// Copyright 2018-2025 the Deno authors. MIT license.

// The logic of this module is heavily influenced by
// https://github.com/microsoft/vscode/blob/main/extensions/typescript-language-features/src/languageFeatures/refactor.ts

use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use lsp_types::Uri;
use once_cell::sync::Lazy;
use tower_lsp::lsp_types as lsp;

pub struct RefactorCodeActionKind {
  pub kind: lsp::CodeActionKind,
  matches_callback: Box<dyn Fn(&str) -> bool + Send + Sync>,
}

impl RefactorCodeActionKind {
  pub fn matches(&self, tag: &str) -> bool {
    (self.matches_callback)(tag)
  }
}

pub static EXTRACT_FUNCTION: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR_EXTRACT.as_str(), "function"]
      .join(".")
      .into(),
    matches_callback: Box::new(|tag: &str| tag.starts_with("function_")),
  });

pub static EXTRACT_CONSTANT: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR_EXTRACT.as_str(), "constant"]
      .join(".")
      .into(),
    matches_callback: Box::new(|tag: &str| tag.starts_with("constant_")),
  });

pub static EXTRACT_TYPE: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR_EXTRACT.as_str(), "type"]
      .join(".")
      .into(),
    matches_callback: Box::new(|tag: &str| {
      tag.starts_with("Extract to type alias")
    }),
  });

pub static EXTRACT_INTERFACE: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR_EXTRACT.as_str(), "interface"]
      .join(".")
      .into(),
    matches_callback: Box::new(|tag: &str| {
      tag.starts_with("Extract to interface")
    }),
  });

pub static MOVE_NEWFILE: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR.as_str(), "move", "newFile"]
      .join(".")
      .into(),
    matches_callback: Box::new(|tag: &str| {
      tag.starts_with("Move to a new file")
    }),
  });

pub static REWRITE_IMPORT: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR_REWRITE.as_str(), "import"]
      .join(".")
      .into(),
    matches_callback: Box::new(|tag: &str| {
      tag.starts_with("Convert namespace import")
        || tag.starts_with("Convert named imports")
    }),
  });

pub static REWRITE_EXPORT: Lazy<RefactorCodeActionKind> =
  Lazy::new(|| RefactorCodeActionKind {
    kind: [lsp::CodeActionKind::REFACTOR_REWRITE.as_str(), "export"]
      .join(".")
      .into(),
    matches_callback: Box::new(|tag: &str| {
      tag.starts_with("Convert default export")
        || tag.starts_with("Convert named export")
    }),
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
    matches_callback: Box::new(|tag: &str| {
      tag.starts_with("Add or remove braces in an arrow function")
    }),
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
    matches_callback: Box::new(|tag: &str| {
      tag.starts_with("Convert parameters to destructured object")
    }),
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
    matches_callback: Box::new(|tag: &str| {
      tag.starts_with("Generate 'get' and 'set' accessors")
    }),
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

pub fn prune_invalid_actions(
  actions: Vec<lsp::CodeAction>,
  number_of_invalid: usize,
) -> Vec<lsp::CodeAction> {
  let mut available_actions = Vec::<lsp::CodeAction>::new();
  let mut invalid_common_actions = Vec::<lsp::CodeAction>::new();
  let mut invalid_uncommon_actions = Vec::<lsp::CodeAction>::new();
  for action in actions {
    if action.disabled.is_none() {
      available_actions.push(action);
      continue;
    }

    // These are the common refactors that we should always show if applicable.
    let action_kind =
      action.kind.as_ref().map(|a| a.as_str()).unwrap_or_default();
    if action_kind.starts_with(EXTRACT_CONSTANT.kind.as_str())
      || action_kind.starts_with(EXTRACT_FUNCTION.kind.as_str())
    {
      invalid_common_actions.push(action);
      continue;
    }

    // These are the remaining refactors that we can show if we haven't reached the max limit with just common refactors.
    invalid_uncommon_actions.push(action);
  }

  let mut prioritized_actions = Vec::<lsp::CodeAction>::new();
  prioritized_actions.extend(invalid_common_actions);
  prioritized_actions.extend(invalid_uncommon_actions);
  let top_n_invalid = prioritized_actions
    [0..std::cmp::min(number_of_invalid, prioritized_actions.len())]
    .to_vec();
  available_actions.extend(top_n_invalid);
  available_actions
}
