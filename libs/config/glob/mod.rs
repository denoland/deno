// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use deno_error::JsError;
use deno_path_util::normalize_path;
use deno_path_util::url_to_file_path;
use indexmap::IndexMap;
use thiserror::Error;
use url::Url;

use crate::UrlToFilePathError;

mod collector;
mod gitignore;

pub use collector::FileCollector;
pub use collector::WalkEntry;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FilePatternsMatch {
  /// File passes as matching, but further exclude matching (ex. .gitignore)
  /// may be necessary.
  Passed,
  /// File passes matching and further exclude matching (ex. .gitignore)
  /// should NOT be done.
  PassedOptedOutExclude,
  /// File was excluded.
  Excluded,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PathKind {
  File,
  Directory,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FilePatterns {
  /// Default traversal base used when calling `split_by_base()` without
  /// any `include` patterns.
  pub base: PathBuf,
  pub include: Option<PathOrPatternSet>,
  pub exclude: PathOrPatternSet,
}

impl FilePatterns {
  pub fn new_with_base(base: PathBuf) -> Self {
    Self {
      base,
      include: Default::default(),
      exclude: Default::default(),
    }
  }

  pub fn with_new_base(self, new_base: PathBuf) -> Self {
    Self {
      base: new_base,
      ..self
    }
  }

  pub fn matches_specifier(&self, specifier: &Url) -> bool {
    self.matches_specifier_detail(specifier) != FilePatternsMatch::Excluded
  }

  pub fn matches_specifier_detail(&self, specifier: &Url) -> FilePatternsMatch {
    if specifier.scheme() != "file" {
      // can't do .gitignore on a non-file specifier
      return FilePatternsMatch::PassedOptedOutExclude;
    }
    let path = match url_to_file_path(specifier) {
      Ok(path) => path,
      Err(_) => return FilePatternsMatch::PassedOptedOutExclude,
    };
    self.matches_path_detail(&path, PathKind::File) // use file matching behavior
  }

  pub fn matches_path(&self, path: &Path, path_kind: PathKind) -> bool {
    self.matches_path_detail(path, path_kind) != FilePatternsMatch::Excluded
  }

  pub fn matches_path_detail(
    &self,
    path: &Path,
    path_kind: PathKind,
  ) -> FilePatternsMatch {
    // if there's an include list, only include files that match it
    // the include list is a closed set
    if let Some(include) = &self.include {
      match path_kind {
        PathKind::File => {
          if include.matches_path_detail(path) != PathOrPatternsMatch::Matched {
            return FilePatternsMatch::Excluded;
          }
        }
        PathKind::Directory => {
          // for now ignore the include list unless there's a negated
          // glob for the directory
          for p in include.0.iter().rev() {
            match p.matches_path(path) {
              PathGlobMatch::Matched => {
                break;
              }
              PathGlobMatch::MatchedNegated => {
                return FilePatternsMatch::Excluded;
              }
              PathGlobMatch::NotMatched => {
                // keep going
              }
            }
          }
        }
      }
    }

    // the exclude list is an open set and we skip files not in the exclude list
    match self.exclude.matches_path_detail(path) {
      PathOrPatternsMatch::Matched => FilePatternsMatch::Excluded,
      PathOrPatternsMatch::NotMatched => FilePatternsMatch::Passed,
      PathOrPatternsMatch::Excluded => FilePatternsMatch::PassedOptedOutExclude,
    }
  }

  /// Creates a collection of `FilePatterns` where the containing patterns
  /// are only the ones applicable to the base.
  ///
  /// The order these are returned in is the order that the directory traversal
  /// should occur in.
  pub fn split_by_base(&self) -> Vec<Self> {
    let negated_excludes = self
      .exclude
      .0
      .iter()
      .filter(|e| e.is_negated())
      .collect::<Vec<_>>();
    let include = match &self.include {
      Some(include) => Cow::Borrowed(include),
      None => {
        if negated_excludes.is_empty() {
          return vec![self.clone()];
        } else {
          Cow::Owned(PathOrPatternSet::new(vec![PathOrPattern::Path(
            self.base.clone(),
          )]))
        }
      }
    };

    let mut include_paths = Vec::with_capacity(include.0.len());
    let mut include_patterns = Vec::with_capacity(include.0.len());
    let mut exclude_patterns =
      Vec::with_capacity(include.0.len() + self.exclude.0.len());

    for path_or_pattern in &include.0 {
      match path_or_pattern {
        PathOrPattern::Path(path) => include_paths.push(path),
        PathOrPattern::NegatedPath(path) => {
          exclude_patterns.push(PathOrPattern::Path(path.clone()));
        }
        PathOrPattern::Pattern(pattern) => {
          if pattern.is_negated() {
            exclude_patterns.push(PathOrPattern::Pattern(pattern.as_negated()));
          } else {
            include_patterns.push(pattern.clone());
          }
        }
        PathOrPattern::RemoteUrl(_) => {}
      }
    }

    let capacity = include_patterns.len() + negated_excludes.len();
    let mut include_patterns_by_base_path = include_patterns.into_iter().fold(
      IndexMap::with_capacity(capacity),
      |mut map: IndexMap<_, Vec<_>>, p| {
        map.entry(p.base_path()).or_default().push(p);
        map
      },
    );
    for p in &negated_excludes {
      if let Some(base_path) = p.base_path()
        && !include_patterns_by_base_path.contains_key(&base_path)
      {
        let has_any_base_parent = include_patterns_by_base_path
          .keys()
          .any(|k| base_path.starts_with(k))
          || include_paths.iter().any(|p| base_path.starts_with(p));
        // don't include an orphaned negated pattern
        if has_any_base_parent {
          include_patterns_by_base_path.insert(base_path, Vec::new());
        }
      }
    }

    let exclude_by_base_path = exclude_patterns
      .iter()
      .chain(self.exclude.0.iter())
      .filter_map(|s| Some((s.base_path()?, s)))
      .collect::<Vec<_>>();
    let get_applicable_excludes = |base_path: &PathBuf| -> Vec<PathOrPattern> {
      exclude_by_base_path
        .iter()
        .filter_map(|(exclude_base_path, exclude)| {
          match exclude {
            PathOrPattern::RemoteUrl(_) => None,
            PathOrPattern::Path(exclude_path)
            | PathOrPattern::NegatedPath(exclude_path) => {
              // include paths that's are sub paths or an ancestor path
              if base_path.starts_with(exclude_path)
                || exclude_path.starts_with(base_path)
              {
                Some((*exclude).clone())
              } else {
                None
              }
            }
            PathOrPattern::Pattern(_) => {
              // include globs that's are sub paths or an ancestor path
              if exclude_base_path.starts_with(base_path)
                || base_path.starts_with(exclude_base_path)
              {
                Some((*exclude).clone())
              } else {
                None
              }
            }
          }
        })
        .collect::<Vec<_>>()
    };

    let mut result = Vec::with_capacity(
      include_paths.len() + include_patterns_by_base_path.len(),
    );
    for path in include_paths {
      let applicable_excludes = get_applicable_excludes(path);
      result.push(Self {
        base: path.clone(),
        include: if self.include.is_none() {
          None
        } else {
          Some(PathOrPatternSet::new(vec![PathOrPattern::Path(
            path.clone(),
          )]))
        },
        exclude: PathOrPatternSet::new(applicable_excludes),
      });
    }

    // todo(dsherret): This could be further optimized by not including
    // patterns that will only ever match another base.
    for base_path in include_patterns_by_base_path.keys() {
      let applicable_excludes = get_applicable_excludes(base_path);
      let mut applicable_includes = Vec::new();
      // get all patterns that apply to the current or ancestor directories
      for path in base_path.ancestors() {
        if let Some(patterns) = include_patterns_by_base_path.get(path) {
          applicable_includes.extend(
            patterns
              .iter()
              .map(|p| PathOrPattern::Pattern((*p).clone())),
          );
        }
      }
      result.push(Self {
        base: base_path.clone(),
        include: if self.include.is_none()
          || applicable_includes.is_empty()
            && self
              .include
              .as_ref()
              .map(|i| !i.0.is_empty())
              .unwrap_or(false)
        {
          None
        } else {
          Some(PathOrPatternSet::new(applicable_includes))
        },
        exclude: PathOrPatternSet::new(applicable_excludes),
      });
    }

    // Sort by the longest base path first. This ensures that we visit opted into
    // nested directories first before visiting the parent directory. The directory
    // traverser will handle not going into directories it's already been in.
    result.sort_by(|a, b| {
      // try looking at the parents first so that files in the same
      // folder are kept in the same order that they're provided
      let (a, b) =
        if let (Some(a), Some(b)) = (a.base.parent(), b.base.parent()) {
          (a, b)
        } else {
          (a.base.as_path(), b.base.as_path())
        };
      b.as_os_str().len().cmp(&a.as_os_str().len())
    });

    result
  }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum PathOrPatternsMatch {
  Matched,
  NotMatched,
  Excluded,
}

#[derive(Debug, Error, JsError)]
pub enum FromExcludeRelativePathOrPatternsError {
  #[class(type)]
  #[error(
    "The negation of '{negated_entry}' is never reached due to the higher priority '{entry}' exclude. Move '{negated_entry}' after '{entry}'."
  )]
  HigherPriorityExclude {
    negated_entry: String,
    entry: String,
  },
  #[class(inherit)]
  #[error("{0}")]
  PathOrPatternParse(#[from] PathOrPatternParseError),
}

#[derive(Clone, Default, Debug, Hash, Eq, PartialEq)]
pub struct PathOrPatternSet(Vec<PathOrPattern>);

impl PathOrPatternSet {
  pub fn new(elements: Vec<PathOrPattern>) -> Self {
    Self(elements)
  }

  pub fn from_absolute_paths(
    paths: &[String],
  ) -> Result<Self, PathOrPatternParseError> {
    Ok(Self(
      paths
        .iter()
        .map(|p| PathOrPattern::new(p))
        .collect::<Result<Vec<_>, _>>()?,
    ))
  }

  /// Builds the set of path and patterns for an "include" list.
  pub fn from_include_relative_path_or_patterns(
    base: &Path,
    entries: &[String],
  ) -> Result<Self, PathOrPatternParseError> {
    Ok(Self(
      entries
        .iter()
        .map(|p| PathOrPattern::from_relative(base, p))
        .collect::<Result<Vec<_>, _>>()?,
    ))
  }

  /// Builds the set and ensures no negations are overruled by
  /// higher priority entries.
  pub fn from_exclude_relative_path_or_patterns(
    base: &Path,
    entries: &[String],
  ) -> Result<Self, FromExcludeRelativePathOrPatternsError> {
    // error when someone does something like:
    // exclude: ["!./a/b", "./a"] as it should be the opposite
    fn validate_entry(
      found_negated_paths: &Vec<(&str, PathBuf)>,
      entry: &str,
      entry_path: &Path,
    ) -> Result<(), FromExcludeRelativePathOrPatternsError> {
      for (negated_entry, negated_path) in found_negated_paths {
        if negated_path.starts_with(entry_path) {
          return Err(
            FromExcludeRelativePathOrPatternsError::HigherPriorityExclude {
              negated_entry: negated_entry.to_string(),
              entry: entry.to_string(),
            },
          );
        }
      }
      Ok(())
    }

    let mut found_negated_paths: Vec<(&str, PathBuf)> =
      Vec::with_capacity(entries.len());
    let mut result = Vec::with_capacity(entries.len());
    for entry in entries {
      let p = PathOrPattern::from_relative(base, entry)?;
      match &p {
        PathOrPattern::Path(p) => {
          validate_entry(&found_negated_paths, entry, p)?;
        }
        PathOrPattern::NegatedPath(p) => {
          found_negated_paths.push((entry.as_str(), p.clone()));
        }
        PathOrPattern::RemoteUrl(_) => {
          // ignore
        }
        PathOrPattern::Pattern(p) => {
          if p.is_negated() {
            let base_path = p.base_path();
            found_negated_paths.push((entry.as_str(), base_path));
          }
        }
      }
      result.push(p);
    }
    Ok(Self(result))
  }

  pub fn inner(&self) -> &Vec<PathOrPattern> {
    &self.0
  }

  pub fn inner_mut(&mut self) -> &mut Vec<PathOrPattern> {
    &mut self.0
  }

  pub fn into_path_or_patterns(self) -> Vec<PathOrPattern> {
    self.0
  }

  pub fn matches_path(&self, path: &Path) -> bool {
    self.matches_path_detail(path) == PathOrPatternsMatch::Matched
  }

  pub fn matches_path_detail(&self, path: &Path) -> PathOrPatternsMatch {
    for p in self.0.iter().rev() {
      match p.matches_path(path) {
        PathGlobMatch::Matched => return PathOrPatternsMatch::Matched,
        PathGlobMatch::MatchedNegated => return PathOrPatternsMatch::Excluded,
        PathGlobMatch::NotMatched => {
          // ignore
        }
      }
    }
    PathOrPatternsMatch::NotMatched
  }

  pub fn base_paths(&self) -> Vec<PathBuf> {
    let mut result = Vec::with_capacity(self.0.len());
    for element in &self.0 {
      match element {
        PathOrPattern::Path(path) | PathOrPattern::NegatedPath(path) => {
          result.push(path.to_path_buf());
        }
        PathOrPattern::RemoteUrl(_) => {
          // ignore
        }
        PathOrPattern::Pattern(pattern) => {
          result.push(pattern.base_path());
        }
      }
    }
    result
  }

  pub fn push(&mut self, item: PathOrPattern) {
    self.0.push(item);
  }

  pub fn append(&mut self, items: impl Iterator<Item = PathOrPattern>) {
    self.0.extend(items)
  }
}

#[derive(Debug, Error, JsError, Clone)]
#[class(inherit)]
#[error("Invalid URL '{}'", url)]
pub struct UrlParseError {
  url: String,
  #[source]
  #[inherit]
  source: url::ParseError,
}

#[derive(Debug, Error, JsError)]
pub enum PathOrPatternParseError {
  #[class(inherit)]
  #[error(transparent)]
  UrlParse(#[from] UrlParseError),
  #[class(inherit)]
  #[error(transparent)]
  UrlToFilePathError(#[from] UrlToFilePathError),
  #[class(inherit)]
  #[error(transparent)]
  GlobParse(#[from] GlobPatternParseError),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum PathOrPattern {
  Path(PathBuf),
  NegatedPath(PathBuf),
  RemoteUrl(Url),
  Pattern(GlobPattern),
}

impl PathOrPattern {
  pub fn new(path: &str) -> Result<Self, PathOrPatternParseError> {
    if has_url_prefix(path) {
      let url = Url::parse(path).map_err(|err| UrlParseError {
        url: path.to_string(),
        source: err,
      })?;
      if url.scheme() == "file" {
        let path = url_to_file_path(&url)?;
        return Ok(Self::Path(path));
      } else {
        return Ok(Self::RemoteUrl(url));
      }
    }

    GlobPattern::new_if_pattern(path)
      .map(|maybe_pattern| {
        maybe_pattern
          .map(PathOrPattern::Pattern)
          .unwrap_or_else(|| {
            PathOrPattern::Path(
              normalize_path(Cow::Borrowed(Path::new(path))).into_owned(),
            )
          })
      })
      .map_err(|err| err.into())
  }

  pub fn from_relative(
    base: &Path,
    p: &str,
  ) -> Result<PathOrPattern, PathOrPatternParseError> {
    if is_glob_pattern(p) {
      GlobPattern::from_relative(base, p)
        .map(PathOrPattern::Pattern)
        .map_err(|err| err.into())
    } else if has_url_prefix(p) {
      PathOrPattern::new(p)
    } else if let Some(path) = p.strip_prefix('!') {
      Ok(PathOrPattern::NegatedPath(
        normalize_path(Cow::Owned(base.join(path))).into_owned(),
      ))
    } else {
      Ok(PathOrPattern::Path(
        normalize_path(Cow::Owned(base.join(p))).into_owned(),
      ))
    }
  }

  pub fn matches_path(&self, path: &Path) -> PathGlobMatch {
    match self {
      PathOrPattern::Path(p) => {
        if path.starts_with(p) {
          PathGlobMatch::Matched
        } else {
          PathGlobMatch::NotMatched
        }
      }
      PathOrPattern::NegatedPath(p) => {
        if path.starts_with(p) {
          PathGlobMatch::MatchedNegated
        } else {
          PathGlobMatch::NotMatched
        }
      }
      PathOrPattern::RemoteUrl(_) => PathGlobMatch::NotMatched,
      PathOrPattern::Pattern(p) => p.matches_path(path),
    }
  }

  /// Returns the base path of the pattern if it's not a remote url pattern.
  pub fn base_path(&self) -> Option<PathBuf> {
    match self {
      PathOrPattern::Path(p) | PathOrPattern::NegatedPath(p) => Some(p.clone()),
      PathOrPattern::RemoteUrl(_) => None,
      PathOrPattern::Pattern(p) => Some(p.base_path()),
    }
  }

  /// If this is a negated pattern.
  pub fn is_negated(&self) -> bool {
    match self {
      PathOrPattern::Path(_) => false,
      PathOrPattern::NegatedPath(_) => true,
      PathOrPattern::RemoteUrl(_) => false,
      PathOrPattern::Pattern(p) => p.is_negated(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathGlobMatch {
  Matched,
  MatchedNegated,
  NotMatched,
}

#[derive(Debug, Error, JsError)]
#[class(type)]
#[error("Failed to expand glob: \"{pattern}\"")]
pub struct GlobPatternParseError {
  pattern: String,
  #[source]
  source: glob::PatternError,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobPattern {
  is_negated: bool,
  pattern: glob::Pattern,
}

impl GlobPattern {
  pub fn new_if_pattern(
    pattern: &str,
  ) -> Result<Option<Self>, GlobPatternParseError> {
    if !is_glob_pattern(pattern) {
      return Ok(None);
    }
    Self::new(pattern).map(Some)
  }

  pub fn new(pattern: &str) -> Result<Self, GlobPatternParseError> {
    let (is_negated, pattern) = match pattern.strip_prefix('!') {
      Some(pattern) => (true, pattern),
      None => (false, pattern),
    };
    let pattern = escape_brackets(pattern).replace('\\', "/");
    let pattern =
      glob::Pattern::new(&pattern).map_err(|source| GlobPatternParseError {
        pattern: pattern.to_string(),
        source,
      })?;
    Ok(Self {
      is_negated,
      pattern,
    })
  }

  pub fn from_relative(
    base: &Path,
    p: &str,
  ) -> Result<Self, GlobPatternParseError> {
    let (is_negated, p) = match p.strip_prefix('!') {
      Some(p) => (true, p),
      None => (false, p),
    };
    let base_str = base.to_string_lossy().replace('\\', "/");
    let p = p.strip_prefix("./").unwrap_or(p);
    let p = p.strip_suffix('/').unwrap_or(p);
    let pattern = capacity_builder::StringBuilder::<String>::build(|builder| {
      if is_negated {
        builder.append('!');
      }
      builder.append(&base_str);
      if !base_str.ends_with('/') {
        builder.append('/');
      }
      builder.append(p);
    })
    .unwrap();
    GlobPattern::new(&pattern)
  }

  pub fn as_str(&self) -> Cow<'_, str> {
    if self.is_negated {
      Cow::Owned(format!("!{}", self.pattern.as_str()))
    } else {
      Cow::Borrowed(self.pattern.as_str())
    }
  }

  pub fn matches_path(&self, path: &Path) -> PathGlobMatch {
    if self.pattern.matches_path_with(path, match_options()) {
      if self.is_negated {
        PathGlobMatch::MatchedNegated
      } else {
        PathGlobMatch::Matched
      }
    } else {
      PathGlobMatch::NotMatched
    }
  }

  pub fn base_path(&self) -> PathBuf {
    let base_path = self
      .pattern
      .as_str()
      .split('/')
      .take_while(|c| !has_glob_chars(c))
      .collect::<Vec<_>>()
      .join(std::path::MAIN_SEPARATOR_STR);
    PathBuf::from(base_path)
  }

  pub fn is_negated(&self) -> bool {
    self.is_negated
  }

  fn as_negated(&self) -> GlobPattern {
    Self {
      is_negated: !self.is_negated,
      pattern: self.pattern.clone(),
    }
  }
}

pub fn is_glob_pattern(path: &str) -> bool {
  !has_url_prefix(path) && has_glob_chars(path)
}

fn has_url_prefix(pattern: &str) -> bool {
  pattern.starts_with("http://")
    || pattern.starts_with("https://")
    || pattern.starts_with("file://")
    || pattern.starts_with("npm:")
    || pattern.starts_with("jsr:")
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

#[cfg(test)]
mod test {
  use std::error::Error;

  use deno_path_util::url_from_directory_path;
  use pretty_assertions::assert_eq;
  use tempfile::TempDir;

  use super::*;

  // For easier comparisons in tests.
  #[derive(Debug, PartialEq, Eq)]
  struct ComparableFilePatterns {
    base: String,
    include: Option<Vec<String>>,
    exclude: Vec<String>,
  }

  impl ComparableFilePatterns {
    pub fn new(root: &Path, file_patterns: &FilePatterns) -> Self {
      fn path_to_string(root: &Path, path: &Path) -> String {
        path
          .strip_prefix(root)
          .unwrap()
          .to_string_lossy()
          .replace('\\', "/")
      }

      fn path_or_pattern_to_string(
        root: &Path,
        p: &PathOrPattern,
      ) -> Option<String> {
        match p {
          PathOrPattern::RemoteUrl(_) => None,
          PathOrPattern::Path(p) => Some(path_to_string(root, p)),
          PathOrPattern::NegatedPath(p) => {
            Some(format!("!{}", path_to_string(root, p)))
          }
          PathOrPattern::Pattern(p) => {
            let was_negated = p.is_negated();
            let p = if was_negated {
              p.as_negated()
            } else {
              p.clone()
            };
            let text = p
              .as_str()
              .strip_prefix(&format!(
                "{}/",
                root.to_string_lossy().replace('\\', "/")
              ))
              .unwrap_or_else(|| panic!("pattern: {:?}, root: {:?}", p, root))
              .to_string();
            Some(if was_negated {
              format!("!{}", text)
            } else {
              text
            })
          }
        }
      }

      Self {
        base: path_to_string(root, &file_patterns.base),
        include: file_patterns.include.as_ref().map(|p| {
          p.0
            .iter()
            .filter_map(|p| path_or_pattern_to_string(root, p))
            .collect()
        }),
        exclude: file_patterns
          .exclude
          .0
          .iter()
          .filter_map(|p| path_or_pattern_to_string(root, p))
          .collect(),
      }
    }

    pub fn from_split(
      root: &Path,
      patterns_by_base: &[FilePatterns],
    ) -> Vec<ComparableFilePatterns> {
      patterns_by_base
        .iter()
        .map(|file_patterns| ComparableFilePatterns::new(root, file_patterns))
        .collect()
    }
  }

  #[test]
  fn file_patterns_split_by_base_dir() {
    let temp_dir = TempDir::new().unwrap();
    let patterns = FilePatterns {
      base: temp_dir.path().to_path_buf(),
      include: Some(PathOrPatternSet::new(vec![
        PathOrPattern::Pattern(
          GlobPattern::new(&format!(
            "{}/inner/**/*.ts",
            temp_dir.path().to_string_lossy().replace('\\', "/")
          ))
          .unwrap(),
        ),
        PathOrPattern::Pattern(
          GlobPattern::new(&format!(
            "{}/inner/sub/deeper/**/*.js",
            temp_dir.path().to_string_lossy().replace('\\', "/")
          ))
          .unwrap(),
        ),
        PathOrPattern::Pattern(
          GlobPattern::new(&format!(
            "{}/other/**/*.js",
            temp_dir.path().to_string_lossy().replace('\\', "/")
          ))
          .unwrap(),
        ),
        PathOrPattern::from_relative(temp_dir.path(), "!./other/**/*.ts")
          .unwrap(),
        PathOrPattern::from_relative(temp_dir.path(), "sub/file.ts").unwrap(),
      ])),
      exclude: PathOrPatternSet::new(vec![
        PathOrPattern::Pattern(
          GlobPattern::new(&format!(
            "{}/inner/other/**/*.ts",
            temp_dir.path().to_string_lossy().replace('\\', "/")
          ))
          .unwrap(),
        ),
        PathOrPattern::Path(
          temp_dir
            .path()
            .join("inner/sub/deeper/file.js")
            .to_path_buf(),
        ),
      ]),
    };
    let split = ComparableFilePatterns::from_split(
      temp_dir.path(),
      &patterns.split_by_base(),
    );
    assert_eq!(
      split,
      vec![
        ComparableFilePatterns {
          base: "inner/sub/deeper".to_string(),
          include: Some(vec![
            "inner/sub/deeper/**/*.js".to_string(),
            "inner/**/*.ts".to_string(),
          ]),
          exclude: vec!["inner/sub/deeper/file.js".to_string()],
        },
        ComparableFilePatterns {
          base: "sub/file.ts".to_string(),
          include: Some(vec!["sub/file.ts".to_string()]),
          exclude: vec![],
        },
        ComparableFilePatterns {
          base: "inner".to_string(),
          include: Some(vec!["inner/**/*.ts".to_string()]),
          exclude: vec![
            "inner/other/**/*.ts".to_string(),
            "inner/sub/deeper/file.js".to_string(),
          ],
        },
        ComparableFilePatterns {
          base: "other".to_string(),
          include: Some(vec!["other/**/*.js".to_string()]),
          exclude: vec!["other/**/*.ts".to_string()],
        }
      ]
    );
  }

  #[test]
  fn file_patterns_split_by_base_dir_unexcluded() {
    let temp_dir = TempDir::new().unwrap();
    let patterns = FilePatterns {
      base: temp_dir.path().to_path_buf(),
      include: None,
      exclude: PathOrPatternSet::new(vec![
        PathOrPattern::from_relative(temp_dir.path(), "./ignored").unwrap(),
        PathOrPattern::from_relative(temp_dir.path(), "!./ignored/unexcluded")
          .unwrap(),
        PathOrPattern::from_relative(temp_dir.path(), "!./ignored/test/**")
          .unwrap(),
      ]),
    };
    let split = ComparableFilePatterns::from_split(
      temp_dir.path(),
      &patterns.split_by_base(),
    );
    assert_eq!(
      split,
      vec![
        ComparableFilePatterns {
          base: "ignored/unexcluded".to_string(),
          include: None,
          exclude: vec![
            // still keeps the higher level exclude for cases
            // where these two are accidentally swapped
            "ignored".to_string(),
            // keep the glob for the current dir because it
            // could be used to override the .gitignore
            "!ignored/unexcluded".to_string(),
          ],
        },
        ComparableFilePatterns {
          base: "ignored/test".to_string(),
          include: None,
          exclude: vec!["ignored".to_string(), "!ignored/test/**".to_string(),],
        },
        ComparableFilePatterns {
          base: "".to_string(),
          include: None,
          exclude: vec![
            "ignored".to_string(),
            "!ignored/unexcluded".to_string(),
            "!ignored/test/**".to_string(),
          ],
        },
      ]
    );
  }

  #[test]
  fn file_patterns_split_by_base_dir_unexcluded_with_path_includes() {
    let temp_dir = TempDir::new().unwrap();
    let patterns = FilePatterns {
      base: temp_dir.path().to_path_buf(),
      include: Some(PathOrPatternSet::new(vec![
        PathOrPattern::from_relative(temp_dir.path(), "./sub").unwrap(),
      ])),
      exclude: PathOrPatternSet::new(vec![
        PathOrPattern::from_relative(temp_dir.path(), "./sub/ignored").unwrap(),
        PathOrPattern::from_relative(temp_dir.path(), "!./sub/ignored/test/**")
          .unwrap(),
        PathOrPattern::from_relative(temp_dir.path(), "./orphan").unwrap(),
        PathOrPattern::from_relative(temp_dir.path(), "!./orphan/test/**")
          .unwrap(),
      ]),
    };
    let split = ComparableFilePatterns::from_split(
      temp_dir.path(),
      &patterns.split_by_base(),
    );
    assert_eq!(
      split,
      vec![
        ComparableFilePatterns {
          base: "sub/ignored/test".to_string(),
          include: None,
          exclude: vec![
            "sub/ignored".to_string(),
            "!sub/ignored/test/**".to_string(),
          ],
        },
        ComparableFilePatterns {
          base: "sub".to_string(),
          include: Some(vec!["sub".to_string()]),
          exclude: vec![
            "sub/ignored".to_string(),
            "!sub/ignored/test/**".to_string(),
          ],
        },
      ]
    );
  }

  #[test]
  fn file_patterns_split_by_base_dir_unexcluded_with_glob_includes() {
    let temp_dir = TempDir::new().unwrap();
    let patterns = FilePatterns {
      base: temp_dir.path().to_path_buf(),
      include: Some(PathOrPatternSet::new(vec![
        PathOrPattern::from_relative(temp_dir.path(), "./sub/**").unwrap(),
      ])),
      exclude: PathOrPatternSet::new(vec![
        PathOrPattern::from_relative(temp_dir.path(), "./sub/ignored").unwrap(),
        PathOrPattern::from_relative(temp_dir.path(), "!./sub/ignored/test/**")
          .unwrap(),
        PathOrPattern::from_relative(temp_dir.path(), "!./orphan/test/**")
          .unwrap(),
        PathOrPattern::from_relative(temp_dir.path(), "!orphan/other").unwrap(),
      ]),
    };
    let split = ComparableFilePatterns::from_split(
      temp_dir.path(),
      &patterns.split_by_base(),
    );
    assert_eq!(
      split,
      vec![
        ComparableFilePatterns {
          base: "sub/ignored/test".to_string(),
          include: Some(vec!["sub/**".to_string()]),
          exclude: vec![
            "sub/ignored".to_string(),
            "!sub/ignored/test/**".to_string()
          ],
        },
        ComparableFilePatterns {
          base: "sub".to_string(),
          include: Some(vec!["sub/**".to_string()]),
          exclude: vec![
            "sub/ignored".to_string(),
            "!sub/ignored/test/**".to_string(),
          ],
        }
      ]
    );
  }

  #[test]
  fn file_patterns_split_by_base_dir_opposite_exclude() {
    let temp_dir = TempDir::new().unwrap();
    let patterns = FilePatterns {
      base: temp_dir.path().to_path_buf(),
      include: None,
      // this will actually error before it gets here in integration,
      // but it's best to ensure it's handled anyway
      exclude: PathOrPatternSet::new(vec![
        // this won't be unexcluded because it's lower priority than the entry below
        PathOrPattern::from_relative(temp_dir.path(), "!./sub/ignored/test/")
          .unwrap(),
        // this is higher priority
        PathOrPattern::from_relative(temp_dir.path(), "./sub/ignored").unwrap(),
      ]),
    };
    let split = ComparableFilePatterns::from_split(
      temp_dir.path(),
      &patterns.split_by_base(),
    );
    assert_eq!(
      split,
      vec![
        ComparableFilePatterns {
          base: "sub/ignored/test".to_string(),
          include: None,
          exclude: vec![
            "!sub/ignored/test".to_string(),
            "sub/ignored".to_string(),
          ],
        },
        ComparableFilePatterns {
          base: "".to_string(),
          include: None,
          exclude: vec![
            "!sub/ignored/test".to_string(),
            "sub/ignored".to_string(),
          ],
        },
      ]
    );
  }

  #[test]
  fn file_patterns_split_by_base_dir_exclude_unexcluded_and_glob() {
    let temp_dir = TempDir::new().unwrap();
    let patterns = FilePatterns {
      base: temp_dir.path().to_path_buf(),
      include: None,
      exclude: PathOrPatternSet::new(vec![
        PathOrPattern::from_relative(temp_dir.path(), "./sub/ignored").unwrap(),
        PathOrPattern::from_relative(temp_dir.path(), "!./sub/ignored/test/")
          .unwrap(),
        PathOrPattern::from_relative(temp_dir.path(), "./sub/ignored/**/*.ts")
          .unwrap(),
      ]),
    };
    let split = ComparableFilePatterns::from_split(
      temp_dir.path(),
      &patterns.split_by_base(),
    );
    assert_eq!(
      split,
      vec![
        ComparableFilePatterns {
          base: "sub/ignored/test".to_string(),
          include: None,
          exclude: vec![
            "sub/ignored".to_string(),
            "!sub/ignored/test".to_string(),
            "sub/ignored/**/*.ts".to_string()
          ],
        },
        ComparableFilePatterns {
          base: "".to_string(),
          include: None,
          exclude: vec![
            "sub/ignored".to_string(),
            "!sub/ignored/test".to_string(),
            "sub/ignored/**/*.ts".to_string(),
          ],
        },
      ]
    );
  }

  #[track_caller]
  fn run_file_patterns_match_test(
    file_patterns: &FilePatterns,
    path: &Path,
    kind: PathKind,
    expected: FilePatternsMatch,
  ) {
    assert_eq!(
      file_patterns.matches_path_detail(path, kind),
      expected,
      "path: {:?}, kind: {:?}",
      path,
      kind
    );
    assert_eq!(
      file_patterns.matches_path(path, kind),
      match expected {
        FilePatternsMatch::Passed
        | FilePatternsMatch::PassedOptedOutExclude => true,
        FilePatternsMatch::Excluded => false,
      }
    )
  }

  #[test]
  fn file_patterns_include() {
    let cwd = current_dir();
    // include is a closed set
    let file_patterns = FilePatterns {
      base: cwd.clone(),
      include: Some(PathOrPatternSet(vec![
        PathOrPattern::from_relative(&cwd, "target").unwrap(),
        PathOrPattern::from_relative(&cwd, "other/**/*.ts").unwrap(),
      ])),
      exclude: PathOrPatternSet(vec![]),
    };
    let run_test =
      |path: &Path, kind: PathKind, expected: FilePatternsMatch| {
        run_file_patterns_match_test(&file_patterns, path, kind, expected);
      };
    run_test(&cwd, PathKind::Directory, FilePatternsMatch::Passed);
    run_test(
      &cwd.join("other"),
      PathKind::Directory,
      FilePatternsMatch::Passed,
    );
    run_test(
      &cwd.join("other/sub_dir"),
      PathKind::Directory,
      FilePatternsMatch::Passed,
    );
    run_test(
      &cwd.join("not_matched"),
      PathKind::File,
      FilePatternsMatch::Excluded,
    );
    run_test(
      &cwd.join("other/test.ts"),
      PathKind::File,
      FilePatternsMatch::Passed,
    );
    run_test(
      &cwd.join("other/test.js"),
      PathKind::File,
      FilePatternsMatch::Excluded,
    );
  }

  #[test]
  fn file_patterns_exclude() {
    let cwd = current_dir();
    let file_patterns = FilePatterns {
      base: cwd.clone(),
      include: None,
      exclude: PathOrPatternSet(vec![
        PathOrPattern::from_relative(&cwd, "target").unwrap(),
        PathOrPattern::from_relative(&cwd, "!not_excluded").unwrap(),
        // lower items take priority
        PathOrPattern::from_relative(&cwd, "excluded_then_not_excluded")
          .unwrap(),
        PathOrPattern::from_relative(&cwd, "!excluded_then_not_excluded")
          .unwrap(),
        PathOrPattern::from_relative(&cwd, "!not_excluded_then_excluded")
          .unwrap(),
        PathOrPattern::from_relative(&cwd, "not_excluded_then_excluded")
          .unwrap(),
      ]),
    };
    let run_test =
      |path: &Path, kind: PathKind, expected: FilePatternsMatch| {
        run_file_patterns_match_test(&file_patterns, path, kind, expected);
      };
    run_test(&cwd, PathKind::Directory, FilePatternsMatch::Passed);
    run_test(
      &cwd.join("target"),
      PathKind::File,
      FilePatternsMatch::Excluded,
    );
    run_test(
      &cwd.join("not_excluded"),
      PathKind::File,
      FilePatternsMatch::PassedOptedOutExclude,
    );
    run_test(
      &cwd.join("excluded_then_not_excluded"),
      PathKind::File,
      FilePatternsMatch::PassedOptedOutExclude,
    );
    run_test(
      &cwd.join("not_excluded_then_excluded"),
      PathKind::File,
      FilePatternsMatch::Excluded,
    );
  }

  #[test]
  fn file_patterns_include_exclude() {
    let cwd = current_dir();
    let file_patterns = FilePatterns {
      base: cwd.clone(),
      include: Some(PathOrPatternSet(vec![
        PathOrPattern::from_relative(&cwd, "other").unwrap(),
        PathOrPattern::from_relative(&cwd, "target").unwrap(),
        PathOrPattern::from_relative(&cwd, "**/*.js").unwrap(),
        PathOrPattern::from_relative(&cwd, "**/file.ts").unwrap(),
      ])),
      exclude: PathOrPatternSet(vec![
        PathOrPattern::from_relative(&cwd, "target").unwrap(),
        PathOrPattern::from_relative(&cwd, "!target/unexcluded/").unwrap(),
        PathOrPattern::from_relative(&cwd, "!target/other/**").unwrap(),
        PathOrPattern::from_relative(&cwd, "**/*.ts").unwrap(),
        PathOrPattern::from_relative(&cwd, "!**/file.ts").unwrap(),
      ]),
    };
    let run_test =
      |path: &Path, kind: PathKind, expected: FilePatternsMatch| {
        run_file_patterns_match_test(&file_patterns, path, kind, expected);
      };
    // matches other
    run_test(
      &cwd.join("other/test.txt"),
      PathKind::File,
      FilePatternsMatch::Passed,
    );
    // matches **/*.js
    run_test(
      &cwd.join("sub_dir/test.js"),
      PathKind::File,
      FilePatternsMatch::Passed,
    );
    // not in include set
    run_test(
      &cwd.join("sub_dir/test.txt"),
      PathKind::File,
      FilePatternsMatch::Excluded,
    );
    // .ts extension not matched
    run_test(
      &cwd.join("other/test.ts"),
      PathKind::File,
      FilePatternsMatch::Excluded,
    );
    // file.ts excluded from excludes
    run_test(
      &cwd.join("other/file.ts"),
      PathKind::File,
      FilePatternsMatch::PassedOptedOutExclude,
    );
    // not allowed target dir
    run_test(
      &cwd.join("target/test.txt"),
      PathKind::File,
      FilePatternsMatch::Excluded,
    );
    run_test(
      &cwd.join("target/sub_dir/test.txt"),
      PathKind::File,
      FilePatternsMatch::Excluded,
    );
    // but allowed target/other dir
    run_test(
      &cwd.join("target/other/test.txt"),
      PathKind::File,
      FilePatternsMatch::PassedOptedOutExclude,
    );
    run_test(
      &cwd.join("target/other/sub/dir/test.txt"),
      PathKind::File,
      FilePatternsMatch::PassedOptedOutExclude,
    );
    // and in target/unexcluded
    run_test(
      &cwd.join("target/unexcluded/test.txt"),
      PathKind::File,
      FilePatternsMatch::PassedOptedOutExclude,
    );
  }

  #[test]
  fn file_patterns_include_excluded() {
    let cwd = current_dir();
    let file_patterns = FilePatterns {
      base: cwd.clone(),
      include: None,
      exclude: PathOrPatternSet(vec![
        PathOrPattern::from_relative(&cwd, "js/").unwrap(),
        PathOrPattern::from_relative(&cwd, "!js/sub_dir/").unwrap(),
      ]),
    };
    let run_test =
      |path: &Path, kind: PathKind, expected: FilePatternsMatch| {
        run_file_patterns_match_test(&file_patterns, path, kind, expected);
      };
    run_test(
      &cwd.join("js/test.txt"),
      PathKind::File,
      FilePatternsMatch::Excluded,
    );
    run_test(
      &cwd.join("js/sub_dir/test.txt"),
      PathKind::File,
      FilePatternsMatch::PassedOptedOutExclude,
    );
  }

  #[test]
  fn file_patterns_opposite_incorrect_excluded_include() {
    let cwd = current_dir();
    let file_patterns = FilePatterns {
      base: cwd.clone(),
      include: None,
      exclude: PathOrPatternSet(vec![
        // this is lower priority
        PathOrPattern::from_relative(&cwd, "!js/sub_dir/").unwrap(),
        // this wins because it's higher priority
        PathOrPattern::from_relative(&cwd, "js/").unwrap(),
      ]),
    };
    let run_test =
      |path: &Path, kind: PathKind, expected: FilePatternsMatch| {
        run_file_patterns_match_test(&file_patterns, path, kind, expected);
      };
    run_test(
      &cwd.join("js/test.txt"),
      PathKind::File,
      FilePatternsMatch::Excluded,
    );
    run_test(
      &cwd.join("js/sub_dir/test.txt"),
      PathKind::File,
      FilePatternsMatch::Excluded,
    );
  }

  #[test]
  fn from_relative() {
    let cwd = current_dir();
    // leading dot slash
    {
      let pattern = PathOrPattern::from_relative(&cwd, "./**/*.ts").unwrap();
      assert_eq!(
        pattern.matches_path(&cwd.join("foo.ts")),
        PathGlobMatch::Matched
      );
      assert_eq!(
        pattern.matches_path(&cwd.join("dir/foo.ts")),
        PathGlobMatch::Matched
      );
      assert_eq!(
        pattern.matches_path(&cwd.join("foo.js")),
        PathGlobMatch::NotMatched
      );
      assert_eq!(
        pattern.matches_path(&cwd.join("dir/foo.js")),
        PathGlobMatch::NotMatched
      );
    }
    // no leading dot slash
    {
      let pattern = PathOrPattern::from_relative(&cwd, "**/*.ts").unwrap();
      assert_eq!(
        pattern.matches_path(&cwd.join("foo.ts")),
        PathGlobMatch::Matched
      );
      assert_eq!(
        pattern.matches_path(&cwd.join("dir/foo.ts")),
        PathGlobMatch::Matched
      );
      assert_eq!(
        pattern.matches_path(&cwd.join("foo.js")),
        PathGlobMatch::NotMatched
      );
      assert_eq!(
        pattern.matches_path(&cwd.join("dir/foo.js")),
        PathGlobMatch::NotMatched
      );
    }
    // exact file, leading dot slash
    {
      let pattern = PathOrPattern::from_relative(&cwd, "./foo.ts").unwrap();
      assert_eq!(
        pattern.matches_path(&cwd.join("foo.ts")),
        PathGlobMatch::Matched
      );
      assert_eq!(
        pattern.matches_path(&cwd.join("dir/foo.ts")),
        PathGlobMatch::NotMatched
      );
      assert_eq!(
        pattern.matches_path(&cwd.join("foo.js")),
        PathGlobMatch::NotMatched
      );
    }
    // exact file, no leading dot slash
    {
      let pattern = PathOrPattern::from_relative(&cwd, "foo.ts").unwrap();
      assert_eq!(
        pattern.matches_path(&cwd.join("foo.ts")),
        PathGlobMatch::Matched
      );
      assert_eq!(
        pattern.matches_path(&cwd.join("dir/foo.ts")),
        PathGlobMatch::NotMatched
      );
      assert_eq!(
        pattern.matches_path(&cwd.join("foo.js")),
        PathGlobMatch::NotMatched
      );
    }
    // error for invalid url
    {
      let err = PathOrPattern::from_relative(&cwd, "https://raw.githubusercontent.com%2Fdyedgreen%2Fdeno-sqlite%2Frework_api%2Fmod.ts").unwrap_err();
      assert_eq!(
        format!("{:#}", err),
        "Invalid URL 'https://raw.githubusercontent.com%2Fdyedgreen%2Fdeno-sqlite%2Frework_api%2Fmod.ts'"
      );
      assert_eq!(
        format!("{:#}", err.source().unwrap()),
        "invalid international domain name"
      );
    }
    // sibling dir
    {
      let pattern = PathOrPattern::from_relative(&cwd, "../sibling").unwrap();
      let parent_dir = cwd.parent().unwrap();
      assert_eq!(pattern.base_path().unwrap(), parent_dir.join("sibling"));
      assert_eq!(
        pattern.matches_path(&parent_dir.join("sibling/foo.ts")),
        PathGlobMatch::Matched
      );
      assert_eq!(
        pattern.matches_path(&parent_dir.join("./other/foo.js")),
        PathGlobMatch::NotMatched
      );
    }
  }

  #[test]
  fn from_relative_dot_slash() {
    let cwd = current_dir();
    let pattern = PathOrPattern::from_relative(&cwd, "./").unwrap();
    match pattern {
      PathOrPattern::Path(p) => assert_eq!(p, cwd),
      _ => unreachable!(),
    }
  }

  #[test]
  fn new_ctor() {
    let cwd = current_dir();
    for scheme in &["http", "https"] {
      let url = format!("{}://deno.land/x/test", scheme);
      let pattern = PathOrPattern::new(&url).unwrap();
      match pattern {
        PathOrPattern::RemoteUrl(p) => {
          assert_eq!(p.as_str(), url)
        }
        _ => unreachable!(),
      }
    }
    for scheme in &["npm", "jsr"] {
      let url = format!("{}:@denotest/basic", scheme);
      let pattern = PathOrPattern::new(&url).unwrap();
      match pattern {
        PathOrPattern::RemoteUrl(p) => {
          assert_eq!(p.as_str(), url)
        }
        _ => unreachable!(),
      }
    }
    {
      let file_specifier = url_from_directory_path(&cwd).unwrap();
      let pattern = PathOrPattern::new(file_specifier.as_str()).unwrap();
      match pattern {
        PathOrPattern::Path(p) => {
          assert_eq!(p, cwd);
        }
        _ => {
          unreachable!()
        }
      }
    }
  }

  #[test]
  fn from_relative_specifier() {
    let cwd = current_dir();
    for scheme in &["http", "https"] {
      let url = format!("{}://deno.land/x/test", scheme);
      let pattern = PathOrPattern::from_relative(&cwd, &url).unwrap();
      match pattern {
        PathOrPattern::RemoteUrl(p) => {
          assert_eq!(p.as_str(), url)
        }
        _ => unreachable!(),
      }
    }
    for scheme in &["npm", "jsr"] {
      let url = format!("{}:@denotest/basic", scheme);
      let pattern = PathOrPattern::from_relative(&cwd, &url).unwrap();
      match pattern {
        PathOrPattern::RemoteUrl(p) => {
          assert_eq!(p.as_str(), url)
        }
        _ => unreachable!(),
      }
    }
    {
      let file_specifier = url_from_directory_path(&cwd).unwrap();
      let pattern =
        PathOrPattern::from_relative(&cwd, file_specifier.as_str()).unwrap();
      match pattern {
        PathOrPattern::Path(p) => {
          assert_eq!(p, cwd);
        }
        _ => {
          unreachable!()
        }
      }
    }
  }

  #[test]
  fn negated_globs() {
    #[allow(clippy::disallowed_methods)]
    let cwd = current_dir();
    {
      let pattern = GlobPattern::from_relative(&cwd, "!./**/*.ts").unwrap();
      assert!(pattern.is_negated());
      assert_eq!(pattern.base_path(), cwd);
      assert!(pattern.as_str().starts_with('!'));
      assert_eq!(
        pattern.matches_path(&cwd.join("foo.ts")),
        PathGlobMatch::MatchedNegated
      );
      assert_eq!(
        pattern.matches_path(&cwd.join("foo.js")),
        PathGlobMatch::NotMatched
      );
      let pattern = pattern.as_negated();
      assert!(!pattern.is_negated());
      assert_eq!(pattern.base_path(), cwd);
      assert!(!pattern.as_str().starts_with('!'));
      assert_eq!(
        pattern.matches_path(&cwd.join("foo.ts")),
        PathGlobMatch::Matched
      );
      let pattern = pattern.as_negated();
      assert!(pattern.is_negated());
      assert_eq!(pattern.base_path(), cwd);
      assert!(pattern.as_str().starts_with('!'));
      assert_eq!(
        pattern.matches_path(&cwd.join("foo.ts")),
        PathGlobMatch::MatchedNegated
      );
    }
  }

  #[test]
  fn test_is_glob_pattern() {
    assert!(!is_glob_pattern("npm:@scope/pkg@*"));
    assert!(!is_glob_pattern("jsr:@scope/pkg@*"));
    assert!(!is_glob_pattern("https://deno.land/x/?"));
    assert!(!is_glob_pattern("http://deno.land/x/?"));
    assert!(!is_glob_pattern("file:///deno.land/x/?"));
    assert!(is_glob_pattern("**/*.ts"));
    assert!(is_glob_pattern("test/?"));
    assert!(!is_glob_pattern("test/test"));
  }

  fn current_dir() -> PathBuf {
    // ok because this is test code
    #[allow(clippy::disallowed_methods)]
    std::env::current_dir().unwrap()
  }
}
