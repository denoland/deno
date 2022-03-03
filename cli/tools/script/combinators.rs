// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// Inspired by nom, but simplified and with custom errors.

pub enum ParseError<'a> {
  /// Parsing should completely fail.
  Failure(FailureParseError<'a>),
  Backtrace,
}

pub struct FailureParseError<'a> {
  pub input: &'a str,
  pub message: String,
}

impl<'a> ParseError<'a> {
  pub fn fail<O>(input: &'a str, message: String) -> ParseResult<'a, O> {
    Err(ParseError::Failure(FailureParseError { input, message }))
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
pub fn tag<'a>(value: String) -> impl Fn(&'a str) -> ParseResult<'a, &'a str> {
  move |input| {
    if input.starts_with(&value) {
      Ok((&input[value.len()..], &input[..value.len()]))
    } else {
      Err(ParseError::Backtrace)
    }
  }
}

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

pub fn map<'a, O, R>(
  combinator: impl Fn(&'a str) -> ParseResult<O>,
  func: impl Fn(O) -> R,
) -> impl Fn(&'a str) -> ParseResult<R> {
  move |input| {
    let (input, result) = combinator(input)?;
    Ok((input, func(result)))
  }
}

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

/// Ensures backtracing occurs instead of a hard error.
pub fn ensure_backtrace<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  move |input| {
    match combinator(input) {
      Ok(result) => Ok(result),
      Err(ParseError::Failure(_)) => {
        // switch to backtrace
        Err(ParseError::Backtrace)
      }
      Err(err) => Err(err),
    }
  }
}

pub fn assert<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  message: &'static str,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  move |input| match combinator(input) {
    Ok(result) => Ok(result),
    Err(ParseError::Failure(err)) => {
      let mut message = message.to_string();
      message.push_str("\n\n");
      message.push_str(&err.message);
      ParseError::fail(err.input, message)
    }
    Err(ParseError::Backtrace) => ParseError::fail(input, message.to_string()),
  }
}

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

pub fn many_till<'a, O, OCondition>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  condition: impl Fn(&'a str) -> ParseResult<'a, OCondition>,
) -> impl Fn(&'a str) -> ParseResult<'a, Vec<O>> {
  move |mut input| {
    let mut results = Vec::new();
    while !input.is_empty() && condition(input).is_err() {
      match combinator(input) {
        Ok((result_input, value)) => {
          results.push(value);
          input = result_input;
        }
        Err(ParseError::Failure(e)) => return Err(ParseError::Failure(e)),
        Err(ParseError::Backtrace) => {
          return Ok((input, results));
        }
      }
    }
    Ok((input, results))
  }
}

/// Applies the parser 0 or more times and returns the
pub fn many0<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, Vec<O>> {
  many_till(combinator, |_| {
    // tells it to keep going
    let result: ParseResult<()> = Err(ParseError::Backtrace);
    result
  })
}

pub fn skip_whitespace(input: &str) -> ParseResult<()> {
  for (pos, c) in input.char_indices() {
    if !c.is_whitespace() {
      return Ok((&input[pos..], ()));
    }
  }

  Ok(("", ()))
}
