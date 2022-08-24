// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use once_cell::sync::Lazy;
use regex::Regex;

use super::resolution::NpmVersionMatcher;

static MINOR_SPECIFIER_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r#"^[0-9]+\.[0-9]+$"#).unwrap());

/// Version requirement found in npm specifiers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SpecifierVersionReq(semver::VersionReq);

impl std::fmt::Display for SpecifierVersionReq {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl SpecifierVersionReq {
  // in order to keep using semver, we do some pre-processing to change the behavior
  pub fn parse(text: &str) -> Result<Self, AnyError> {
    // for now, we don't support these scenarios
    if text.contains("||") {
      bail!("not supported '||'");
    }
    if text.contains(',') {
      bail!("not supported ','");
    }
    // force exact versions to be matched exactly
    let text = if semver::Version::parse(text).is_ok() {
      Cow::Owned(format!("={}", text))
    } else {
      Cow::Borrowed(text)
    };
    // force requirements like 1.2 to be ~1.2 instead of ^1.2
    let text = if MINOR_SPECIFIER_RE.is_match(&text) {
      Cow::Owned(format!("~{}", text))
    } else {
      text
    };
    Ok(Self(semver::VersionReq::parse(&text)?))
  }

  pub fn matches(&self, version: &semver::Version) -> bool {
    self.0.matches(version)
  }
}

/// A version requirement found in an npm package's dependencies.
pub struct NpmVersionReq {
  raw_text: String,
  comparators: Vec<semver::VersionReq>,
}

impl NpmVersionReq {
  pub fn parse(text: &str) -> Result<NpmVersionReq, AnyError> {
    // semver::VersionReq doesn't support spaces between comparators
    // and it doesn't support using || for "OR", so we pre-process
    // the version requirement in order to make this work.
    let raw_text = text.to_string();
    let part_texts = text.split("||").collect::<Vec<_>>();
    let mut comparators = Vec::with_capacity(part_texts.len());
    for part in part_texts {
      comparators.push(npm_version_req_parse_part(part)?);
    }
    Ok(NpmVersionReq {
      raw_text,
      comparators,
    })
  }
}

impl NpmVersionMatcher for NpmVersionReq {
  fn matches(&self, version: &semver::Version) -> bool {
    self.comparators.iter().any(|c| c.matches(version))
  }

  fn version_text(&self) -> String {
    self.raw_text.to_string()
  }
}

fn npm_version_req_parse_part(
  text: &str,
) -> Result<semver::VersionReq, AnyError> {
  let text = text.trim();
  let text = text.strip_prefix('v').unwrap_or(text);
  // force exact versions to be matched exactly
  let text = if semver::Version::parse(text).is_ok() {
    Cow::Owned(format!("={}", text))
  } else {
    Cow::Borrowed(text)
  };
  // force requirements like 1.2 to be ~1.2 instead of ^1.2
  let text = if MINOR_SPECIFIER_RE.is_match(&text) {
    Cow::Owned(format!("~{}", text))
  } else {
    text
  };
  let mut chars = text.chars().enumerate().peekable();
  let mut final_text = String::new();
  while chars.peek().is_some() {
    let (i, c) = chars.next().unwrap();
    let is_greater_or_less_than = c == '<' || c == '>';
    if is_greater_or_less_than || c == '=' {
      if i > 0 {
        final_text = final_text.trim().to_string();
        // add a comma to make semver::VersionReq parse this
        final_text.push(',');
      }
      final_text.push(c);
      let next_char = chars.peek().map(|(_, c)| c);
      if is_greater_or_less_than && matches!(next_char, Some('=')) {
        let c = chars.next().unwrap().1; // skip
        final_text.push(c);
      }
    } else {
      final_text.push(c);
    }
  }
  Ok(semver::VersionReq::parse(&final_text)?)
}

#[cfg(test)]
mod tests {
  use super::*;

  struct VersionReqTester(SpecifierVersionReq);

  impl VersionReqTester {
    fn new(text: &str) -> Self {
      Self(SpecifierVersionReq::parse(text).unwrap())
    }

    fn matches(&self, version: &str) -> bool {
      self.0.matches(&semver::Version::parse(version).unwrap())
    }
  }

  #[test]
  fn version_req_exact() {
    let tester = VersionReqTester::new("1.0.1");
    assert!(!tester.matches("1.0.0"));
    assert!(tester.matches("1.0.1"));
    assert!(!tester.matches("1.0.2"));
    assert!(!tester.matches("1.1.1"));
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

  struct NpmVersionReqTester(NpmVersionReq);

  impl NpmVersionReqTester {
    fn new(text: &str) -> Self {
      Self(NpmVersionReq::parse(text).unwrap())
    }

    fn matches(&self, version: &str) -> bool {
      self.0.matches(&semver::Version::parse(version).unwrap())
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
}
