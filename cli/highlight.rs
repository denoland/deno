use crate::ast;
use crate::ast::TokenOrComment;
use crate::colors;
use crate::media_type::MediaType;
use swc_ecmascript::parser::token::Token;
use swc_ecmascript::parser::token::Word;

pub fn highlight_line(line: &str, media_type: &MediaType) -> String {
  let mut out_line = String::from(line);

  for item in ast::lex("", line, media_type) {
    // Adding color adds more bytes to the string,
    // so an offset is needed to stop spans falling out of sync.
    let offset = out_line.len() - line.len();
    let span = item.span_as_range();

    out_line.replace_range(
      span.start + offset..span.end + offset,
      &match item.inner {
        TokenOrComment::Token(token) => match token {
          Token::Str { .. } | Token::Template { .. } | Token::BackQuote => {
            colors::green(&line[span]).to_string()
          }
          Token::Regex(_, _) => colors::red(&line[span]).to_string(),
          Token::Num(_) | Token::BigInt(_) => {
            colors::yellow(&line[span]).to_string()
          }
          Token::Word(word) => match word {
            Word::True | Word::False | Word::Null => {
              colors::yellow(&line[span]).to_string()
            }
            Word::Keyword(_) => colors::cyan(&line[span]).to_string(),
            Word::Ident(ident) => {
              if ident == *"undefined" {
                colors::gray(&line[span]).to_string()
              } else if ident == *"Infinity" || ident == *"NaN" {
                colors::yellow(&line[span]).to_string()
              } else if matches!(
                ident.as_ref(),
                "async" | "of" | "enum" | "type" | "interface"
              ) {
                colors::cyan(&line[span]).to_string()
              } else {
                line[span].to_string()
              }
            }
          },
          _ => line[span].to_string(),
        },
        TokenOrComment::Comment { .. } => colors::gray(&line[span]).to_string(),
      },
    );
  }

  out_line
}
