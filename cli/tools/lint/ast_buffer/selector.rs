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
  GreaterEqual,
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
  Not(Vec<Selector>),
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
  Asteriks,
}

#[derive(Debug)]
enum ParseError {
  UnexpectedEnd(usize),
  UnexpectedToken,
  UnknownPseudo(String),
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

  fn peek(&mut self) -> Option<Token> {
    let i = self.i;
    let iter = self.iter.clone();

    let result = self.next_token();
    self.i = i;
    self.iter = iter;

    result
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
        '*' => {
          return Some(Token::Asteriks);
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

  fn expect_next(&mut self) -> Result<Token, ParseError> {
    match self.next_token() {
      Some(tk) => Ok(tk),
      None => Err(ParseError::UnexpectedEnd(self.i)),
    }
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

trait SelectorMatcher {
  fn matches(&mut self, offset: usize) -> bool;
}

struct Wildcard;
struct Elem(u8);

impl SelectorMatcher for Elem {
  fn matches(&mut self, offset: usize) -> bool {
    todo!()
  }
}

#[derive(Debug, PartialEq)]
struct Selector {
  parts: Vec<SelPart>,
}

impl Selector {
  fn new() -> Self {
    Self { parts: vec![] }
  }
}

impl SelectorMatcher for Selector {
  fn matches(&mut self, offset: usize) -> bool {
    self.parts.iter().all(|part| match part {
      SelPart::Wildcard => todo!(),
      SelPart::Elem(_) => todo!(),
      SelPart::Relation(relation_op) => todo!(),
      SelPart::AttrExists(vec) => todo!(),
      SelPart::AttrBin(attr_op, vec, _) => todo!(),
      SelPart::FirstChild => todo!(),
      SelPart::LastChild => todo!(),
      SelPart::NthChild => todo!(),
      SelPart::Not(vec) => todo!(),
    })
  }
}

trait SelStrMapper {
  fn str_to_prop(&self, s: &str) -> u8;
  fn str_to_type(&self, s: &str) -> u8;
}

fn parse_selector(
  input: &str,
  mapper: &impl SelStrMapper,
) -> Result<Vec<Selector>, ParseError> {
  let mut l = Lexer::new(input);

  let mut stack: Vec<Selector> = vec![Selector::new()];

  #[inline]
  fn append(stack: &mut Vec<Selector>, part: SelPart) {
    stack.last_mut().unwrap().parts.push(part);
  }

  #[inline]
  fn pop_selector(stack: &mut Vec<Selector>) {
    let last = stack.last_mut().unwrap();
  }

  // Some subselectors like `:nth-child(.. of <selector>)` must have
  // a single selector instead of selector list.
  let mut throw_on_comma = false;

  while let Some(tk) = l.next_token() {
    match tk {
      Token::Word(s) => {
        let kind = mapper.str_to_type(&s);
        append(&mut stack, SelPart::Elem(kind));
      }
      Token::Space => {
        if let Some(next) = l.peek() {
          if let Token::Word(_) | Token::Asteriks = next {
            append(&mut stack, SelPart::Relation(RelationOp::Space))
          }
        }
      }
      Token::Op(op_value) => {
        let op = match op_value {
          OpValue::Plus => RelationOp::Plus,
          OpValue::Tilde => RelationOp::Tilde,
          _ => return Err(ParseError::UnexpectedToken),
        };
        append(&mut stack, SelPart::Relation(op))
      }
      Token::Colon => {
        let Token::Word(word) = l.expect_next()? else {
          return Err(ParseError::UnexpectedToken);
        };

        match word.as_str() {
          "first-child" => append(&mut stack, SelPart::FirstChild),
          "last-child" => append(&mut stack, SelPart::LastChild),
          "nth-child" => {
            //
            todo!()
          }
          "has" | "is" | "where" => {
            //
            todo!()
          }
          "not" => {
            if Token::BraceOpen != l.expect_next()? {
              return Err(ParseError::UnexpectedToken);
            };

            append(&mut stack, SelPart::Not(vec![]));
            stack.push(Selector::new());
          }
          s => return Err(ParseError::UnknownPseudo(s.to_string())),
        }
      }
      Token::Comma => {
        if throw_on_comma {
          return Err(ParseError::UnexpectedToken);
        }

        // TODO Consume space

        pop_selector(&mut stack);
        stack.push(Selector::new())
      }
      Token::BraceOpen => todo!(),
      Token::BraceClose => {
        // TODO(@marvinhagemeister) Nested pseudos?
        throw_on_comma = false;
        pop_selector(&mut stack);
        stack.push(Selector::new())
      }
      Token::BracketOpen => {
        let Token::Word(word) = l.expect_next()? else {
          return Err(ParseError::UnexpectedToken);
        };

        let name_path: Vec<u8> = vec![mapper.str_to_prop(word.as_str())];

        // TODO dot

        let next = l.expect_next()?;

        match next {
          Token::Op(op) => {
            // TODO
            let value = "".to_string();
            let attr_op = match op {
              OpValue::Equal => AttrOp::Equal,
              OpValue::NotEqual => AttrOp::NotEqual,
              OpValue::Greater => AttrOp::Greater,
              OpValue::GreaterThan => AttrOp::GreaterEqual,
              OpValue::Less => AttrOp::Less,
              OpValue::LessThan => AttrOp::LessEqual,
              OpValue::Plus | OpValue::Tilde => {
                return Err(ParseError::UnexpectedToken)
              }
            };
            append(&mut stack, SelPart::AttrBin(attr_op, name_path, value))
          }
          Token::BracketClose => return Err(ParseError::UnexpectedToken),
          _ => append(&mut stack, SelPart::AttrExists(name_path)),
        }

        let Token::BracketClose = l.expect_next()? else {
          return Err(ParseError::UnexpectedToken);
        };
      }
      Token::BracketClose => todo!(),
      Token::String(_) => todo!(),
      Token::Number => todo!(),
      Token::Bool => todo!(),
      Token::Null => todo!(),
      Token::Undefined => todo!(),
      Token::Dot => todo!(),
      Token::Minus => todo!(),
      Token::Asteriks => {
        append(&mut stack, SelPart::Wildcard);
      }
    }
  }

  Ok(stack)
}

#[cfg(test)]
mod test {

  use crate::tools::lint::ast_buffer::selector::{
    RelationOp, SelPart, Selector,
  };

  use super::{
    parse_selector, Lexer, OpValue, ParseError, SelStrMapper, Token,
  };

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

  struct TestMapper;
  impl SelStrMapper for TestMapper {
    fn str_to_prop(&self, s: &str) -> u8 {
      match s {
        "foo" => 1,
        "bar" => 2,
        "baz" => 3,
        _ => 0, // TODO
      }
    }

    fn str_to_type(&self, s: &str) -> u8 {
      match s {
        "Foo" => 1,
        "Bar" => 2,
        "Baz" => 3,
        _ => 0, // TODO
      }
    }
  }

  #[test]
  fn parse_elem() -> Result<(), ParseError> {
    let mapper = TestMapper {};
    let result = parse_selector("Foo", &mapper)?;

    let mut s = Selector::new();
    s.parts.push(SelPart::Elem(mapper.str_to_type("Foo")));
    assert_eq!(result, vec![s]);

    Ok(())
  }

  #[test]
  fn parse_space_relation() -> Result<(), ParseError> {
    let mapper = TestMapper {};
    let result = parse_selector("Foo Bar", &mapper)?;

    let mut s = Selector::new();
    s.parts.push(SelPart::Elem(mapper.str_to_type("Foo")));
    s.parts.push(SelPart::Relation(RelationOp::Space));
    s.parts.push(SelPart::Elem(mapper.str_to_type("Bar")));
    assert_eq!(result, vec![s]);

    Ok(())
  }
}
