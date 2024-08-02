// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Validation logic in this file is shared with registry/api/src/ids.rs

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_core::error::AnyError;
use thiserror::Error;

use crate::args::CliOptions;

use super::diagnostics::PublishDiagnostic;
use super::diagnostics::PublishDiagnosticsCollector;

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

pub struct CollectedPublishPath {
  pub specifier: ModuleSpecifier,
  pub path: PathBuf,
  /// Relative path to use in the tarball. This should be prefixed with a `/`.
  pub relative_path: String,
  /// Specify the contents for any injected paths.
  pub maybe_content: Option<Vec<u8>>,
}

pub struct CollectPublishPathsOptions<'a> {
  pub root_dir: &'a Path,
  pub cli_options: &'a CliOptions,
  pub file_patterns: FilePatterns,
  pub force_include_paths: Vec<PathBuf>,
  pub diagnostics_collector: &'a PublishDiagnosticsCollector,
}

pub fn collect_publish_paths(
  opts: CollectPublishPathsOptions,
) -> Result<Vec<CollectedPublishPath>, AnyError> {
  let diagnostics_collector = opts.diagnostics_collector;
  let publish_paths =
    collect_paths(opts.cli_options, diagnostics_collector, opts.file_patterns)?;
  let publish_paths_set = publish_paths.iter().cloned().collect::<HashSet<_>>();
  let capacity = publish_paths.len() + opts.force_include_paths.len();
  let mut paths = HashSet::with_capacity(capacity);
  let mut result = Vec::with_capacity(capacity);
  let force_include_paths = opts
    .force_include_paths
    .into_iter()
    .filter(|path| !publish_paths_set.contains(path));
  for path in publish_paths.into_iter().chain(force_include_paths) {
    let Ok(specifier) = ModuleSpecifier::from_file_path(&path) else {
      diagnostics_collector
        .to_owned()
        .push(PublishDiagnostic::InvalidPath {
          path: path.to_path_buf(),
          message: "unable to convert path to url".to_string(),
        });
      continue;
    };

    let Ok(relative_path) = path.strip_prefix(opts.root_dir) else {
      diagnostics_collector
        .to_owned()
        .push(PublishDiagnostic::InvalidPath {
          path: path.to_path_buf(),
          message: "path is not in publish directory".to_string(),
        });
      continue;
    };

    let relative_path =
      relative_path
        .components()
        .fold("".to_string(), |mut path, component| {
          path.push('/');
          match component {
            std::path::Component::Normal(normal) => {
              path.push_str(&normal.to_string_lossy())
            }
            std::path::Component::CurDir => path.push('.'),
            std::path::Component::ParentDir => path.push_str(".."),
            _ => unreachable!(),
          }
          path
        });

    match PackagePath::new(relative_path.clone()) {
      Ok(package_path) => {
        if !paths.insert(package_path) {
          diagnostics_collector.to_owned().push(
            PublishDiagnostic::DuplicatePath {
              path: path.to_path_buf(),
            },
          );
        }
      }
      Err(err) => {
        diagnostics_collector
          .to_owned()
          .push(PublishDiagnostic::InvalidPath {
            path: path.to_path_buf(),
            message: err.to_string(),
          });
      }
    }

    let media_type = MediaType::from_specifier(&specifier);
    if matches!(media_type, MediaType::Jsx | MediaType::Tsx) {
      diagnostics_collector.push(PublishDiagnostic::UnsupportedJsxTsx {
        specifier: specifier.clone(),
      });
    }

    result.push(CollectedPublishPath {
      specifier,
      path,
      relative_path,
      maybe_content: None,
    });
  }

  Ok(result)
}

fn collect_paths(
  cli_options: &CliOptions,
  diagnostics_collector: &PublishDiagnosticsCollector,
  file_patterns: FilePatterns,
) -> Result<Vec<PathBuf>, AnyError> {
  FileCollector::new(|e| {
    if !e.metadata.is_file {
      if let Ok(specifier) = ModuleSpecifier::from_file_path(e.path) {
        diagnostics_collector.push(PublishDiagnostic::UnsupportedFileType {
          specifier,
          kind: if e.metadata.is_symlink {
            "symlink".to_string()
          } else {
            "Unknown".to_string()
          },
        });
      }
      return false;
    }
    e.path
      .file_name()
      .map(|s| s != ".DS_Store" && s != ".gitignore")
      .unwrap_or(true)
  })
  .ignore_git_folder()
  .ignore_node_modules()
  .set_vendor_folder(cli_options.vendor_dir_path().map(ToOwned::to_owned))
  .use_gitignore()
  .collect_file_patterns(&deno_config::fs::RealDenoConfigFs, file_patterns)
}
