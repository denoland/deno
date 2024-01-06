// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::normalize_path;

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct PathOrPatternSet(Vec<PathOrPattern>);

impl PathOrPatternSet {
  pub fn new(elements: Vec<PathOrPattern>) -> Self {
    Self(elements)
  }

  pub fn from_absolute_paths(path: Vec<PathBuf>) -> Result<Self, AnyError> {
    Ok(Self(
      path
        .into_iter()
        .map(PathOrPattern::new)
        .collect::<Result<Vec<_>, _>>()?,
    ))
  }

  pub fn into_path_or_patterns(self) -> Vec<PathOrPattern> {
    self.0
  }

  pub fn matches_path(&self, path: &PathBuf) -> bool {
    self.0.iter().any(|p| p.matches_path(path))
  }

  pub fn base_paths(&self) -> Vec<PathBuf> {
    let mut result = Vec::with_capacity(self.0.len());
    for element in &self.0 {
      match element {
        PathOrPattern::Path(path) => {
          result.push(path.to_path_buf());
        }
        PathOrPattern::Pattern(pattern) => {
          result.push(pattern.base_path());
        }
      }
    }
    result
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PathOrPattern {
  Path(PathBuf),
  Pattern(GlobPattern),
}

impl PathOrPattern {
  pub fn new(path: PathBuf) -> Result<Self, AnyError> {
    GlobPattern::new_if_pattern(&path.to_string_lossy()).map(|maybe_pattern| {
      maybe_pattern
        .map(PathOrPattern::Pattern)
        .unwrap_or_else(|| PathOrPattern::Path(normalize_path(path)))
    })
  }

  pub fn matches_path(&self, path: &Path) -> bool {
    match self {
      PathOrPattern::Path(p) => path.starts_with(p),
      PathOrPattern::Pattern(p) => p.matches_path(path),
    }
  }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GlobPattern(glob::Pattern);

impl GlobPattern {
  pub fn new_if_pattern(pattern: &str) -> Result<Option<Self>, AnyError> {
    if !is_glob_pattern(pattern) {
      return Ok(None);
    }
    Self::new(pattern).map(Some)
  }

  pub fn new(pattern: &str) -> Result<Self, AnyError> {
    let pattern =
      glob::Pattern::new(&escape_brackets(pattern).replace('\\', "/"))
        .with_context(|| format!("Failed to expand glob: \"{}\"", pattern))?;
    Ok(Self(pattern))
  }

  pub fn matches_path(&self, path: &Path) -> bool {
    self.0.matches_path_with(path, match_options())
  }

  pub fn base_path(&self) -> PathBuf {
    let base_path = self
      .0
      .as_str()
      .split('/')
      .take_while(|c| !has_glob_chars(c))
      .collect::<Vec<_>>()
      .join(&std::path::MAIN_SEPARATOR.to_string());
    PathBuf::from(base_path)
  }
}

pub fn is_glob_pattern(path: &str) -> bool {
  !path.starts_with("http:")
    && !path.starts_with("https:")
    && !path.starts_with("file:")
    && has_glob_chars(path)
}

fn has_glob_chars(pattern: &str) -> bool {
  // we don't support [ and ]
  pattern.chars().any(|c| matches!(c, '*' | '?'))
}

fn escape_brackets(pattern: &str) -> String {
  // Escape brackets - we currently don't support them, because with introduction
  // of glob expansion paths like "pages/[id].ts" would suddenly start giving
  // wrong results. We might want to revisit that in the future.
  pattern.replace('[', "[[]").replace(']', "[]]")
}

fn match_options() -> glob::MatchOptions {
  // Matches what `deno_task_shell` does
  glob::MatchOptions {
    // false because it should work the same way on case insensitive file systems
    case_sensitive: false,
    // true because it copies what sh does
    require_literal_separator: true,
    // true because it copies with sh does—these files are considered "hidden"
    require_literal_leading_dot: true,
  }
}
