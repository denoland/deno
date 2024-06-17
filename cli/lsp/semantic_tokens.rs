// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// The logic of this module is heavily influenced by
// https://github.com/microsoft/vscode/blob/main/extensions/typescript-language-features/src/languageFeatures/semanticTokens.ts
// and https://github.com/microsoft/vscode/blob/main/src/vs/workbench/api/common/extHostTypes.ts
// for the SemanticTokensBuilder implementation.

use std::ops::Index;
use std::ops::IndexMut;
use tower_lsp::lsp_types as lsp;
use tower_lsp::lsp_types::SemanticToken;
use tower_lsp::lsp_types::SemanticTokenModifier;
use tower_lsp::lsp_types::SemanticTokenType;
use tower_lsp::lsp_types::SemanticTokens;
use tower_lsp::lsp_types::SemanticTokensLegend;

pub const MODIFIER_MASK: u32 = 255;
pub const TYPE_OFFSET: u32 = 8;

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

pub fn tokens_within_range(
  tokens: &SemanticTokens,
  range: lsp::Range,
) -> SemanticTokens {
  let mut line = 0;
  let mut character = 0;

  let mut first_token_line = 0;
  let mut first_token_char = 0;
  let mut keep_start_idx = tokens.data.len();
  let mut keep_end_idx = keep_start_idx;
  for (i, token) in tokens.data.iter().enumerate() {
    if token.delta_line != 0 {
      character = 0;
    }
    line += token.delta_line;
    character += token.delta_start;
    let token_start = lsp::Position::new(line, character);
    if i < keep_start_idx && token_start >= range.start {
      keep_start_idx = i;
      first_token_line = line;
      first_token_char = character;
    }
    if token_start > range.end {
      keep_end_idx = i;
      break;
    }
  }
  if keep_end_idx == keep_start_idx {
    return SemanticTokens {
      result_id: None,
      data: Vec::new(),
    };
  }

  let mut data = tokens.data[keep_start_idx..keep_end_idx].to_vec();
  // we need to adjust the delta_line and delta_start on the first token
  // as it is relative to 0 now, not the previous token
  let first_token = &mut data[0];
  first_token.delta_line = first_token_line;
  first_token.delta_start = first_token_char;

  SemanticTokens {
    result_id: None,
    data,
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

  #[test]
  fn test_tokens_within_range() {
    let mut builder = SemanticTokensBuilder::new();
    builder.push(1, 0, 5, 0, 0);
    builder.push(2, 1, 1, 1, 0);
    builder.push(2, 2, 3, 2, 0);
    builder.push(2, 5, 5, 3, 0);
    builder.push(3, 0, 4, 4, 0);
    builder.push(5, 2, 3, 5, 0);
    let tokens = builder.build(None);
    let range = lsp::Range {
      start: lsp::Position {
        line: 2,
        character: 2,
      },
      end: lsp::Position {
        line: 4,
        character: 0,
      },
    };

    let result = tokens_within_range(&tokens, range);

    assert_eq!(
      result.data,
      vec![
        // line 2 char 2
        SemanticToken {
          delta_line: 2,
          delta_start: 2,
          length: 3,
          token_type: 2,
          token_modifiers_bitset: 0
        },
        // line 2 char 5
        SemanticToken {
          delta_line: 0,
          delta_start: 3,
          length: 5,
          token_type: 3,
          token_modifiers_bitset: 0
        },
        // line 3 char 0
        SemanticToken {
          delta_line: 1,
          delta_start: 0,
          length: 4,
          token_type: 4,
          token_modifiers_bitset: 0
        }
      ]
    );
  }

  #[test]
  fn test_tokens_within_range_include_end() {
    let mut builder = SemanticTokensBuilder::new();
    builder.push(1, 0, 1, 0, 0);
    builder.push(2, 1, 2, 1, 0);
    builder.push(2, 3, 3, 2, 0);
    builder.push(3, 0, 4, 3, 0);
    let tokens = builder.build(None);
    let range = lsp::Range {
      start: lsp::Position {
        line: 2,
        character: 2,
      },
      end: lsp::Position {
        line: 3,
        character: 4,
      },
    };
    let result = tokens_within_range(&tokens, range);

    assert_eq!(
      result.data,
      vec![
        // line 2 char 3
        SemanticToken {
          delta_line: 2,
          delta_start: 3,
          length: 3,
          token_type: 2,
          token_modifiers_bitset: 0
        },
        // line 3 char 0
        SemanticToken {
          delta_line: 1,
          delta_start: 0,
          length: 4,
          token_type: 3,
          token_modifiers_bitset: 0
        }
      ]
    );
  }

  #[test]
  fn test_tokens_within_range_empty() {
    let mut builder = SemanticTokensBuilder::new();
    builder.push(1, 0, 1, 0, 0);
    builder.push(2, 1, 2, 1, 0);
    builder.push(2, 3, 3, 2, 0);
    builder.push(3, 0, 4, 3, 0);
    let tokens = builder.build(None);
    let range = lsp::Range {
      start: lsp::Position {
        line: 3,
        character: 2,
      },
      end: lsp::Position {
        line: 3,
        character: 4,
      },
    };
    let result = tokens_within_range(&tokens, range);

    assert_eq!(result.data, vec![]);

    assert_eq!(
      tokens_within_range(&SemanticTokens::default(), range).data,
      vec![]
    );
  }
}
