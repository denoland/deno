// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::bail;
use deno_core::error::AnyError;

use super::combinators::assert;
use super::combinators::assert_exists;
use super::combinators::char;
use super::combinators::delimited;
use super::combinators::if_not;
use super::combinators::many0;
use super::combinators::many_till;
use super::combinators::map;
use super::combinators::maybe;
use super::combinators::or;
use super::combinators::separated_list;
use super::combinators::skip_whitespace;
use super::combinators::tag;
use super::combinators::take_while;
use super::combinators::terminated;
use super::combinators::with_error_context;
use super::combinators::ParseError;
use super::combinators::ParseErrorFailure;
use super::combinators::ParseResult;

// Shell grammar rules this is loosely based on:
// https://pubs.opengroup.org/onlinepubs/009604499/utilities/xcu_chap02.html#tag_02_10_02

#[derive(Debug, Clone, PartialEq)]
pub struct SequentialList {
  pub items: Vec<SequentialListItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequentialListItem {
  pub is_async: bool,
  pub sequence: Sequence,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Sequence {
  /// `MY_VAR=5` or `export MY_VAR=5`
  EnvVar(SetEnvVarCommand),
  // cmd_name <args...>
  Command(Command),
  // cmd1 | cmd2
  Pipeline(Box<Pipeline>),
  // cmd1 && cmd2 || cmd3
  BooleanList(Box<BooleanList>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetEnvVarCommand {
  pub exported: bool,
  pub var: EnvVar,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BooleanList {
  pub current: Sequence,
  pub op: BooleanListOperator,
  pub next: Sequence,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pipeline {
  pub current: Sequence,
  pub next: Sequence,
}

impl Pipeline {
  pub fn into_vec(self) -> Vec<Sequence> {
    let mut sequences = vec![self.current];
    match self.next {
      Sequence::Pipeline(pipeline) => {
        sequences.extend(pipeline.into_vec());
      }
      next => sequences.push(next),
    }
    sequences
  }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BooleanListOperator {
  // &&
  And,
  // ||
  Or,
}

impl BooleanListOperator {
  pub fn as_str(&self) -> &'static str {
    match self {
      BooleanListOperator::And => "&&",
      BooleanListOperator::Or => "||",
    }
  }

  pub fn moves_next_for_exit_code(&self, exit_code: i32) -> bool {
    *self == BooleanListOperator::Or && exit_code != 0
      || *self == BooleanListOperator::And && exit_code == 0
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Command {
  pub env_vars: Vec<EnvVar>,
  pub args: Vec<String>,
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

pub fn parse(input: &str) -> Result<SequentialList, AnyError> {
  fn error_for_failure(
    e: ParseErrorFailure,
  ) -> Result<SequentialList, AnyError> {
    bail!(
      "{}\n  {}\n  ~",
      e.message,
      // truncate the output to prevent wrapping in the console
      e.input.chars().take(60).collect::<String>()
    )
  }

  match parse_sequential_list(input) {
    Ok((input, expr)) => {
      if input.trim().is_empty() {
        if expr.items.is_empty() {
          bail!("Empty command.")
        } else {
          Ok(expr)
        }
      } else {
        error_for_failure(fail_for_trailing_input(input))
      }
    }
    Err(ParseError::Backtrace) => {
      error_for_failure(fail_for_trailing_input(input))
    }
    Err(ParseError::Failure(e)) => error_for_failure(e),
  }
}

fn parse_sequential_list(input: &str) -> ParseResult<SequentialList> {
  let (input, items) = separated_list(
    terminated(parse_sequential_list_item, skip_whitespace),
    terminated(
      skip_whitespace,
      or(parse_sequential_list_op, parse_async_list_op),
    ),
  )(input)?;
  Ok((input, SequentialList { items }))
}

fn parse_sequential_list_item(input: &str) -> ParseResult<SequentialListItem> {
  let (input, sequence) = parse_sequence(input)?;
  Ok((
    input,
    SequentialListItem {
      is_async: maybe(parse_async_list_op)(input)?.1.is_some(),
      sequence,
    },
  ))
}

fn parse_sequence(input: &str) -> ParseResult<Sequence> {
  let (input, current) = or(
    map(parse_set_env_var_command, Sequence::EnvVar),
    map(parse_command, Sequence::Command),
  )(input)?;
  let (input, current) = match parse_boolean_list_op(input) {
    Ok((input, op)) => {
      let (input, next_sequence) = assert_exists(
        &parse_sequence,
        "Expected command following boolean operator.",
      )(input)?;
      (
        input,
        Sequence::BooleanList(Box::new(BooleanList {
          current,
          op,
          next: next_sequence,
        })),
      )
    }
    Err(ParseError::Backtrace) => match parse_pipeline_op(input) {
      Ok((input, _)) => {
        let (input, next_sequence) = assert_exists(
          &parse_sequence,
          "Expected command following pipeline operator.",
        )(input)?;
        (
          input,
          Sequence::Pipeline(Box::new(Pipeline {
            current,
            next: next_sequence,
          })),
        )
      }
      Err(ParseError::Backtrace) => (input, current),
      Err(err) => return Err(err),
    },
    Err(err) => return Err(err),
  };

  Ok((input, current))
}

fn parse_set_env_var_command(input: &str) -> ParseResult<SetEnvVarCommand> {
  let env_vars_input = input;
  let (input, maybe_export) =
    maybe(terminated(parse_word_with_text("export"), skip_whitespace))(input)?;
  let (input, mut env_vars) = parse_env_vars(input)?;
  if env_vars.is_empty() {
    return ParseError::backtrace();
  }
  let (input, args) = parse_command_args(input)?;
  if !args.is_empty() {
    return ParseError::backtrace();
  }
  if env_vars.len() > 1 {
    ParseError::fail(env_vars_input, "Cannot set multiple environment variables when there is no following command.")
  } else {
    ParseResult::Ok((
      input,
      SetEnvVarCommand {
        exported: maybe_export.is_some(),
        var: env_vars.remove(0),
      },
    ))
  }
}

fn parse_command(input: &str) -> ParseResult<Command> {
  let (input, env_vars) = parse_env_vars(input)?;
  let (input, args) = parse_command_args(input)?;
  if args.is_empty() {
    return ParseError::backtrace();
  }
  ParseResult::Ok((input, Command { env_vars, args }))
}

fn parse_command_args(input: &str) -> ParseResult<Vec<String>> {
  many_till(
    terminated(parse_shell_arg, assert_whitespace_or_end_and_skip),
    or(
      parse_list_op,
      or(map(parse_redirect, |_| ()), parse_pipeline_op),
    ),
  )(input)
}

fn parse_shell_arg(input: &str) -> ParseResult<String> {
  let (input, value) =
    or(parse_quoted_string, map(parse_word, ToString::to_string))(input)?;
  if value.trim().is_empty() {
    ParseError::backtrace()
  } else {
    Ok((input, value))
  }
}

fn parse_list_op(input: &str) -> ParseResult<()> {
  or(
    map(parse_boolean_list_op, |_| ()),
    map(or(parse_sequential_list_op, parse_async_list_op), |_| ()),
  )(input)
}

fn parse_boolean_list_op(input: &str) -> ParseResult<BooleanListOperator> {
  or(
    map(parse_op_str(BooleanListOperator::And.as_str()), |_| {
      BooleanListOperator::And
    }),
    map(parse_op_str(BooleanListOperator::Or.as_str()), |_| {
      BooleanListOperator::Or
    }),
  )(input)
}

fn parse_sequential_list_op(input: &str) -> ParseResult<&str> {
  parse_op_str(";")(input)
}

fn parse_async_list_op(input: &str) -> ParseResult<&str> {
  parse_op_str("&")(input)
}

fn parse_op_str<'a>(
  operator: &str,
) -> impl Fn(&'a str) -> ParseResult<'a, &'a str> {
  let operator = operator.to_string();
  terminated(
    tag(operator),
    terminated(if_not(special_char), skip_whitespace),
  )
}

fn parse_pipeline_op(input: &str) -> ParseResult<()> {
  terminated(
    map(char('|'), |_| ()),
    terminated(if_not(special_char), skip_whitespace),
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
    take_while(|c| !c.is_whitespace() && !is_special_char(c)),
    |result| {
      result
        .ok()
        .map(|(_, text)| !is_reserved_word(text))
        .unwrap_or(true)
    },
    "Unsupported reserved word.",
  )(input)
}

fn parse_word_with_text(
  text: &'static str,
) -> impl Fn(&str) -> ParseResult<&str> {
  move |input| {
    let (input, word) = parse_word(input)?;
    if word == text {
      ParseResult::Ok((input, word))
    } else {
      ParseError::backtrace()
    }
  }
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
      return Err(ParseError::Failure(fail_for_trailing_input(input)));
    }
  }
  Ok((input, ()))
}

fn special_char(input: &str) -> ParseResult<char> {
  if let Some((index, next_char)) = input.char_indices().next() {
    if is_special_char(next_char) {
      return Ok((&input[index..], next_char));
    }
  }
  ParseError::backtrace()
}

fn is_special_char(c: char) -> bool {
  "*~(){}<>$|&;\"'".contains(c)
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

fn fail_for_trailing_input(input: &str) -> ParseErrorFailure {
  if parse_redirect(input).is_ok() {
    ParseErrorFailure::new(input, "Redirects are currently not supported.")
  } else if input.starts_with('*') {
    ParseErrorFailure::new(input, "Globs are currently not supported.")
  } else {
    ParseErrorFailure::new(input, "Unsupported character.")
  }
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
      concat!("Unsupported character.\n", "  && testing\n", "  ~",),
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
    assert_eq!(
      parse("cp test/* other").err().unwrap().to_string(),
      concat!("Globs are currently not supported.\n", "  * other\n", "  ~",),
    );
  }

  #[test]
  fn test_sequential_list() {
    run_test(
      parse_sequential_list,
      "Name=Value OtherVar=Other command arg1 || command2 arg12 arg13 ; command3 && command4 & command5 ; export ENV6=5 ; ENV7=other && command8 || command9",
      Ok(SequentialList {
        items: vec![
          SequentialListItem {
            is_async: false,
            sequence: Sequence::BooleanList(Box::new(BooleanList {
              current: Sequence::Command(Command {
                env_vars: vec![
                  EnvVar::new("Name".to_string(), "Value".to_string()),
                  EnvVar::new("OtherVar".to_string(), "Other".to_string()),
                ],
                args: vec!["command".to_string(), "arg1".to_string()],
              }),
              op: BooleanListOperator::Or,
              next: Sequence::Command(Command {
                env_vars: vec![],
                args: vec![
                  "command2".to_string(),
                  "arg12".to_string(),
                  "arg13".to_string(),
                ],
              }),
            })),
          },
          SequentialListItem {
            is_async: true,
            sequence: Sequence::BooleanList(Box::new(BooleanList {
              current: Sequence::Command(Command {
                env_vars: vec![],
                args: vec!["command3".to_string()],
              }),
              op: BooleanListOperator::And,
              next: Sequence::Command(Command {
                env_vars: vec![],
                args: vec![
                  "command4".to_string(),
                ],
              }),
            })),
          },
          SequentialListItem {
            is_async: false,
            sequence: Sequence::Command(Command {
              env_vars: vec![],
              args: vec![
                "command5".to_string(),
              ],
            }),
          },
          SequentialListItem {
            is_async: false,
            sequence: Sequence::EnvVar(SetEnvVarCommand {
              exported: true,
              var: EnvVar::new("ENV6".to_string(), "5".to_string()),
            }),
          },
          SequentialListItem {
            is_async: false,
            sequence: Sequence::BooleanList(Box::new(BooleanList {
              current: Sequence::EnvVar(SetEnvVarCommand {
                exported: false,
                var: EnvVar::new("ENV7".to_string(), "other".to_string()),
              }),
              op: BooleanListOperator::And,
              next: Sequence::BooleanList(Box::new(BooleanList {
                current: Sequence::Command(Command {
                  env_vars: vec![],
                  args: vec!["command8".to_string()],
                }),
                op: BooleanListOperator::Or,
                next: Sequence::Command(Command {
                  env_vars: vec![],
                  args: vec!["command9".to_string()],
                }),
              })),
            })),
          },
        ],
      })
    );

    run_test(
      parse_sequential_list,
      "command1 ; command2 ; A='b' command3",
      Ok(SequentialList {
        items: vec![
          SequentialListItem {
            is_async: false,
            sequence: Sequence::Command(Command {
              env_vars: vec![],
              args: vec!["command1".to_string()],
            }),
          },
          SequentialListItem {
            is_async: false,
            sequence: Sequence::Command(Command {
              env_vars: vec![],
              args: vec!["command2".to_string()],
            }),
          },
          SequentialListItem {
            is_async: false,
            sequence: Sequence::Command(Command {
              env_vars: vec![EnvVar::new("A".to_string(), "b".to_string())],
              args: vec!["command3".to_string()],
            }),
          },
        ],
      }),
    );

    run_test(
      parse_sequential_list,
      "test &&",
      Err("Expected command following boolean operator."),
    );

    run_test(
      parse_sequential_list,
      "command &",
      Ok(SequentialList {
        items: vec![SequentialListItem {
          is_async: true,
          sequence: Sequence::Command(Command {
            env_vars: vec![],
            args: vec!["command".to_string()],
          }),
        }],
      }),
    );

    run_test(
      parse_sequential_list,
      "test | other",
      Ok(SequentialList {
        items: vec![SequentialListItem {
          is_async: false,
          sequence: Sequence::Pipeline(Box::new(Pipeline {
            current: Sequence::Command(Command {
              env_vars: vec![],
              args: vec!["test".to_string()],
            }),
            next: Sequence::Command(Command {
              env_vars: vec![],
              args: vec!["other".to_string()],
            }),
          })),
        }],
      }),
    );

    run_test(
      parse_sequential_list,
      "ENV=1 ENV2=3 && test",
      Err("Cannot set multiple environment variables when there is no following command."),
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
        assert_eq!(value, expected.unwrap());
        assert_eq!(input, expected_end);
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
