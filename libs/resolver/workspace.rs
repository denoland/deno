// Copyright 2018-2025 the Deno authors. MIT license.

// use super::UrlRc;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;

use deno_config::deno_json::ConfigFileError;
use deno_config::workspace::ResolverWorkspaceJsrPackage;
use deno_config::workspace::Workspace;
use deno_error::JsError;
use deno_maybe_sync::MaybeDashMap;
use deno_maybe_sync::new_rc;
use deno_media_type::MediaType;
use deno_npm::registry::NpmPackageVersionInfo;
use deno_package_json::PackageJsonDepValue;
use deno_package_json::PackageJsonDepValueParseError;
use deno_package_json::PackageJsonDepWorkspaceReq;
use deno_package_json::PackageJsonDepsRc;
use deno_package_json::PackageJsonRc;
use deno_path_util::url_from_directory_path;
use deno_path_util::url_from_file_path;
use deno_path_util::url_to_file_path;
use deno_semver::RangeSetOrTag;
use deno_semver::SmallStackString;
use deno_semver::StackString;
use deno_semver::Version;
use deno_semver::VersionReq;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::package::PackageName;
use deno_semver::package::PackageReq;
use deno_terminal::colors;
use import_map::ImportMap;
use import_map::ImportMapDiagnostic;
use import_map::ImportMapError;
use import_map::ImportMapErrorKind;
use import_map::ImportMapWithDiagnostics;
use import_map::specifier::SpecifierError;
use indexmap::IndexMap;
use node_resolver::NodeResolutionKind;
use parking_lot::RwLock;
use serde::Deserialize;
use serde::Serialize;
use sys_traits::FsMetadata;
use sys_traits::FsMetadataValue;
use sys_traits::FsRead;
use thiserror::Error;
use url::Url;

use crate::collections::FolderScopedMap;
use crate::deno_json::CompilerOptionsModuleResolution;
use crate::deno_json::CompilerOptionsPaths;
use crate::deno_json::CompilerOptionsResolverRc;

#[allow(clippy::disallowed_types)]
type UrlRc = deno_maybe_sync::MaybeArc<Url>;

#[derive(Debug)]
struct PkgJsonResolverFolderConfig {
  deps: PackageJsonDepsRc,
  pkg_json: PackageJsonRc,
}

#[derive(Debug, Error, JsError)]
pub enum WorkspaceResolverCreateError {
  #[class(inherit)]
  #[error("Failed loading import map specified in '{referrer}'")]
  ImportMapFetch {
    referrer: Url,
    #[source]
    #[inherit]
    source: Box<ConfigFileError>,
  },
  #[class(inherit)]
  #[error(transparent)]
  ImportMap(
    #[from]
    #[inherit]
    ImportMapError,
  ),
}

/// Whether to resolve dependencies by reading the dependencies list
/// from a package.json
#[derive(
  Debug, Default, Serialize, Deserialize, Copy, Clone, PartialEq, Eq,
)]
pub enum PackageJsonDepResolution {
  /// Resolves based on the dep entries in the package.json.
  #[default]
  Enabled,
  /// Doesn't use the package.json to resolve dependencies. Let's the caller
  /// resolve based on the file system.
  Disabled,
}

#[derive(
  Debug, Default, Serialize, Deserialize, Copy, Clone, PartialEq, Eq,
)]
pub enum SloppyImportsOptions {
  Enabled,
  #[default]
  Unspecified,
}

/// Toggle FS metadata caching when probing files for sloppy imports and
/// `compilerOptions.rootDirs` resolution.
#[derive(
  Debug, Default, Serialize, Deserialize, Copy, Clone, PartialEq, Eq,
)]
pub enum FsCacheOptions {
  #[default]
  Enabled,
  Disabled,
}

#[derive(Debug, Default, Clone)]
pub struct CreateResolverOptions {
  pub pkg_json_dep_resolution: PackageJsonDepResolution,
  pub specified_import_map: Option<SpecifiedImportMap>,
  pub sloppy_imports_options: SloppyImportsOptions,
  pub fs_cache_options: FsCacheOptions,
}

#[derive(Debug, Clone)]
pub struct SpecifiedImportMap {
  pub base_url: Url,
  pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappedResolutionDiagnostic {
  ConstraintNotMatchedLocalVersion {
    /// If it was for a link (true) or workspace (false) member.
    is_link: bool,
    reference: JsrPackageReqReference,
    local_version: Version,
  },
}

impl std::fmt::Display for MappedResolutionDiagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::ConstraintNotMatchedLocalVersion {
        is_link,
        reference,
        local_version,
      } => {
        write!(
          f,
          "{0} '{1}@{2}' was not used because it did not match '{1}@{3}'",
          if *is_link {
            "Linked package"
          } else {
            "Workspace member"
          },
          reference.req().name,
          local_version,
          reference.req().version_req
        )
      }
    }
  }
}

#[derive(Debug, Clone)]
pub enum MappedResolution<'a> {
  Normal {
    specifier: Url,
    sloppy_reason: Option<SloppyImportsResolutionReason>,
    used_import_map: bool,
    used_compiler_options_root_dirs: bool,
    maybe_diagnostic: Option<Box<MappedResolutionDiagnostic>>,
  },
  WorkspaceJsrPackage {
    specifier: Url,
    pkg_req_ref: JsrPackageReqReference,
  },
  /// Resolved a bare specifier to a package.json that was a workspace member.
  WorkspaceNpmPackage {
    target_pkg_json: &'a PackageJsonRc,
    pkg_name: &'a str,
    sub_path: Option<String>,
  },
  PackageJson {
    pkg_json: &'a PackageJsonRc,
    alias: &'a str,
    sub_path: Option<String>,
    dep_result: &'a Result<PackageJsonDepValue, PackageJsonDepValueParseError>,
  },
  PackageJsonImport {
    pkg_json: &'a PackageJsonRc,
  },
}

#[derive(Debug, Clone, Error, JsError)]
#[class(type)]
pub enum WorkspaceResolveError {
  #[error("Failed joining '{}' to '{}'. {:#}", .sub_path, .base, .error)]
  InvalidExportPath {
    base: Url,
    sub_path: String,
    error: url::ParseError,
  },
  #[error("Unknown export '{}' for '{}'.\n  Package exports:\n{}", export_name, package_name, .exports.iter().map(|e| format!(" * {}", e)).collect::<Vec<_>>().join("\n"))]
  UnknownExport {
    package_name: String,
    export_name: String,
    exports: Vec<String>,
  },
}

#[derive(Debug, Error, JsError)]
#[class(type)]
#[error(
  "Import \"{}\" via 'compilerOptions.paths[\"{}\"]' did not match an existing file", prior_resolution.as_ref().map(|s| s.as_str()).unwrap_or(specifier.as_str()), matched_key
)]
pub struct NotFoundInCompilerOptionsPathsError {
  specifier: String,
  referrer: Url,
  matched_key: String,
  prior_resolution: Option<Url>,
}

#[derive(Debug, Error, JsError)]
pub enum MappedResolutionError {
  #[class(inherit)]
  #[error(transparent)]
  Specifier(#[from] SpecifierError),
  #[class(inherit)]
  #[error(transparent)]
  ImportMap(#[from] ImportMapError),
  #[class(inherit)]
  #[error(transparent)]
  Workspace(#[from] WorkspaceResolveError),
  #[class(inherit)]
  #[error(transparent)]
  NotFoundInCompilerOptionsPaths(
    #[from] Box<NotFoundInCompilerOptionsPathsError>,
  ),
}

impl MappedResolutionError {
  pub fn is_unmapped_bare_specifier(&self) -> bool {
    match self {
      MappedResolutionError::Specifier(err) => match err {
        SpecifierError::InvalidUrl(_) => false,
        SpecifierError::ImportPrefixMissing { .. } => true,
      },
      MappedResolutionError::ImportMap(err) => {
        matches!(**err, ImportMapErrorKind::UnmappedBareSpecifier(_, _))
      }
      MappedResolutionError::Workspace(_) => false,
      MappedResolutionError::NotFoundInCompilerOptionsPaths(_) => false,
    }
  }
}

#[derive(Error, Debug, JsError)]
#[class(inherit)]
#[error(transparent)]
pub struct WorkspaceResolvePkgJsonFolderError(
  Box<WorkspaceResolvePkgJsonFolderErrorKind>,
);

impl WorkspaceResolvePkgJsonFolderError {
  pub fn as_kind(&self) -> &WorkspaceResolvePkgJsonFolderErrorKind {
    &self.0
  }

  pub fn into_kind(self) -> WorkspaceResolvePkgJsonFolderErrorKind {
    *self.0
  }
}

impl<E> From<E> for WorkspaceResolvePkgJsonFolderError
where
  WorkspaceResolvePkgJsonFolderErrorKind: From<E>,
{
  fn from(err: E) -> Self {
    WorkspaceResolvePkgJsonFolderError(Box::new(
      WorkspaceResolvePkgJsonFolderErrorKind::from(err),
    ))
  }
}

#[derive(Debug, Error, JsError, Clone, PartialEq, Eq)]
#[class(type)]
pub enum WorkspaceResolvePkgJsonFolderErrorKind {
  #[error("Could not find package.json with name '{0}' in workspace.")]
  NotFound(String),
  #[error(
    "Found package.json in workspace, but version '{1}' didn't satisfy constraint '{0}'."
  )]
  VersionNotSatisfied(VersionReq, Version),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CachedMetadataFsEntry {
  File,
  Dir,
}

#[derive(Debug)]
struct CachedMetadataFs<TSys: FsMetadata> {
  sys: TSys,
  cache: Option<MaybeDashMap<PathBuf, Option<CachedMetadataFsEntry>>>,
}

impl<TSys: FsMetadata> CachedMetadataFs<TSys> {
  fn new(sys: TSys, options: FsCacheOptions) -> Self {
    Self {
      sys,
      cache: match options {
        FsCacheOptions::Enabled => Some(Default::default()),
        FsCacheOptions::Disabled => None,
      },
    }
  }

  fn stat_sync(&self, path: &Path) -> Option<CachedMetadataFsEntry> {
    if let Some(cache) = &self.cache
      && let Some(entry) = cache.get(path)
    {
      return *entry;
    }
    let entry = self.sys.fs_metadata(path).ok().and_then(|stat| {
      if stat.file_type().is_file() {
        Some(CachedMetadataFsEntry::File)
      } else if stat.file_type().is_dir() {
        Some(CachedMetadataFsEntry::Dir)
      } else {
        None
      }
    });
    if let Some(cache) = &self.cache {
      cache.insert(path.to_owned(), entry);
    }
    entry
  }

  fn is_file(&self, path: &Path) -> bool {
    self.stat_sync(path) == Some(CachedMetadataFsEntry::File)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SloppyImportsResolutionReason {
  /// Ex. `./file.js` to `./file.ts`
  JsToTs,
  /// Ex. `./file` to `./file.ts`
  NoExtension,
  /// Ex. `./dir` to `./dir/index.ts`
  Directory,
}

impl SloppyImportsResolutionReason {
  pub fn suggestion_message_for_specifier(&self, specifier: &Url) -> String {
    format!("Maybe {}", self.base_message_for_specifier(specifier))
  }

  pub fn quick_fix_message_for_specifier(&self, specifier: &Url) -> String {
    let message = self.base_message_for_specifier(specifier);
    let mut chars = message.chars();
    format!(
      "{}{}.",
      chars.next().unwrap().to_uppercase(),
      chars.as_str()
    )
  }

  fn base_message_for_specifier(&self, specifier: &Url) -> String {
    match self {
      Self::JsToTs => {
        let media_type = MediaType::from_specifier(specifier);
        format!("change the extension to '{}'", media_type.as_ts_extension())
      }
      Self::NoExtension => {
        let media_type = MediaType::from_specifier(specifier);
        format!("add a '{}' extension", media_type.as_ts_extension())
      }
      Self::Directory => {
        let file_name = specifier
          .path()
          .rsplit_once('/')
          .map(|(_, file_name)| file_name)
          .unwrap_or(specifier.path());
        format!("specify path to '{}' file in directory instead", file_name)
      }
    }
  }
}

#[derive(Debug)]
struct SloppyImportsResolver<TSys: FsMetadata> {
  compiler_options_resolver: CompilerOptionsResolverCellRc,
  fs: CachedMetadataFs<TSys>,
  enabled_by_options: bool,
}

impl<TSys: FsMetadata> SloppyImportsResolver<TSys> {
  fn new(
    fs: CachedMetadataFs<TSys>,
    compiler_options_resolver: CompilerOptionsResolverCellRc,
    options: SloppyImportsOptions,
  ) -> Self {
    Self {
      fs,
      compiler_options_resolver,
      enabled_by_options: match options {
        SloppyImportsOptions::Enabled => true,
        SloppyImportsOptions::Unspecified => false,
      },
    }
  }

  fn resolve(
    &self,
    specifier: &Url,
    referrer: &Url,
    resolution_kind: ResolutionKind,
  ) -> Option<(Url, SloppyImportsResolutionReason)> {
    if !self.enabled_by_options
      && self
        .compiler_options_resolver
        .read()
        .for_specifier(referrer)
        .module_resolution()
        != CompilerOptionsModuleResolution::Bundler
    {
      return None;
    }

    fn path_without_ext(
      path: &Path,
      media_type: MediaType,
    ) -> Option<Cow<'_, str>> {
      let old_path_str = path.to_string_lossy();
      match media_type {
        MediaType::Unknown => Some(old_path_str),
        _ => old_path_str
          .strip_suffix(media_type.as_ts_extension())
          .map(|s| Cow::Owned(s.to_string())),
      }
    }

    fn media_types_to_paths(
      path_no_ext: &str,
      original_media_type: MediaType,
      probe_media_type_types: Vec<MediaType>,
      reason: SloppyImportsResolutionReason,
    ) -> Vec<(PathBuf, SloppyImportsResolutionReason)> {
      probe_media_type_types
        .into_iter()
        .filter(|media_type| *media_type != original_media_type)
        .map(|media_type| {
          (
            PathBuf::from(format!(
              "{}{}",
              path_no_ext,
              media_type.as_ts_extension()
            )),
            reason,
          )
        })
        .collect::<Vec<_>>()
    }

    if specifier.scheme() != "file" {
      return None;
    }

    let path = url_to_file_path(specifier).ok()?;

    let probe_paths: Vec<(PathBuf, SloppyImportsResolutionReason)> =
      match self.fs.stat_sync(&path) {
        Some(CachedMetadataFsEntry::File) => {
          if resolution_kind.is_types() {
            let media_type = MediaType::from_specifier(specifier);
            // attempt to resolve the .d.ts file before the .js file
            let probe_media_type_types = match media_type {
              MediaType::JavaScript => {
                vec![(MediaType::Dts), MediaType::JavaScript]
              }
              MediaType::Mjs => {
                vec![MediaType::Dmts, MediaType::Dts, MediaType::Mjs]
              }
              MediaType::Cjs => {
                vec![MediaType::Dcts, MediaType::Dts, MediaType::Cjs]
              }
              _ => return None,
            };
            let path_no_ext = path_without_ext(&path, media_type)?;
            media_types_to_paths(
              &path_no_ext,
              media_type,
              probe_media_type_types,
              SloppyImportsResolutionReason::JsToTs,
            )
          } else {
            return None;
          }
        }
        entry @ None | entry @ Some(CachedMetadataFsEntry::Dir) => {
          let media_type = MediaType::from_specifier(specifier);
          let probe_media_type_types = match media_type {
            MediaType::JavaScript => (
              if resolution_kind.is_types() {
                vec![MediaType::TypeScript, MediaType::Tsx, MediaType::Dts]
              } else {
                vec![MediaType::TypeScript, MediaType::Tsx]
              },
              SloppyImportsResolutionReason::JsToTs,
            ),
            MediaType::Jsx => {
              (vec![MediaType::Tsx], SloppyImportsResolutionReason::JsToTs)
            }
            MediaType::Mjs => (
              if resolution_kind.is_types() {
                vec![MediaType::Mts, MediaType::Dmts, MediaType::Dts]
              } else {
                vec![MediaType::Mts]
              },
              SloppyImportsResolutionReason::JsToTs,
            ),
            MediaType::Cjs => (
              if resolution_kind.is_types() {
                vec![MediaType::Cts, MediaType::Dcts, MediaType::Dts]
              } else {
                vec![MediaType::Cts]
              },
              SloppyImportsResolutionReason::JsToTs,
            ),
            MediaType::TypeScript
            | MediaType::Mts
            | MediaType::Cts
            | MediaType::Dts
            | MediaType::Dmts
            | MediaType::Dcts
            | MediaType::Tsx
            | MediaType::Json
            | MediaType::Jsonc
            | MediaType::Json5
            | MediaType::Wasm
            | MediaType::Css
            | MediaType::Html
            | MediaType::Sql
            | MediaType::SourceMap => {
              return None;
            }
            MediaType::Unknown => (
              if resolution_kind.is_types() {
                vec![
                  MediaType::TypeScript,
                  MediaType::Tsx,
                  MediaType::Mts,
                  MediaType::Dts,
                  MediaType::Dmts,
                  MediaType::Dcts,
                  MediaType::JavaScript,
                  MediaType::Jsx,
                  MediaType::Mjs,
                ]
              } else {
                vec![
                  MediaType::TypeScript,
                  MediaType::JavaScript,
                  MediaType::Tsx,
                  MediaType::Jsx,
                  MediaType::Mts,
                  MediaType::Mjs,
                ]
              },
              SloppyImportsResolutionReason::NoExtension,
            ),
          };
          let mut probe_paths = match path_without_ext(&path, media_type) {
            Some(path_no_ext) => media_types_to_paths(
              &path_no_ext,
              media_type,
              probe_media_type_types.0,
              probe_media_type_types.1,
            ),
            None => vec![],
          };

          if matches!(entry, Some(CachedMetadataFsEntry::Dir)) {
            // try to resolve at the index file
            if resolution_kind.is_types() {
              probe_paths.push((
                path.join("index.ts"),
                SloppyImportsResolutionReason::Directory,
              ));

              probe_paths.push((
                path.join("index.mts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.d.ts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.d.mts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.js"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.mjs"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.tsx"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.jsx"),
                SloppyImportsResolutionReason::Directory,
              ));
            } else {
              probe_paths.push((
                path.join("index.ts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.mts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.tsx"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.js"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.mjs"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.jsx"),
                SloppyImportsResolutionReason::Directory,
              ));
            }
          }
          if probe_paths.is_empty() {
            return None;
          }
          probe_paths
        }
      };

    for (probe_path, reason) in probe_paths {
      if self.fs.is_file(&probe_path)
        && let Ok(specifier) = url_from_file_path(&probe_path)
      {
        return Some((specifier, reason));
      }
    }

    None
  }
}

pub fn sloppy_imports_resolve<TSys: FsMetadata>(
  specifier: &Url,
  resolution_kind: ResolutionKind,
  sys: TSys,
) -> Option<(Url, SloppyImportsResolutionReason)> {
  SloppyImportsResolver::new(
    CachedMetadataFs::new(sys, FsCacheOptions::Enabled),
    Default::default(),
    SloppyImportsOptions::Enabled,
  )
  // The referrer is used to determine the applicable compiler options, which
  // can force-override `SloppyImportOptions::Disabled` depending on
  // `moduleResolution`. But `SloppyImportOptions::Enabled` is set.
  .resolve(specifier, &Url::parse("unknown:").unwrap(), resolution_kind)
}

#[allow(clippy::disallowed_types)]
type SloppyImportsResolverRc<T> =
  deno_maybe_sync::MaybeArc<SloppyImportsResolver<T>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilerOptionsRootDirsDiagnostic {
  InvalidType(Url),
  InvalidEntryType(Url, usize),
  UnexpectedError(Url, String),
  UnexpectedEntryError(Url, usize, String),
}

impl fmt::Display for CompilerOptionsRootDirsDiagnostic {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Self::InvalidType(s) => write!(
        f,
        "Invalid value for \"compilerOptions.rootDirs\" (\"{s}\"). Expected a string."
      ),
      Self::InvalidEntryType(s, i) => write!(
        f,
        "Invalid value for \"compilerOptions.rootDirs[{i}]\" (\"{s}\"). Expected a string."
      ),
      Self::UnexpectedError(s, message) => write!(
        f,
        "Unexpected error while parsing \"compilerOptions.rootDirs\" (\"{s}\"): {message}"
      ),
      Self::UnexpectedEntryError(s, i, message) => write!(
        f,
        "Unexpected error while parsing \"compilerOptions.rootDirs[{i}]\" (\"{s}\"): {message}"
      ),
    }
  }
}

fn resolve_types_with_compiler_options_paths(
  specifier: &str,
  referrer: &Url,
  paths: &CompilerOptionsPaths,
  sloppy_imports_resolver: &SloppyImportsResolver<impl FsMetadata>,
) -> Option<Result<(Url, Option<SloppyImportsResolutionReason>), String>> {
  if referrer.scheme() != "file" {
    return None;
  }
  let (candidates, matched_key) = paths.resolve_candidates(specifier)?;
  for candidate_specifier in candidates {
    let Ok(candidate_path) = url_to_file_path(&candidate_specifier) else {
      continue;
    };
    if sloppy_imports_resolver.fs.is_file(&candidate_path) {
      return Some(Ok((candidate_specifier, None)));
    } else if let Some((candidate_specifier, sloppy_reason)) =
      sloppy_imports_resolver.resolve(
        &candidate_specifier,
        referrer,
        ResolutionKind::Types,
      )
    {
      return Some(Ok((candidate_specifier, Some(sloppy_reason))));
    }
  }
  Some(Err(matched_key))
}

fn resolve_types_with_compiler_options_root_dirs(
  specifier: &Url,
  referrer: &Url,
  root_dirs: &[Url],
  sloppy_imports_resolver: &SloppyImportsResolver<impl FsMetadata>,
) -> Option<(Url, Option<SloppyImportsResolutionReason>)> {
  if specifier.scheme() != "file" || referrer.scheme() != "file" {
    return None;
  }
  let (matched_root_dir, suffix) = root_dirs
    .iter()
    .filter_map(|r| {
      let suffix = specifier.as_str().strip_prefix(r.as_str())?;
      Some((r, suffix))
    })
    .max_by_key(|(r, _)| r.as_str().len())?;
  for root_dir in root_dirs {
    if root_dir == matched_root_dir {
      continue;
    }
    let Ok(candidate_specifier) = root_dir.join(suffix) else {
      continue;
    };
    let Ok(candidate_path) = url_to_file_path(&candidate_specifier) else {
      continue;
    };
    if sloppy_imports_resolver.fs.is_file(&candidate_path) {
      return Some((candidate_specifier, None));
    } else if let Some((candidate_specifier, sloppy_reason)) =
      sloppy_imports_resolver.resolve(
        &candidate_specifier,
        referrer,
        ResolutionKind::Types,
      )
    {
      return Some((candidate_specifier, Some(sloppy_reason)));
    }
  }
  None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolutionKind {
  /// Resolving for code that will be executed.
  Execution,
  /// Resolving for code that will be used for type information.
  Types,
}

impl ResolutionKind {
  pub fn is_types(&self) -> bool {
    *self == ResolutionKind::Types
  }
}

impl From<NodeResolutionKind> for ResolutionKind {
  fn from(value: NodeResolutionKind) -> Self {
    match value {
      NodeResolutionKind::Execution => Self::Execution,
      NodeResolutionKind::Types => Self::Types,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceResolverDiagnostic<'a> {
  ImportMap(&'a ImportMapDiagnostic),
  CompilerOptionsRootDirs(&'a CompilerOptionsRootDirsDiagnostic),
}

impl fmt::Display for WorkspaceResolverDiagnostic<'_> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Self::ImportMap(d) => write!(f, "Import map: {d}"),
      Self::CompilerOptionsRootDirs(d) => d.fmt(f),
    }
  }
}

#[allow(clippy::disallowed_types)]
type CompilerOptionsResolverCellRc =
  deno_maybe_sync::MaybeArc<RwLock<CompilerOptionsResolverRc>>;

#[derive(Debug)]
pub struct WorkspaceResolver<TSys: FsMetadata + FsRead> {
  workspace_root: UrlRc,
  jsr_pkgs: Vec<ResolverWorkspaceJsrPackage>,
  maybe_import_map: Option<ImportMapWithDiagnostics>,
  pkg_jsons: FolderScopedMap<PkgJsonResolverFolderConfig>,
  pkg_json_dep_resolution: PackageJsonDepResolution,
  sloppy_imports_options: SloppyImportsOptions,
  fs_cache_options: FsCacheOptions,
  compiler_options_resolver: CompilerOptionsResolverCellRc,
  sloppy_imports_resolver: SloppyImportsResolverRc<TSys>,
}

impl<TSys: FsMetadata + FsRead> WorkspaceResolver<TSys> {
  pub fn from_workspace(
    workspace: &Workspace,
    sys: TSys,
    options: CreateResolverOptions,
  ) -> Result<Self, WorkspaceResolverCreateError> {
    fn resolve_import_map(
      sys: &impl FsRead,
      workspace: &Workspace,
      specified_import_map: Option<SpecifiedImportMap>,
    ) -> Result<Option<ImportMapWithDiagnostics>, WorkspaceResolverCreateError>
    {
      let root_deno_json = workspace.root_deno_json();
      let deno_jsons = workspace.resolver_deno_jsons().collect::<Vec<_>>();

      let (import_map_url, import_map) = match specified_import_map {
        Some(SpecifiedImportMap {
          base_url,
          value: import_map,
        }) => (base_url, import_map),
        None => {
          if !deno_jsons.iter().any(|p| p.is_package())
            && !deno_jsons.iter().any(|c| {
              c.json.import_map.is_some()
                || c.json.scopes.is_some()
                || c.json.imports.is_some()
                || c
                  .json
                  .compiler_options
                  .as_ref()
                  .and_then(|v| v.as_object()?.get("rootDirs")?.as_array())
                  .is_some_and(|a| a.len() > 1)
            })
          {
            // no configs have an import map and none are a package, so exit
            return Ok(None);
          }

          let config_specified_import_map = match root_deno_json.as_ref() {
            Some(deno_json) => deno_json
              .to_import_map_value(sys)
              .map_err(|source| WorkspaceResolverCreateError::ImportMapFetch {
                referrer: deno_json.specifier.clone(),
                source: Box::new(source),
              })?
              .unwrap_or_else(|| {
                (
                  Cow::Borrowed(&deno_json.specifier),
                  serde_json::Value::Object(Default::default()),
                )
              }),
            None => (
              Cow::Owned(workspace.root_dir_url().join("deno.json").unwrap()),
              serde_json::Value::Object(Default::default()),
            ),
          };
          let base_import_map_config = import_map::ext::ImportMapConfig {
            base_url: config_specified_import_map.0.into_owned(),
            import_map_value: config_specified_import_map.1,
          };
          let child_import_map_configs = deno_jsons
            .iter()
            .filter(|f| {
              Some(&f.specifier)
                != root_deno_json.as_ref().map(|c| &c.specifier)
            })
            .map(|config| import_map::ext::ImportMapConfig {
              base_url: config.specifier.clone(),
              import_map_value: {
                // don't include scopes here
                let mut value = serde_json::Map::with_capacity(1);
                if let Some(imports) = &config.json.imports {
                  value.insert("imports".to_string(), imports.clone());
                }
                value.into()
              },
            })
            .collect::<Vec<_>>();
          let (import_map_url, import_map) =
            ::import_map::ext::create_synthetic_import_map(
              base_import_map_config,
              child_import_map_configs,
            );
          let import_map = import_map::ext::expand_import_map_value(import_map);
          log::debug!(
            "Workspace config generated this import map {}",
            serde_json::to_string_pretty(&import_map).unwrap()
          );
          (import_map_url, import_map)
        }
      };
      Ok(Some(import_map::parse_from_value(
        import_map_url,
        import_map,
      )?))
    }

    let maybe_import_map =
      resolve_import_map(&sys, workspace, options.specified_import_map)?;
    let jsr_pkgs = workspace.resolver_jsr_pkgs().collect::<Vec<_>>();
    let pkg_jsons = workspace
      .resolver_pkg_jsons()
      .map(|(dir_url, pkg_json)| {
        let deps = pkg_json.resolve_local_package_json_deps();
        (
          dir_url.clone(),
          PkgJsonResolverFolderConfig {
            deps: deps.clone(),
            pkg_json: pkg_json.clone(),
          },
        )
      })
      .collect::<BTreeMap<_, _>>();

    let fs = CachedMetadataFs::new(sys, options.fs_cache_options);
    let compiler_options_resolver = CompilerOptionsResolverCellRc::default();
    let sloppy_imports_resolver = new_rc(SloppyImportsResolver::new(
      fs,
      compiler_options_resolver.clone(),
      options.sloppy_imports_options,
    ));

    Ok(Self {
      workspace_root: workspace.root_dir_url().clone(),
      pkg_json_dep_resolution: options.pkg_json_dep_resolution,
      jsr_pkgs,
      maybe_import_map,
      pkg_jsons: FolderScopedMap::from_map(pkg_jsons),
      sloppy_imports_options: options.sloppy_imports_options,
      fs_cache_options: options.fs_cache_options,
      compiler_options_resolver,
      sloppy_imports_resolver,
    })
  }

  /// Creates a new WorkspaceResolver from the specified import map and package.jsons.
  ///
  /// Generally, create this from a Workspace instead.
  #[allow(clippy::too_many_arguments)]
  pub fn new_raw(
    workspace_root: UrlRc,
    maybe_import_map: Option<ImportMap>,
    jsr_pkgs: Vec<ResolverWorkspaceJsrPackage>,
    pkg_jsons: Vec<PackageJsonRc>,
    pkg_json_dep_resolution: PackageJsonDepResolution,
    sloppy_imports_options: SloppyImportsOptions,
    fs_cache_options: FsCacheOptions,
    sys: TSys,
  ) -> Self {
    let maybe_import_map =
      maybe_import_map.map(|import_map| ImportMapWithDiagnostics {
        import_map,
        diagnostics: Default::default(),
      });
    let pkg_jsons = pkg_jsons
      .into_iter()
      .map(|pkg_json| {
        let deps = pkg_json.resolve_local_package_json_deps();
        (
          new_rc(
            url_from_directory_path(pkg_json.path.parent().unwrap()).unwrap(),
          ),
          PkgJsonResolverFolderConfig {
            deps: deps.clone(),
            pkg_json,
          },
        )
      })
      .collect::<BTreeMap<_, _>>();
    let fs = CachedMetadataFs::new(sys, fs_cache_options);
    let compiler_options_resolver = CompilerOptionsResolverCellRc::default();
    let sloppy_imports_resolver = new_rc(SloppyImportsResolver::new(
      fs,
      compiler_options_resolver.clone(),
      sloppy_imports_options,
    ));
    Self {
      workspace_root,
      jsr_pkgs,
      maybe_import_map,
      pkg_jsons: FolderScopedMap::from_map(pkg_jsons),
      pkg_json_dep_resolution,
      sloppy_imports_options,
      fs_cache_options,
      compiler_options_resolver,
      sloppy_imports_resolver,
    }
  }

  /// Prepare the workspace resolver for serialization
  ///
  /// The most significant preparation involves converting
  /// absolute paths into relative (based on `root_dir_url`).
  /// It also takes care of pre-serializing non-serde internal data.
  pub fn to_serializable(
    &self,
    root_dir_url: &Url,
  ) -> SerializableWorkspaceResolver<'_> {
    let root_dir_url = BaseUrl(root_dir_url);
    SerializableWorkspaceResolver {
      import_map: self.maybe_import_map().map(|i| {
        SerializedWorkspaceResolverImportMap {
          specifier: root_dir_url.make_relative_if_descendant(i.base_url()),
          json: Cow::Owned(i.to_json()),
        }
      }),
      jsr_pkgs: self
        .jsr_packages()
        .iter()
        .map(|pkg| SerializedResolverWorkspaceJsrPackage {
          relative_base: root_dir_url.make_relative_if_descendant(&pkg.base),
          name: Cow::Borrowed(&pkg.name),
          version: Cow::Borrowed(&pkg.version),
          exports: Cow::Borrowed(&pkg.exports),
        })
        .collect(),
      package_jsons: self
        .package_jsons()
        .map(|pkg_json| {
          (
            root_dir_url
              .make_relative_if_descendant(&pkg_json.specifier())
              .into_owned(),
            serde_json::to_value(pkg_json).unwrap(),
          )
        })
        .collect(),
      pkg_json_resolution: self.pkg_json_dep_resolution(),
      sloppy_imports_options: self.sloppy_imports_options,
      fs_cache_options: self.fs_cache_options,
    }
  }

  /// Deserialize a `WorkspaceResolver`
  ///
  /// Deserialization of `WorkspaceResolver`s is made in two steps. First
  /// the serialized data must be deserialized in to `SerializableWorkspaceResolver`
  /// (usually with serde), and then this method converts it into a `WorkspaceResolver`.
  ///
  /// This second step involves mainly converting the relative paths within
  /// `SerializableWorkspaceResolver` into absolute paths using `root_dir_url`.
  pub fn try_from_serializable(
    root_dir_url: Url,
    serializable_workspace_resolver: SerializableWorkspaceResolver,
    sys: TSys,
  ) -> Result<Self, ImportMapError> {
    let import_map = match serializable_workspace_resolver.import_map {
      Some(import_map) => Some(
        import_map::parse_from_json_with_options(
          root_dir_url.join(&import_map.specifier).unwrap(),
          &import_map.json,
          import_map::ImportMapOptions {
            address_hook: None,
            expand_imports: true,
          },
        )?
        .import_map,
      ),
      None => None,
    };
    let pkg_jsons = serializable_workspace_resolver
      .package_jsons
      .into_iter()
      .map(|(relative_path, json)| {
        let path =
          url_to_file_path(&root_dir_url.join(&relative_path).unwrap())
            .unwrap();
        let pkg_json =
          deno_package_json::PackageJson::load_from_value(path, json).unwrap();
        PackageJsonRc::new(pkg_json)
      })
      .collect();
    let jsr_packages = serializable_workspace_resolver
      .jsr_pkgs
      .into_iter()
      .map(|pkg| ResolverWorkspaceJsrPackage {
        is_link: false, // only used for enhancing the diagnostics, which are discarded when serializing
        base: root_dir_url.join(&pkg.relative_base).unwrap(),
        name: pkg.name.into_owned(),
        version: pkg.version.into_owned(),
        exports: pkg.exports.into_owned(),
      })
      .collect();
    Ok(Self::new_raw(
      UrlRc::new(root_dir_url),
      import_map,
      jsr_packages,
      pkg_jsons,
      serializable_workspace_resolver.pkg_json_resolution,
      serializable_workspace_resolver.sloppy_imports_options,
      serializable_workspace_resolver.fs_cache_options,
      sys,
    ))
  }

  pub fn set_compiler_options_resolver(
    &self,
    value: CompilerOptionsResolverRc,
  ) {
    *self.compiler_options_resolver.write() = value;
  }

  pub fn maybe_import_map(&self) -> Option<&ImportMap> {
    self.maybe_import_map.as_ref().map(|c| &c.import_map)
  }

  pub fn package_jsons(&self) -> impl Iterator<Item = &PackageJsonRc> {
    self.pkg_jsons.values().map(|c| &c.pkg_json)
  }

  pub fn jsr_packages(&self) -> &[ResolverWorkspaceJsrPackage] {
    &self.jsr_pkgs
  }

  pub fn diagnostics(&self) -> Vec<WorkspaceResolverDiagnostic<'_>> {
    self
      .maybe_import_map
      .as_ref()
      .iter()
      .flat_map(|c| &c.diagnostics)
      .map(WorkspaceResolverDiagnostic::ImportMap)
      .collect()
  }

  pub fn resolve<'a>(
    &'a self,
    specifier: &str,
    referrer: &Url,
    resolution_kind: ResolutionKind,
  ) -> Result<MappedResolution<'a>, MappedResolutionError> {
    // 1.0. Attempt to resolve with the import map and normally first
    let compiler_options_resolver = self.compiler_options_resolver.read();
    let compiler_options_data =
      compiler_options_resolver.for_specifier(referrer);
    let compiler_options_paths = compiler_options_data.paths();
    let mut used_import_map = false;
    let resolve_result = if let Some(import_map) = &self.maybe_import_map {
      used_import_map = true;
      import_map
        .import_map
        .resolve(specifier, referrer)
        .map_err(MappedResolutionError::ImportMap)
    } else {
      import_map::specifier::resolve_import(specifier, referrer)
        .map_err(MappedResolutionError::Specifier)
    };
    let resolve_error = match resolve_result {
      Ok(mut resolved_specifier) => {
        let mut used_compiler_options_root_dirs = false;
        let mut sloppy_reason = None;
        if let Some((probed_specifier, probed_sloppy_reason)) = self
          .sloppy_imports_resolver
          .resolve(&resolved_specifier, referrer, resolution_kind)
        {
          resolved_specifier = probed_specifier;
          sloppy_reason = Some(probed_sloppy_reason);
        } else if resolution_kind.is_types() {
          // 1.1. Try to match the resolved specifier against
          // `compilerOptions.paths`
          if let Some(paths_result) = resolve_types_with_compiler_options_paths(
            resolved_specifier.as_str(),
            referrer,
            compiler_options_paths,
            &self.sloppy_imports_resolver,
          ) {
            let (probed_specifier, probed_sloppy_reason) = match paths_result {
              Ok(r) => r,
              Err(matched_key) => {
                return Err(
                  MappedResolutionError::NotFoundInCompilerOptionsPaths(
                    Box::new(NotFoundInCompilerOptionsPathsError {
                      specifier: specifier.to_string(),
                      referrer: referrer.clone(),
                      matched_key,
                      prior_resolution: Some(resolved_specifier),
                    }),
                  ),
                );
              }
            };
            {
              resolved_specifier = probed_specifier;
              sloppy_reason = probed_sloppy_reason;
            }

          // 1.2. Try to match the resolved specifier against
          // `compilerOptions.rootDirs`
          } else if let Some((probed_specifier, probed_sloppy_reason)) =
            resolve_types_with_compiler_options_root_dirs(
              &resolved_specifier,
              referrer,
              compiler_options_data.root_dirs(),
              &self.sloppy_imports_resolver,
            )
          {
            used_compiler_options_root_dirs = true;
            resolved_specifier = probed_specifier;
            sloppy_reason = probed_sloppy_reason;
          }
        }
        return self.maybe_resolve_specifier_to_workspace_jsr_pkg(
          MappedResolution::Normal {
            specifier: resolved_specifier,
            sloppy_reason,
            used_import_map,
            used_compiler_options_root_dirs,
            maybe_diagnostic: None,
          },
        );
      }
      Err(err) => err,
    };

    if resolve_error.is_unmapped_bare_specifier() {
      // 2.0. Try to resolve the bare specifier with `compilerOptions.paths`
      if resolution_kind.is_types()
        && let Some(paths_result) = resolve_types_with_compiler_options_paths(
          specifier,
          referrer,
          compiler_options_paths,
          &self.sloppy_imports_resolver,
        )
      {
        let (probed_specifier, probed_sloppy_reason) = match paths_result {
          Ok(r) => r,
          Err(matched_key) => {
            return Err(MappedResolutionError::NotFoundInCompilerOptionsPaths(
              Box::new(NotFoundInCompilerOptionsPathsError {
                specifier: specifier.to_string(),
                referrer: referrer.clone(),
                matched_key,
                prior_resolution: None,
              }),
            ));
          }
        };
        return self.maybe_resolve_specifier_to_workspace_jsr_pkg(
          MappedResolution::Normal {
            specifier: probed_specifier,
            sloppy_reason: probed_sloppy_reason,
            used_import_map: false,
            used_compiler_options_root_dirs: false,
            maybe_diagnostic: None,
          },
        );
      }

      // 2.1. Try to resolve the bare specifier to a workspace member
      for member in &self.jsr_pkgs {
        if let Some(path) = specifier.strip_prefix(&member.name)
          && (path.is_empty() || path.starts_with('/'))
        {
          let path = path.strip_prefix('/').unwrap_or(path);
          let pkg_req_ref = match JsrPackageReqReference::from_str(&format!(
            "jsr:{}{}/{}",
            member.name,
            member
              .version
              .as_ref()
              .map(|v| format!("@^{}", v))
              .unwrap_or_else(String::new),
            path
          )) {
            Ok(pkg_req_ref) => pkg_req_ref,
            Err(_) => {
              // Ignore the error as it will be surfaced as a diagnostic
              // in workspace.diagnostics() routine.
              continue;
            }
          };
          return self.resolve_workspace_jsr_pkg(member, pkg_req_ref);
        }
      }
    }

    // 3. Attempt to resolve from the package.json dependencies or imports.
    if specifier.starts_with('#') && specifier.len() > 1 {
      if let Some((_, pkg_json_folder)) =
        self.pkg_jsons.entry_for_specifier(referrer)
      {
        return Ok(MappedResolution::PackageJsonImport {
          pkg_json: &pkg_json_folder.pkg_json,
        });
      }
    } else if self.pkg_json_dep_resolution == PackageJsonDepResolution::Enabled
    {
      for (_dir_url, pkg_json_folder) in
        self.pkg_jsons.entries_for_specifier(referrer)
      {
        for (bare_specifier, dep_result) in pkg_json_folder
          .deps
          .dependencies
          .iter()
          .chain(pkg_json_folder.deps.dev_dependencies.iter())
        {
          if let Some(path) = specifier.strip_prefix(bare_specifier.as_str())
            && (path.is_empty() || path.starts_with('/'))
          {
            let sub_path = path.strip_prefix('/').unwrap_or(path);
            return Ok(MappedResolution::PackageJson {
              pkg_json: &pkg_json_folder.pkg_json,
              alias: bare_specifier,
              sub_path: if sub_path.is_empty() {
                None
              } else {
                Some(sub_path.to_string())
              },
              dep_result,
            });
          }
        }
      }

      // 4. Try to resolve to a workspace npm package if inside the workspace.
      if referrer.as_str().starts_with(self.workspace_root.as_str()) {
        for pkg_json_folder in self.pkg_jsons.values() {
          let Some(name) = &pkg_json_folder.pkg_json.name else {
            continue;
          };
          let Some(path) = specifier.strip_prefix(name) else {
            continue;
          };
          if path.is_empty() || path.starts_with('/') {
            let sub_path = path.strip_prefix('/').unwrap_or(path);
            return Ok(MappedResolution::WorkspaceNpmPackage {
              target_pkg_json: &pkg_json_folder.pkg_json,
              pkg_name: name,
              sub_path: if sub_path.is_empty() {
                None
              } else {
                Some(sub_path.to_string())
              },
            });
          }
        }
      }
    }

    // wasn't found, so surface the initial resolve error
    Err(resolve_error)
  }

  fn maybe_resolve_specifier_to_workspace_jsr_pkg<'a>(
    &'a self,
    resolution: MappedResolution<'a>,
  ) -> Result<MappedResolution<'a>, MappedResolutionError> {
    let specifier = match resolution {
      MappedResolution::Normal { ref specifier, .. } => specifier,
      _ => return Ok(resolution),
    };
    if specifier.scheme() != "jsr" {
      return Ok(resolution);
    }
    let mut maybe_diagnostic = None;
    if let Ok(package_req_ref) =
      JsrPackageReqReference::from_specifier(specifier)
    {
      for pkg in &self.jsr_pkgs {
        if pkg.name == package_req_ref.req().name {
          if let Some(version) = &pkg.version {
            if package_req_ref.req().version_req.matches(version) {
              return self.resolve_workspace_jsr_pkg(pkg, package_req_ref);
            } else {
              maybe_diagnostic = Some(Box::new(
                MappedResolutionDiagnostic::ConstraintNotMatchedLocalVersion {
                  is_link: pkg.is_link,
                  reference: package_req_ref.clone(),
                  local_version: version.clone(),
                },
              ));
            }
          } else {
            // always resolve to workspace packages with no version
            return self.resolve_workspace_jsr_pkg(pkg, package_req_ref);
          }
        }
      }
    }
    Ok(match resolution {
      MappedResolution::Normal {
        specifier,
        sloppy_reason,
        used_import_map,
        used_compiler_options_root_dirs,
        ..
      } => MappedResolution::Normal {
        specifier,
        sloppy_reason,
        used_import_map,
        used_compiler_options_root_dirs,
        maybe_diagnostic,
      },
      _ => return Ok(resolution),
    })
  }

  fn resolve_workspace_jsr_pkg<'a>(
    &'a self,
    pkg: &'a ResolverWorkspaceJsrPackage,
    pkg_req_ref: JsrPackageReqReference,
  ) -> Result<MappedResolution<'a>, MappedResolutionError> {
    let export_name = pkg_req_ref.export_name();
    match pkg.exports.get(export_name.as_ref()) {
      Some(sub_path) => match pkg.base.join(sub_path) {
        Ok(specifier) => Ok(MappedResolution::WorkspaceJsrPackage {
          specifier,
          pkg_req_ref,
        }),
        Err(err) => Err(
          WorkspaceResolveError::InvalidExportPath {
            base: pkg.base.clone(),
            sub_path: sub_path.to_string(),
            error: err,
          }
          .into(),
        ),
      },
      None => Err(
        WorkspaceResolveError::UnknownExport {
          package_name: pkg.name.clone(),
          export_name: export_name.to_string(),
          exports: pkg.exports.keys().cloned().collect(),
        }
        .into(),
      ),
    }
  }

  pub fn resolve_workspace_pkg_json_folder_for_npm_specifier(
    &self,
    pkg_req: &PackageReq,
  ) -> Option<&Path> {
    if pkg_req.version_req.tag().is_some() {
      return None;
    }

    self
      .resolve_workspace_pkg_json_folder_for_pkg_json_dep(
        &pkg_req.name,
        &PackageJsonDepWorkspaceReq::VersionReq(pkg_req.version_req.clone()),
      )
      .ok()
  }

  pub fn resolve_workspace_pkg_json_folder_for_pkg_json_dep(
    &self,
    name: &str,
    workspace_version_req: &PackageJsonDepWorkspaceReq,
  ) -> Result<&Path, WorkspaceResolvePkgJsonFolderError> {
    // this is not conditional on pkg_json_dep_resolution because we want
    // to be able to do this resolution to figure out mapping an npm specifier
    // to a workspace folder when using BYONM
    let pkg_json = self
      .package_jsons()
      .find(|p| p.name.as_deref() == Some(name));
    let Some(pkg_json) = pkg_json else {
      return Err(
        WorkspaceResolvePkgJsonFolderErrorKind::NotFound(name.to_string())
          .into(),
      );
    };
    match workspace_version_req {
      PackageJsonDepWorkspaceReq::VersionReq(version_req) => {
        match version_req.inner() {
          RangeSetOrTag::RangeSet(set) => {
            match pkg_json
              .version
              .as_ref()
              .and_then(|v| Version::parse_from_npm(v).ok())
            {
              Some(version) => {
                if set.satisfies(&version) {
                  Ok(pkg_json.dir_path())
                } else {
                  Err(
                  WorkspaceResolvePkgJsonFolderErrorKind::VersionNotSatisfied(
                    version_req.clone(),
                    version,
                  )
                  .into(),
                )
                }
              }
              _ => {
                // just match it
                Ok(pkg_json.dir_path())
              }
            }
          }
          RangeSetOrTag::Tag(_) => {
            // always match tags
            Ok(pkg_json.dir_path())
          }
        }
      }
      PackageJsonDepWorkspaceReq::Tilde | PackageJsonDepWorkspaceReq::Caret => {
        // always match tilde and caret requirements
        Ok(pkg_json.dir_path())
      }
    }
  }

  pub fn pkg_json_dep_resolution(&self) -> PackageJsonDepResolution {
    self.pkg_json_dep_resolution
  }

  pub fn sloppy_imports_enabled(&self) -> bool {
    match self.sloppy_imports_options {
      SloppyImportsOptions::Enabled => true,
      SloppyImportsOptions::Unspecified => false,
    }
  }

  pub fn has_compiler_options_root_dirs(&self) -> bool {
    self.compiler_options_resolver.read().has_root_dirs()
  }
}

#[derive(Deserialize, Serialize)]
pub struct SerializedWorkspaceResolverImportMap<'a> {
  #[serde(borrow)]
  pub specifier: Cow<'a, str>,
  #[serde(borrow)]
  pub json: Cow<'a, str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerializedResolverWorkspaceJsrPackage<'a> {
  #[serde(borrow)]
  pub relative_base: Cow<'a, str>,
  #[serde(borrow)]
  pub name: Cow<'a, str>,
  pub version: Cow<'a, Option<Version>>,
  pub exports: Cow<'a, IndexMap<String, String>>,
}

#[derive(Deserialize, Serialize)]
pub struct SerializableWorkspaceResolver<'a> {
  #[serde(borrow)]
  pub import_map: Option<SerializedWorkspaceResolverImportMap<'a>>,
  #[serde(borrow)]
  pub jsr_pkgs: Vec<SerializedResolverWorkspaceJsrPackage<'a>>,
  pub package_jsons: Vec<(String, serde_json::Value)>,
  pub pkg_json_resolution: PackageJsonDepResolution,
  pub sloppy_imports_options: SloppyImportsOptions,
  pub fs_cache_options: FsCacheOptions,
}

#[derive(Debug, Clone, Copy)]
struct BaseUrl<'a>(&'a Url);

impl BaseUrl<'_> {
  fn make_relative_if_descendant<'a>(&self, target: &'a Url) -> Cow<'a, str> {
    if target.scheme() != "file" {
      return Cow::Borrowed(target.as_str());
    }

    match self.0.make_relative(target) {
      Some(relative) => {
        if relative.starts_with("../") {
          Cow::Borrowed(target.as_str())
        } else {
          Cow::Owned(relative)
        }
      }
      None => Cow::Borrowed(target.as_str()),
    }
  }
}

#[allow(clippy::disallowed_types)] // ok, because definition
#[derive(Debug, Default, Clone)]
pub struct WorkspaceNpmLinkPackagesRc(
  pub std::sync::Arc<HashMap<PackageName, Vec<NpmPackageVersionInfo>>>,
);

impl WorkspaceNpmLinkPackagesRc {
  pub fn from_workspace(workspace: &Workspace) -> Self {
    let mut entries: HashMap<PackageName, Vec<NpmPackageVersionInfo>> =
      HashMap::new();
    for pkg_json in workspace.link_pkg_jsons() {
      let Some(name) = pkg_json.name.as_ref() else {
        log::warn!(
          "{} Link package ignored because package.json was missing name field.\n    at {}",
          colors::yellow("Warning"),
          pkg_json.path.display(),
        );
        continue;
      };
      match pkg_json_to_version_info(pkg_json) {
        Ok(version_info) => {
          let entry = entries.entry(PackageName::from_str(name)).or_default();
          entry.push(version_info);
        }
        Err(err) => {
          log::warn!(
            "{} {}\n    at {}",
            colors::yellow("Warning"),
            err,
            pkg_json.path.display(),
          );
        }
      }
    }
    Self(deno_maybe_sync::new_arc(entries))
  }
}

#[derive(Debug, Error)]
enum PkgJsonToVersionInfoError {
  #[error(
    "Linked package ignored because package.json was missing version field."
  )]
  VersionMissing,
  #[error(
    "Linked package ignored because package.json version field could not be parsed."
  )]
  VersionInvalid {
    #[source]
    source: deno_semver::npm::NpmVersionParseError,
  },
}

fn pkg_json_to_version_info(
  pkg_json: &deno_package_json::PackageJson,
) -> Result<NpmPackageVersionInfo, PkgJsonToVersionInfoError> {
  fn parse_deps(
    deps: Option<&IndexMap<String, String>>,
  ) -> HashMap<StackString, StackString> {
    deps
      .map(|d| {
        d.into_iter()
          .map(|(k, v)| (StackString::from_str(k), StackString::from_str(v)))
          .collect()
      })
      .unwrap_or_default()
  }

  fn parse_small_stack_string_array(v: &[String]) -> Vec<SmallStackString> {
    v.iter().map(|s| SmallStackString::from_str(s)).collect()
  }

  fn parse_stack_string_array(v: &[String]) -> Vec<StackString> {
    v.iter().map(|s| StackString::from_str(s)).collect()
  }

  let Some(version) = &pkg_json.version else {
    return Err(PkgJsonToVersionInfoError::VersionMissing);
  };

  let version = Version::parse_from_npm(version)
    .map_err(|source| PkgJsonToVersionInfoError::VersionInvalid { source })?;
  Ok(NpmPackageVersionInfo {
    version,
    dist: None,
    bin: pkg_json
      .bin
      .as_ref()
      .and_then(|v| serde_json::from_value(v.clone()).ok()),
    dependencies: parse_deps(pkg_json.dependencies.as_ref()),
    bundle_dependencies: pkg_json
      .bundle_dependencies
      .as_ref()
      .map(|d| parse_stack_string_array(d))
      .unwrap_or_default(),
    bundled_dependencies: Vec::new(),
    optional_dependencies: parse_deps(pkg_json.optional_dependencies.as_ref()),
    peer_dependencies: parse_deps(pkg_json.peer_dependencies.as_ref()),
    peer_dependencies_meta: pkg_json
      .peer_dependencies_meta
      .clone()
      .and_then(|m| serde_json::from_value(m).ok())
      .unwrap_or_default(),
    os: pkg_json
      .os
      .as_deref()
      .map(parse_small_stack_string_array)
      .unwrap_or_default(),
    cpu: pkg_json
      .cpu
      .as_deref()
      .map(parse_small_stack_string_array)
      .unwrap_or_default(),
    scripts: pkg_json
      .scripts
      .as_ref()
      .map(|scripts| {
        scripts
          .iter()
          .map(|(k, v)| (SmallStackString::from_str(k), v.clone()))
          .collect()
      })
      .unwrap_or_default(),
    // not worth increasing memory for showing a deprecated
    // message for linked packages
    deprecated: None,
  })
}

#[cfg(test)]
mod test {
  use std::path::Path;
  use std::path::PathBuf;

  use deno_config::workspace::WorkspaceDirectory;
  use deno_config::workspace::WorkspaceDirectoryRc;
  use deno_config::workspace::WorkspaceDiscoverOptions;
  use deno_config::workspace::WorkspaceDiscoverStart;
  use deno_npm::registry::NpmPeerDependencyMeta;
  use deno_path_util::url_from_directory_path;
  use deno_path_util::url_from_file_path;
  use deno_semver::VersionReq;
  use node_resolver::DenoIsBuiltInNodeModuleChecker;
  use node_resolver::NodeResolver;
  use node_resolver::NodeResolverOptions;
  use node_resolver::NpmPackageFolderResolver;
  use node_resolver::PackageJsonResolver;
  use node_resolver::cache::NodeResolutionSys;
  use node_resolver::errors::PackageFolderResolveError;
  use node_resolver::errors::PackageFolderResolveErrorKind;
  use node_resolver::errors::PackageNotFoundError;
  use serde_json::json;
  use sys_traits::FsCanonicalize;
  use sys_traits::impls::InMemorySys;
  use url::Url;

  use super::*;
  use crate::deno_json::CompilerOptionsResolver;
  use crate::factory::ConfigDiscoveryOption;
  use crate::npm::CreateInNpmPkgCheckerOptions;
  use crate::npm::DenoInNpmPackageChecker;
  use crate::npm::NpmResolverSys;

  pub struct UnreachableSys;

  impl sys_traits::BaseFsMetadata for UnreachableSys {
    type Metadata = sys_traits::impls::RealFsMetadata;

    #[doc(hidden)]
    fn base_fs_metadata(
      &self,
      _path: &Path,
    ) -> std::io::Result<Self::Metadata> {
      unreachable!()
    }

    #[doc(hidden)]
    fn base_fs_symlink_metadata(
      &self,
      _path: &Path,
    ) -> std::io::Result<Self::Metadata> {
      unreachable!()
    }
  }

  impl sys_traits::BaseFsRead for UnreachableSys {
    fn base_fs_read(
      &self,
      _path: &Path,
    ) -> std::io::Result<Cow<'static, [u8]>> {
      unreachable!()
    }
  }

  fn root_dir() -> PathBuf {
    if cfg!(windows) {
      PathBuf::from("C:\\Users\\user")
    } else {
      PathBuf::from("/home/user")
    }
  }

  #[derive(Debug)]
  struct TestNpmPackageFolderResolver;

  impl NpmPackageFolderResolver for TestNpmPackageFolderResolver {
    fn resolve_package_folder_from_package(
      &self,
      specifier: &str,
      referrer: &node_resolver::UrlOrPathRef,
    ) -> Result<PathBuf, PackageFolderResolveError> {
      Err(PackageFolderResolveError(Box::new(
        PackageFolderResolveErrorKind::PackageNotFound(PackageNotFoundError {
          package_name: specifier.to_string(),
          referrer: referrer.display(),
          referrer_extra: None,
        }),
      )))
    }

    fn resolve_types_package_folder(
      &self,
      _types_package_name: &str,
      _maybe_package_version: Option<&Version>,
      _maybe_referrer: Option<&node_resolver::UrlOrPathRef>,
    ) -> Option<PathBuf> {
      None
    }
  }

  #[allow(clippy::disallowed_types)]
  fn setup_node_resolver<TSys: NpmResolverSys>(
    sys: &TSys,
  ) -> crate::deno_json::TsConfigNodeResolver<TSys, TestNpmPackageFolderResolver>
  {
    let package_json_resolver =
      new_rc(PackageJsonResolver::new(sys.clone(), None));
    NodeResolver::new(
      DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Byonm),
      DenoIsBuiltInNodeModuleChecker,
      TestNpmPackageFolderResolver,
      package_json_resolver,
      NodeResolutionSys::new(sys.clone(), None),
      NodeResolverOptions::default(),
    )
  }

  #[test]
  fn pkg_json_resolution() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("deno.json"),
      json!({
        "workspace": [
          "a",
          "b",
          "c",
        ]
      }),
    );
    sys.fs_insert_json(
      root_dir().join("a/deno.json"),
      json!({
        "imports": {
          "b": "./index.js",
        },
      }),
    );
    sys.fs_insert_json(
      root_dir().join("b/package.json"),
      json!({
        "dependencies": {
          "pkg": "npm:pkg@^1.0.0",
        },
      }),
    );
    sys.fs_insert_json(
      root_dir().join("c/package.json"),
      json!({
        "name": "pkg",
        "version": "0.5.0"
      }),
    );
    let workspace = workspace_at_start_dir(&sys, &root_dir());
    let resolver = create_resolver(&workspace);
    assert_eq!(resolver.diagnostics(), Vec::new());
    let resolve = |name: &str, referrer: &str| {
      resolver.resolve(
        name,
        &url_from_file_path(&deno_path_util::normalize_path(Cow::Owned(
          root_dir().join(referrer),
        )))
        .unwrap(),
        ResolutionKind::Execution,
      )
    };
    match resolve("pkg", "b/index.js").unwrap() {
      MappedResolution::PackageJson {
        alias,
        sub_path,
        dep_result,
        ..
      } => {
        assert_eq!(alias, "pkg");
        assert_eq!(sub_path, None);
        dep_result.as_ref().unwrap();
      }
      value => unreachable!("{:?}", value),
    }
    match resolve("pkg/sub-path", "b/index.js").unwrap() {
      MappedResolution::PackageJson {
        alias,
        sub_path,
        dep_result,
        ..
      } => {
        assert_eq!(alias, "pkg");
        assert_eq!(sub_path.unwrap(), "sub-path");
        dep_result.as_ref().unwrap();
      }
      value => unreachable!("{:?}", value),
    }

    // pkg is not a dependency in this folder, so it should resolve
    // to the workspace member
    match resolve("pkg", "index.js").unwrap() {
      MappedResolution::WorkspaceNpmPackage {
        pkg_name,
        sub_path,
        target_pkg_json,
      } => {
        assert_eq!(pkg_name, "pkg");
        assert_eq!(sub_path, None);
        assert_eq!(target_pkg_json.dir_path(), root_dir().join("c"));
      }
      _ => unreachable!(),
    }
    match resolve("pkg/sub-path", "index.js").unwrap() {
      MappedResolution::WorkspaceNpmPackage {
        pkg_name,
        sub_path,
        target_pkg_json,
      } => {
        assert_eq!(pkg_name, "pkg");
        assert_eq!(sub_path.unwrap(), "sub-path");
        assert_eq!(target_pkg_json.dir_path(), root_dir().join("c"));
      }
      _ => unreachable!(),
    }

    // won't resolve the package outside the workspace
    assert!(resolve("pkg", "../outside-workspace.js").is_err());
  }

  #[test]
  fn single_pkg_no_import_map() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("deno.json"),
      json!({
        "name": "@scope/pkg",
        "version": "1.0.0",
        "exports": "./mod.ts"
      }),
    );
    let workspace = workspace_at_start_dir(&sys, &root_dir());
    let resolver = create_resolver(&workspace);
    assert_eq!(resolver.diagnostics(), Vec::new());
    let result = resolver
      .resolve(
        "@scope/pkg",
        &url_from_file_path(&root_dir().join("file.ts")).unwrap(),
        ResolutionKind::Execution,
      )
      .unwrap();
    match result {
      MappedResolution::WorkspaceJsrPackage { specifier, .. } => {
        assert_eq!(
          specifier,
          url_from_file_path(&root_dir().join("mod.ts")).unwrap()
        );
      }
      _ => unreachable!(),
    }
  }

  #[test]
  fn resolve_workspace_pkg_json_folder() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("package.json"),
      json!({
        "workspaces": [
          "a",
          "b",
          "no-version"
        ]
      }),
    );
    sys.fs_insert_json(
      root_dir().join("a/package.json"),
      json!({
        "name": "@scope/a",
        "version": "1.0.0",
      }),
    );
    sys.fs_insert_json(
      root_dir().join("b/package.json"),
      json!({
        "name": "@scope/b",
        "version": "2.0.0",
      }),
    );
    sys.fs_insert_json(
      root_dir().join("no-version/package.json"),
      json!({
        "name": "@scope/no-version",
      }),
    );
    let workspace = workspace_at_start_dir(&sys, &root_dir());
    let resolver = create_resolver(&workspace);
    // resolve for pkg json dep
    {
      let resolve = |name: &str, req: &str| {
        resolver.resolve_workspace_pkg_json_folder_for_pkg_json_dep(
          name,
          &PackageJsonDepWorkspaceReq::VersionReq(
            VersionReq::parse_from_npm(req).unwrap(),
          ),
        )
      };
      assert_eq!(
        resolve("non-existent", "*").map_err(|e| e.into_kind()),
        Err(WorkspaceResolvePkgJsonFolderErrorKind::NotFound(
          "non-existent".to_string()
        ))
      );
      assert_eq!(
        resolve("@scope/a", "6").map_err(|e| e.into_kind()),
        Err(WorkspaceResolvePkgJsonFolderErrorKind::VersionNotSatisfied(
          VersionReq::parse_from_npm("6").unwrap(),
          Version::parse_from_npm("1.0.0").unwrap(),
        ))
      );
      assert_eq!(resolve("@scope/a", "1").unwrap(), root_dir().join("a"));
      assert_eq!(resolve("@scope/a", "*").unwrap(), root_dir().join("a"));
      assert_eq!(
        resolve("@scope/a", "workspace").unwrap(),
        root_dir().join("a")
      );
      assert_eq!(resolve("@scope/b", "2").unwrap(), root_dir().join("b"));
      // just match any tags with the workspace
      assert_eq!(resolve("@scope/a", "latest").unwrap(), root_dir().join("a"));

      // match any version for a pkg with no version
      assert_eq!(
        resolve("@scope/no-version", "1").unwrap(),
        root_dir().join("no-version")
      );
      assert_eq!(
        resolve("@scope/no-version", "20").unwrap(),
        root_dir().join("no-version")
      );
    }
    // resolve for specifier
    {
      let resolve = |pkg_req: &str| {
        resolver.resolve_workspace_pkg_json_folder_for_npm_specifier(
          &PackageReq::from_str(pkg_req).unwrap(),
        )
      };
      assert_eq!(resolve("non-existent@*"), None);
      assert_eq!(
        resolve("@scope/no-version@1").unwrap(),
        root_dir().join("no-version")
      );

      // won't match for tags
      assert_eq!(resolve("@scope/a@workspace"), None);
      assert_eq!(resolve("@scope/a@latest"), None);
    }
  }

  #[test]
  fn resolve_workspace_pkg_json_workspace_deno_json_import_map() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("package.json"),
      json!({
        "workspaces": ["*"]
      }),
    );
    sys.fs_insert_json(
      root_dir().join("a/package.json"),
      json!({
        "name": "@scope/a",
        "version": "1.0.0",
      }),
    );
    sys.fs_insert_json(
      root_dir().join("a/deno.json"),
      json!({
        "name": "@scope/jsr-pkg",
        "version": "1.0.0",
        "exports": "./mod.ts"
      }),
    );

    let workspace = workspace_at_start_dir(&sys, &root_dir());
    let resolver = create_resolver(&workspace);
    {
      let resolution = resolver
        .resolve(
          "@scope/jsr-pkg",
          &url_from_file_path(&root_dir().join("b.ts")).unwrap(),
          ResolutionKind::Execution,
        )
        .unwrap();
      match resolution {
        MappedResolution::WorkspaceJsrPackage { specifier, .. } => {
          assert_eq!(
            specifier,
            url_from_file_path(&root_dir().join("a/mod.ts")).unwrap()
          );
        }
        _ => unreachable!(),
      }
    }
    {
      let resolution_err = resolver
        .resolve(
          "@scope/jsr-pkg/not-found-export",
          &url_from_file_path(&root_dir().join("b.ts")).unwrap(),
          ResolutionKind::Execution,
        )
        .unwrap_err();
      match resolution_err {
        MappedResolutionError::Workspace(
          WorkspaceResolveError::UnknownExport {
            package_name,
            export_name,
            exports,
          },
        ) => {
          assert_eq!(package_name, "@scope/jsr-pkg");
          assert_eq!(export_name, "./not-found-export");
          assert_eq!(exports, vec!["."]);
        }
        _ => unreachable!(),
      }
    }
  }

  #[test]
  fn root_member_imports_and_scopes() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("deno.json"),
      json!({
        "workspace": ["member"],
        "imports": {
          "@scope/pkg": "jsr:@scope/pkg@1",
        },
        "scopes": {
          "https://deno.land/x/": {
            "@scope/pkg": "jsr:@scope/pkg@2",
          },
        },
      }),
    );
    // Overrides `rootDirs` from workspace root.
    sys.fs_insert_json(
      root_dir().join("member/deno.json"),
      json!({
        "imports": {
          "@scope/pkg": "jsr:@scope/pkg@3",
        },
        // will ignore this scopes because it's not in the root
        "scopes": {
          "https://deno.land/x/other": {
            "@scope/pkg": "jsr:@scope/pkg@4",
          },
        },
      }),
    );

    let workspace_dir = workspace_at_start_dir(&sys, &root_dir());
    let resolver = WorkspaceResolver::from_workspace(
      &workspace_dir.workspace,
      sys.clone(),
      super::CreateResolverOptions {
        pkg_json_dep_resolution: PackageJsonDepResolution::Enabled,
        specified_import_map: None,
        sloppy_imports_options: SloppyImportsOptions::Unspecified,
        fs_cache_options: FsCacheOptions::Enabled,
      },
    )
    .unwrap();
    assert_eq!(
      serde_json::from_str::<serde_json::Value>(
        &resolver.maybe_import_map().unwrap().to_json()
      )
      .unwrap(),
      json!({
        "imports": {
          "@scope/pkg": "jsr:@scope/pkg@1",
          "@scope/pkg/": "jsr:/@scope/pkg@1/",
        },
        "scopes": {
          "https://deno.land/x/": {
            "@scope/pkg": "jsr:@scope/pkg@2",
            "@scope/pkg/": "jsr:/@scope/pkg@2/",
          },
          "./member/": {
            "@scope/pkg": "jsr:@scope/pkg@3",
            "@scope/pkg/": "jsr:/@scope/pkg@3/",
          },
        },
      }),
    );
  }

  #[test]
  fn resolve_sloppy_imports() {
    let sys = InMemorySys::default();
    let root_url = url_from_file_path(
      &sys_traits::impls::RealSys.fs_canonicalize("/").unwrap(),
    )
    .unwrap();
    let fs = CachedMetadataFs::new(sys.clone(), FsCacheOptions::Enabled);
    let sloppy_imports_resolver = SloppyImportsResolver::new(
      fs,
      Default::default(),
      SloppyImportsOptions::Enabled,
    );
    let referrer = root_url.join("main.ts").unwrap();

    // scenarios like resolving ./example.js to ./example.ts
    for (file_from, file_to) in [
      ("file1.js", "file1.ts"),
      ("file2.js", "file2.tsx"),
      ("file3.mjs", "file3.mts"),
    ] {
      let specifier = root_url.join(file_to).unwrap();
      sys.fs_insert(url_to_file_path(&specifier).unwrap(), "");
      let sloppy_specifier = root_url.join(file_from).unwrap();
      assert_eq!(
        sloppy_imports_resolver.resolve(
          &specifier,
          &referrer,
          ResolutionKind::Execution
        ),
        None,
      );
      assert_eq!(
        sloppy_imports_resolver.resolve(
          &sloppy_specifier,
          &referrer,
          ResolutionKind::Execution
        ),
        Some((specifier, SloppyImportsResolutionReason::JsToTs)),
      );
    }

    // no extension scenarios
    for file in [
      "file10.js",
      "file11.ts",
      "file12.js",
      "file13.tsx",
      "file14.jsx",
      "file15.mjs",
      "file16.mts",
    ] {
      let specifier = root_url.join(file).unwrap();
      sys.fs_insert(url_to_file_path(&specifier).unwrap(), "");
      let sloppy_specifier =
        root_url.join(file.split_once('.').unwrap().0).unwrap();
      assert_eq!(
        sloppy_imports_resolver.resolve(
          &specifier,
          &referrer,
          ResolutionKind::Execution
        ),
        None,
      );
      assert_eq!(
        sloppy_imports_resolver.resolve(
          &sloppy_specifier,
          &referrer,
          ResolutionKind::Execution
        ),
        Some((specifier, SloppyImportsResolutionReason::NoExtension)),
      );
    }

    // .ts and .js exists, .js specified (goes to specified)
    {
      let ts_specifier = root_url.join("ts_and_js.ts").unwrap();
      sys.fs_insert(url_to_file_path(&ts_specifier).unwrap(), "");
      let js_specifier = root_url.join("ts_and_js.js").unwrap();
      sys.fs_insert(url_to_file_path(&js_specifier).unwrap(), "");
      assert_eq!(
        sloppy_imports_resolver.resolve(
          &js_specifier,
          &referrer,
          ResolutionKind::Execution
        ),
        None,
      );
    }

    // only js exists, .js specified
    {
      let specifier = root_url.join("js_only.js").unwrap();
      sys.fs_insert(url_to_file_path(&specifier).unwrap(), "");
      assert_eq!(
        sloppy_imports_resolver.resolve(
          &specifier,
          &referrer,
          ResolutionKind::Execution
        ),
        None,
      );
      assert_eq!(
        sloppy_imports_resolver.resolve(
          &specifier,
          &referrer,
          ResolutionKind::Types
        ),
        None,
      );
    }

    // resolving a directory to an index file
    {
      let specifier = root_url.join("routes/index.ts").unwrap();
      sys.fs_insert(url_to_file_path(&specifier).unwrap(), "");
      let sloppy_specifier = root_url.join("routes").unwrap();
      assert_eq!(
        sloppy_imports_resolver.resolve(
          &sloppy_specifier,
          &referrer,
          ResolutionKind::Execution
        ),
        Some((specifier, SloppyImportsResolutionReason::Directory)),
      );
    }

    // both a directory and a file with specifier is present
    {
      let specifier = root_url.join("api.ts").unwrap();
      sys.fs_insert(url_to_file_path(&specifier).unwrap(), "");
      let bar_specifier = root_url.join("api/bar.ts").unwrap();
      sys.fs_insert(url_to_file_path(&bar_specifier).unwrap(), "");
      let sloppy_specifier = root_url.join("api").unwrap();
      assert_eq!(
        sloppy_imports_resolver.resolve(
          &sloppy_specifier,
          &referrer,
          ResolutionKind::Execution
        ),
        Some((specifier, SloppyImportsResolutionReason::NoExtension)),
      );
    }
  }

  #[test]
  fn test_sloppy_import_resolution_suggestion_message() {
    // directory
    assert_eq!(
      SloppyImportsResolutionReason::Directory
        .suggestion_message_for_specifier(
          &Url::parse("file:///dir/index.js").unwrap()
        )
        .as_str(),
      "Maybe specify path to 'index.js' file in directory instead"
    );
    // no ext
    assert_eq!(
      SloppyImportsResolutionReason::NoExtension
        .suggestion_message_for_specifier(
          &Url::parse("file:///dir/index.mjs").unwrap()
        )
        .as_str(),
      "Maybe add a '.mjs' extension"
    );
    // js to ts
    assert_eq!(
      SloppyImportsResolutionReason::JsToTs
        .suggestion_message_for_specifier(
          &Url::parse("file:///dir/index.mts").unwrap()
        )
        .as_str(),
      "Maybe change the extension to '.mts'"
    );
  }

  #[test]
  fn resolve_compiler_options_paths_and_sloppy_imports() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("deno.json"),
      json!({
        "compilerOptions": {
          "paths": {
            "@lib": ["lib"],
            "@lib/*": ["lib/*", "lib/*/mod.ts"],
            "./src/*": ["./src/*", "./types/*"],
            "@unmapped/*": [],
          },
        },
      }),
    );
    sys.fs_insert(root_dir().join("lib/index.ts"), "");
    sys.fs_insert(root_dir().join("lib/foo.ts"), "");
    sys.fs_insert(root_dir().join("lib/bar/mod.ts"), "");
    sys.fs_insert(root_dir().join("src/baz.ts"), "");
    sys.fs_insert(root_dir().join("types/qux.ts"), "");

    let workspace_dir = workspace_at_start_dir(&sys, &root_dir());
    let resolver = WorkspaceResolver::from_workspace(
      &workspace_dir.workspace,
      sys.clone(),
      super::CreateResolverOptions {
        pkg_json_dep_resolution: PackageJsonDepResolution::Enabled,
        specified_import_map: None,
        sloppy_imports_options: SloppyImportsOptions::Enabled,
        fs_cache_options: FsCacheOptions::Enabled,
      },
    )
    .unwrap();
    let compiler_options_resolver = new_rc(CompilerOptionsResolver::new(
      &sys,
      &workspace_dir.workspace,
      &setup_node_resolver(&sys),
      &ConfigDiscoveryOption::DiscoverCwd,
      &Default::default(),
    ));
    resolver.set_compiler_options_resolver(compiler_options_resolver);
    let root_dir_url = workspace_dir.workspace.root_dir_url();
    let referrer = root_dir_url.join("main.ts").unwrap();

    let resolution = resolver
      .resolve("@lib", &referrer, ResolutionKind::Types)
      .unwrap();
    let MappedResolution::Normal {
      specifier,
      sloppy_reason,
      ..
    } = &resolution
    else {
      unreachable!("{:#?}", &resolution);
    };
    assert_eq!(
      specifier.as_str(),
      root_dir_url.join("lib/index.ts").unwrap().as_str()
    );
    assert_eq!(
      sloppy_reason,
      &Some(SloppyImportsResolutionReason::Directory)
    );

    let resolution = resolver
      .resolve("@lib/foo", &referrer, ResolutionKind::Types)
      .unwrap();
    let MappedResolution::Normal {
      specifier,
      sloppy_reason,
      ..
    } = &resolution
    else {
      unreachable!("{:#?}", &resolution);
    };
    assert_eq!(
      specifier.as_str(),
      root_dir_url.join("lib/foo.ts").unwrap().as_str()
    );
    assert_eq!(
      sloppy_reason,
      &Some(SloppyImportsResolutionReason::NoExtension)
    );

    let resolution = resolver
      .resolve("@lib/bar", &referrer, ResolutionKind::Types)
      .unwrap();
    let MappedResolution::Normal {
      specifier,
      sloppy_reason,
      ..
    } = &resolution
    else {
      unreachable!("{:#?}", &resolution);
    };
    assert_eq!(
      specifier.as_str(),
      root_dir_url.join("lib/bar/mod.ts").unwrap().as_str()
    );
    assert_eq!(sloppy_reason, &None);

    let resolution = resolver
      .resolve("./src/baz.ts", &referrer, ResolutionKind::Types)
      .unwrap();
    let MappedResolution::Normal {
      specifier,
      sloppy_reason,
      ..
    } = &resolution
    else {
      unreachable!("{:#?}", &resolution);
    };
    assert_eq!(
      specifier.as_str(),
      root_dir_url.join("src/baz.ts").unwrap().as_str()
    );
    assert_eq!(sloppy_reason, &None);

    let resolution = resolver
      .resolve("./src/qux.ts", &referrer, ResolutionKind::Types)
      .unwrap();
    let MappedResolution::Normal {
      specifier,
      sloppy_reason,
      ..
    } = &resolution
    else {
      unreachable!("{:#?}", &resolution);
    };
    assert_eq!(
      specifier.as_str(),
      root_dir_url.join("types/qux.ts").unwrap().as_str()
    );
    assert_eq!(sloppy_reason, &None);

    let resolution =
      resolver.resolve("@unmapped/foo", &referrer, ResolutionKind::Types);
    let Err(MappedResolutionError::NotFoundInCompilerOptionsPaths(err)) =
      &resolution
    else {
      unreachable!("{:#?}", &resolution);
    };
    assert_eq!(err.specifier.as_str(), "@unmapped/foo");
    assert_eq!(&err.referrer, &referrer);
    assert_eq!(&err.matched_key, "@unmapped/*");
  }

  #[test]
  fn resolve_compiler_options_root_dirs() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("deno.json"),
      json!({
        "workspace": ["member", "member2"],
        "compilerOptions": {
          "rootDirs": ["member", "member2", "member2_types"],
        },
      }),
    );
    // Overrides `rootDirs` from workspace root.
    sys.fs_insert_json(
      root_dir().join("member/deno.json"),
      json!({
        "compilerOptions": {
          "rootDirs": ["foo", "foo_types"],
        },
      }),
    );
    // Use `rootDirs` from workspace root.
    sys.fs_insert_json(root_dir().join("member2/deno.json"), json!({}));
    sys.fs_insert(root_dir().join("member/foo_types/import.ts"), "");
    sys.fs_insert(root_dir().join("member2_types/import.ts"), "");
    // This file should be ignored. It would be used if `member/deno.json` had
    // no `rootDirs`.
    sys.fs_insert(root_dir().join("member2_types/foo/import.ts"), "");

    let workspace_dir = workspace_at_start_dir(&sys, &root_dir());
    let resolver = WorkspaceResolver::from_workspace(
      &workspace_dir.workspace,
      sys.clone(),
      super::CreateResolverOptions {
        pkg_json_dep_resolution: PackageJsonDepResolution::Enabled,
        specified_import_map: None,
        sloppy_imports_options: SloppyImportsOptions::Unspecified,
        fs_cache_options: FsCacheOptions::Enabled,
      },
    )
    .unwrap();
    let compiler_options_resolver = new_rc(CompilerOptionsResolver::new(
      &sys,
      &workspace_dir.workspace,
      &setup_node_resolver(&sys),
      &ConfigDiscoveryOption::DiscoverCwd,
      &Default::default(),
    ));
    resolver.set_compiler_options_resolver(compiler_options_resolver);
    let root_dir_url = workspace_dir.workspace.root_dir_url();

    let referrer = root_dir_url.join("member/foo/mod.ts").unwrap();
    let resolution = resolver
      .resolve("./import.ts", &referrer, ResolutionKind::Types)
      .unwrap();
    let MappedResolution::Normal {
      specifier,
      sloppy_reason,
      used_compiler_options_root_dirs,
      ..
    } = &resolution
    else {
      unreachable!("{:#?}", &resolution);
    };
    assert_eq!(
      specifier.as_str(),
      root_dir_url
        .join("member/foo_types/import.ts")
        .unwrap()
        .as_str()
    );
    assert_eq!(sloppy_reason, &None);
    assert!(used_compiler_options_root_dirs);

    let referrer = root_dir_url.join("member2/mod.ts").unwrap();
    let resolution = resolver
      .resolve("./import.ts", &referrer, ResolutionKind::Types)
      .unwrap();
    let MappedResolution::Normal {
      specifier,
      sloppy_reason,
      used_compiler_options_root_dirs,
      ..
    } = &resolution
    else {
      unreachable!("{:#?}", &resolution);
    };
    assert_eq!(
      specifier.as_str(),
      root_dir_url
        .join("member2_types/import.ts")
        .unwrap()
        .as_str()
    );
    assert_eq!(sloppy_reason, &None);
    assert!(used_compiler_options_root_dirs);

    // Ignore rootDirs for `ResolutionKind::Execution`.
    let referrer = root_dir_url.join("member/foo/mod.ts").unwrap();
    let resolution = resolver
      .resolve("./import.ts", &referrer, ResolutionKind::Execution)
      .unwrap();
    let MappedResolution::Normal {
      specifier,
      sloppy_reason,
      used_compiler_options_root_dirs,
      ..
    } = &resolution
    else {
      unreachable!("{:#?}", &resolution);
    };
    assert_eq!(
      specifier.as_str(),
      root_dir_url.join("member/foo/import.ts").unwrap().as_str()
    );
    assert_eq!(sloppy_reason, &None);
    assert!(!used_compiler_options_root_dirs);

    // Ignore rootDirs for `ResolutionKind::Execution`.
    let referrer = root_dir_url.join("member2/mod.ts").unwrap();
    let resolution = resolver
      .resolve("./import.ts", &referrer, ResolutionKind::Execution)
      .unwrap();
    let MappedResolution::Normal {
      specifier,
      sloppy_reason,
      used_compiler_options_root_dirs,
      ..
    } = &resolution
    else {
      unreachable!("{:#?}", &resolution);
    };
    assert_eq!(
      specifier.as_str(),
      root_dir_url.join("member2/import.ts").unwrap().as_str()
    );
    assert_eq!(sloppy_reason, &None);
    assert!(!used_compiler_options_root_dirs);
  }

  #[test]
  fn resolve_compiler_options_root_dirs_and_sloppy_imports() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("deno.json"),
      json!({
        "compilerOptions": {
          "rootDirs": ["subdir", "subdir_types"],
        },
      }),
    );
    sys.fs_insert(root_dir().join("subdir_types/import.ts"), "");

    let workspace_dir = workspace_at_start_dir(&sys, &root_dir());
    let resolver = WorkspaceResolver::from_workspace(
      &workspace_dir.workspace,
      sys.clone(),
      super::CreateResolverOptions {
        pkg_json_dep_resolution: PackageJsonDepResolution::Enabled,
        specified_import_map: None,
        sloppy_imports_options: SloppyImportsOptions::Enabled,
        fs_cache_options: FsCacheOptions::Enabled,
      },
    )
    .unwrap();
    let compiler_options_resolver = new_rc(CompilerOptionsResolver::new(
      &sys,
      &workspace_dir.workspace,
      &setup_node_resolver(&sys),
      &ConfigDiscoveryOption::DiscoverCwd,
      &Default::default(),
    ));
    resolver.set_compiler_options_resolver(compiler_options_resolver);
    let root_dir_url = workspace_dir.workspace.root_dir_url();

    let referrer = root_dir_url.join("subdir/mod.ts").unwrap();
    let resolution = resolver
      .resolve("./import", &referrer, ResolutionKind::Types)
      .unwrap();
    let MappedResolution::Normal {
      specifier,
      sloppy_reason,
      used_compiler_options_root_dirs,
      ..
    } = &resolution
    else {
      unreachable!("{:#?}", &resolution);
    };
    assert_eq!(
      specifier.as_str(),
      root_dir_url
        .join("subdir_types/import.ts")
        .unwrap()
        .as_str()
    );
    assert_eq!(
      sloppy_reason,
      &Some(SloppyImportsResolutionReason::NoExtension)
    );
    assert!(used_compiler_options_root_dirs);
  }

  #[test]
  fn specified_import_map() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(root_dir().join("deno.json"), json!({}));
    let workspace_dir = workspace_at_start_dir(&sys, &root_dir());
    let resolver = WorkspaceResolver::from_workspace(
      &workspace_dir.workspace,
      sys,
      super::CreateResolverOptions {
        pkg_json_dep_resolution: PackageJsonDepResolution::Enabled,
        specified_import_map: Some(SpecifiedImportMap {
          base_url: url_from_directory_path(&root_dir()).unwrap(),
          value: json!({
            "imports": {
              "b": "./b/mod.ts",
            },
          }),
        }),
        sloppy_imports_options: SloppyImportsOptions::Unspecified,
        fs_cache_options: FsCacheOptions::Enabled,
      },
    )
    .unwrap();
    let root = url_from_directory_path(&root_dir()).unwrap();
    match resolver
      .resolve(
        "b",
        &root.join("main.ts").unwrap(),
        ResolutionKind::Execution,
      )
      .unwrap()
    {
      MappedResolution::Normal { specifier, .. } => {
        assert_eq!(specifier, root.join("b/mod.ts").unwrap());
      }
      _ => unreachable!(),
    }
  }

  #[test]
  fn workspace_specified_import_map() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("deno.json"),
      json!({
        "workspace": ["./a"]
      }),
    );
    sys.fs_insert_json(root_dir().join("a").join("deno.json"), json!({}));
    let workspace_dir = workspace_at_start_dir(&sys, &root_dir());
    WorkspaceResolver::from_workspace(
      &workspace_dir.workspace,
      UnreachableSys,
      super::CreateResolverOptions {
        pkg_json_dep_resolution: PackageJsonDepResolution::Enabled,
        specified_import_map: Some(SpecifiedImportMap {
          base_url: url_from_directory_path(&root_dir()).unwrap(),
          value: json!({
            "imports": {
              "b": "./b/mod.ts",
            },
          }),
        }),
        sloppy_imports_options: SloppyImportsOptions::Unspecified,
        fs_cache_options: FsCacheOptions::Enabled,
      },
    )
    .unwrap();
  }

  #[test]
  fn resolves_link_member_with_version() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("deno.json"),
      json!({
        "links": ["../link"]
      }),
    );
    sys.fs_insert_json(
      root_dir().join("../link/deno.json"),
      json!({
        "name": "@scope/link",
        "version": "1.0.0",
        "exports": "./mod.ts"
      }),
    );
    let workspace_dir = workspace_at_start_dir(&sys, &root_dir());
    let resolver = create_resolver(&workspace_dir);
    let root = url_from_directory_path(&root_dir()).unwrap();
    match resolver
      .resolve(
        "@scope/link",
        &root.join("main.ts").unwrap(),
        ResolutionKind::Execution,
      )
      .unwrap()
    {
      MappedResolution::WorkspaceJsrPackage { specifier, .. } => {
        assert_eq!(specifier, root.join("../link/mod.ts").unwrap());
      }
      _ => unreachable!(),
    }
    // matching version
    match resolver
      .resolve(
        "jsr:@scope/link@1",
        &root.join("main.ts").unwrap(),
        ResolutionKind::Execution,
      )
      .unwrap()
    {
      MappedResolution::WorkspaceJsrPackage { specifier, .. } => {
        assert_eq!(specifier, root.join("../link/mod.ts").unwrap());
      }
      _ => unreachable!(),
    }
    // not matching version
    match resolver
      .resolve(
        "jsr:@scope/link@2",
        &root.join("main.ts").unwrap(),
        ResolutionKind::Execution,
      )
      .unwrap()
    {
      MappedResolution::Normal {
        specifier,
        maybe_diagnostic,
        ..
      } => {
        assert_eq!(specifier, Url::parse("jsr:@scope/link@2").unwrap());
        assert_eq!(
          maybe_diagnostic,
          Some(Box::new(
            MappedResolutionDiagnostic::ConstraintNotMatchedLocalVersion {
              is_link: true,
              reference: JsrPackageReqReference::from_str("jsr:@scope/link@2")
                .unwrap(),
              local_version: Version::parse_from_npm("1.0.0").unwrap(),
            }
          ))
        );
      }
      _ => unreachable!(),
    }
  }

  #[test]
  fn resolves_link_member_no_version() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("deno.json"),
      json!({
        "links": ["../link"]
      }),
    );
    sys.fs_insert_json(
      root_dir().join("../link/deno.json"),
      json!({
        "name": "@scope/link",
        "exports": "./mod.ts"
      }),
    );
    let workspace_dir = workspace_at_start_dir(&sys, &root_dir());
    let resolver = create_resolver(&workspace_dir);
    let root = url_from_directory_path(&root_dir()).unwrap();
    match resolver
      .resolve(
        "@scope/link",
        &root.join("main.ts").unwrap(),
        ResolutionKind::Execution,
      )
      .unwrap()
    {
      MappedResolution::WorkspaceJsrPackage { specifier, .. } => {
        assert_eq!(specifier, root.join("../link/mod.ts").unwrap());
      }
      _ => unreachable!(),
    }
    // always resolves, no matter what version
    match resolver
      .resolve(
        "jsr:@scope/link@12",
        &root.join("main.ts").unwrap(),
        ResolutionKind::Execution,
      )
      .unwrap()
    {
      MappedResolution::WorkspaceJsrPackage { specifier, .. } => {
        assert_eq!(specifier, root.join("../link/mod.ts").unwrap());
      }
      _ => unreachable!(),
    }
  }

  #[test]
  fn resolves_workspace_member() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("deno.json"),
      json!({
        "workspace": ["./member"]
      }),
    );
    sys.fs_insert_json(
      root_dir().join("./member/deno.json"),
      json!({
        "name": "@scope/member",
        "version": "1.0.0",
        "exports": "./mod.ts"
      }),
    );
    let workspace_dir = workspace_at_start_dir(&sys, &root_dir());
    let resolver = create_resolver(&workspace_dir);
    let root = url_from_directory_path(&root_dir()).unwrap();
    match resolver
      .resolve(
        "@scope/member",
        &root.join("main.ts").unwrap(),
        ResolutionKind::Execution,
      )
      .unwrap()
    {
      MappedResolution::WorkspaceJsrPackage { specifier, .. } => {
        assert_eq!(specifier, root.join("./member/mod.ts").unwrap());
      }
      _ => unreachable!(),
    }
    // matching version
    match resolver
      .resolve(
        "jsr:@scope/member@1",
        &root.join("main.ts").unwrap(),
        ResolutionKind::Execution,
      )
      .unwrap()
    {
      MappedResolution::WorkspaceJsrPackage { specifier, .. } => {
        assert_eq!(specifier, root.join("./member/mod.ts").unwrap());
      }
      _ => unreachable!(),
    }
    // not matching version
    match resolver
      .resolve(
        "jsr:@scope/member@2",
        &root.join("main.ts").unwrap(),
        ResolutionKind::Execution,
      )
      .unwrap()
    {
      MappedResolution::Normal {
        specifier,
        maybe_diagnostic,
        ..
      } => {
        assert_eq!(specifier, Url::parse("jsr:@scope/member@2").unwrap());
        assert_eq!(
          maybe_diagnostic,
          Some(Box::new(
            MappedResolutionDiagnostic::ConstraintNotMatchedLocalVersion {
              is_link: false,
              reference: JsrPackageReqReference::from_str(
                "jsr:@scope/member@2"
              )
              .unwrap(),
              local_version: Version::parse_from_npm("1.0.0").unwrap(),
            }
          ))
        );
      }
      _ => unreachable!(),
    }
  }

  #[test]
  fn resolves_link_workspace() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("deno.json"),
      json!({
        "imports": {
          "@std/fs": "jsr:@std/fs@0.200.0"
        },
        "links": ["../link"]
      }),
    );
    sys.fs_insert_json(
      root_dir().join("../link/deno.json"),
      json!({
        "workspace": ["./member"]
      }),
    );
    sys.fs_insert_json(
      root_dir().join("../link/member/deno.json"),
      json!({
        "name": "@scope/link",
        "version": "1.0.0",
        "exports": "./mod.ts",
        "imports": {
          "@std/fs": "jsr:@std/fs@1"
        }
      }),
    );
    let workspace_dir = workspace_at_start_dir(&sys, &root_dir());
    let resolver = create_resolver(&workspace_dir);
    let root = url_from_directory_path(&root_dir()).unwrap();
    match resolver
      .resolve(
        "jsr:@scope/link@1",
        &root.join("main.ts").unwrap(),
        ResolutionKind::Execution,
      )
      .unwrap()
    {
      MappedResolution::WorkspaceJsrPackage { specifier, .. } => {
        assert_eq!(specifier, root.join("../link/member/mod.ts").unwrap());
      }
      _ => unreachable!(),
    }
    // resolving @std/fs from root
    match resolver
      .resolve(
        "@std/fs",
        &root.join("main.ts").unwrap(),
        ResolutionKind::Execution,
      )
      .unwrap()
    {
      MappedResolution::Normal { specifier, .. } => {
        assert_eq!(specifier, Url::parse("jsr:@std/fs@0.200.0").unwrap());
      }
      _ => unreachable!(),
    }
    // resolving @std/fs in linked package
    match resolver
      .resolve(
        "@std/fs",
        &root.join("../link/member/mod.ts").unwrap(),
        ResolutionKind::Execution,
      )
      .unwrap()
    {
      MappedResolution::Normal { specifier, .. } => {
        assert_eq!(specifier, Url::parse("jsr:@std/fs@1").unwrap());
      }
      _ => unreachable!(),
    }
  }

  #[test]
  fn invalid_package_name_with_slashes() {
    let sys = InMemorySys::default();
    sys.fs_insert_json(
      root_dir().join("deno.json"),
      json!({
        "workspace": ["./libs/math"]
      }),
    );
    sys.fs_insert_json(
      root_dir().join("libs/math/deno.json"),
      json!({
        "name": "@deno-test/libs/math", // Invalid package name containing slashes
        "version": "1.0.0",
        "exports": "./mod.ts"
      }),
    );
    let workspace = workspace_at_start_dir(&sys, &root_dir());
    let resolver = create_resolver(&workspace);
    let result = resolver.resolve(
      "@deno-test/libs/math",
      &url_from_file_path(&root_dir().join("main.ts")).unwrap(),
      ResolutionKind::Execution,
    );
    // Resolve shouldn't panic and tt should result in unmapped
    // bare specifier error as the package name is invalid.
    assert!(result.err().unwrap().is_unmapped_bare_specifier());

    let diagnostics = workspace.workspace.diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert!(
      diagnostics.first().unwrap().to_string().starts_with(
        r#"Invalid workspace member name "@deno-test/libs/math"."#
      )
    );
  }

  fn create_resolver(
    workspace_dir: &WorkspaceDirectory,
  ) -> WorkspaceResolver<UnreachableSys> {
    WorkspaceResolver::from_workspace(
      &workspace_dir.workspace,
      UnreachableSys,
      super::CreateResolverOptions {
        pkg_json_dep_resolution: PackageJsonDepResolution::Enabled,
        specified_import_map: None,
        sloppy_imports_options: SloppyImportsOptions::Unspecified,
        fs_cache_options: FsCacheOptions::Enabled,
      },
    )
    .unwrap()
  }

  fn workspace_at_start_dir(
    sys: &InMemorySys,
    start_dir: &Path,
  ) -> WorkspaceDirectoryRc {
    WorkspaceDirectory::discover(
      sys,
      WorkspaceDiscoverStart::Paths(&[start_dir.to_path_buf()]),
      &WorkspaceDiscoverOptions {
        discover_pkg_json: true,
        ..Default::default()
      },
    )
    .unwrap()
  }

  #[test]
  fn test_pkg_json_to_version_info() {
    fn convert(
      text: &str,
    ) -> Result<NpmPackageVersionInfo, PkgJsonToVersionInfoError> {
      let pkg_json = deno_package_json::PackageJson::load_from_string(
        PathBuf::from("package.json"),
        text,
      )
      .unwrap();
      pkg_json_to_version_info(&pkg_json)
    }

    assert_eq!(
      convert(
        r#"{
  "name": "pkg",
  "version": "1.0.0",
  "bin": "./bin.js",
  "dependencies": {
    "my-dep": "1"
  },
  "optionalDependencies": {
    "optional-dep": "~1"
  },
  "peerDependencies": {
    "my-peer-dep": "^2"
  },
  "bundleDependencies": [
    "my-dep"
  ],
  "peerDependenciesMeta": {
    "my-peer-dep": {
      "optional": true
    }
  },
  "os": ["win32"],
  "cpu": ["x86_64"],
  "scripts": {
    "script": "testing",
    "postInstall": "testing2"
  },
  "deprecated": "ignored for now"
}"#
      )
      .unwrap(),
      NpmPackageVersionInfo {
        version: Version::parse_from_npm("1.0.0").unwrap(),
        dist: None,
        bin: Some(deno_npm::registry::NpmPackageVersionBinEntry::String(
          "./bin.js".to_string()
        )),
        dependencies: HashMap::from([(
          StackString::from_static("my-dep"),
          StackString::from_static("1")
        )]),
        bundle_dependencies: Vec::from([StackString::from_static("my-dep")]),
        bundled_dependencies: Vec::new(),
        optional_dependencies: HashMap::from([(
          StackString::from_static("optional-dep"),
          StackString::from_static("~1")
        )]),
        peer_dependencies: HashMap::from([(
          StackString::from_static("my-peer-dep"),
          StackString::from_static("^2")
        )]),
        peer_dependencies_meta: HashMap::from([(
          StackString::from_static("my-peer-dep"),
          NpmPeerDependencyMeta { optional: true }
        )]),
        os: vec![SmallStackString::from_static("win32")],
        cpu: vec![SmallStackString::from_static("x86_64")],
        scripts: HashMap::from([
          (
            SmallStackString::from_static("script"),
            "testing".to_string(),
          ),
          (
            SmallStackString::from_static("postInstall"),
            "testing2".to_string(),
          )
        ]),
        // we don't bother ever setting this because we don't store it in deno_package_json
        deprecated: None,
      }
    );

    match convert("{}").unwrap_err() {
      PkgJsonToVersionInfoError::VersionMissing => {
        // ok
      }
      _ => unreachable!(),
    }
    match convert(r#"{ "version": "1.0.~" }"#).unwrap_err() {
      PkgJsonToVersionInfoError::VersionInvalid { source: err } => {
        assert_eq!(err.to_string(), "Invalid npm version");
      }
      _ => unreachable!(),
    }
  }
}
