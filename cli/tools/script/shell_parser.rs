// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::combinators::assert;
use super::combinators::char;
use super::combinators::delimited;
use super::combinators::many0;
use super::combinators::many_till;
use super::combinators::map;
use super::combinators::or;
use super::combinators::skip_whitespace;
use super::combinators::tag;
use super::combinators::take_while;
use super::combinators::terminated;
use super::combinators::with_error_context;
use super::combinators::FailureParseError;
use super::combinators::ParseError;
use super::combinators::ParseResult;

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
  pub fn as_str(&self) -> &str {
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

pub fn parse_expr(input: &str) -> ParseResult<Expr> {
  let (input, command) = parse_command(input)?;
  if let Ok((input, op)) = parse_operator(input) {
    let (input, right_command) =
      assert(parse_expr, "Expected command following operator.")(input)?;
    Ok((
      input,
      Expr::BinExpr(BinExpr {
        left: Box::new(Expr::Command(command)),
        op,
        right: Box::new(right_command),
      }),
    ))
  } else {
    Ok((input, Expr::Command(command)))
  }
}

fn parse_command(input: &str) -> ParseResult<ShellCommand> {
  let (input, env_vars) = parse_env_vars(input)?;
  let (input, args) = parse_shell_args(input)?;
  Ok((input, ShellCommand { env_vars, args }))
}

fn parse_shell_args(input: &str) -> ParseResult<Vec<String>> {
  many_till(
    terminated(parse_shell_arg, |input| {
      assert_whitespace_or_end(input)?;
      skip_whitespace(input)
    }),
    parse_operator,
  )(input)
}

fn parse_shell_arg(input: &str) -> ParseResult<String> {
  or(
    parse_quoted_string,
    map(parse_plain_string, |v| v.to_string()),
  )(input)
}

fn parse_operator(input: &str) -> ParseResult<Operator> {
  fn operator_kind<'a>(
    operator: Operator,
  ) -> impl Fn(&'a str) -> ParseResult<'a, Operator> {
    map(tag(operator.as_str().to_string()), move |_| operator)
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

fn parse_env_vars(input: &str) -> ParseResult<Vec<EnvVar>> {
  many0(terminated(parse_env_var, |input| {
    assert_whitespace_or_end(input)?;
    skip_whitespace(input)
  }))(input)
}

fn parse_env_var(input: &str) -> ParseResult<EnvVar> {
  let (input, name) = parse_env_var_name(input)?;
  let (input, _) = char('=')(input)?;
  let (input, value) = with_error_context(
    parse_env_var_value,
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
  or(
    parse_quoted_string,
    map(parse_plain_string, |v| v.to_string()),
  )(input)
}

fn parse_plain_string(input: &str) -> ParseResult<&str> {
  let (input, value) = take_while(|c| !is_special_char(c))(input)?;
  assert_whitespace_or_end(input)?;
  Ok((input, value))
}

fn assert_whitespace_or_end(input: &str) -> Result<(), ParseError> {
  if let Some(next_char) = input.chars().next() {
    if !next_char.is_whitespace() {
      return Err(ParseError::Failure(FailureParseError {
        input,
        message:
          "Unsupported character. Expected whitespace or end of command."
            .to_string(),
      }));
    }
  }
  Ok(())
}

fn is_special_char(c: char) -> bool {
  // https://github.com/yarnpkg/berry/blob/master/packages/yarnpkg-parsers/sources/grammars/shell.pegjs
  "(){}<>$|&; \t\"'".contains(c)
}

// TODO(THIS PR): need to hard error when someone uses shell expansion in a string
fn parse_quoted_string(input: &str) -> ParseResult<String> {
  fn inner_quoted<'a>(
    quote_char: char,
  ) -> impl Fn(&'a str) -> ParseResult<'a, String> {
    move |input| {
      // this doesn't seem like the nom way of doing things, but it was a
      // quick implementation
      let mut new_text = String::new();
      let mut last_escape = false;
      let mut index = 0;
      for c in input.chars() {
        if c == quote_char && !last_escape {
          break;
        }

        if last_escape {
          if c != quote_char {
            new_text.push('\\');
          }
          new_text.push(c);
          last_escape = false;
        } else if c == '\\' {
          last_escape = true;
        } else {
          new_text.push(c);
        }
        index += 1;
      }
      Ok((&input[index..], new_text))
    }
  }

  or(
    delimited(
      char('\''),
      inner_quoted('\''),
      assert(char('\''), "Expected closing single quote."),
    ),
    delimited(
      char('"'),
      inner_quoted('"'),
      assert(char('"'), "Expected closing double quote."),
    ),
  )(input)
}

#[cfg(test)]
mod test {
  use super::*;
  use pretty_assertions::assert_eq;

  #[test]
  fn test_parse() {
    run_test(
      parse_expr,
      "Name=Value OtherVar=Other command arg1 && command2 arg12 arg13",
      Ok(Expr::BinExpr(BinExpr {
        left: Box::new(Expr::Command(ShellCommand {
          env_vars: vec![
            EnvVar::new("Name".to_string(), "Value".to_string()),
            EnvVar::new("OtherVar".to_string(), "Other".to_string()),
          ],
          args: vec!["command".to_string(), "arg1".to_string()],
        })),
        op: Operator::AndAnd,
        right: Box::new(Expr::Command(ShellCommand {
          env_vars: vec![],
          args: vec![
            "command2".to_string(),
            "arg12".to_string(),
            "arg13".to_string(),
          ],
        })),
      })),
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
  }

  #[test]
  fn test_quoted_string() {
    run_quoted_string_test("'test'", Ok("test"));
    run_quoted_string_test(r#"'te\\'"#, Ok(r#"te\\"#));
    run_quoted_string_test(r#"'te\'st'"#, Ok("te'st"));
    run_quoted_string_test("'  '", Ok("  "));
    run_quoted_string_test("'  ", Err("Expected closing single quote."));

    run_quoted_string_test(r#""  ""#, Ok("  "));
    run_quoted_string_test(r#""test""#, Ok("test"));
    run_quoted_string_test(r#""te\"st""#, Ok(r#"te"st"#));
    run_quoted_string_test(r#""  "#, Err("Expected closing double quote."));

    run_quoted_string_test_with_end(r#""test" asdf"#, Ok("test"), " asdf");
  }

  fn run_quoted_string_test(input: &str, expected: Result<&str, &str>) {
    run_quoted_string_test_with_end(input, expected, "")
  }

  fn run_quoted_string_test_with_end(
    input: &str,
    expected: Result<&str, &str>,
    expected_end: &str,
  ) {
    let expected = expected.map(ToOwned::to_owned);
    run_test_with_end(parse_quoted_string, input, expected, expected_end);
  }

  fn run_test<T: PartialEq + std::fmt::Debug>(
    combinator: impl Fn(&str) -> ParseResult<T>,
    input: &str,
    expected: Result<T, &str>,
  ) {
    run_test_with_end(combinator, input, expected, "");
  }

  fn run_test_with_end<T: PartialEq + std::fmt::Debug>(
    combinator: impl Fn(&str) -> ParseResult<T>,
    input: &str,
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
