// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// Inspired by nom, but simplified and with custom errors.

#[derive(Debug)]
pub enum ParseError<'a> {
  Backtrace,
  /// Parsing should completely fail.
  Failure(ParseErrorFailure<'a>),
}

#[derive(Debug)]
pub struct ParseErrorFailure<'a> {
  pub input: &'a str,
  pub message: String,
}

impl<'a> ParseErrorFailure<'a> {
  pub fn new(input: &'a str, message: impl AsRef<str>) -> Self {
    ParseErrorFailure {
      input,
      message: message.as_ref().to_owned(),
    }
  }
}

impl<'a> ParseError<'a> {
  pub fn fail<O>(
    input: &'a str,
    message: impl AsRef<str>,
  ) -> ParseResult<'a, O> {
    Err(ParseError::Failure(ParseErrorFailure::new(input, message)))
  }

  pub fn backtrace<O>() -> ParseResult<'a, O> {
    Err(ParseError::Backtrace)
  }
}

pub type ParseResult<'a, O> = Result<(&'a str, O), ParseError<'a>>;

/// Recognizes a character.
pub fn char<'a>(c: char) -> impl Fn(&'a str) -> ParseResult<'a, char> {
  move |input| match input.chars().next() {
    Some(next_char) if next_char == c => {
      Ok((&input[next_char.len_utf8()..], c))
    }
    _ => Err(ParseError::Backtrace),
  }
}

/// Recognizes a string.
pub fn tag<'a>(
  value: impl AsRef<str>,
) -> impl Fn(&'a str) -> ParseResult<'a, &'a str> {
  let value = value.as_ref().to_string();
  move |input| {
    if input.starts_with(&value) {
      Ok((&input[value.len()..], &input[..value.len()]))
    } else {
      Err(ParseError::Backtrace)
    }
  }
}

/// Takes while the condition is true.
pub fn take_while(
  cond: impl Fn(char) -> bool,
) -> impl Fn(&str) -> ParseResult<&str> {
  move |input| {
    for (pos, c) in input.char_indices() {
      if !cond(c) {
        return Ok((&input[pos..], &input[..pos]));
      }
    }
    Ok(("", input))
  }
}

/// Maps a success to `Some(T)` and a backtrace to `None`.
pub fn maybe<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<O>,
) -> impl Fn(&'a str) -> ParseResult<Option<O>> {
  move |input| match combinator(input) {
    Ok((input, value)) => Ok((input, Some(value))),
    Err(ParseError::Backtrace) => Ok((input, None)),
    Err(err) => Err(err),
  }
}

/// Maps the combinator by a function.
pub fn map<'a, O, R>(
  combinator: impl Fn(&'a str) -> ParseResult<O>,
  func: impl Fn(O) -> R,
) -> impl Fn(&'a str) -> ParseResult<R> {
  move |input| {
    let (input, result) = combinator(input)?;
    Ok((input, func(result)))
  }
}

/// Checks for either to match.
pub fn or<'a, O>(
  a: impl Fn(&'a str) -> ParseResult<'a, O>,
  b: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  move |input| match a(input) {
    Ok(result) => Ok(result),
    Err(ParseError::Backtrace) => b(input),
    Err(err) => Err(err),
  }
}

/// Returns the first value and discards the second.
pub fn terminated<'a, First, Second>(
  first: impl Fn(&'a str) -> ParseResult<'a, First>,
  second: impl Fn(&'a str) -> ParseResult<'a, Second>,
) -> impl Fn(&'a str) -> ParseResult<'a, First> {
  move |input| {
    let (input, return_value) = first(input)?;
    let (input, _) = second(input)?;
    Ok((input, return_value))
  }
}

/// Gets a second value that is delimited by a first and third.
pub fn delimited<'a, First, Second, Third>(
  first: impl Fn(&'a str) -> ParseResult<'a, First>,
  second: impl Fn(&'a str) -> ParseResult<'a, Second>,
  third: impl Fn(&'a str) -> ParseResult<'a, Third>,
) -> impl Fn(&'a str) -> ParseResult<'a, Second> {
  move |input| {
    let (input, _) = first(input)?;
    let (input, return_value) = second(input)?;
    let (input, _) = third(input)?;
    Ok((input, return_value))
  }
}

/// Asserts that a combinator resolves. If backtracing occurs, returns a failure.
pub fn assert_exists<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  message: &'static str,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  assert(combinator, |result| result.is_ok(), message)
}

/// Asserts that a given condition is true about the combinator.
/// Otherwise returns an error with the message.
pub fn assert<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  condition: impl Fn(Result<&(&'a str, O), &ParseError<'a>>) -> bool,
  message: &'static str,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  move |input| {
    let result = combinator(input);
    if condition(result.as_ref()) {
      result
    } else {
      match combinator(input) {
        Err(ParseError::Failure(err)) => {
          let mut message = message.to_string();
          message.push_str("\n\n");
          message.push_str(&err.message);
          ParseError::fail(err.input, message)
        }
        _ => ParseError::fail(input, message),
      }
    }
  }
}

/// Provides some context to a failure.
pub fn with_error_context<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  message: &'static str,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  move |input| match combinator(input) {
    Ok(result) => Ok(result),
    Err(ParseError::Backtrace) => Err(ParseError::Backtrace),
    Err(ParseError::Failure(err)) => {
      let mut message = message.to_string();
      message.push_str("\n\n");
      message.push_str(&err.message);
      ParseError::fail(err.input, message)
    }
  }
}

/// Keeps consuming a combinator into an array until a condition
/// is met or backtracing occurs.
pub fn many_till<'a, O, OCondition>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  condition: impl Fn(&'a str) -> ParseResult<'a, OCondition>,
) -> impl Fn(&'a str) -> ParseResult<'a, Vec<O>> {
  move |mut input| {
    let mut results = Vec::new();
    while !input.is_empty() && is_backtrace(condition(input))? {
      match combinator(input) {
        Ok((result_input, value)) => {
          results.push(value);
          input = result_input;
        }
        Err(ParseError::Backtrace) => {
          return Ok((input, results));
        }
        Err(err) => return Err(err),
      }
    }
    Ok((input, results))
  }
}

/// Keeps consuming a combinator into an array until a condition
/// is met or backtracing occurs.
pub fn separated_list<'a, O, OSeparator>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  separator: impl Fn(&'a str) -> ParseResult<'a, OSeparator>,
) -> impl Fn(&'a str) -> ParseResult<'a, Vec<O>> {
  move |mut input| {
    let mut results = Vec::new();
    while !input.is_empty() {
      match combinator(input) {
        Ok((result_input, value)) => {
          results.push(value);
          input = result_input;
        }
        Err(ParseError::Backtrace) => {
          return Ok((input, results));
        }
        Err(err) => return Err(err),
      }
      input = match separator(input) {
        Ok((input, _)) => input,
        Err(ParseError::Backtrace) => break,
        Err(err) => return Err(err),
      };
    }
    Ok((input, results))
  }
}

/// Applies the parser 0 or more times and returns a vector
/// of all the parsed results.
pub fn many0<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, Vec<O>> {
  many_till(combinator, |_| ParseError::backtrace::<()>())
}

/// Skips the whitespace.
pub fn skip_whitespace(input: &str) -> ParseResult<()> {
  for (pos, c) in input.char_indices() {
    if !c.is_whitespace() {
      return Ok((&input[pos..], ()));
    }
  }

  Ok(("", ()))
}

/// Checks if a combinator is false without consuming the input.
pub fn if_not<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, ()> {
  move |input| match combinator(input) {
    Ok(_) => ParseError::backtrace(),
    Err(_) => Ok((input, ())),
  }
}

fn is_backtrace<O>(result: ParseResult<O>) -> Result<bool, ParseError> {
  match result {
    Ok(_) => Ok(false),
    Err(ParseError::Backtrace) => Ok(true),
    Err(err) => Err(err),
  }
}
