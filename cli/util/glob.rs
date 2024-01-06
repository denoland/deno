// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::normalize_path;
use deno_core::url::Url;

use super::path::specifier_to_file_path;

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct FilePatterns {
  pub include: Option<PathOrPatternSet>,
  pub exclude: PathOrPatternSet,
}

impl FilePatterns {
  pub fn matches_specifier(&self, specifier: &Url) -> bool {
    let path = match specifier_to_file_path(specifier) {
      Ok(path) => path,
      Err(_) => return true,
    };
    self.matches_path(&path)
  }

  pub fn matches_path(&self, path: &Path) -> bool {
    // Skip files which is in the exclude list.
    if self.exclude.matches_path(path) {
      // Allow someone to override an exclude by providing an exact match
      if let Some(set) = &self.include {
        for pattern in &set.0 {
          if let PathOrPattern::Path(p) = pattern {
            if p == path {
              return true;
            }
          }
        }
      }
      return false;
    }

    // Ignore files not in the include list if it's present.
    self
      .include
      .as_ref()
      .map(|m| m.matches_path(path))
      .unwrap_or(true)
  }

  /// Creates a collection of `FilePatterns` by base where the containing patterns
  /// are only the ones applicable to the base.
  ///
  /// The order these are returned in is the order that the directory traversal
  /// should occur in.
  pub fn split_by_base(self) -> Vec<(PathBuf, Self)> {
    let Some(include) = self.include else {
      return Vec::new();
    };

    let include_by_base_path = include
      .0
      .into_iter()
      .map(|s| (s.base_path(), s))
      .collect::<Vec<_>>();
    let exclude_by_base_path = self
      .exclude
      .0
      .into_iter()
      .map(|s| (s.base_path(), s))
      .collect::<Vec<_>>();

    // todo(dsherret): This could be further optimized by not including
    // patterns that will only ever match another base.
    let mut result = Vec::with_capacity(include_by_base_path.len());
    for (base_path, include) in include_by_base_path {
      let applicable_excludes = exclude_by_base_path
        .iter()
        .filter_map(|(exclude_base_path, exclude)| {
          // only include paths that are sub paths or any globs that's a sub path or parent path
          match exclude {
            PathOrPattern::Path(exclude_path) => {
              if exclude_path.starts_with(&base_path) {
                Some(exclude.clone())
              } else {
                None
              }
            }
            PathOrPattern::Pattern(_) => {
              if exclude_base_path.starts_with(&base_path)
                || base_path.starts_with(exclude_base_path)
              {
                Some(exclude.clone())
              } else {
                None
              }
            }
          }
        })
        .collect::<Vec<_>>();
      result.push((
        base_path,
        Self {
          include: Some(PathOrPatternSet::new(vec![include])),
          exclude: PathOrPatternSet::new(applicable_excludes),
        },
      ));
    }

    // Sort by the longest base path first. This ensures that we visit opted into
    // nested directories first before visiting the parent directory. The directory
    // traverser will handle not going into directories it's already been in.
    result.sort_by(|a, b| {
      b.0
        .to_string_lossy()
        .len()
        .cmp(&a.0.to_string_lossy().len())
    });

    result
  }
}

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

  pub fn matches_path(&self, path: &Path) -> bool {
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
    let path_str = path.to_string_lossy();
    // todo(dsherret): don't store URLs in PathBufs
    if path_str.starts_with("http:")
      || path_str.starts_with("https:")
      || path_str.starts_with("file:")
    {
      return Ok(Self::Path(path));
    }

    GlobPattern::new_if_pattern(&path_str).map(|maybe_pattern| {
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

  pub fn base_path(&self) -> PathBuf {
    match self {
      PathOrPattern::Path(p) => p.clone(),
      PathOrPattern::Pattern(p) => p.base_path(),
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
      .join(std::path::MAIN_SEPARATOR_STR);
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
