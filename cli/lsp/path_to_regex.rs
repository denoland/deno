// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// The logic of this module is heavily influenced by path-to-regexp at:
// https://github.com/pillarjs/path-to-regexp/ which is licensed as follows:

// The MIT License (MIT)
//
// Copyright (c) 2014 Blake Embrey (hello@blakeembrey.com)
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
//

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use std::collections::HashMap;
use std::fmt;
use std::iter::Peekable;

lazy_static::lazy_static! {
  static ref ESCAPE_STRING_RE: Regex =
    Regex::new(r"([.+*?=^!:${}()\[\]|/\\])").unwrap();
}

#[derive(Debug, PartialEq, Eq)]
enum TokenType {
  Open,
  Close,
  Pattern,
  Name,
  Char,
  EscapedChar,
  Modifier,
  End,
}

#[derive(Debug)]
struct LexToken {
  token_type: TokenType,
  index: usize,
  value: String,
}

fn escape_string(s: &str) -> String {
  ESCAPE_STRING_RE.replace_all(s, r"\$1").to_string()
}

fn lexer(s: &str) -> Result<Vec<LexToken>, AnyError> {
  let mut tokens = Vec::new();
  let mut chars = s.chars().peekable();
  let mut index = 0_usize;

  loop {
    match chars.next() {
      None => break,
      Some(c) if c == '*' || c == '+' || c == '?' => {
        tokens.push(LexToken {
          token_type: TokenType::Modifier,
          index,
          value: c.to_string(),
        });
        index += 1;
      }
      Some('\\') => {
        index += 1;
        let value = chars
          .next()
          .ok_or_else(|| anyhow!("Unexpected end of string at {}.", index))?;
        tokens.push(LexToken {
          token_type: TokenType::EscapedChar,
          index,
          value: value.to_string(),
        });
        index += 1;
      }
      Some('{') => {
        tokens.push(LexToken {
          token_type: TokenType::Open,
          index,
          value: '{'.to_string(),
        });
        index += 1;
      }
      Some('}') => {
        tokens.push(LexToken {
          token_type: TokenType::Close,
          index,
          value: '}'.to_string(),
        });
        index += 1;
      }
      Some(':') => {
        let mut name = String::new();
        while let Some(c) = chars.peek() {
          if (*c >= '0' && *c <= '9')
            || (*c >= 'A' && *c <= 'Z')
            || (*c >= 'a' && *c <= 'z')
            || *c == '_'
          {
            let ch = chars.next().unwrap();
            name.push(ch);
          } else {
            break;
          }
        }
        if name.is_empty() {
          return Err(anyhow!("Missing parameter name at {}", index));
        }
        let name_len = name.len();
        tokens.push(LexToken {
          token_type: TokenType::Name,
          index,
          value: name,
        });
        index += 1 + name_len;
      }
      Some('(') => {
        let mut count = 1;
        let mut pattern = String::new();

        if chars.peek() == Some(&'?') {
          return Err(anyhow!(
            "Pattern cannot start with \"?\" at {}.",
            index + 1
          ));
        }

        loop {
          let next_char = chars.peek();
          if next_char.is_none() {
            break;
          }
          if next_char == Some(&'\\') {
            pattern.push(chars.next().unwrap());
            pattern.push(
              chars
                .next()
                .ok_or_else(|| anyhow!("Unexpected termination of string."))?,
            );
            continue;
          }
          if next_char == Some(&')') {
            count -= 1;
            if count == 0 {
              chars.next();
              break;
            }
          } else if next_char == Some(&'(') {
            count += 1;
            pattern.push(chars.next().unwrap());
            if chars.peek() != Some(&'?') {
              return Err(anyhow!(
                "Capturing groups are not allowed at {}.",
                index + pattern.len()
              ));
            }
            continue;
          }

          pattern.push(chars.next().unwrap());
        }

        if count > 0 {
          return Err(anyhow!("Unbalanced pattern at {}.", index));
        }
        if pattern.is_empty() {
          return Err(anyhow!("Missing pattern at {}.", index));
        }
        let pattern_len = pattern.len();
        tokens.push(LexToken {
          token_type: TokenType::Pattern,
          index,
          value: pattern,
        });
        index += 2 + pattern_len;
      }
      Some(c) => {
        tokens.push(LexToken {
          token_type: TokenType::Char,
          index,
          value: c.to_string(),
        });
        index += 1;
      }
    }
  }

  tokens.push(LexToken {
    token_type: TokenType::End,
    index,
    value: "".to_string(),
  });

  Ok(tokens)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StringOrNumber {
  String(String),
  Number(usize),
}

impl fmt::Display for StringOrNumber {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match &self {
      Self::Number(n) => write!(f, "{}", n),
      Self::String(s) => write!(f, "{}", s),
    }
  }
}

#[derive(Debug, Clone)]
pub enum StringOrVec {
  String(String),
  Vec(Vec<String>),
}

impl StringOrVec {
  pub fn from_str(s: &str, key: &Key) -> StringOrVec {
    match &key.modifier {
      Some(m) if m == "+" || m == "*" => {
        let pat = format!(
          "{}{}",
          key.prefix.clone().unwrap_or_default(),
          key.suffix.clone().unwrap_or_default()
        );
        s.split(&pat)
          .map(String::from)
          .collect::<Vec<String>>()
          .into()
      }
      _ => s.into(),
    }
  }

  pub fn to_string(&self, maybe_key: Option<&Key>) -> String {
    match self {
      Self::String(s) => s.clone(),
      Self::Vec(v) => {
        let (prefix, suffix) = if let Some(key) = maybe_key {
          (
            key.prefix.clone().unwrap_or_default(),
            key.suffix.clone().unwrap_or_default(),
          )
        } else {
          ("/".to_string(), "".to_string())
        };
        let mut s = String::new();
        for segment in v {
          s.push_str(&format!("{}{}{}", prefix, segment, suffix));
        }
        s
      }
    }
  }
}

impl Default for StringOrVec {
  fn default() -> Self {
    Self::String("".to_string())
  }
}

impl<'a> From<&'a str> for StringOrVec {
  fn from(s: &'a str) -> Self {
    Self::String(s.to_string())
  }
}

impl From<Vec<String>> for StringOrVec {
  fn from(v: Vec<String>) -> Self {
    Self::Vec(v)
  }
}

#[derive(Debug, Clone)]
pub struct Key {
  pub name: StringOrNumber,
  pub prefix: Option<String>,
  pub suffix: Option<String>,
  pub pattern: String,
  pub modifier: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Token {
  String(String),
  Key(Key),
}

#[derive(Debug, Default)]
pub struct ParseOptions {
  delimiter: Option<String>,
  prefixes: Option<String>,
}

#[derive(Debug)]
pub struct TokensToCompilerOptions {
  sensitive: bool,
  validate: bool,
}

impl Default for TokensToCompilerOptions {
  fn default() -> Self {
    Self {
      sensitive: false,
      validate: true,
    }
  }
}

#[derive(Debug)]
pub struct TokensToRegexOptions {
  sensitive: bool,
  strict: bool,
  end: bool,
  start: bool,
  delimiter: Option<String>,
  ends_with: Option<String>,
}

impl Default for TokensToRegexOptions {
  fn default() -> Self {
    Self {
      sensitive: false,
      strict: false,
      end: true,
      start: true,
      delimiter: None,
      ends_with: None,
    }
  }
}

#[derive(Debug, Default)]
pub struct PathToRegexOptions {
  parse_options: Option<ParseOptions>,
  token_to_regex_options: Option<TokensToRegexOptions>,
}

fn try_consume(
  token_type: &TokenType,
  it: &mut Peekable<impl Iterator<Item = LexToken>>,
) -> Option<String> {
  if let Some(token) = it.peek() {
    if &token.token_type == token_type {
      let token = it.next().unwrap();
      return Some(token.value);
    }
  }
  None
}

fn must_consume(
  token_type: &TokenType,
  it: &mut Peekable<impl Iterator<Item = LexToken>>,
) -> Result<String, AnyError> {
  try_consume(token_type, it).ok_or_else(|| {
    let maybe_token = it.next();
    if let Some(token) = maybe_token {
      anyhow!(
        "Unexpected {:?} at {}, expected {:?}",
        token.token_type,
        token.index,
        token_type
      )
    } else {
      anyhow!("Unexpected end of tokens, expected {:?}", token_type)
    }
  })
}

fn consume_text(
  it: &mut Peekable<impl Iterator<Item = LexToken>>,
) -> Option<String> {
  let mut result = String::new();
  loop {
    if let Some(value) = try_consume(&TokenType::Char, it) {
      result.push_str(&value);
    }
    if let Some(value) = try_consume(&TokenType::EscapedChar, it) {
      result.push_str(&value);
    } else {
      break;
    }
  }
  if result.is_empty() {
    None
  } else {
    Some(result)
  }
}

pub fn parse(
  s: &str,
  maybe_options: Option<ParseOptions>,
) -> Result<Vec<Token>, AnyError> {
  let mut tokens = lexer(s)?.into_iter().peekable();
  let options = maybe_options.unwrap_or_default();
  let prefixes = options.prefixes.unwrap_or_else(|| "./".to_string());
  let default_pattern = if let Some(delimiter) = options.delimiter {
    format!("[^{}]+?", escape_string(&delimiter))
  } else {
    "[^/#?]+?".to_string()
  };
  let mut result = Vec::new();
  let mut key = 0_usize;
  let mut path = String::new();

  loop {
    let char = try_consume(&TokenType::Char, &mut tokens);
    let name = try_consume(&TokenType::Name, &mut tokens);
    let pattern = try_consume(&TokenType::Pattern, &mut tokens);

    if name.is_some() || pattern.is_some() {
      let mut prefix = char.unwrap_or_default();
      if !prefixes.contains(&prefix) {
        path.push_str(&prefix);
        prefix = String::new();
      }

      if !path.is_empty() {
        result.push(Token::String(path.clone()));
        path = String::new();
      }

      let name = name.map_or_else(
        || {
          let default = StringOrNumber::Number(key);
          key += 1;
          default
        },
        StringOrNumber::String,
      );
      let prefix = if prefix.is_empty() {
        None
      } else {
        Some(prefix)
      };
      result.push(Token::Key(Key {
        name,
        prefix,
        suffix: None,
        pattern: pattern.unwrap_or_else(|| default_pattern.clone()),
        modifier: try_consume(&TokenType::Modifier, &mut tokens),
      }));
      continue;
    }

    if let Some(value) = char {
      path.push_str(&value);
      continue;
    } else if let Some(value) =
      try_consume(&TokenType::EscapedChar, &mut tokens)
    {
      path.push_str(&value);
      continue;
    }

    if !path.is_empty() {
      result.push(Token::String(path.clone()));
      path = String::new();
    }

    if try_consume(&TokenType::Open, &mut tokens).is_some() {
      let prefix = consume_text(&mut tokens);
      let maybe_name = try_consume(&TokenType::Name, &mut tokens);
      let maybe_pattern = try_consume(&TokenType::Pattern, &mut tokens);
      let suffix = consume_text(&mut tokens);

      must_consume(&TokenType::Close, &mut tokens)?;

      let name = maybe_name.clone().map_or_else(
        || {
          if maybe_pattern.is_some() {
            let default = StringOrNumber::Number(key);
            key += 1;
            default
          } else {
            StringOrNumber::String("".to_string())
          }
        },
        StringOrNumber::String,
      );
      let pattern = if maybe_name.is_some() && maybe_pattern.is_none() {
        default_pattern.clone()
      } else {
        maybe_pattern.unwrap_or_default()
      };
      result.push(Token::Key(Key {
        name,
        prefix,
        pattern,
        suffix,
        modifier: try_consume(&TokenType::Modifier, &mut tokens),
      }));
      continue;
    }

    must_consume(&TokenType::End, &mut tokens)?;
    break;
  }

  Ok(result)
}

/// Transform a vector of tokens into a regular expression, returning the
/// regular expression and optionally any keys that can be matched as part of
/// the expression.
pub fn tokens_to_regex(
  tokens: &[Token],
  maybe_options: Option<TokensToRegexOptions>,
) -> Result<(FancyRegex, Option<Vec<Key>>), AnyError> {
  let TokensToRegexOptions {
    sensitive,
    strict,
    end,
    start,
    delimiter,
    ends_with,
  } = maybe_options.unwrap_or_default();
  let has_ends_with = ends_with.is_some();
  let ends_with = format!(r"[{}]|$", ends_with.unwrap_or_default());
  let delimiter =
    format!(r"[{}]", delimiter.unwrap_or_else(|| "/#?".to_string()));
  let mut route = if start {
    "^".to_string()
  } else {
    String::new()
  };
  let maybe_end_token = tokens.iter().last().cloned();
  let mut keys: Vec<Key> = Vec::new();

  for token in tokens {
    let value = match token {
      Token::String(s) => s.to_string(),
      Token::Key(key) => {
        if !key.pattern.is_empty() {
          keys.push(key.clone());
        }

        let prefix = key
          .prefix
          .clone()
          .map_or_else(|| "".to_string(), |s| escape_string(&s));
        let suffix = key
          .suffix
          .clone()
          .map_or_else(|| "".to_string(), |s| escape_string(&s));

        if !key.pattern.is_empty() {
          if !prefix.is_empty() || !suffix.is_empty() {
            match &key.modifier {
              Some(s) if s == "+" || s == "*" => {
                let modifier = if key.modifier == Some("*".to_string()) {
                  "?"
                } else {
                  ""
                };
                format!(
                  "(?:{}((?:{})(?:{}{}(?:{}))*){}){}",
                  prefix,
                  key.pattern,
                  suffix,
                  prefix,
                  key.pattern,
                  suffix,
                  modifier
                )
              }
              _ => {
                let modifier = key.modifier.clone().unwrap_or_default();
                format!(
                  r"(?:{}({}){}){}",
                  prefix, key.pattern, suffix, modifier
                )
              }
            }
          } else {
            let modifier = key.modifier.clone().unwrap_or_default();
            format!(r"({}){}", key.pattern, modifier)
          }
        } else {
          let modifier = key.modifier.clone().unwrap_or_default();
          format!(r"(?:{}{}){}", prefix, suffix, modifier)
        }
      }
    };
    route.push_str(&value);
  }

  if end {
    if !strict {
      route.push_str(&format!(r"{}?", delimiter));
    }
    if has_ends_with {
      route.push_str(&format!(r"(?={})", ends_with));
    } else {
      route.push('$');
    }
  } else {
    let is_end_deliminated = match maybe_end_token {
      Some(Token::String(mut s)) => {
        if let Some(c) = s.pop() {
          delimiter.contains(c)
        } else {
          false
        }
      }
      Some(_) => false,
      None => true,
    };

    if !strict {
      route.push_str(&format!(r"(?:{}(?={}))?", delimiter, ends_with));
    }

    if !is_end_deliminated {
      route.push_str(&format!(r"(?={}|{})", delimiter, ends_with));
    }
  }

  let flags = if sensitive { "" } else { "(?i)" };
  let re = FancyRegex::new(&format!("{}{}", flags, route))?;
  let maybe_keys = if keys.is_empty() { None } else { Some(keys) };

  Ok((re, maybe_keys))
}

/// Convert a path-like string into a regular expression, returning the regular
/// expression and optionally any keys that can be matched in the string.
pub fn string_to_regex(
  path: &str,
  maybe_options: Option<PathToRegexOptions>,
) -> Result<(FancyRegex, Option<Vec<Key>>), AnyError> {
  let (parse_options, tokens_to_regex_options) =
    if let Some(options) = maybe_options {
      (options.parse_options, options.token_to_regex_options)
    } else {
      (None, None)
    };
  tokens_to_regex(&parse(path, parse_options)?, tokens_to_regex_options)
}

pub struct Compiler {
  matches: Vec<Option<Regex>>,
  tokens: Vec<Token>,
  validate: bool,
}

impl Compiler {
  pub fn new(
    tokens: &[Token],
    maybe_options: Option<TokensToCompilerOptions>,
  ) -> Self {
    let TokensToCompilerOptions {
      sensitive,
      validate,
    } = maybe_options.unwrap_or_default();
    let flags = if sensitive { "" } else { "(?i)" };

    let matches = tokens
      .iter()
      .map(|t| {
        if let Token::Key(k) = t {
          Some(Regex::new(&format!("{}^(?:{})$", flags, k.pattern)).unwrap())
        } else {
          None
        }
      })
      .collect();

    Self {
      matches,
      tokens: tokens.to_vec(),
      validate,
    }
  }

  pub fn to_path(
    &self,
    params: &HashMap<StringOrNumber, StringOrVec>,
  ) -> Result<String, AnyError> {
    let mut path = String::new();

    for (i, token) in self.tokens.iter().enumerate() {
      match token {
        Token::String(s) => path.push_str(s),
        Token::Key(k) => {
          let value = params.get(&k.name);
          let optional = k.modifier == Some("?".to_string())
            || k.modifier == Some("*".to_string());
          let repeat = k.modifier == Some("*".to_string())
            || k.modifier == Some("+".to_string());

          match value {
            Some(StringOrVec::Vec(v)) => {
              if !repeat {
                return Err(anyhow!(
                  "Expected \"{:?}\" to not repeat, but got a vector",
                  k.name
                ));
              }

              if v.is_empty() {
                if !optional {
                  return Err(anyhow!(
                    "Expected \"{:?}\" to not be empty.",
                    k.name
                  ));
                }
              } else {
                let prefix = k.prefix.clone().unwrap_or_default();
                let suffix = k.suffix.clone().unwrap_or_default();
                for segment in v {
                  if self.validate {
                    if let Some(re) = &self.matches[i] {
                      if !re.is_match(segment) {
                        return Err(anyhow!(
                          "Expected all \"{:?}\" to match \"{}\", but got {}",
                          k.name,
                          k.pattern,
                          segment
                        ));
                      }
                    }
                  }
                  path.push_str(&format!("{}{}{}", prefix, segment, suffix));
                }
              }
            }
            Some(StringOrVec::String(s)) => {
              if self.validate {
                if let Some(re) = &self.matches[i] {
                  if !re.is_match(s) {
                    return Err(anyhow!(
                      "Expected \"{:?}\" to match \"{}\", but got \"{}\"",
                      k.name,
                      k.pattern,
                      s
                    ));
                  }
                }
              }
              let prefix = k.prefix.clone().unwrap_or_default();
              let suffix = k.suffix.clone().unwrap_or_default();
              path.push_str(&format!("{}{}{}", prefix, s, suffix));
            }
            None => {
              if !optional {
                let key_type = if repeat { "an array" } else { "a string" };
                return Err(anyhow!(
                  "Expected \"{:?}\" to be {}",
                  k.name,
                  key_type
                ));
              }
            }
          }
        }
      }
    }

    Ok(path)
  }
}

#[derive(Debug)]
pub struct MatchResult {
  pub path: String,
  pub index: usize,
  pub params: HashMap<StringOrNumber, StringOrVec>,
}

impl MatchResult {
  pub fn get(&self, key: &str) -> Option<&StringOrVec> {
    self.params.get(&StringOrNumber::String(key.to_string()))
  }
}

#[derive(Debug)]
pub struct Matcher {
  maybe_keys: Option<Vec<Key>>,
  re: FancyRegex,
}

impl Matcher {
  pub fn new(
    tokens: &[Token],
    maybe_options: Option<TokensToRegexOptions>,
  ) -> Result<Self, AnyError> {
    let (re, maybe_keys) = tokens_to_regex(tokens, maybe_options)?;
    Ok(Self { maybe_keys, re })
  }

  pub fn matches(&self, path: &str) -> Option<MatchResult> {
    let caps = self.re.captures(path).ok()??;
    let m = caps.get(0)?;
    let path = m.as_str().to_string();
    let index = m.start();
    let mut params = HashMap::new();
    if let Some(keys) = &self.maybe_keys {
      for (i, key) in keys.iter().enumerate() {
        if let Some(m) = caps.get(i + 1) {
          let value = if key.modifier == Some("*".to_string())
            || key.modifier == Some("+".to_string())
          {
            let pat = format!(
              "{}{}",
              key.prefix.clone().unwrap_or_default(),
              key.suffix.clone().unwrap_or_default()
            );
            m.as_str()
              .split(&pat)
              .map(String::from)
              .collect::<Vec<String>>()
              .into()
          } else {
            m.as_str().into()
          };
          params.insert(key.name.clone(), value);
        }
      }
    }

    Some(MatchResult {
      path,
      index,
      params,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  type FixtureMatch<'a> = (&'a str, usize, usize);
  type Fixture<'a> = (&'a str, Option<FixtureMatch<'a>>);

  fn test_path(
    path: &str,
    maybe_options: Option<PathToRegexOptions>,
    fixtures: &[Fixture],
  ) {
    let result = string_to_regex(path, maybe_options);
    assert!(result.is_ok(), "Could not parse path: \"{}\"", path);
    let (re, _) = result.unwrap();
    for (fixture, expected) in fixtures {
      let result = re.find(*fixture);
      assert!(
        result.is_ok(),
        "Find failure for path \"{}\" and fixture \"{}\"",
        path,
        fixture
      );
      let actual = result.unwrap();
      if let Some((text, start, end)) = *expected {
        assert!(actual.is_some(), "Match failure for path \"{}\" and fixture \"{}\". Expected Some got None", path, fixture);
        let actual = actual.unwrap();
        assert_eq!(actual.as_str(), text, "Match failure for path \"{}\" and fixture \"{}\".  Expected \"{}\" got \"{}\".", path, fixture, text, actual.as_str());
        assert_eq!(actual.start(), start);
        assert_eq!(actual.end(), end);
      } else {
        assert!(actual.is_none(), "Match failure for path \"{}\" and fixture \"{}\". Expected None got {:?}", path, fixture, actual);
      }
    }
  }

  #[test]
  fn test_compiler() {
    let tokens = parse("/x/:a@:b/:c*", None).expect("could not parse");
    let mut params = HashMap::<StringOrNumber, StringOrVec>::new();
    params.insert(
      StringOrNumber::String("a".to_string()),
      StringOrVec::String("y".to_string()),
    );
    params.insert(
      StringOrNumber::String("b".to_string()),
      StringOrVec::String("v1.0.0".to_string()),
    );
    params.insert(
      StringOrNumber::String("c".to_string()),
      StringOrVec::Vec(vec!["z".to_string(), "example.ts".to_string()]),
    );
    let compiler = Compiler::new(&tokens, None);
    let actual = compiler.to_path(&params);
    println!("{:?}", actual);
    assert!(actual.is_ok());
    let actual = actual.unwrap();
    assert_eq!(actual, "/x/y@v1.0.0/z/example.ts".to_string());
  }

  #[test]
  fn test_string_to_regex() {
    test_path("/", None, &[("/test", None), ("/", Some(("/", 0, 1)))]);
    test_path(
      "/test",
      None,
      &[
        ("/test", Some(("/test", 0, 5))),
        ("/route", None),
        ("/test/route", None),
        ("/test/", Some(("/test/", 0, 6))),
      ],
    );
    test_path(
      "/test/",
      None,
      &[
        ("/test", None),
        ("/test/", Some(("/test/", 0, 6))),
        ("/test//", Some(("/test//", 0, 7))),
      ],
    );
    // case-sensitive paths
    test_path(
      "/test",
      Some(PathToRegexOptions {
        parse_options: None,
        token_to_regex_options: Some(TokensToRegexOptions {
          sensitive: true,
          ..Default::default()
        }),
      }),
      &[("/test", Some(("/test", 0, 5))), ("/TEST", None)],
    );
    test_path(
      "/TEST",
      Some(PathToRegexOptions {
        parse_options: None,
        token_to_regex_options: Some(TokensToRegexOptions {
          sensitive: true,
          ..Default::default()
        }),
      }),
      &[("/TEST", Some(("/TEST", 0, 5))), ("/test", None)],
    );
  }
}
