// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Validation logic in this file is shared with registry/api/src/ids.rs

use thiserror::Error;

/// A package path, like '/foo' or '/foo/bar'. The path is prefixed with a slash
/// and does not end with a slash.
///
/// The path must not contain any double slashes, dot segments, or dot dot
/// segments.
///
/// The path must be less than 160 characters long, including the slash prefix.
///
/// The path must not contain any windows reserved characters, like CON, PRN,
/// AUX, NUL, or COM1.
///
/// The path must not contain any windows path separators, like backslash or
/// colon.
///
/// The path must only contain ascii alphanumeric characters, and the characters
/// '$', '(', ')', '+', '-', '.', '@', '[', ']', '_', '{', '}',  '~'.
///
/// Path's are case sensitive, but comparisons and hashing are case insensitive.
/// This matches the behaviour of the Windows FS APIs.
#[derive(Clone, Default)]
pub struct PackagePath {
  path: String,
  lower: Option<String>,
}

impl PartialEq for PackagePath {
  fn eq(&self, other: &Self) -> bool {
    let self_lower = self.lower.as_ref().unwrap_or(&self.path);
    let other_lower = other.lower.as_ref().unwrap_or(&other.path);
    self_lower == other_lower
  }
}

impl Eq for PackagePath {}

impl std::hash::Hash for PackagePath {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    let lower = self.lower.as_ref().unwrap_or(&self.path);
    lower.hash(state);
  }
}

impl PackagePath {
  pub fn new(path: String) -> Result<Self, PackagePathValidationError> {
    let len = path.len();
    if len > 160 {
      return Err(PackagePathValidationError::TooLong(len));
    }

    if len == 0 {
      return Err(PackagePathValidationError::MissingPrefix);
    }

    let mut components = path.split('/').peekable();
    let Some("") = components.next() else {
      return Err(PackagePathValidationError::MissingPrefix);
    };

    let mut has_upper = false;
    let mut valid_char_mapper = |c: char| {
      if c.is_ascii_uppercase() {
        has_upper = true;
      }
      valid_char(c)
    };
    while let Some(component) = components.next() {
      if component.is_empty() {
        if components.peek().is_none() {
          return Err(PackagePathValidationError::TrailingSlash);
        }
        return Err(PackagePathValidationError::EmptyComponent);
      }

      if component == "." || component == ".." {
        return Err(PackagePathValidationError::DotSegment);
      }

      if let Some(err) = component.chars().find_map(&mut valid_char_mapper) {
        return Err(err);
      }

      let basename = match component.rsplit_once('.') {
        Some((_, "")) => {
          return Err(PackagePathValidationError::TrailingDot(
            component.to_owned(),
          ));
        }
        Some((basename, _)) => basename,
        None => component,
      };

      let lower_basename = basename.to_ascii_lowercase();
      if WINDOWS_RESERVED_NAMES
        .binary_search(&&*lower_basename)
        .is_ok()
      {
        return Err(PackagePathValidationError::ReservedName(
          component.to_owned(),
        ));
      }
    }

    let lower = has_upper.then(|| path.to_ascii_lowercase());

    Ok(Self { path, lower })
  }
}

const WINDOWS_RESERVED_NAMES: [&str; 22] = [
  "aux", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
  "com9", "con", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7",
  "lpt8", "lpt9", "nul", "prn",
];

fn valid_char(c: char) -> Option<PackagePathValidationError> {
  match c {
    'a'..='z'
    | 'A'..='Z'
    | '0'..='9'
    | '$'
    | '('
    | ')'
    | '+'
    | '-'
    | '.'
    | '@'
    | '['
    | ']'
    | '_'
    | '{'
    | '}'
    | '~' => None,
    // informative error messages for some invalid characters
    '\\' | ':' => Some(
      PackagePathValidationError::InvalidWindowsPathSeparatorChar(c),
    ),
    '<' | '>' | '"' | '|' | '?' | '*' => {
      Some(PackagePathValidationError::InvalidWindowsChar(c))
    }
    ' ' | '\t' | '\n' | '\r' => {
      Some(PackagePathValidationError::InvalidWhitespace(c))
    }
    '%' | '#' => Some(PackagePathValidationError::InvalidSpecialUrlChar(c)),
    // other invalid characters
    c => Some(PackagePathValidationError::InvalidOtherChar(c)),
  }
}

#[derive(Debug, Clone, Error)]
pub enum PackagePathValidationError {
  #[error("package path must be at most 160 characters long, but is {0} characters long")]
  TooLong(usize),

  #[error("package path must be prefixed with a slash")]
  MissingPrefix,

  #[error("package path must not end with a slash")]
  TrailingSlash,

  #[error("package path must not contain empty components")]
  EmptyComponent,

  #[error("package path must not contain dot segments like '.' or '..'")]
  DotSegment,

  #[error(
    "package path must not contain windows reserved names like 'CON' or 'PRN' (found '{0}')"
  )]
  ReservedName(String),

  #[error("path segment must not end in a dot (found '{0}')")]
  TrailingDot(String),

  #[error(
    "package path must not contain windows path separators like '\\' or ':' (found '{0}')"
  )]
  InvalidWindowsPathSeparatorChar(char),

  #[error(
    "package path must not contain windows reserved characters like '<', '>', '\"', '|', '?', or '*' (found '{0}')"
  )]
  InvalidWindowsChar(char),

  #[error("package path must not contain whitespace (found '{}')", .0.escape_debug())]
  InvalidWhitespace(char),

  #[error("package path must not contain special URL characters (found '{}')", .0.escape_debug())]
  InvalidSpecialUrlChar(char),

  #[error("package path must not contain invalid characters (found '{}')", .0.escape_debug())]
  InvalidOtherChar(char),
}
