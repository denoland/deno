// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::cmp::Ordering;
use std::fmt;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use monch::*;
use serde::Deserialize;
use serde::Serialize;

use crate::npm::resolution::NpmVersionMatcher;

use self::errors::with_failure_handling;
use self::range::Partial;
use self::range::VersionBoundKind;
use self::range::VersionRange;
use self::range::VersionRangeSet;

use self::range::XRange;
pub use self::specifier::SpecifierVersionReq;

mod errors;
mod range;
mod specifier;

// A lot of the below is a re-implementation of parts of https://github.com/npm/node-semver
// which is Copyright (c) Isaac Z. Schlueter and Contributors (ISC License)

#[derive(
  Clone, Debug, PartialEq, Eq, Default, Hash, Serialize, Deserialize,
)]
pub struct NpmVersion {
  pub major: u64,
  pub minor: u64,
  pub patch: u64,
  pub pre: Vec<String>,
  pub build: Vec<String>,
}

impl fmt::Display for NpmVersion {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
    if !self.pre.is_empty() {
      write!(f, "-")?;
      for (i, part) in self.pre.iter().enumerate() {
        if i > 0 {
          write!(f, ".")?;
        }
        write!(f, "{}", part)?;
      }
    }
    if !self.build.is_empty() {
      write!(f, "+")?;
      for (i, part) in self.build.iter().enumerate() {
        if i > 0 {
          write!(f, ".")?;
        }
        write!(f, "{}", part)?;
      }
    }
    Ok(())
  }
}

impl std::cmp::PartialOrd for NpmVersion {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl std::cmp::Ord for NpmVersion {
  fn cmp(&self, other: &Self) -> Ordering {
    let cmp_result = self.major.cmp(&other.major);
    if cmp_result != Ordering::Equal {
      return cmp_result;
    }

    let cmp_result = self.minor.cmp(&other.minor);
    if cmp_result != Ordering::Equal {
      return cmp_result;
    }

    let cmp_result = self.patch.cmp(&other.patch);
    if cmp_result != Ordering::Equal {
      return cmp_result;
    }

    // only compare the pre-release and not the build as node-semver does
    if self.pre.is_empty() && other.pre.is_empty() {
      Ordering::Equal
    } else if !self.pre.is_empty() && other.pre.is_empty() {
      Ordering::Less
    } else if self.pre.is_empty() && !other.pre.is_empty() {
      Ordering::Greater
    } else {
      let mut i = 0;
      loop {
        let a = self.pre.get(i);
        let b = other.pre.get(i);
        if a.is_none() && b.is_none() {
          return Ordering::Equal;
        }

        // https://github.com/npm/node-semver/blob/4907647d169948a53156502867ed679268063a9f/internal/identifiers.js
        let a = match a {
          Some(a) => a,
          None => return Ordering::Less,
        };
        let b = match b {
          Some(b) => b,
          None => return Ordering::Greater,
        };

        // prefer numbers
        if let Ok(a_num) = a.parse::<u64>() {
          if let Ok(b_num) = b.parse::<u64>() {
            return a_num.cmp(&b_num);
          } else {
            return Ordering::Less;
          }
        } else if b.parse::<u64>().is_ok() {
          return Ordering::Greater;
        }

        let cmp_result = a.cmp(b);
        if cmp_result != Ordering::Equal {
          return cmp_result;
        }
        i += 1;
      }
    }
  }
}

impl NpmVersion {
  pub fn parse(text: &str) -> Result<Self, AnyError> {
    let text = text.trim();
    with_failure_handling(parse_npm_version)(text)
      .with_context(|| format!("Invalid npm version '{}'.", text))
  }
}

fn parse_npm_version(input: &str) -> ParseResult<NpmVersion> {
  let (input, _) = maybe(ch('='))(input)?; // skip leading =
  let (input, _) = skip_whitespace(input)?;
  let (input, _) = maybe(ch('v'))(input)?; // skip leading v
  let (input, _) = skip_whitespace(input)?;
  let (input, major) = nr(input)?;
  let (input, _) = ch('.')(input)?;
  let (input, minor) = nr(input)?;
  let (input, _) = ch('.')(input)?;
  let (input, patch) = nr(input)?;
  let (input, q) = maybe(qualifier)(input)?;
  let q = q.unwrap_or_default();

  Ok((
    input,
    NpmVersion {
      major,
      minor,
      patch,
      pre: q.pre,
      build: q.build,
    },
  ))
}

/// A version requirement found in an npm package's dependencies.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NpmVersionReq {
  raw_text: String,
  range_set: VersionRangeSet,
}

impl NpmVersionMatcher for NpmVersionReq {
  fn tag(&self) -> Option<&str> {
    None
  }

  fn matches(&self, version: &NpmVersion) -> bool {
    self.satisfies(version)
  }

  fn version_text(&self) -> String {
    self.raw_text.clone()
  }
}

impl fmt::Display for NpmVersionReq {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", &self.raw_text)
  }
}

impl NpmVersionReq {
  pub fn parse(text: &str) -> Result<Self, AnyError> {
    let text = text.trim();
    with_failure_handling(parse_npm_version_req)(text)
      .with_context(|| format!("Invalid npm version requirement '{}'.", text))
  }

  pub fn satisfies(&self, version: &NpmVersion) -> bool {
    self.range_set.satisfies(version)
  }
}

fn parse_npm_version_req(input: &str) -> ParseResult<NpmVersionReq> {
  map(range_set, |set| NpmVersionReq {
    raw_text: input.to_string(),
    range_set: set,
  })(input)
}

// https://github.com/npm/node-semver/tree/4907647d169948a53156502867ed679268063a9f#range-grammar
// range-set  ::= range ( logical-or range ) *
// logical-or ::= ( ' ' ) * '||' ( ' ' ) *
// range      ::= hyphen | simple ( ' ' simple ) * | ''
// hyphen     ::= partial ' - ' partial
// simple     ::= primitive | partial | tilde | caret
// primitive  ::= ( '<' | '>' | '>=' | '<=' | '=' ) partial
// partial    ::= xr ( '.' xr ( '.' xr qualifier ? )? )?
// xr         ::= 'x' | 'X' | '*' | nr
// nr         ::= '0' | ['1'-'9'] ( ['0'-'9'] ) *
// tilde      ::= '~' partial
// caret      ::= '^' partial
// qualifier  ::= ( '-' pre )? ( '+' build )?
// pre        ::= parts
// build      ::= parts
// parts      ::= part ( '.' part ) *
// part       ::= nr | [-0-9A-Za-z]+

// range-set ::= range ( logical-or range ) *
fn range_set(input: &str) -> ParseResult<VersionRangeSet> {
  if input.is_empty() {
    return Ok((input, VersionRangeSet(vec![VersionRange::all()])));
  }
  map(if_not_empty(separated_list(range, logical_or)), |ranges| {
    // filter out the ranges that won't match anything for the tests
    VersionRangeSet(ranges.into_iter().filter(|r| !r.is_none()).collect())
  })(input)
}

// range ::= hyphen | simple ( ' ' simple ) * | ''
fn range(input: &str) -> ParseResult<VersionRange> {
  or(
    map(hyphen, |hyphen| VersionRange {
      start: hyphen.start.as_lower_bound(),
      end: hyphen.end.as_upper_bound(),
    }),
    map(separated_list(simple, whitespace), |ranges| {
      let mut final_range = VersionRange::all();
      for range in ranges {
        final_range = final_range.clamp(&range);
      }
      final_range
    }),
  )(input)
}

#[derive(Debug, Clone)]
struct Hyphen {
  start: Partial,
  end: Partial,
}

// hyphen ::= partial ' - ' partial
fn hyphen(input: &str) -> ParseResult<Hyphen> {
  let (input, first) = partial(input)?;
  let (input, _) = whitespace(input)?;
  let (input, _) = tag("-")(input)?;
  let (input, _) = whitespace(input)?;
  let (input, second) = partial(input)?;
  Ok((
    input,
    Hyphen {
      start: first,
      end: second,
    },
  ))
}

// logical-or ::= ( ' ' ) * '||' ( ' ' ) *
fn logical_or(input: &str) -> ParseResult<&str> {
  delimited(skip_whitespace, tag("||"), skip_whitespace)(input)
}

fn skip_whitespace_or_v(input: &str) -> ParseResult<()> {
  map(
    pair(skip_whitespace, pair(maybe(ch('v')), skip_whitespace)),
    |_| (),
  )(input)
}

// simple ::= primitive | partial | tilde | caret
fn simple(input: &str) -> ParseResult<VersionRange> {
  let tilde = pair(or(tag("~>"), tag("~")), skip_whitespace_or_v);
  or4(
    map(preceded(tilde, partial), |partial| {
      partial.as_tilde_version_range()
    }),
    map(
      preceded(pair(ch('^'), skip_whitespace_or_v), partial),
      |partial| partial.as_caret_version_range(),
    ),
    map(primitive, |primitive| {
      let partial = primitive.partial;
      match primitive.kind {
        PrimitiveKind::Equal => partial.as_equal_range(),
        PrimitiveKind::GreaterThan => {
          partial.as_greater_than(VersionBoundKind::Exclusive)
        }
        PrimitiveKind::GreaterThanOrEqual => {
          partial.as_greater_than(VersionBoundKind::Inclusive)
        }
        PrimitiveKind::LessThan => {
          partial.as_less_than(VersionBoundKind::Exclusive)
        }
        PrimitiveKind::LessThanOrEqual => {
          partial.as_less_than(VersionBoundKind::Inclusive)
        }
      }
    }),
    map(partial, |partial| partial.as_equal_range()),
  )(input)
}

#[derive(Debug, Clone, Copy)]
enum PrimitiveKind {
  GreaterThan,
  LessThan,
  GreaterThanOrEqual,
  LessThanOrEqual,
  Equal,
}

#[derive(Debug, Clone)]
struct Primitive {
  kind: PrimitiveKind,
  partial: Partial,
}

fn primitive(input: &str) -> ParseResult<Primitive> {
  let (input, kind) = primitive_kind(input)?;
  let (input, _) = skip_whitespace(input)?;
  let (input, partial) = partial(input)?;
  Ok((input, Primitive { kind, partial }))
}

fn primitive_kind(input: &str) -> ParseResult<PrimitiveKind> {
  or5(
    map(tag(">="), |_| PrimitiveKind::GreaterThanOrEqual),
    map(tag("<="), |_| PrimitiveKind::LessThanOrEqual),
    map(ch('<'), |_| PrimitiveKind::LessThan),
    map(ch('>'), |_| PrimitiveKind::GreaterThan),
    map(ch('='), |_| PrimitiveKind::Equal),
  )(input)
}

// partial ::= xr ( '.' xr ( '.' xr qualifier ? )? )?
fn partial(input: &str) -> ParseResult<Partial> {
  let (input, _) = maybe(ch('v'))(input)?; // skip leading v
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
  // we do loose parsing to support people doing stuff like 01.02.03
  let (input, result) =
    if_not_empty(substring(skip_while(|c| c.is_ascii_digit())))(input)?;
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
  preceded(maybe(ch('-')), parts)(input)
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
  use pretty_assertions::assert_eq;
  use std::cmp::Ordering;

  use super::*;

  struct NpmVersionReqTester(NpmVersionReq);

  impl NpmVersionReqTester {
    fn new(text: &str) -> Self {
      Self(NpmVersionReq::parse(text).unwrap())
    }

    fn matches(&self, version: &str) -> bool {
      self.0.matches(&NpmVersion::parse(version).unwrap())
    }
  }

  #[test]
  pub fn npm_version_req_with_v() {
    assert!(NpmVersionReq::parse("v1.0.0").is_ok());
  }

  #[test]
  pub fn npm_version_req_exact() {
    let tester = NpmVersionReqTester::new("2.1.2");
    assert!(!tester.matches("2.1.1"));
    assert!(tester.matches("2.1.2"));
    assert!(!tester.matches("2.1.3"));

    let tester = NpmVersionReqTester::new("2.1.2 || 2.1.5");
    assert!(!tester.matches("2.1.1"));
    assert!(tester.matches("2.1.2"));
    assert!(!tester.matches("2.1.3"));
    assert!(!tester.matches("2.1.4"));
    assert!(tester.matches("2.1.5"));
    assert!(!tester.matches("2.1.6"));
  }

  #[test]
  pub fn npm_version_req_minor() {
    let tester = NpmVersionReqTester::new("1.1");
    assert!(!tester.matches("1.0.0"));
    assert!(tester.matches("1.1.0"));
    assert!(tester.matches("1.1.1"));
    assert!(!tester.matches("1.2.0"));
    assert!(!tester.matches("1.2.1"));
  }

  #[test]
  pub fn npm_version_req_ranges() {
    let tester = NpmVersionReqTester::new(">= 2.1.2 < 3.0.0 || 5.x");
    assert!(!tester.matches("2.1.1"));
    assert!(tester.matches("2.1.2"));
    assert!(tester.matches("2.9.9"));
    assert!(!tester.matches("3.0.0"));
    assert!(tester.matches("5.0.0"));
    assert!(tester.matches("5.1.0"));
    assert!(!tester.matches("6.1.0"));
  }

  macro_rules! assert_cmp {
    ($a:expr, $b:expr, $expected:expr) => {
      assert_eq!(
        $a.cmp(&$b),
        $expected,
        "expected {} to be {:?} {}",
        $a,
        $expected,
        $b
      );
    };
  }

  macro_rules! test_compare {
    ($a:expr, $b:expr, $expected:expr) => {
      let a = NpmVersion::parse($a).unwrap();
      let b = NpmVersion::parse($b).unwrap();
      assert_cmp!(a, b, $expected);
    };
  }

  #[test]
  fn version_compare() {
    test_compare!("1.2.3", "2.3.4", Ordering::Less);
    test_compare!("1.2.3", "1.2.4", Ordering::Less);
    test_compare!("1.2.3", "1.2.3", Ordering::Equal);
    test_compare!("1.2.3", "1.2.2", Ordering::Greater);
    test_compare!("1.2.3", "1.1.5", Ordering::Greater);
  }

  #[test]
  fn version_compare_equal() {
    // https://github.com/npm/node-semver/blob/bce42589d33e1a99454530a8fd52c7178e2b11c1/test/fixtures/equality.js
    let fixtures = &[
      ("1.2.3", "v1.2.3"),
      ("1.2.3", "=1.2.3"),
      ("1.2.3", "v 1.2.3"),
      ("1.2.3", "= 1.2.3"),
      ("1.2.3", " v1.2.3"),
      ("1.2.3", " =1.2.3"),
      ("1.2.3", " v 1.2.3"),
      ("1.2.3", " = 1.2.3"),
      ("1.2.3-0", "v1.2.3-0"),
      ("1.2.3-0", "=1.2.3-0"),
      ("1.2.3-0", "v 1.2.3-0"),
      ("1.2.3-0", "= 1.2.3-0"),
      ("1.2.3-0", " v1.2.3-0"),
      ("1.2.3-0", " =1.2.3-0"),
      ("1.2.3-0", " v 1.2.3-0"),
      ("1.2.3-0", " = 1.2.3-0"),
      ("1.2.3-1", "v1.2.3-1"),
      ("1.2.3-1", "=1.2.3-1"),
      ("1.2.3-1", "v 1.2.3-1"),
      ("1.2.3-1", "= 1.2.3-1"),
      ("1.2.3-1", " v1.2.3-1"),
      ("1.2.3-1", " =1.2.3-1"),
      ("1.2.3-1", " v 1.2.3-1"),
      ("1.2.3-1", " = 1.2.3-1"),
      ("1.2.3-beta", "v1.2.3-beta"),
      ("1.2.3-beta", "=1.2.3-beta"),
      ("1.2.3-beta", "v 1.2.3-beta"),
      ("1.2.3-beta", "= 1.2.3-beta"),
      ("1.2.3-beta", " v1.2.3-beta"),
      ("1.2.3-beta", " =1.2.3-beta"),
      ("1.2.3-beta", " v 1.2.3-beta"),
      ("1.2.3-beta", " = 1.2.3-beta"),
      ("1.2.3-beta+build", " = 1.2.3-beta+otherbuild"),
      ("1.2.3+build", " = 1.2.3+otherbuild"),
      ("1.2.3-beta+build", "1.2.3-beta+otherbuild"),
      ("1.2.3+build", "1.2.3+otherbuild"),
      ("  v1.2.3+build", "1.2.3+otherbuild"),
    ];
    for (a, b) in fixtures {
      test_compare!(a, b, Ordering::Equal);
    }
  }

  #[test]
  fn version_comparisons_test() {
    // https://github.com/npm/node-semver/blob/bce42589d33e1a99454530a8fd52c7178e2b11c1/test/fixtures/comparisons.js
    let fixtures = &[
      ("0.0.0", "0.0.0-foo"),
      ("0.0.1", "0.0.0"),
      ("1.0.0", "0.9.9"),
      ("0.10.0", "0.9.0"),
      ("0.99.0", "0.10.0"),
      ("2.0.0", "1.2.3"),
      ("v0.0.0", "0.0.0-foo"),
      ("v0.0.1", "0.0.0"),
      ("v1.0.0", "0.9.9"),
      ("v0.10.0", "0.9.0"),
      ("v0.99.0", "0.10.0"),
      ("v2.0.0", "1.2.3"),
      ("0.0.0", "v0.0.0-foo"),
      ("0.0.1", "v0.0.0"),
      ("1.0.0", "v0.9.9"),
      ("0.10.0", "v0.9.0"),
      ("0.99.0", "v0.10.0"),
      ("2.0.0", "v1.2.3"),
      ("1.2.3", "1.2.3-asdf"),
      ("1.2.3", "1.2.3-4"),
      ("1.2.3", "1.2.3-4-foo"),
      ("1.2.3-5-foo", "1.2.3-5"),
      ("1.2.3-5", "1.2.3-4"),
      ("1.2.3-5-foo", "1.2.3-5-Foo"),
      ("3.0.0", "2.7.2+asdf"),
      ("1.2.3-a.10", "1.2.3-a.5"),
      ("1.2.3-a.b", "1.2.3-a.5"),
      ("1.2.3-a.b", "1.2.3-a"),
      ("1.2.3-a.b.c.10.d.5", "1.2.3-a.b.c.5.d.100"),
      ("1.2.3-r2", "1.2.3-r100"),
      ("1.2.3-r100", "1.2.3-R2"),
    ];
    for (a, b) in fixtures {
      let a = NpmVersion::parse(a).unwrap();
      let b = NpmVersion::parse(b).unwrap();
      assert_cmp!(a, b, Ordering::Greater);
      assert_cmp!(b, a, Ordering::Less);
      assert_cmp!(a, a, Ordering::Equal);
      assert_cmp!(b, b, Ordering::Equal);
    }
  }

  #[test]
  fn range_parse() {
    // https://github.com/npm/node-semver/blob/4907647d169948a53156502867ed679268063a9f/test/fixtures/range-parse.js
    let fixtures = &[
      ("1.0.0 - 2.0.0", ">=1.0.0 <=2.0.0"),
      ("1 - 2", ">=1.0.0 <3.0.0-0"),
      ("1.0 - 2.0", ">=1.0.0 <2.1.0-0"),
      ("1.0.0", "1.0.0"),
      (">=*", "*"),
      ("", "*"),
      ("*", "*"),
      ("*", "*"),
      (">=1.0.0", ">=1.0.0"),
      (">1.0.0", ">1.0.0"),
      ("<=2.0.0", "<=2.0.0"),
      ("1", ">=1.0.0 <2.0.0-0"),
      ("<=2.0.0", "<=2.0.0"),
      ("<=2.0.0", "<=2.0.0"),
      ("<2.0.0", "<2.0.0"),
      ("<2.0.0", "<2.0.0"),
      (">= 1.0.0", ">=1.0.0"),
      (">=  1.0.0", ">=1.0.0"),
      (">=   1.0.0", ">=1.0.0"),
      ("> 1.0.0", ">1.0.0"),
      (">  1.0.0", ">1.0.0"),
      ("<=   2.0.0", "<=2.0.0"),
      ("<= 2.0.0", "<=2.0.0"),
      ("<=  2.0.0", "<=2.0.0"),
      ("<    2.0.0", "<2.0.0"),
      ("<\t2.0.0", "<2.0.0"),
      (">=0.1.97", ">=0.1.97"),
      (">=0.1.97", ">=0.1.97"),
      ("0.1.20 || 1.2.4", "0.1.20||1.2.4"),
      (">=0.2.3 || <0.0.1", ">=0.2.3||<0.0.1"),
      (">=0.2.3 || <0.0.1", ">=0.2.3||<0.0.1"),
      (">=0.2.3 || <0.0.1", ">=0.2.3||<0.0.1"),
      ("||", "*"),
      ("2.x.x", ">=2.0.0 <3.0.0-0"),
      ("1.2.x", ">=1.2.0 <1.3.0-0"),
      ("1.2.x || 2.x", ">=1.2.0 <1.3.0-0||>=2.0.0 <3.0.0-0"),
      ("1.2.x || 2.x", ">=1.2.0 <1.3.0-0||>=2.0.0 <3.0.0-0"),
      ("x", "*"),
      ("2.*.*", ">=2.0.0 <3.0.0-0"),
      ("1.2.*", ">=1.2.0 <1.3.0-0"),
      ("1.2.* || 2.*", ">=1.2.0 <1.3.0-0||>=2.0.0 <3.0.0-0"),
      ("*", "*"),
      ("2", ">=2.0.0 <3.0.0-0"),
      ("2.3", ">=2.3.0 <2.4.0-0"),
      ("~2.4", ">=2.4.0 <2.5.0-0"),
      ("~2.4", ">=2.4.0 <2.5.0-0"),
      ("~>3.2.1", ">=3.2.1 <3.3.0-0"),
      ("~1", ">=1.0.0 <2.0.0-0"),
      ("~>1", ">=1.0.0 <2.0.0-0"),
      ("~> 1", ">=1.0.0 <2.0.0-0"),
      ("~1.0", ">=1.0.0 <1.1.0-0"),
      ("~ 1.0", ">=1.0.0 <1.1.0-0"),
      ("^0", "<1.0.0-0"),
      ("^ 1", ">=1.0.0 <2.0.0-0"),
      ("^0.1", ">=0.1.0 <0.2.0-0"),
      ("^1.0", ">=1.0.0 <2.0.0-0"),
      ("^1.2", ">=1.2.0 <2.0.0-0"),
      ("^0.0.1", ">=0.0.1 <0.0.2-0"),
      ("^0.0.1-beta", ">=0.0.1-beta <0.0.2-0"),
      ("^0.1.2", ">=0.1.2 <0.2.0-0"),
      ("^1.2.3", ">=1.2.3 <2.0.0-0"),
      ("^1.2.3-beta.4", ">=1.2.3-beta.4 <2.0.0-0"),
      ("<1", "<1.0.0-0"),
      ("< 1", "<1.0.0-0"),
      (">=1", ">=1.0.0"),
      (">= 1", ">=1.0.0"),
      ("<1.2", "<1.2.0-0"),
      ("< 1.2", "<1.2.0-0"),
      ("1", ">=1.0.0 <2.0.0-0"),
      ("^ 1.2 ^ 1", ">=1.2.0 <2.0.0-0 >=1.0.0"),
      ("1.2 - 3.4.5", ">=1.2.0 <=3.4.5"),
      ("1.2.3 - 3.4", ">=1.2.3 <3.5.0-0"),
      ("1.2 - 3.4", ">=1.2.0 <3.5.0-0"),
      (">1", ">=2.0.0"),
      (">1.2", ">=1.3.0"),
      (">X", "<0.0.0-0"),
      ("<X", "<0.0.0-0"),
      ("<x <* || >* 2.x", "<0.0.0-0"),
      (">x 2.x || * || <x", "*"),
      (">01.02.03", ">1.2.3"),
      ("~1.2.3beta", ">=1.2.3-beta <1.3.0-0"),
      (">=09090", ">=9090.0.0"),
    ];
    for (range_text, expected) in fixtures {
      let range = NpmVersionReq::parse(range_text).unwrap();
      let expected_range = NpmVersionReq::parse(expected).unwrap();
      assert_eq!(
        range.range_set, expected_range.range_set,
        "failed for {} and {}",
        range_text, expected
      );
    }
  }

  #[test]
  fn range_satisfies() {
    // https://github.com/npm/node-semver/blob/4907647d169948a53156502867ed679268063a9f/test/fixtures/range-include.js
    let fixtures = &[
      ("1.0.0 - 2.0.0", "1.2.3"),
      ("^1.2.3+build", "1.2.3"),
      ("^1.2.3+build", "1.3.0"),
      ("1.2.3-pre+asdf - 2.4.3-pre+asdf", "1.2.3"),
      ("1.2.3pre+asdf - 2.4.3-pre+asdf", "1.2.3"),
      ("1.2.3-pre+asdf - 2.4.3pre+asdf", "1.2.3"),
      ("1.2.3pre+asdf - 2.4.3pre+asdf", "1.2.3"),
      ("1.2.3-pre+asdf - 2.4.3-pre+asdf", "1.2.3-pre.2"),
      ("1.2.3-pre+asdf - 2.4.3-pre+asdf", "2.4.3-alpha"),
      ("1.2.3+asdf - 2.4.3+asdf", "1.2.3"),
      ("1.0.0", "1.0.0"),
      (">=*", "0.2.4"),
      ("", "1.0.0"),
      ("*", "1.2.3"),
      ("*", "v1.2.3"),
      (">=1.0.0", "1.0.0"),
      (">=1.0.0", "1.0.1"),
      (">=1.0.0", "1.1.0"),
      (">1.0.0", "1.0.1"),
      (">1.0.0", "1.1.0"),
      ("<=2.0.0", "2.0.0"),
      ("<=2.0.0", "1.9999.9999"),
      ("<=2.0.0", "0.2.9"),
      ("<2.0.0", "1.9999.9999"),
      ("<2.0.0", "0.2.9"),
      (">= 1.0.0", "1.0.0"),
      (">=  1.0.0", "1.0.1"),
      (">=   1.0.0", "1.1.0"),
      ("> 1.0.0", "1.0.1"),
      (">  1.0.0", "1.1.0"),
      ("<=   2.0.0", "2.0.0"),
      ("<= 2.0.0", "1.9999.9999"),
      ("<=  2.0.0", "0.2.9"),
      ("<    2.0.0", "1.9999.9999"),
      ("<\t2.0.0", "0.2.9"),
      (">=0.1.97", "v0.1.97"),
      (">=0.1.97", "0.1.97"),
      ("0.1.20 || 1.2.4", "1.2.4"),
      (">=0.2.3 || <0.0.1", "0.0.0"),
      (">=0.2.3 || <0.0.1", "0.2.3"),
      (">=0.2.3 || <0.0.1", "0.2.4"),
      ("||", "1.3.4"),
      ("2.x.x", "2.1.3"),
      ("1.2.x", "1.2.3"),
      ("1.2.x || 2.x", "2.1.3"),
      ("1.2.x || 2.x", "1.2.3"),
      ("x", "1.2.3"),
      ("2.*.*", "2.1.3"),
      ("1.2.*", "1.2.3"),
      ("1.2.* || 2.*", "2.1.3"),
      ("1.2.* || 2.*", "1.2.3"),
      ("*", "1.2.3"),
      ("2", "2.1.2"),
      ("2.3", "2.3.1"),
      ("~0.0.1", "0.0.1"),
      ("~0.0.1", "0.0.2"),
      ("~x", "0.0.9"),   // >=2.4.0 <2.5.0
      ("~2", "2.0.9"),   // >=2.4.0 <2.5.0
      ("~2.4", "2.4.0"), // >=2.4.0 <2.5.0
      ("~2.4", "2.4.5"),
      ("~>3.2.1", "3.2.2"), // >=3.2.1 <3.3.0,
      ("~1", "1.2.3"),      // >=1.0.0 <2.0.0
      ("~>1", "1.2.3"),
      ("~> 1", "1.2.3"),
      ("~1.0", "1.0.2"), // >=1.0.0 <1.1.0,
      ("~ 1.0", "1.0.2"),
      ("~ 1.0.3", "1.0.12"),
      ("~ 1.0.3alpha", "1.0.12"),
      (">=1", "1.0.0"),
      (">= 1", "1.0.0"),
      ("<1.2", "1.1.1"),
      ("< 1.2", "1.1.1"),
      ("~v0.5.4-pre", "0.5.5"),
      ("~v0.5.4-pre", "0.5.4"),
      ("=0.7.x", "0.7.2"),
      ("<=0.7.x", "0.7.2"),
      (">=0.7.x", "0.7.2"),
      ("<=0.7.x", "0.6.2"),
      ("~1.2.1 >=1.2.3", "1.2.3"),
      ("~1.2.1 =1.2.3", "1.2.3"),
      ("~1.2.1 1.2.3", "1.2.3"),
      ("~1.2.1 >=1.2.3 1.2.3", "1.2.3"),
      ("~1.2.1 1.2.3 >=1.2.3", "1.2.3"),
      ("~1.2.1 1.2.3", "1.2.3"),
      (">=1.2.1 1.2.3", "1.2.3"),
      ("1.2.3 >=1.2.1", "1.2.3"),
      (">=1.2.3 >=1.2.1", "1.2.3"),
      (">=1.2.1 >=1.2.3", "1.2.3"),
      (">=1.2", "1.2.8"),
      ("^1.2.3", "1.8.1"),
      ("^0.1.2", "0.1.2"),
      ("^0.1", "0.1.2"),
      ("^0.0.1", "0.0.1"),
      ("^1.2", "1.4.2"),
      ("^1.2 ^1", "1.4.2"),
      ("^1.2.3-alpha", "1.2.3-pre"),
      ("^1.2.0-alpha", "1.2.0-pre"),
      ("^0.0.1-alpha", "0.0.1-beta"),
      ("^0.0.1-alpha", "0.0.1"),
      ("^0.1.1-alpha", "0.1.1-beta"),
      ("^x", "1.2.3"),
      ("x - 1.0.0", "0.9.7"),
      ("x - 1.x", "0.9.7"),
      ("1.0.0 - x", "1.9.7"),
      ("1.x - x", "1.9.7"),
      ("<=7.x", "7.9.9"),
      // additional tests
      ("1.0.0-alpha.13", "1.0.0-alpha.13"),
    ];
    for (req_text, version_text) in fixtures {
      let req = NpmVersionReq::parse(req_text).unwrap();
      let version = NpmVersion::parse(version_text).unwrap();
      assert!(
        req.satisfies(&version),
        "Checking {} satisfies {}",
        req_text,
        version_text
      );
    }
  }

  #[test]
  fn range_not_satisfies() {
    let fixtures = &[
      ("1.0.0 - 2.0.0", "2.2.3"),
      ("1.2.3+asdf - 2.4.3+asdf", "1.2.3-pre.2"),
      ("1.2.3+asdf - 2.4.3+asdf", "2.4.3-alpha"),
      ("^1.2.3+build", "2.0.0"),
      ("^1.2.3+build", "1.2.0"),
      ("^1.2.3", "1.2.3-pre"),
      ("^1.2", "1.2.0-pre"),
      (">1.2", "1.3.0-beta"),
      ("<=1.2.3", "1.2.3-beta"),
      ("^1.2.3", "1.2.3-beta"),
      ("=0.7.x", "0.7.0-asdf"),
      (">=0.7.x", "0.7.0-asdf"),
      ("<=0.7.x", "0.7.0-asdf"),
      ("1", "1.0.0beta"),
      ("<1", "1.0.0beta"),
      ("< 1", "1.0.0beta"),
      ("1.0.0", "1.0.1"),
      (">=1.0.0", "0.0.0"),
      (">=1.0.0", "0.0.1"),
      (">=1.0.0", "0.1.0"),
      (">1.0.0", "0.0.1"),
      (">1.0.0", "0.1.0"),
      ("<=2.0.0", "3.0.0"),
      ("<=2.0.0", "2.9999.9999"),
      ("<=2.0.0", "2.2.9"),
      ("<2.0.0", "2.9999.9999"),
      ("<2.0.0", "2.2.9"),
      (">=0.1.97", "v0.1.93"),
      (">=0.1.97", "0.1.93"),
      ("0.1.20 || 1.2.4", "1.2.3"),
      (">=0.2.3 || <0.0.1", "0.0.3"),
      (">=0.2.3 || <0.0.1", "0.2.2"),
      ("2.x.x", "1.1.3"),
      ("2.x.x", "3.1.3"),
      ("1.2.x", "1.3.3"),
      ("1.2.x || 2.x", "3.1.3"),
      ("1.2.x || 2.x", "1.1.3"),
      ("2.*.*", "1.1.3"),
      ("2.*.*", "3.1.3"),
      ("1.2.*", "1.3.3"),
      ("1.2.* || 2.*", "3.1.3"),
      ("1.2.* || 2.*", "1.1.3"),
      ("2", "1.1.2"),
      ("2.3", "2.4.1"),
      ("~0.0.1", "0.1.0-alpha"),
      ("~0.0.1", "0.1.0"),
      ("~2.4", "2.5.0"), // >=2.4.0 <2.5.0
      ("~2.4", "2.3.9"),
      ("~>3.2.1", "3.3.2"), // >=3.2.1 <3.3.0
      ("~>3.2.1", "3.2.0"), // >=3.2.1 <3.3.0
      ("~1", "0.2.3"),      // >=1.0.0 <2.0.0
      ("~>1", "2.2.3"),
      ("~1.0", "1.1.0"), // >=1.0.0 <1.1.0
      ("<1", "1.0.0"),
      (">=1.2", "1.1.1"),
      ("1", "2.0.0beta"),
      ("~v0.5.4-beta", "0.5.4-alpha"),
      ("=0.7.x", "0.8.2"),
      (">=0.7.x", "0.6.2"),
      ("<0.7.x", "0.7.2"),
      ("<1.2.3", "1.2.3-beta"),
      ("=1.2.3", "1.2.3-beta"),
      (">1.2", "1.2.8"),
      ("^0.0.1", "0.0.2-alpha"),
      ("^0.0.1", "0.0.2"),
      ("^1.2.3", "2.0.0-alpha"),
      ("^1.2.3", "1.2.2"),
      ("^1.2", "1.1.9"),
      ("*", "v1.2.3-foo"),
      ("^1.0.0", "2.0.0-rc1"),
      ("1 - 2", "2.0.0-pre"),
      ("1 - 2", "1.0.0-pre"),
      ("1.0 - 2", "1.0.0-pre"),
      ("1.1.x", "1.0.0-a"),
      ("1.1.x", "1.1.0-a"),
      ("1.1.x", "1.2.0-a"),
      ("1.x", "1.0.0-a"),
      ("1.x", "1.1.0-a"),
      ("1.x", "1.2.0-a"),
      (">=1.0.0 <1.1.0", "1.1.0"),
      (">=1.0.0 <1.1.0", "1.1.0-pre"),
      (">=1.0.0 <1.1.0-pre", "1.1.0-pre"),
    ];

    for (req_text, version_text) in fixtures {
      let req = NpmVersionReq::parse(req_text).unwrap();
      let version = NpmVersion::parse(version_text).unwrap();
      assert!(
        !req.satisfies(&version),
        "Checking {} not satisfies {}",
        req_text,
        version_text
      );
    }
  }
}
