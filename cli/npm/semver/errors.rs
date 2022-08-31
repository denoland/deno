// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use monch::ParseError;
use monch::ParseErrorFailure;
use monch::ParseResult;

pub fn with_failure_handling<'a, T>(
  combinator: impl Fn(&'a str) -> ParseResult<T>,
) -> impl Fn(&'a str) -> Result<T, AnyError> {
  move |input| match combinator(input) {
    Ok((input, result)) => {
      if !input.is_empty() {
        error_for_failure(fail_for_trailing_input(input))
      } else {
        Ok(result)
      }
    }
    Err(ParseError::Backtrace) => {
      error_for_failure(fail_for_trailing_input(input))
    }
    Err(ParseError::Failure(e)) => error_for_failure(e),
  }
}

fn error_for_failure<T>(e: ParseErrorFailure) -> Result<T, AnyError> {
  bail!(
    "{}\n  {}\n  ~",
    e.message,
    // truncate the output to prevent wrapping in the console
    e.input.chars().take(60).collect::<String>()
  )
}

fn fail_for_trailing_input(input: &str) -> ParseErrorFailure {
  ParseErrorFailure::new(input, "Unexpected character.")
}
