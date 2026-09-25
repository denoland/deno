// Copyright 2018-2026 the Deno authors. MIT license.

// The logic of this module is heavily influenced by
// https://github.com/microsoft/vscode/blob/main/extensions/typescript-language-features/src/languageFeatures/semanticTokens.ts
// and https://github.com/microsoft/vscode/blob/main/src/vs/workbench/api/common/extHostTypes.ts
// for the SemanticTokensBuilder implementation.

use std::ops::Index;
use std::ops::IndexMut;

use tower_lsp::lsp_types::SemanticTokenModifier;
use tower_lsp::lsp_types::SemanticTokenType;
use tower_lsp::lsp_types::SemanticTokensLegend;

enum TokenType {
  Class = 0,
  Enum = 1,
  Interface = 2,
  Namespace = 3,
  TypeParameter = 4,
  Type = 5,
  Parameter = 6,
  Variable = 7,
  EnumMember = 8,
  Property = 9,
  Function = 10,
  Method = 11,
}

impl<T> Index<TokenType> for Vec<T> {
  type Output = T;
  fn index(&self, idx: TokenType) -> &T {
    &self[idx as usize]
  }
}

impl<T> IndexMut<TokenType> for Vec<T> {
  fn index_mut(&mut self, idx: TokenType) -> &mut T {
    &mut self[idx as usize]
  }
}

enum TokenModifier {
  Declaration = 0,
  Static = 1,
  Async = 2,
  Readonly = 3,
  DefaultLibrary = 4,
  Local = 5,
}

impl<T> Index<TokenModifier> for Vec<T> {
  type Output = T;
  fn index(&self, idx: TokenModifier) -> &T {
    &self[idx as usize]
  }
}

impl<T> IndexMut<TokenModifier> for Vec<T> {
  fn index_mut(&mut self, idx: TokenModifier) -> &mut T {
    &mut self[idx as usize]
  }
}

pub fn get_legend() -> SemanticTokensLegend {
  let mut token_types = vec![SemanticTokenType::from(""); 12];
  token_types[TokenType::Class] = "class".into();
  token_types[TokenType::Enum] = "enum".into();
  token_types[TokenType::Interface] = "interface".into();
  token_types[TokenType::Namespace] = "namespace".into();
  token_types[TokenType::TypeParameter] = "typeParameter".into();
  token_types[TokenType::Type] = "type".into();
  token_types[TokenType::Parameter] = "parameter".into();
  token_types[TokenType::Variable] = "variable".into();
  token_types[TokenType::EnumMember] = "enumMember".into();
  token_types[TokenType::Property] = "property".into();
  token_types[TokenType::Function] = "function".into();
  token_types[TokenType::Method] = "method".into();

  let mut token_modifiers = vec![SemanticTokenModifier::from(""); 6];
  token_modifiers[TokenModifier::Async] = "async".into();
  token_modifiers[TokenModifier::Declaration] = "declaration".into();
  token_modifiers[TokenModifier::Readonly] = "readonly".into();
  token_modifiers[TokenModifier::Static] = "static".into();
  token_modifiers[TokenModifier::Local] = "local".into();
  token_modifiers[TokenModifier::DefaultLibrary] = "defaultLibrary".into();

  SemanticTokensLegend {
    token_types,
    token_modifiers,
  }
}
