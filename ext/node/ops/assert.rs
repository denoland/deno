// Copyright 2018-2026 the Deno authors. MIT license.

use deno_ast::MediaType;
use deno_ast::TokenOrComment;
use deno_ast::swc::parser::token::Token;
use deno_ast::swc::parser::token::Word;
use deno_core::op2;

/// Tokens that represent member access operators: `.`, `[`, `]`.
/// Optional chaining `?.` is handled by detecting `?` + `.` token sequence.
fn is_member_access_token(token: &Token) -> bool {
  matches!(token, Token::Dot | Token::LBracket | Token::RBracket)
}

fn is_member_name_token(token: &Token) -> bool {
  matches!(
    token,
    Token::Word(..) | Token::Str { .. } | Token::Num { .. }
  )
}

fn is_ident_word(token: &Token) -> bool {
  matches!(token, Token::Word(Word::Ident(..)))
}

fn token_text<'a>(
  code: &'a str,
  range: &std::ops::Range<usize>,
) -> Option<&'a str> {
  if range.start <= range.end
    && range.end <= code.len()
    && code.is_char_boundary(range.start)
    && code.is_char_boundary(range.end)
  {
    Some(&code[range.start..range.end])
  } else {
    None
  }
}

fn is_question_token(code: &str, range: &std::ops::Range<usize>) -> bool {
  token_text(code, range) == Some("?")
}

fn adjust_start_column_for_non_ascii(
  code: &str,
  mut start_column: usize,
) -> usize {
  // Match the JS behavior that used `charCodeAt` on UTF-16 code units.
  let utf16_code_units: Vec<u16> = code.encode_utf16().collect();
  let mut index = 0;
  while index < start_column {
    if utf16_code_units.get(index).copied().unwrap_or_default() > 127 {
      start_column += 1;
    }
    index += 1;
  }
  start_column
}

/// Get the first expression in a code string at the start_column.
///
/// This mirrors Node.js's implementation
/// https://github.com/nodejs/node/blob/70f6b58ac655234435a99d72b857dd7b316d34bf/lib/internal/errors/error_source.js#L61-L142
#[op2(fast)]
pub fn op_node_get_first_expression(
  #[string] code: &str,
  #[smi] original_start_col_index: usize,
  #[buffer] out_buf: &mut [u32],
) {
  let start_index =
    adjust_start_column_for_non_ascii(code, original_start_col_index);

  let items = deno_ast::lex(code, MediaType::JavaScript);
  let tokens: Vec<(Token, std::ops::Range<usize>)> = items
    .into_iter()
    .filter_map(|item| match item.inner {
      TokenOrComment::Token(token) => Some((token, item.range)),
      TokenOrComment::Comment { .. } => None,
    })
    .collect();

  let mut last_token = None;
  let mut second_last_token = None;
  let mut first_member_access_name_token = None; // start position
  let mut terminating_col = None;
  let mut paren_lvl = 0;

  for (token, range) in &tokens {
    // Peek before the startColumn.
    if range.start < start_index {
      // There is a semicolon. This is a statement before the startColumn,
      // so reset the memo.
      if matches!(token, Token::Semi) {
        first_member_access_name_token = None;
        second_last_token = last_token;
        last_token = Some((token, range));
        continue;
      }

      // Try to memo the member access expressions before the startColumn,
      // so that the returned source code contains more info:
      //   assert.ok(value)
      //          ^ startColumn
      // The member expression can also be like
      //   assert['ok'](value) or assert?.ok(value)
      //               ^ startColumn      ^ startColumn
      let prev_is_question = last_token
        .map(|(_, last_range)| is_question_token(code, last_range))
        .unwrap_or(false);

      let is_optional_chain_dot =
        matches!(token, Token::Dot) && prev_is_question;

      let is_member_access =
        is_member_access_token(token) || is_optional_chain_dot;

      let member_access_base_token = if is_optional_chain_dot {
        second_last_token
      } else {
        last_token
      };

      if is_member_access
        && first_member_access_name_token.is_none()
        && let Some((last_tok, last_range)) = member_access_base_token
        && is_ident_word(last_tok)
      {
        first_member_access_name_token = Some(last_range.start);
      } else if !is_member_access
        && !is_member_name_token(token)
        && !is_question_token(code, range)
      {
        // Reset the memo if it is not a simple member access.
        // For example: assert[(() => 'ok')()](value)
        //                                    ^ startColumn
        first_member_access_name_token = None;
      }

      second_last_token = last_token;
      last_token = Some((token, range));
      continue;
    }

    // Now after the startColumn, this must be an expression.
    if matches!(token, Token::LParen) {
      paren_lvl += 1;
      continue;
    }

    if matches!(token, Token::RParen) {
      paren_lvl -= 1;
      if paren_lvl == 0 {
        // A matched closing parenthesis found after the startColumn,
        // terminate here. Include the token.
        //   (assert.ok(false), assert.ok(true))
        //           ^ startColumn
        terminating_col = Some(range.start + 1);
        break;
      }
      continue;
    }

    if matches!(token, Token::Semi) {
      // A semicolon found after the startColumn, terminate here.
      //   assert.ok(false); assert.ok(true));
      //          ^ startColumn
      terminating_col = Some(range.start);
      break;
    }
    // If no semicolon found after the startColumn. The string after the
    // startColumn must be the expression.
    //   assert.ok(false)
    //          ^ startColumn
  }

  let start = first_member_access_name_token.unwrap_or(start_index);
  let end = terminating_col.unwrap_or(code.len());
  if start <= end && end <= code.len() {
    out_buf[0] = code[..start].encode_utf16().count() as _;
    out_buf[1] = code[..end].encode_utf16().count() as _;
  } else {
    out_buf[0] = original_start_col_index as _;
    out_buf[1] = code.encode_utf16().count() as _;
  }
}
