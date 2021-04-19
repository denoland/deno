// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// The logic of this module is heavily influenced by
// https://github.com/microsoft/vscode/blob/main/extensions/typescript-language-features/src/languageFeatures/semanticTokens.ts
// and https://github.com/microsoft/vscode/blob/main/src/vs/workbench/api/common/extHostTypes.ts
// for the SemanticTokensBuilder implementation.

use lspower::lsp::SemanticToken;
use lspower::lsp::SemanticTokenModifier;
use lspower::lsp::SemanticTokenType;
use lspower::lsp::SemanticTokens;
use lspower::lsp::SemanticTokensLegend;
use std::ops::{Index, IndexMut};

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
  token_modifiers[TokenModifier::Declaration] = "declaration".into();
  token_modifiers[TokenModifier::Static] = "static".into();
  token_modifiers[TokenModifier::Async] = "async".into();
  token_modifiers[TokenModifier::Readonly] = "readonly".into();
  token_modifiers[TokenModifier::DefaultLibrary] = "defaultLibrary".into();
  token_modifiers[TokenModifier::Local] = "local".into();

  SemanticTokensLegend {
    token_types,
    token_modifiers,
  }
}

pub enum TsTokenEncodingConsts {
  TypeOffset = 8,
  ModifierMask = 255,
}

pub struct SemanticTokensBuilder {
  prev_line: u32,
  prev_char: u32,
  data_is_sorted_and_delta_encoded: bool,
  data: Vec<u32>,
}

impl SemanticTokensBuilder {
  pub fn new() -> Self {
    Self {
      prev_line: 0,
      prev_char: 0,
      data_is_sorted_and_delta_encoded: true,
      data: Vec::new(),
    }
  }

  pub fn push(
    &mut self,
    line: u32,
    char: u32,
    length: u32,
    token_type: u32,
    token_modifiers: u32,
  ) {
    if self.data_is_sorted_and_delta_encoded
      && (line < self.prev_line
        || (line == self.prev_line && char < self.prev_char))
    {
      // push calls were ordered and are no longer ordered
      self.data_is_sorted_and_delta_encoded = false;

      // Remove delta encoding from data
      let token_count = self.data.len() / 5;
      let mut prev_line = 0;
      let mut prev_char = 0;
      for i in 0..token_count {
        let mut line = self.data[5 * i];
        let mut char = self.data[5 * i + 1];

        if line == 0 {
          // on the same line as previous token
          line = prev_line;
          char += prev_char;
        } else {
          // on a different line than previous token
          line += prev_line;
        }

        self.data[5 * i] = line;
        self.data[5 * i + 1] = char;

        prev_line = line;
        prev_char = char;
      }
    }

    let mut push_line = line;
    let mut push_char = char;
    if self.data_is_sorted_and_delta_encoded && !self.data.is_empty() {
      push_line -= self.prev_line;
      if push_line == 0 {
        push_char -= self.prev_char;
      }
    }

    self.data.reserve(5);
    self.data.push(push_line);
    self.data.push(push_char);
    self.data.push(length);
    self.data.push(token_type);
    self.data.push(token_modifiers);

    self.prev_line = line;
    self.prev_char = char;
  }

  fn data_to_semantic_token_vec(
    data: &[u32],
    data_is_sorted_and_delta_encoded: bool,
  ) -> Vec<SemanticToken> {
    let token_count = data.len() / 5;
    let mut result: Vec<SemanticToken> = Vec::with_capacity(token_count);
    if data_is_sorted_and_delta_encoded {
      for i in 0..token_count {
        let src_offset = 5 * i;
        result.push(SemanticToken {
          delta_line: data[src_offset],
          delta_start: data[src_offset + 1],
          length: data[src_offset + 2],
          token_type: data[src_offset + 3],
          token_modifiers_bitset: data[src_offset + 4],
        });
      }
      return result;
    }

    let mut pos: Vec<usize> = (0..token_count).collect();
    pos.sort_by(|a, b| {
      let a_line = data[5 * a];
      let b_line = data[5 * b];
      if a_line == b_line {
        let a_char = data[5 * a + 1];
        let b_char = data[5 * b + 1];
        return a_char.cmp(&b_char);
      }
      a_line.cmp(&b_line)
    });

    let mut prev_line = 0;
    let mut prev_char = 0;
    for i in pos.iter() {
      let src_offset = 5 * i;
      let line = data[src_offset];
      let char = data[src_offset + 1];
      let length = data[src_offset + 2];
      let token_type = data[src_offset + 3];
      let token_modifiers_bitset = data[src_offset + 4];

      let delta_line = line - prev_line;
      let delta_start = if delta_line == 0 {
        char - prev_char
      } else {
        char
      };

      result.push(SemanticToken {
        delta_line,
        delta_start,
        length,
        token_type,
        token_modifiers_bitset,
      });

      prev_line = line;
      prev_char = char;
    }

    result
  }

  pub fn build(&self, result_id: Option<String>) -> SemanticTokens {
    SemanticTokens {
      result_id,
      data: SemanticTokensBuilder::data_to_semantic_token_vec(
        &self.data,
        self.data_is_sorted_and_delta_encoded,
      ),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_semantic_tokens_builder_simple() {
    let mut builder = SemanticTokensBuilder::new();
    builder.push(1, 0, 5, 1, 1);
    builder.push(1, 10, 4, 2, 2);
    builder.push(2, 2, 3, 2, 2);
    assert_eq!(
      builder.build(None).data,
      vec![
        SemanticToken {
          delta_line: 1,
          delta_start: 0,
          length: 5,
          token_type: 1,
          token_modifiers_bitset: 1
        },
        SemanticToken {
          delta_line: 0,
          delta_start: 10,
          length: 4,
          token_type: 2,
          token_modifiers_bitset: 2
        },
        SemanticToken {
          delta_line: 1,
          delta_start: 2,
          length: 3,
          token_type: 2,
          token_modifiers_bitset: 2
        }
      ]
    );
  }

  #[test]
  fn test_semantic_tokens_builder_out_of_order_1() {
    let mut builder = SemanticTokensBuilder::new();
    builder.push(2, 0, 5, 1, 1);
    builder.push(2, 10, 1, 2, 2);
    builder.push(2, 15, 2, 3, 3);
    builder.push(1, 0, 4, 4, 4);
    assert_eq!(
      builder.build(None).data,
      vec![
        SemanticToken {
          delta_line: 1,
          delta_start: 0,
          length: 4,
          token_type: 4,
          token_modifiers_bitset: 4
        },
        SemanticToken {
          delta_line: 1,
          delta_start: 0,
          length: 5,
          token_type: 1,
          token_modifiers_bitset: 1
        },
        SemanticToken {
          delta_line: 0,
          delta_start: 10,
          length: 1,
          token_type: 2,
          token_modifiers_bitset: 2
        },
        SemanticToken {
          delta_line: 0,
          delta_start: 5,
          length: 2,
          token_type: 3,
          token_modifiers_bitset: 3
        }
      ]
    );
  }

  #[test]
  fn test_semantic_tokens_builder_out_of_order_2() {
    let mut builder = SemanticTokensBuilder::new();
    builder.push(2, 10, 5, 1, 1);
    builder.push(2, 2, 4, 2, 2);
    assert_eq!(
      builder.build(None).data,
      vec![
        SemanticToken {
          delta_line: 2,
          delta_start: 2,
          length: 4,
          token_type: 2,
          token_modifiers_bitset: 2
        },
        SemanticToken {
          delta_line: 0,
          delta_start: 8,
          length: 5,
          token_type: 1,
          token_modifiers_bitset: 1
        }
      ]
    );
  }
}
