// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::bail;
use deno_core::error::AnyError;

use super::combinators::assert;
use super::combinators::assert_exists;
use super::combinators::char;
use super::combinators::delimited;
use super::combinators::many0;
use super::combinators::many_till;
use super::combinators::map;
use super::combinators::maybe;
use super::combinators::or;
use super::combinators::skip_whitespace;
use super::combinators::tag;
use super::combinators::take_while;
use super::combinators::terminated;
use super::combinators::with_error_context;
use super::combinators::ParseError;
use super::combinators::ParseResult;

// Shell grammar rules this is loosely based on:
// https://pubs.opengroup.org/onlinepubs/009604499/utilities/xcu_chap02.html#tag_02_10_02

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
  Command(ShellCommand),
  BinExpr(BinExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinExpr {
  pub left: Box<Expr>,
  pub op: Operator,
  pub right: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellCommand {
  pub env_vars: Vec<EnvVar>,
  pub args: Vec<String>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Operator {
  // &&
  AndAnd,
  // &
  And,
  // ||
  OrOr,
  // |
  Or,
}

impl Operator {
  pub fn as_str(&self) -> &'static str {
    match self {
      Operator::AndAnd => "&&",
      Operator::And => "&",
      Operator::OrOr => "||",
      Operator::Or => "|",
    }
  }
}

#[derive(Debug, PartialEq, Clone)]
pub struct EnvVar {
  pub name: String,
  pub value: String,
}

impl EnvVar {
  pub fn new(name: String, value: String) -> Self {
    EnvVar { name, value }
  }
}

/// Note: Only used to detect redirects in order to give a better error.
/// Redirects are not part of the first pass of this feature.
pub struct Redirect {
  pub maybe_fd: Option<usize>,
  pub op: RedirectOp,
  pub word: Option<String>,
}

pub enum RedirectOp {
  /// >
  Redirect,
  /// >>
  Append,
}

pub fn parse(input: &str) -> Result<Expr, AnyError> {
  fn error_for_input(message: &str, input: &str) -> Result<Expr, AnyError> {
    bail!(
      "{}\n  {}\n  ~",
      message,
      input.chars().take(60).collect::<String>()
    )
  }

  match parse_expr(input) {
    Ok((_, expr)) => Ok(expr),
    Err(ParseError::Backtrace) => {
      if input.trim().is_empty() {
        bail!("Empty command.")
      } else {
        error_for_input("Invalid character for command.", input)
      }
    }
    Err(ParseError::Failure(e)) => error_for_input(&e.message, e.input),
  }
}

fn parse_expr(input: &str) -> ParseResult<Expr> {
  let (input, command) = parse_command(input)?;
  if parse_redirect(input).is_ok() {
    return ParseError::fail(input, "Redirects are currently not supported.");
  }
  match parse_operator(input) {
    Ok((input, op)) => {
      let (input, right_command) = assert_exists(
        parse_expr,
        "Expected command following operator.",
      )(input)?;
      Ok((
        input,
        Expr::BinExpr(BinExpr {
          left: Box::new(Expr::Command(command)),
          op,
          right: Box::new(right_command),
        }),
      ))
    }
    Err(ParseError::Backtrace) => Ok((input, Expr::Command(command))),
    Err(ParseError::Failure(err)) => Err(ParseError::Failure(err)),
  }
}

fn parse_command(input: &str) -> ParseResult<ShellCommand> {
  let (input, env_vars) = parse_env_vars(input)?;
  let (input, args) = parse_command_args(input)?;
  if args.is_empty() {
    ParseError::backtrace()
  } else {
    Ok((input, ShellCommand { env_vars, args }))
  }
}

fn parse_command_args(input: &str) -> ParseResult<Vec<String>> {
  many_till(
    terminated(parse_shell_arg, assert_whitespace_or_end_and_skip),
    or(map(parse_operator, |_| ()), map(parse_redirect, |_| ())),
  )(input)
}

fn parse_shell_arg(input: &str) -> ParseResult<String> {
  or(parse_quoted_string, map(parse_word, ToString::to_string))(input)
}

fn parse_operator(input: &str) -> ParseResult<Operator> {
  fn operator_kind<'a>(
    operator: Operator,
  ) -> impl Fn(&'a str) -> ParseResult<'a, Operator> {
    map(tag(operator.as_str()), move |_| operator)
  }

  terminated(
    or(
      operator_kind(Operator::AndAnd),
      or(
        operator_kind(Operator::And),
        or(operator_kind(Operator::OrOr), operator_kind(Operator::Or)),
      ),
    ),
    skip_whitespace,
  )(input)
}

fn parse_redirect(input: &str) -> ParseResult<Redirect> {
  // https://pubs.opengroup.org/onlinepubs/009604499/utilities/xcu_chap02.html#tag_02_07
  let (input, maybe_fd) = maybe(parse_usize)(input)?;
  let (input, op) = or(
    map(or(tag(">"), tag(">|")), |_| RedirectOp::Redirect),
    map(tag(">>"), |_| RedirectOp::Append),
  )(input)?;
  let (input, word) = maybe(parse_word)(input)?;

  Ok((
    input,
    Redirect {
      maybe_fd,
      op,
      word: word.map(ToString::to_string),
    },
  ))
}

fn parse_env_vars(input: &str) -> ParseResult<Vec<EnvVar>> {
  many0(terminated(parse_env_var, skip_whitespace))(input)
}

fn parse_env_var(input: &str) -> ParseResult<EnvVar> {
  let (input, name) = parse_env_var_name(input)?;
  let (input, _) = char('=')(input)?;
  let (input, value) = with_error_context(
    terminated(parse_env_var_value, assert_whitespace_or_end),
    concat!(
      "Environment variable values may only be assigned quoted strings ",
      "or a plain string (consisting only of letters, numbers, and underscores)",
    ),
  )(input)?;
  Ok((input, EnvVar::new(name.to_string(), value)))
}

fn parse_env_var_name(input: &str) -> ParseResult<&str> {
  // [a-zA-Z0-9_]+
  take_while(|c: char| c.is_ascii_alphanumeric() || c == '_')(input)
}

fn parse_env_var_value(input: &str) -> ParseResult<String> {
  or(parse_quoted_string, map(parse_word, |v| v.to_string()))(input)
}

fn parse_word(input: &str) -> ParseResult<&str> {
  assert(
    take_while(|c| !is_special_char(c)),
    |result| {
      result
        .ok()
        .map(|(_, text)| !is_reserved_word(text))
        .unwrap_or(true)
    },
    "Unsupported reserved word.",
  )(input)
}

fn parse_quoted_string(input: &str) -> ParseResult<String> {
  or(
    map(parse_single_quoted_string, ToString::to_string),
    parse_double_quoted_string,
  )(input)
}

fn parse_single_quoted_string(input: &str) -> ParseResult<&str> {
  // single quoted strings cannot contain a single quote
  // https://pubs.opengroup.org/onlinepubs/009604499/utilities/xcu_chap02.html#tag_02_02_02
  delimited(
    char('\''),
    take_while(|c| c != '\''),
    assert_exists(char('\''), "Expected closing single quote."),
  )(input)
}

fn parse_double_quoted_string(input: &str) -> ParseResult<String> {
  // https://pubs.opengroup.org/onlinepubs/009604499/utilities/xcu_chap02.html#tag_02_02_03
  // Double quotes may have escaped
  delimited(
    char('"'),
    |input| {
      // this doesn't seem like the nom way of doing things, but it was a
      // quick implementation
      let mut new_text = String::new();
      let mut last_escape = false;
      let mut index = 0;
      for c in input.chars() {
        if c == '"' && !last_escape {
          break;
        }

        if last_escape {
          if !"\"$`".contains(c) {
            new_text.push('\\');
          }
          new_text.push(c);
          last_escape = false;
        } else if c == '\\' {
          last_escape = true;
        } else {
          if matches!(c, '$' | '`') {
            return ParseError::fail(
              &input[index..],
              "Substitution in strings is currently not supported.",
            );
          }
          new_text.push(c);
        }
        index += 1;
      }
      Ok((&input[index..], new_text))
    },
    assert_exists(char('"'), "Expected closing double quote."),
  )(input)
}

fn parse_usize(input: &str) -> ParseResult<usize> {
  let mut value = 0;
  let mut byte_index = 0;
  for c in input.chars() {
    if c.is_ascii_digit() {
      value = value * 10 + (c.to_digit(10).unwrap() as usize);
    } else if byte_index == 0 {
      return ParseError::backtrace();
    } else {
      break;
    }
    byte_index += c.len_utf8();
  }
  Ok((&input[byte_index..], value))
}

fn assert_whitespace_or_end_and_skip(input: &str) -> ParseResult<()> {
  terminated(assert_whitespace_or_end, skip_whitespace)(input)
}

fn assert_whitespace_or_end(input: &str) -> ParseResult<()> {
  if let Some(next_char) = input.chars().next() {
    if !next_char.is_whitespace() {
      return ParseError::fail(input, "Unsupported character.");
    }
  }
  Ok((input, ()))
}

fn is_special_char(c: char) -> bool {
  // https://github.com/yarnpkg/berry/blob/master/packages/yarnpkg-parsers/sources/grammars/shell.pegjs
  "(){}<>$|&; \t\"'".contains(c)
}

fn is_reserved_word(text: &str) -> bool {
  matches!(
    text,
    "if"
      | "then"
      | "else"
      | "elif"
      | "fi"
      | "do"
      | "done"
      | "case"
      | "esac"
      | "while"
      | "until"
      | "for"
      | "in"
  )
}

#[cfg(test)]
mod test {
  use super::*;
  use pretty_assertions::assert_eq;

  #[test]
  fn test_main() {
    assert_eq!(parse("").err().unwrap().to_string(), "Empty command.");
    assert_eq!(
      parse("&& testing").err().unwrap().to_string(),
      concat!("Invalid character for command.\n", "  && testing\n", "  ~",),
    );
    assert_eq!(
      parse("test { test").err().unwrap().to_string(),
      concat!("Unsupported character.\n", "  { test\n", "  ~",),
    );
    assert_eq!(
      parse("test > redirect").err().unwrap().to_string(),
      concat!(
        "Redirects are currently not supported.\n",
        "  > redirect\n",
        "  ~",
      ),
    );
  }

  #[test]
  fn test_parse_expr() {
    run_test(
      parse_expr,
      "Name=Value OtherVar=Other command arg1 && command2 arg12 arg13 || command3 & command4 | command5",
      Ok(Expr::BinExpr(BinExpr {
        left: Box::new(Expr::Command(ShellCommand {
          env_vars: vec![
            EnvVar::new("Name".to_string(), "Value".to_string()),
            EnvVar::new("OtherVar".to_string(), "Other".to_string()),
          ],
          args: vec!["command".to_string(), "arg1".to_string()],
        })),
        op: Operator::AndAnd,
        right: Box::new(Expr::BinExpr(BinExpr {
          left: Box::new(Expr::Command(ShellCommand {
            env_vars: vec![],
            args: vec![
              "command2".to_string(),
              "arg12".to_string(),
              "arg13".to_string(),
            ],
          })),
          op: Operator::OrOr,
          right: Box::new(Expr::BinExpr(BinExpr {
            left: Box::new(Expr::Command(ShellCommand {
              env_vars: vec![],
              args: vec!["command3".to_string()],
            })),
            op: Operator::And,
            right: Box::new(Expr::BinExpr(BinExpr {
              left: Box::new(Expr::Command(ShellCommand {
                env_vars: vec![],
                args: vec!["command4".to_string()],
              })),
              op: Operator::Or,
              right: Box::new(Expr::Command(ShellCommand {
                env_vars: vec![],
                args: vec!["command5".to_string()],
              })),
            })),
          })),
        })),
      })),
    );

    run_test(
      parse_expr,
      "test &&",
      Err("Expected command following operator."),
    );
  }

  #[test]
  fn test_env_var() {
    run_test(
      parse_env_var,
      "Name=Value",
      Ok(EnvVar {
        name: "Name".to_string(),
        value: "Value".to_string(),
      }),
    );
    run_test(
      parse_env_var,
      "Name='quoted value'",
      Ok(EnvVar {
        name: "Name".to_string(),
        value: "quoted value".to_string(),
      }),
    );
    run_test(
      parse_env_var,
      "Name=\"double quoted value\"",
      Ok(EnvVar {
        name: "Name".to_string(),
        value: "double quoted value".to_string(),
      }),
    );
    run_test_with_end(
      parse_env_var,
      "Name= command_name",
      Ok(EnvVar {
        name: "Name".to_string(),
        value: "".to_string(),
      }),
      " command_name",
    );
    run_test(
      parse_env_var,
      "Name=$(test)",
      Err(concat!(
        "Environment variable values may only be assigned quoted strings or a ",
        "plain string (consisting only of letters, numbers, and underscores)\n\n",
        "Unsupported character.")),
    );
  }

  #[test]
  fn test_single_quotes() {
    run_string_test(parse_quoted_string, "'test'", Ok("test"));
    run_string_test(parse_quoted_string, r#"'te\\'"#, Ok(r#"te\\"#));
    run_string_test_with_end(
      parse_quoted_string,
      r#"'te\'st'"#,
      Ok(r#"te\"#),
      "st'",
    );
    run_string_test(parse_quoted_string, "'  '", Ok("  "));
    run_string_test(
      parse_quoted_string,
      "'  ",
      Err("Expected closing single quote."),
    );
  }

  #[test]
  fn test_double_quotes() {
    run_string_test(parse_quoted_string, r#""  ""#, Ok("  "));
    run_string_test(parse_quoted_string, r#""test""#, Ok("test"));
    run_string_test(parse_quoted_string, r#""te\"\$\`st""#, Ok(r#"te"$`st"#));
    run_string_test(
      parse_quoted_string,
      r#""  "#,
      Err("Expected closing double quote."),
    );
    run_string_test(
      parse_quoted_string,
      r#""$Test""#,
      Err("Substitution in strings is currently not supported."),
    );
    run_string_test(
      parse_quoted_string,
      r#""asdf`""#,
      Err("Substitution in strings is currently not supported."),
    );

    run_string_test_with_end(
      parse_quoted_string,
      r#""test" asdf"#,
      Ok("test"),
      " asdf",
    );
  }

  #[test]
  fn test_parse_word() {
    run_test(parse_word, "if", Err("Unsupported reserved word."));
  }

  #[test]
  fn test_parse_usize() {
    run_test(parse_usize, "999", Ok(999));
    run_test(parse_usize, "11", Ok(11));
    run_test(parse_usize, "0", Ok(0));
    run_test_with_end(parse_usize, "1>", Ok(1), ">");
    run_test(parse_usize, "-1", Err("backtrace"));
    run_test(parse_usize, "a", Err("backtrace"));
  }

  fn run_string_test(
    combinator: impl Fn(&str) -> ParseResult<String>,
    input: &str,
    expected: Result<&str, &str>,
  ) {
    run_string_test_with_end(combinator, input, expected, "")
  }

  fn run_string_test_with_end(
    combinator: impl Fn(&str) -> ParseResult<String>,
    input: &str,
    expected: Result<&str, &str>,
    expected_end: &str,
  ) {
    let expected = expected.map(ToOwned::to_owned);
    run_test_with_end(combinator, input, expected, expected_end);
  }

  fn run_test<'a, T: PartialEq + std::fmt::Debug>(
    combinator: impl Fn(&'a str) -> ParseResult<'a, T>,
    input: &'a str,
    expected: Result<T, &str>,
  ) {
    run_test_with_end(combinator, input, expected, "");
  }

  fn run_test_with_end<'a, T: PartialEq + std::fmt::Debug>(
    combinator: impl Fn(&'a str) -> ParseResult<'a, T>,
    input: &'a str,
    expected: Result<T, &str>,
    expected_end: &str,
  ) {
    match combinator(input) {
      Ok((input, value)) => {
        assert_eq!(input, expected_end);
        assert_eq!(value, expected.unwrap());
      }
      Err(ParseError::Backtrace) => {
        assert_eq!("backtrace", expected.err().unwrap());
      }
      Err(ParseError::Failure(err)) => {
        assert_eq!(err.message, expected.err().unwrap());
      }
    }
  }
}
