use std::{iter::Peekable, str::Chars};

#[derive(Debug, PartialEq)]
enum RelationOp {
  Space,
  Plus,
  Tilde,
}

#[derive(Debug, PartialEq)]
enum AttrOp {
  Equal,
  NotEqual,
  Greater,
  GreaterEqaul,
  Less,
  LessEqual,
}

#[derive(Debug, PartialEq)]
enum AttrValue {
  True,
  False,
  Null,
  Undefined,
  Str(String),
  Num,
  Regex(String),
}

#[derive(Debug, PartialEq)]
enum SelPart {
  Wildcard,
  Elem(u8),
  Relation(RelationOp),
  AttrExists(Vec<u8>),
  AttrBin(AttrOp, Vec<u8>, String),
  FirstChild,
  LastChild,
  NthChild,
}

#[derive(Debug, PartialEq)]
enum OpValue {
  Plus,
  Tilde,
  Equal,
  NotEqual,
  Greater,
  GreaterThan,
  Less,
  LessThan,
}

#[derive(Debug, PartialEq)]
enum Token {
  Word(String),
  Space,
  Op(OpValue),
  Colon,
  Comma,
  BraceOpen,
  BraceClose,
  BracketOpen,
  BracketClose,
  String(String),
  Number,
  Bool,
  Null,
  Undefined,
  Dot,
  Minus,
}

enum ParseError {
  UnexpectedEnd(usize),
}

struct Lexer<'a> {
  i: usize,
  input: &'a str,
  iter: Peekable<Chars<'a>>,
}

impl<'a> Lexer<'a> {
  fn new(input: &'a str) -> Self {
    Self {
      i: 0,
      input,
      iter: input.chars().peekable(),
    }
  }

  fn step(&mut self) -> Option<char> {
    self.i += 1;
    self.iter.next()
  }

  fn skip_whitespace(&mut self, ch: char) -> char {
    while let Some(ch) = self.iter.peek() {
      if !is_whitespace(&ch) {
        return *ch;
      }

      self.step();
    }

    return ch;
  }

  fn next_token(&mut self) -> Option<Token> {
    while let Some(next) = self.step() {
      let mut ch = next;

      match ch {
        ' ' => {
          ch = self.skip_whitespace(ch);

          if is_op_continue(&ch) {
            continue;
          }

          return Some(Token::Space);
        }
        '[' => {
          return Some(Token::BracketOpen);
        }
        ']' => {
          return Some(Token::BracketClose);
        }
        '(' => {
          return Some(Token::BraceOpen);
        }
        ')' => {
          return Some(Token::BraceClose);
        }
        ',' => {
          self.skip_whitespace(ch);
          return Some(Token::Comma);
        }
        '.' => {
          return Some(Token::Dot);
        }
        ':' => {
          return Some(Token::Colon);
        }
        '_' => {
          return Some(Token::Minus);
        }
        '+' => {
          return Some(Token::Op(OpValue::Plus));
        }
        '~' => {
          return Some(Token::Op(OpValue::Tilde));
        }
        '=' => {
          return Some(Token::Op(OpValue::Equal));
        }
        '>' => {
          if let Some(next) = self.iter.peek() {
            if *next == '=' {
              let ch = self.step().unwrap();
              self.skip_whitespace(ch);
              return Some(Token::Op(OpValue::GreaterThan));
            }
          }

          self.skip_whitespace(ch);
          return Some(Token::Op(OpValue::Greater));
        }
        '<' => {
          if let Some(next) = self.iter.peek() {
            if *next == '=' {
              let ch = self.step().unwrap();
              self.skip_whitespace(ch);
              return Some(Token::Op(OpValue::LessThan));
            }
          }

          self.skip_whitespace(ch);
          return Some(Token::Op(OpValue::Less));
        }
        '!' => {
          if let Some(next) = self.iter.peek() {
            if *next == '=' {
              let ch = self.step().unwrap();
              self.skip_whitespace(ch);
              return Some(Token::Op(OpValue::NotEqual));
            }
          }

          return None;
        }
        '\'' | '"' => {
          let start_ch = ch;

          let start = self.i;
          self.step();

          while let Some(next) = self.iter.peek() {
            if *next == start_ch {
              break;
            }

            self.step();
          }

          let s = self.input[start..self.i].to_string();
          // TODO: Parse error

          self.step();
          return Some(Token::String(s));
        }

        _ => {
          let start = self.i - 1;

          while let Some(next) = self.iter.peek() {
            if !is_word_continue(&next) {
              break;
            }

            self.step();
          }

          let s = self.input[start..self.i].to_string();
          return Some(Token::Word(s));
        }
      }
    }

    None
  }
}

impl<'a> Iterator for Lexer<'a> {
  type Item = Token;

  fn next(&mut self) -> Option<Self::Item> {
    self.next_token()
  }
}

fn is_word_continue(ch: &char) -> bool {
  matches!( ch,
    '-' | '_' | 'a'..='z' | 'A'..='Z' | '0'..='9')
}

fn is_op_continue(ch: &char) -> bool {
  matches!(ch, '=' | '!' | '>' | '<' | '~' | '+')
}

fn is_whitespace(ch: &char) -> bool {
  matches!(ch, ' ' | '\t')
}

#[cfg(test)]
mod test {
  use super::{Lexer, OpValue, Token};

  fn test_lex(s: &str) -> Vec<Token> {
    let lex = Lexer::new(s);
    lex.into_iter().collect::<Vec<_>>()
  }

  #[test]
  fn lex_space() {
    let result = test_lex("foo bar");

    assert_eq!(
      result,
      vec![
        Token::Word("foo".to_string()),
        Token::Space,
        Token::Word("bar".to_string())
      ]
    );

    let result = test_lex("foo  bar");

    assert_eq!(
      result,
      vec![
        Token::Word("foo".to_string()),
        Token::Space,
        Token::Word("bar".to_string())
      ]
    );
  }

  #[test]
  fn lex_child() {
    let result = test_lex("foo>bar");

    assert_eq!(
      result,
      vec![
        Token::Word("foo".to_string()),
        Token::Op(OpValue::Greater),
        Token::Word("bar".to_string())
      ]
    );

    let result = test_lex("foo  >  bar");

    assert_eq!(
      result,
      vec![
        Token::Word("foo".to_string()),
        Token::Op(OpValue::Greater),
        Token::Word("bar".to_string())
      ]
    );
  }

  #[test]
  fn lex_attr() {
    let result = test_lex("foo[attr]");

    assert_eq!(
      result,
      vec![
        Token::Word("foo".to_string()),
        Token::BracketOpen,
        Token::Word("attr".to_string()),
        Token::BracketClose,
      ]
    );
  }

  #[test]
  fn lex_attr_value() {
    let result = test_lex("foo[attr='value']");

    assert_eq!(
      result,
      vec![
        Token::Word("foo".to_string()),
        Token::BracketOpen,
        Token::Word("attr".to_string()),
        Token::Op(OpValue::Equal),
        Token::String("value".to_string()),
        Token::BracketClose,
      ]
    );
  }

  #[test]
  fn lex_attr_pseudo() {
    let result = test_lex(":has-child(foo, bar)");

    assert_eq!(
      result,
      vec![
        Token::Colon,
        Token::Word("has-child".to_string()),
        Token::BraceOpen,
        Token::Word("foo".to_string()),
        Token::Comma,
        Token::Word("bar".to_string()),
        Token::BraceClose,
      ]
    );
  }
}
