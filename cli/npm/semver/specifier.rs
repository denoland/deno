// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use monch::*;
use serde::Deserialize;
use serde::Serialize;

use super::errors::with_failure_handling;
use super::range::Partial;
use super::range::VersionRange;
use super::range::XRange;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum SpecifierVersionReqInner {
  Range(VersionRange),
  Tag(String),
}

/// Version requirement found in npm specifiers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpecifierVersionReq {
  raw_text: String,
  inner: SpecifierVersionReqInner,
}

impl std::fmt::Display for SpecifierVersionReq {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.raw_text)
  }
}

impl SpecifierVersionReq {
  pub fn parse(text: &str) -> Result<Self, AnyError> {
    with_failure_handling(parse_npm_specifier)(text).with_context(|| {
      format!("Invalid npm specifier version requirement '{}'.", text)
    })
  }

  pub fn range(&self) -> Option<&VersionRange> {
    match &self.inner {
      SpecifierVersionReqInner::Range(range) => Some(range),
      SpecifierVersionReqInner::Tag(_) => None,
    }
  }

  pub fn tag(&self) -> Option<&str> {
    match &self.inner {
      SpecifierVersionReqInner::Range(_) => None,
      SpecifierVersionReqInner::Tag(tag) => Some(tag.as_str()),
    }
  }
}

fn parse_npm_specifier(input: &str) -> ParseResult<SpecifierVersionReq> {
  map_res(version_range, |result| {
    let (new_input, range_result) = match result {
      Ok((input, range)) => (input, Ok(range)),
      // use an empty string because we'll consider the tag
      Err(err) => ("", Err(err)),
    };
    Ok((
      new_input,
      SpecifierVersionReq {
        raw_text: input.to_string(),
        inner: match range_result {
          Ok(range) => SpecifierVersionReqInner::Range(range),
          Err(err) => {
            // npm seems to be extremely lax on what it supports for a dist-tag (any non-valid semver range),
            // so just make any error here be a dist tag unless it starts or ends with whitespace
            if input.trim() != input {
              return Err(err);
            } else {
              SpecifierVersionReqInner::Tag(input.to_string())
            }
          }
        },
      },
    ))
  })(input)
}

// Note: Although the code below looks very similar to what's used for
// parsing npm version requirements, the code here is more strict
// in order to not allow for people to get ridiculous when using
// npm specifiers.

// version_range ::= partial | tilde | caret
fn version_range(input: &str) -> ParseResult<VersionRange> {
  or3(
    map(preceded(ch('~'), partial), |partial| {
      partial.as_tilde_version_range()
    }),
    map(preceded(ch('^'), partial), |partial| {
      partial.as_caret_version_range()
    }),
    map(partial, |partial| partial.as_equal_range()),
  )(input)
}

// partial ::= xr ( '.' xr ( '.' xr qualifier ? )? )?
fn partial(input: &str) -> ParseResult<Partial> {
  let (input, major) = xr()(input)?;
  let (input, maybe_minor) = maybe(preceded(ch('.'), xr()))(input)?;
  let (input, maybe_patch) = if maybe_minor.is_some() {
    maybe(preceded(ch('.'), xr()))(input)?
  } else {
    (input, None)
  };
  let (input, qual) = if maybe_patch.is_some() {
    maybe(qualifier)(input)?
  } else {
    (input, None)
  };
  let qual = qual.unwrap_or_default();
  Ok((
    input,
    Partial {
      major,
      minor: maybe_minor.unwrap_or(XRange::Wildcard),
      patch: maybe_patch.unwrap_or(XRange::Wildcard),
      pre: qual.pre,
      build: qual.build,
    },
  ))
}

// xr ::= 'x' | 'X' | '*' | nr
fn xr<'a>() -> impl Fn(&'a str) -> ParseResult<'a, XRange> {
  or(
    map(or3(tag("x"), tag("X"), tag("*")), |_| XRange::Wildcard),
    map(nr, XRange::Val),
  )
}

// nr ::= '0' | ['1'-'9'] ( ['0'-'9'] ) *
fn nr(input: &str) -> ParseResult<u64> {
  or(map(tag("0"), |_| 0), move |input| {
    let (input, result) = if_not_empty(substring(pair(
      if_true(next_char, |c| c.is_ascii_digit() && *c != '0'),
      skip_while(|c| c.is_ascii_digit()),
    )))(input)?;
    let val = match result.parse::<u64>() {
      Ok(val) => val,
      Err(err) => {
        return ParseError::fail(
          input,
          format!("Error parsing '{}' to u64.\n\n{:#}", result, err),
        )
      }
    };
    Ok((input, val))
  })(input)
}

#[derive(Debug, Clone, Default)]
struct Qualifier {
  pre: Vec<String>,
  build: Vec<String>,
}

// qualifier ::= ( '-' pre )? ( '+' build )?
fn qualifier(input: &str) -> ParseResult<Qualifier> {
  let (input, pre_parts) = maybe(pre)(input)?;
  let (input, build_parts) = maybe(build)(input)?;
  Ok((
    input,
    Qualifier {
      pre: pre_parts.unwrap_or_default(),
      build: build_parts.unwrap_or_default(),
    },
  ))
}

// pre ::= parts
fn pre(input: &str) -> ParseResult<Vec<String>> {
  preceded(ch('-'), parts)(input)
}

// build ::= parts
fn build(input: &str) -> ParseResult<Vec<String>> {
  preceded(ch('+'), parts)(input)
}

// parts ::= part ( '.' part ) *
fn parts(input: &str) -> ParseResult<Vec<String>> {
  if_not_empty(map(separated_list(part, ch('.')), |text| {
    text.into_iter().map(ToOwned::to_owned).collect()
  }))(input)
}

// part ::= nr | [-0-9A-Za-z]+
fn part(input: &str) -> ParseResult<&str> {
  // nr is in the other set, so don't bother checking for it
  if_true(
    take_while(|c| c.is_ascii_alphanumeric() || c == '-'),
    |result| !result.is_empty(),
  )(input)
}

#[cfg(test)]
mod tests {
  use crate::npm::semver::NpmVersion;

  use super::*;

  struct VersionReqTester(SpecifierVersionReq);

  impl VersionReqTester {
    fn new(text: &str) -> Self {
      Self(SpecifierVersionReq::parse(text).unwrap())
    }

    fn matches(&self, version: &str) -> bool {
      self
        .0
        .range()
        .map(|r| r.satisfies(&NpmVersion::parse(version).unwrap()))
        .unwrap_or(false)
    }
  }

  #[test]
  fn version_req_exact() {
    let tester = VersionReqTester::new("1.0.1");
    assert!(!tester.matches("1.0.0"));
    assert!(tester.matches("1.0.1"));
    assert!(!tester.matches("1.0.2"));
    assert!(!tester.matches("1.1.1"));

    // pre-release
    let tester = VersionReqTester::new("1.0.0-alpha.13");
    assert!(tester.matches("1.0.0-alpha.13"));
  }

  #[test]
  fn version_req_minor() {
    let tester = VersionReqTester::new("1.1");
    assert!(!tester.matches("1.0.0"));
    assert!(tester.matches("1.1.0"));
    assert!(tester.matches("1.1.1"));
    assert!(!tester.matches("1.2.0"));
    assert!(!tester.matches("1.2.1"));
  }

  #[test]
  fn version_req_caret() {
    let tester = VersionReqTester::new("^1.1.1");
    assert!(!tester.matches("1.1.0"));
    assert!(tester.matches("1.1.1"));
    assert!(tester.matches("1.1.2"));
    assert!(tester.matches("1.2.0"));
    assert!(!tester.matches("2.0.0"));

    let tester = VersionReqTester::new("^0.1.1");
    assert!(!tester.matches("0.0.0"));
    assert!(!tester.matches("0.1.0"));
    assert!(tester.matches("0.1.1"));
    assert!(tester.matches("0.1.2"));
    assert!(!tester.matches("0.2.0"));
    assert!(!tester.matches("1.0.0"));

    let tester = VersionReqTester::new("^0.0.1");
    assert!(!tester.matches("0.0.0"));
    assert!(tester.matches("0.0.1"));
    assert!(!tester.matches("0.0.2"));
    assert!(!tester.matches("0.1.0"));
    assert!(!tester.matches("1.0.0"));
  }

  #[test]
  fn version_req_tilde() {
    let tester = VersionReqTester::new("~1.1.1");
    assert!(!tester.matches("1.1.0"));
    assert!(tester.matches("1.1.1"));
    assert!(tester.matches("1.1.2"));
    assert!(!tester.matches("1.2.0"));
    assert!(!tester.matches("2.0.0"));

    let tester = VersionReqTester::new("~0.1.1");
    assert!(!tester.matches("0.0.0"));
    assert!(!tester.matches("0.1.0"));
    assert!(tester.matches("0.1.1"));
    assert!(tester.matches("0.1.2"));
    assert!(!tester.matches("0.2.0"));
    assert!(!tester.matches("1.0.0"));

    let tester = VersionReqTester::new("~0.0.1");
    assert!(!tester.matches("0.0.0"));
    assert!(tester.matches("0.0.1"));
    assert!(tester.matches("0.0.2")); // for some reason this matches, but not with ^
    assert!(!tester.matches("0.1.0"));
    assert!(!tester.matches("1.0.0"));
  }

  #[test]
  fn parses_tag() {
    let latest_tag = SpecifierVersionReq::parse("latest").unwrap();
    assert_eq!(latest_tag.tag().unwrap(), "latest");
  }
}
