use std::str::{CharIndices, Chars};

enum RelationOp {
  Space,
  Plus,
  Tilde,
}

enum AttrOp {
  Equal,
  NotEqual,
  Greater,
  GreaterEqaul,
  Less,
  LessEqual,
}

enum AttrValue {
  True,
  False,
  Null,
  Undefined,
  Str(String),
  Num,
  Regex(String),
}

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

type Selector = Vec<SelPart>;

enum Token {
  Eof,
  Word,
  Space,
  Op,
  Colon,
  Comma,
  BraceOpen,
  BraceClose,
  BracketOpen,
  BracketClose,
  String,
  Number,
  Bool,
  Null,
  Undefined,
  Dot,
  Minus,
}

struct Lexer<'a> {
  i: usize,
  input: String,
  len: usize,
  iter: CharIndices<'a>,
  token: Token,
  start: usize,
  end: usize,
}

impl<'a> Lexer<'a> {
  fn next(&mut self) {
    while let Some((i, ch)) = self.iter.next() {
      match ch {
        ' ' => {
          while is_whitespace(&ch) {
            if let Some((j, next_ch)) = self.iter.next() {}
          }

          if is_op_continue(&ch) {
            continue;
          }

          self.token = Token::Space;
          return;
        }
        '[' => {
          self.token = Token::BracketOpen;
          self.iter.next();
          return;
        }
        ']' => {
          self.token = Token::BracketClose;
          self.iter.next();
          return;
        }
        '(' => {
          self.token = Token::BraceOpen;
          self.iter.next();
          return;
        }
        ')' => {
          self.token = Token::BraceClose;
          self.iter.next();
          return;
        }
        ',' => {
          self.token = Token::Comma;
          self.iter.next();
          return;
        }
        '.' => {
          self.token = Token::Dot;
          self.iter.next();
          return;
        }
        '_' => {
          self.token = Token::Minus;
          self.iter.next();
          return;
        }
        '+' | '~' | '>' | '<' | '=' | '!' => {
          self.token = Token::Op;
          self.start = i;

          self.iter.next();
          //
        }
        _ => {
          // TODO
        }
      }
    }

    self.token = Token::Eof;
  }
}

fn is_word_continue(ch: &char) -> bool {
  match ch {
    '-' | '_' | 'a'..='z' | 'A'..='Z' | '0'..='9' => true,
    _ => false,
  }
}

fn is_op_continue(ch: &char) -> bool {
  match ch {
    '=' | '!' | '>' | '<' | '~' | '+' => true,
    _ => false,
  }
}

fn is_whitespace(ch: &char) -> bool {
  match ch {
    ' ' | '\t' => true,
    _ => false,
  }
}
