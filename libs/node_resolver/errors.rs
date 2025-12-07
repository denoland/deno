// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::fmt::Write;
use std::path::PathBuf;

use boxed_error::Boxed;
use deno_error::JsError;
use deno_package_json::MissingPkgJsonNameError;
use deno_path_util::UrlToFilePathError;
use thiserror::Error;
use url::Url;

use crate::NodeResolutionKind;
use crate::ResolutionMode;
use crate::path::UrlOrPath;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum NodeJsErrorCode {
  ERR_INVALID_MODULE_SPECIFIER,
  ERR_INVALID_PACKAGE_CONFIG,
  ERR_INVALID_PACKAGE_TARGET,
  ERR_MODULE_NOT_FOUND,
  ERR_PACKAGE_IMPORT_NOT_DEFINED,
  ERR_PACKAGE_PATH_NOT_EXPORTED,
  ERR_UNKNOWN_FILE_EXTENSION,
  ERR_UNSUPPORTED_DIR_IMPORT,
  ERR_UNSUPPORTED_ESM_URL_SCHEME,
  ERR_INVALID_FILE_URL_PATH,
  ERR_UNKNOWN_BUILTIN_MODULE,
  /// Deno specific since Node doesn't support type checking TypeScript.
  ERR_TYPES_NOT_FOUND,
}

impl std::fmt::Display for NodeJsErrorCode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.as_str())
  }
}

impl NodeJsErrorCode {
  pub fn as_str(&self) -> &'static str {
    use NodeJsErrorCode::*;
    match self {
      ERR_INVALID_MODULE_SPECIFIER => "ERR_INVALID_MODULE_SPECIFIER",
      ERR_INVALID_PACKAGE_CONFIG => "ERR_INVALID_PACKAGE_CONFIG",
      ERR_INVALID_PACKAGE_TARGET => "ERR_INVALID_PACKAGE_TARGET",
      ERR_MODULE_NOT_FOUND => "ERR_MODULE_NOT_FOUND",
      ERR_PACKAGE_IMPORT_NOT_DEFINED => "ERR_PACKAGE_IMPORT_NOT_DEFINED",
      ERR_PACKAGE_PATH_NOT_EXPORTED => "ERR_PACKAGE_PATH_NOT_EXPORTED",
      ERR_UNKNOWN_FILE_EXTENSION => "ERR_UNKNOWN_FILE_EXTENSION",
      ERR_UNSUPPORTED_DIR_IMPORT => "ERR_UNSUPPORTED_DIR_IMPORT",
      ERR_UNSUPPORTED_ESM_URL_SCHEME => "ERR_UNSUPPORTED_ESM_URL_SCHEME",
      ERR_TYPES_NOT_FOUND => "ERR_TYPES_NOT_FOUND",
      ERR_INVALID_FILE_URL_PATH => "ERR_INVALID_FILE_URL_PATH",
      ERR_UNKNOWN_BUILTIN_MODULE => "ERR_UNKNOWN_BUILTIN_MODULE",
    }
  }
}

impl From<NodeJsErrorCode> for deno_error::PropertyValue {
  fn from(value: NodeJsErrorCode) -> Self {
    deno_error::PropertyValue::String(value.as_str().into())
  }
}

pub trait NodeJsErrorCoded {
  fn code(&self) -> NodeJsErrorCode;
}

#[derive(Debug, Clone, Error, JsError)]
#[error(
  "[{}] Invalid module '{}' {}{}",
  self.code(),
  request,
  reason,
  maybe_referrer.as_ref().map(|referrer| format!(" imported from '{}'", referrer)).unwrap_or_default()
)]
#[class(type)]
#[property("code" = self.code())]
pub struct InvalidModuleSpecifierError {
  pub request: String,
  pub reason: Cow<'static, str>,
  pub maybe_referrer: Option<String>,
}

impl NodeJsErrorCoded for InvalidModuleSpecifierError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_INVALID_MODULE_SPECIFIER
  }
}

#[derive(Debug, Boxed, JsError)]
pub struct LegacyResolveError(pub Box<LegacyResolveErrorKind>);

impl LegacyResolveError {
  pub fn specifier(&self) -> &UrlOrPath {
    match self.as_kind() {
      LegacyResolveErrorKind::TypesNotFound(err) => &err.0.code_specifier,
      LegacyResolveErrorKind::ModuleNotFound(err) => &err.specifier,
    }
  }
}

#[derive(Debug, Error, JsError)]
pub enum LegacyResolveErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  TypesNotFound(#[from] TypesNotFoundError),
  #[class(inherit)]
  #[error(transparent)]
  ModuleNotFound(#[from] ModuleNotFoundError),
}

impl NodeJsErrorCoded for LegacyResolveError {
  fn code(&self) -> NodeJsErrorCode {
    match self.as_kind() {
      LegacyResolveErrorKind::TypesNotFound(e) => e.code(),
      LegacyResolveErrorKind::ModuleNotFound(e) => e.code(),
    }
  }
}

#[derive(Debug, Error, JsError)]
#[error(
  "Could not find package '{}' from referrer '{}'{}.",
  package_name,
  referrer,
  referrer_extra.as_ref().map(|r| format!(" ({})", r)).unwrap_or_default()
)]
#[class(generic)]
#[property("code" = self.code())]
pub struct PackageNotFoundError {
  pub package_name: String,
  pub referrer: UrlOrPath,
  /// Extra information about the referrer.
  pub referrer_extra: Option<String>,
}

impl NodeJsErrorCoded for PackageNotFoundError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_MODULE_NOT_FOUND
  }
}

#[derive(Debug, Error, JsError)]
#[error(
  "Could not find referrer npm package '{}'{}.",
  referrer,
  referrer_extra.as_ref().map(|r| format!(" ({})", r)).unwrap_or_default()
)]
#[class(generic)]
#[property("code" = self.code())]
pub struct ReferrerNotFoundError {
  pub referrer: UrlOrPath,
  /// Extra information about the referrer.
  pub referrer_extra: Option<String>,
}

impl NodeJsErrorCoded for ReferrerNotFoundError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_MODULE_NOT_FOUND
  }
}

#[derive(Debug, Error, JsError)]
#[class(inherit)]
#[error("Failed resolving '{package_name}' from referrer '{referrer}'.")]
#[property("code" = self.code())]
pub struct PackageFolderResolveIoError {
  pub package_name: String,
  pub referrer: UrlOrPath,
  #[source]
  #[inherit]
  pub source: std::io::Error,
}

impl NodeJsErrorCoded for PackageFolderResolveIoError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_MODULE_NOT_FOUND
  }
}

impl NodeJsErrorCoded for PackageFolderResolveErrorKind {
  fn code(&self) -> NodeJsErrorCode {
    match self {
      PackageFolderResolveErrorKind::PackageNotFound(e) => e.code(),
      PackageFolderResolveErrorKind::ReferrerNotFound(e) => e.code(),
      PackageFolderResolveErrorKind::Io(e) => e.code(),
      PackageFolderResolveErrorKind::PathToUrl(_) => {
        NodeJsErrorCode::ERR_INVALID_FILE_URL_PATH
      }
    }
  }
}

#[derive(Debug, Boxed, JsError)]
pub struct PackageFolderResolveError(pub Box<PackageFolderResolveErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum PackageFolderResolveErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  PackageNotFound(#[from] PackageNotFoundError),
  #[class(inherit)]
  #[error(transparent)]
  ReferrerNotFound(#[from] ReferrerNotFoundError),
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] PackageFolderResolveIoError),
  #[class(inherit)]
  #[error(transparent)]
  #[property("code" = self.code())]
  PathToUrl(#[from] deno_path_util::PathToUrlError),
}

#[derive(Debug, Boxed, JsError)]
pub struct PackageSubpathFromDenoModuleResolveError(
  pub Box<PackageSubpathFromDenoModuleResolveErrorKind>,
);

impl NodeJsErrorCoded for PackageSubpathFromDenoModuleResolveError {
  fn code(&self) -> NodeJsErrorCode {
    match self.as_kind() {
      PackageSubpathFromDenoModuleResolveErrorKind::SubPath(e) => e.code(),
      PackageSubpathFromDenoModuleResolveErrorKind::FinalizeResolution(e) => {
        e.code()
      }
    }
  }
}

impl PackageSubpathFromDenoModuleResolveError {
  pub fn as_types_not_found(&self) -> Option<&TypesNotFoundError> {
    match self.as_kind() {
      PackageSubpathFromDenoModuleResolveErrorKind::SubPath(e) => {
        e.as_types_not_found()
      }
      PackageSubpathFromDenoModuleResolveErrorKind::FinalizeResolution(e) => {
        e.as_types_not_found()
      }
    }
  }

  pub fn maybe_specifier(&self) -> Option<Cow<'_, UrlOrPath>> {
    match self.as_kind() {
      PackageSubpathFromDenoModuleResolveErrorKind::SubPath(e) => {
        e.maybe_specifier()
      }
      PackageSubpathFromDenoModuleResolveErrorKind::FinalizeResolution(e) => {
        e.maybe_specifier()
      }
    }
  }

  pub fn into_node_resolve_error(self) -> NodeResolveError {
    match self.into_kind() {
      PackageSubpathFromDenoModuleResolveErrorKind::SubPath(e) => {
        NodeResolveErrorKind::PackageResolve(
          PackageResolveErrorKind::SubpathResolve(e).into_box(),
        )
      }
      PackageSubpathFromDenoModuleResolveErrorKind::FinalizeResolution(e) => {
        NodeResolveErrorKind::FinalizeResolution(e)
      }
    }
    .into_box()
  }
}

#[derive(Debug, Error, JsError)]
pub enum PackageSubpathFromDenoModuleResolveErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  SubPath(#[from] PackageSubpathResolveError),
  #[class(inherit)]
  #[error(transparent)]
  FinalizeResolution(#[from] FinalizeResolutionError),
}

#[derive(Debug, Boxed, JsError)]
pub struct PackageSubpathResolveError(pub Box<PackageSubpathResolveErrorKind>);

impl NodeJsErrorCoded for PackageSubpathResolveError {
  fn code(&self) -> NodeJsErrorCode {
    match self.as_kind() {
      PackageSubpathResolveErrorKind::PkgJsonLoad(e) => e.code(),
      PackageSubpathResolveErrorKind::Exports(e) => e.code(),
      PackageSubpathResolveErrorKind::LegacyResolve(e) => e.code(),
    }
  }
}

impl PackageSubpathResolveError {
  pub fn as_types_not_found(&self) -> Option<&TypesNotFoundError> {
    self.as_kind().as_types_not_found()
  }

  pub fn maybe_specifier(&self) -> Option<Cow<'_, UrlOrPath>> {
    match self.as_kind() {
      PackageSubpathResolveErrorKind::PkgJsonLoad(_) => None,
      PackageSubpathResolveErrorKind::Exports(err) => err.maybe_specifier(),
      PackageSubpathResolveErrorKind::LegacyResolve(err) => {
        Some(Cow::Borrowed(err.specifier()))
      }
    }
  }
}

#[derive(Debug, Error, JsError)]
pub enum PackageSubpathResolveErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  PkgJsonLoad(#[from] PackageJsonLoadError),
  #[class(inherit)]
  #[error(transparent)]
  Exports(PackageExportsResolveError),
  #[class(inherit)]
  #[error(transparent)]
  LegacyResolve(LegacyResolveError),
}

impl PackageSubpathResolveErrorKind {
  pub fn as_types_not_found(&self) -> Option<&TypesNotFoundError> {
    match self {
      PackageSubpathResolveErrorKind::PkgJsonLoad(_) => None,
      PackageSubpathResolveErrorKind::Exports(err) => match err.as_kind() {
        PackageExportsResolveErrorKind::PackagePathNotExported(_) => None,
        PackageExportsResolveErrorKind::PackageTargetResolve(err) => {
          match err.as_kind() {
            PackageTargetResolveErrorKind::TypesNotFound(not_found) => {
              Some(not_found)
            }
            PackageTargetResolveErrorKind::NotFound(_)
            | PackageTargetResolveErrorKind::InvalidPackageTarget(_)
            | PackageTargetResolveErrorKind::InvalidModuleSpecifier(_)
            | PackageTargetResolveErrorKind::PackageResolve(_)
            | PackageTargetResolveErrorKind::UnknownBuiltInNodeModule(_)
            | PackageTargetResolveErrorKind::UrlToFilePath(_) => None,
          }
        }
      },
      PackageSubpathResolveErrorKind::LegacyResolve(err) => match err.as_kind()
      {
        LegacyResolveErrorKind::TypesNotFound(not_found) => Some(not_found),
        LegacyResolveErrorKind::ModuleNotFound(_) => None,
      },
    }
  }
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error(
  "Target '{}' not found from '{}'{}{}.",
  target,
  pkg_json_path.display(),
  maybe_referrer.as_ref().map(|r|
    format!(
      " from{} referrer {}",
      match resolution_mode {
        ResolutionMode::Import => "",
        ResolutionMode::Require => " cjs",
      },
      r
    )
  ).unwrap_or_default(),
  match resolution_kind {
    NodeResolutionKind::Execution => "",
    NodeResolutionKind::Types => " for types",
  }
)]
#[property("code" = self.code())]
pub struct PackageTargetNotFoundError {
  pub pkg_json_path: PathBuf,
  pub target: String,
  pub maybe_resolved: Option<UrlOrPath>,
  pub maybe_referrer: Option<UrlOrPath>,
  pub resolution_mode: ResolutionMode,
  pub resolution_kind: NodeResolutionKind,
}

impl NodeJsErrorCoded for PackageTargetNotFoundError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_MODULE_NOT_FOUND
  }
}

impl NodeJsErrorCoded for PackageTargetResolveErrorKind {
  fn code(&self) -> NodeJsErrorCode {
    match self {
      PackageTargetResolveErrorKind::NotFound(e) => e.code(),
      PackageTargetResolveErrorKind::InvalidPackageTarget(e) => e.code(),
      PackageTargetResolveErrorKind::InvalidModuleSpecifier(e) => e.code(),
      PackageTargetResolveErrorKind::PackageResolve(e) => e.code(),
      PackageTargetResolveErrorKind::TypesNotFound(e) => e.code(),
      PackageTargetResolveErrorKind::UnknownBuiltInNodeModule(e) => e.code(),
      PackageTargetResolveErrorKind::UrlToFilePath(_) => {
        NodeJsErrorCode::ERR_INVALID_FILE_URL_PATH
      }
    }
  }
}

#[derive(Debug, Boxed, JsError)]
pub struct PackageTargetResolveError(pub Box<PackageTargetResolveErrorKind>);

impl PackageTargetResolveError {
  pub fn maybe_specifier(&self) -> Option<Cow<'_, UrlOrPath>> {
    match self.as_kind() {
      PackageTargetResolveErrorKind::NotFound(err) => {
        err.maybe_resolved.as_ref().map(Cow::Borrowed)
      }
      PackageTargetResolveErrorKind::PackageResolve(err) => {
        err.maybe_specifier()
      }
      PackageTargetResolveErrorKind::TypesNotFound(err) => {
        Some(Cow::Borrowed(&err.0.code_specifier))
      }
      PackageTargetResolveErrorKind::UnknownBuiltInNodeModule(err) => {
        err.maybe_specifier().map(|u| Cow::Owned(UrlOrPath::Url(u)))
      }
      PackageTargetResolveErrorKind::UrlToFilePath(e) => {
        Some(Cow::Owned(UrlOrPath::Url(e.0.clone())))
      }
      PackageTargetResolveErrorKind::InvalidPackageTarget(_)
      | PackageTargetResolveErrorKind::InvalidModuleSpecifier(_) => None,
    }
  }
}

#[derive(Debug, Error, JsError)]
pub enum PackageTargetResolveErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  NotFound(#[from] PackageTargetNotFoundError),
  #[class(inherit)]
  #[error(transparent)]
  InvalidPackageTarget(#[from] InvalidPackageTargetError),
  #[class(inherit)]
  #[error(transparent)]
  InvalidModuleSpecifier(#[from] InvalidModuleSpecifierError),
  #[class(inherit)]
  #[error(transparent)]
  PackageResolve(#[from] PackageResolveError),
  #[class(inherit)]
  #[error(transparent)]
  TypesNotFound(#[from] TypesNotFoundError),
  #[class(inherit)]
  #[error(transparent)]
  UnknownBuiltInNodeModule(#[from] UnknownBuiltInNodeModuleError),
  #[class(inherit)]
  #[error(transparent)]
  #[property("code" = self.code())]
  UrlToFilePath(#[from] deno_path_util::UrlToFilePathError),
}

impl PackageTargetResolveErrorKind {
  pub fn as_types_not_found(&self) -> Option<&TypesNotFoundError> {
    match self {
      Self::TypesNotFound(not_found) => Some(not_found),
      _ => None,
    }
  }
}

impl NodeJsErrorCoded for PackageExportsResolveErrorKind {
  fn code(&self) -> NodeJsErrorCode {
    match self {
      PackageExportsResolveErrorKind::PackagePathNotExported(e) => e.code(),
      PackageExportsResolveErrorKind::PackageTargetResolve(e) => e.code(),
    }
  }
}

#[derive(Debug, Boxed, JsError)]
pub struct PackageExportsResolveError(pub Box<PackageExportsResolveErrorKind>);

impl PackageExportsResolveError {
  pub fn maybe_specifier(&self) -> Option<Cow<'_, UrlOrPath>> {
    match self.as_kind() {
      PackageExportsResolveErrorKind::PackagePathNotExported(_) => None,
      PackageExportsResolveErrorKind::PackageTargetResolve(err) => {
        err.maybe_specifier()
      }
    }
  }
}

#[derive(Debug, Error, JsError)]
pub enum PackageExportsResolveErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  PackagePathNotExported(#[from] PackagePathNotExportedError),
  #[class(inherit)]
  #[error(transparent)]
  PackageTargetResolve(#[from] PackageTargetResolveError),
}

#[derive(Debug, Error, JsError)]
#[error(
    "[{}] Could not find types for '{}'{}",
    self.code(),
    self.0.code_specifier,
    self.0.maybe_referrer.as_ref().map(|r| format!(" imported from '{}'", r)).unwrap_or_default(),
  )]
#[class(generic)]
#[property("code" = self.code())]
pub struct TypesNotFoundError(pub Box<TypesNotFoundErrorData>);

#[derive(Debug)]
pub struct TypesNotFoundErrorData {
  pub code_specifier: UrlOrPath,
  pub maybe_referrer: Option<UrlOrPath>,
}

impl NodeJsErrorCoded for TypesNotFoundError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_TYPES_NOT_FOUND
  }
}

#[derive(Debug, Error, JsError)]
#[class(inherit)]
#[error("[{}] Invalid package config. {}", self.code(), .0)]
pub struct PackageJsonLoadError(pub deno_package_json::PackageJsonLoadError);

impl NodeJsErrorCoded for PackageJsonLoadError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_INVALID_PACKAGE_CONFIG
  }
}

#[derive(Debug, Error, JsError)]
#[class(type)]
#[error(
  "[{}] Package import specifier \"{}\" is not defined{}{}",
  self.code(),
  name,
  package_json_path.as_ref().map(|p| format!(" in package {}", p.display())).unwrap_or_default(),
  maybe_referrer.as_ref().map(|r| format!(" imported from '{}'", r)).unwrap_or_default(),
)]
#[property("code" = self.code())]
pub struct PackageImportNotDefinedError {
  pub name: String,
  pub package_json_path: Option<PathBuf>,
  pub maybe_referrer: Option<UrlOrPath>,
}

impl NodeJsErrorCoded for PackageImportNotDefinedError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_PACKAGE_IMPORT_NOT_DEFINED
  }
}

#[derive(Debug, Boxed, JsError)]
pub struct PackageImportsResolveError(pub Box<PackageImportsResolveErrorKind>);

impl PackageImportsResolveError {
  pub fn maybe_specifier(&self) -> Option<Cow<'_, UrlOrPath>> {
    match self.as_kind() {
      PackageImportsResolveErrorKind::Target(err) => err.maybe_specifier(),
      PackageImportsResolveErrorKind::PkgJsonLoad(_)
      | PackageImportsResolveErrorKind::InvalidModuleSpecifier(_)
      | PackageImportsResolveErrorKind::NotDefined(_) => None,
    }
  }
}

#[derive(Debug, Error, JsError)]
pub enum PackageImportsResolveErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  PkgJsonLoad(PackageJsonLoadError),
  #[class(inherit)]
  #[error(transparent)]
  InvalidModuleSpecifier(#[from] InvalidModuleSpecifierError),
  #[class(inherit)]
  #[error(transparent)]
  NotDefined(#[from] PackageImportNotDefinedError),
  #[class(inherit)]
  #[error(transparent)]
  Target(#[from] PackageTargetResolveError),
}

impl PackageImportsResolveErrorKind {
  pub fn as_types_not_found(&self) -> Option<&TypesNotFoundError> {
    match self {
      Self::Target(err) => err.as_types_not_found(),
      _ => None,
    }
  }
}

impl NodeJsErrorCoded for PackageImportsResolveErrorKind {
  fn code(&self) -> NodeJsErrorCode {
    match self {
      Self::PkgJsonLoad(e) => e.code(),
      Self::InvalidModuleSpecifier(e) => e.code(),
      Self::NotDefined(e) => e.code(),
      Self::Target(e) => e.code(),
    }
  }
}

impl NodeJsErrorCoded for PackageResolveErrorKind {
  fn code(&self) -> NodeJsErrorCode {
    match self {
      PackageResolveErrorKind::PkgJsonLoad(e) => e.code(),
      PackageResolveErrorKind::InvalidModuleSpecifier(e) => e.code(),
      PackageResolveErrorKind::PackageFolderResolve(e) => e.code(),
      PackageResolveErrorKind::ExportsResolve(e) => e.code(),
      PackageResolveErrorKind::SubpathResolve(e) => e.code(),
      PackageResolveErrorKind::UrlToFilePath(_) => {
        NodeJsErrorCode::ERR_INVALID_FILE_URL_PATH
      }
    }
  }
}

#[derive(Debug, Boxed, JsError)]
pub struct PackageResolveError(pub Box<PackageResolveErrorKind>);

impl PackageResolveError {
  pub fn maybe_specifier(&self) -> Option<Cow<'_, UrlOrPath>> {
    match self.as_kind() {
      PackageResolveErrorKind::ExportsResolve(err) => err.maybe_specifier(),
      PackageResolveErrorKind::SubpathResolve(err) => err.maybe_specifier(),
      PackageResolveErrorKind::UrlToFilePath(err) => {
        Some(Cow::Owned(UrlOrPath::Url(err.0.clone())))
      }
      PackageResolveErrorKind::PkgJsonLoad(_)
      | PackageResolveErrorKind::InvalidModuleSpecifier(_)
      | PackageResolveErrorKind::PackageFolderResolve(_) => None,
    }
  }
}

#[derive(Debug, Error, JsError)]
pub enum PackageResolveErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  PkgJsonLoad(#[from] PackageJsonLoadError),
  #[class(inherit)]
  #[error(transparent)]
  InvalidModuleSpecifier(#[from] InvalidModuleSpecifierError),
  #[class(inherit)]
  #[error(transparent)]
  PackageFolderResolve(#[from] PackageFolderResolveError),
  #[class(inherit)]
  #[error(transparent)]
  ExportsResolve(#[from] PackageExportsResolveError),
  #[class(inherit)]
  #[error(transparent)]
  SubpathResolve(#[from] PackageSubpathResolveError),
  #[class(inherit)]
  #[error(transparent)]
  #[property("code" = self.code())]
  UrlToFilePath(#[from] UrlToFilePathError),
}

impl PackageResolveErrorKind {
  pub fn as_types_not_found(&self) -> Option<&TypesNotFoundError> {
    match self {
      PackageResolveErrorKind::PkgJsonLoad(_)
      | PackageResolveErrorKind::InvalidModuleSpecifier(_)
      | PackageResolveErrorKind::PackageFolderResolve(_)
      | PackageResolveErrorKind::ExportsResolve(_)
      | PackageResolveErrorKind::UrlToFilePath(_) => None,
      PackageResolveErrorKind::SubpathResolve(err) => err.as_types_not_found(),
    }
  }
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Failed joining '{path}' from '{base}'.")]
pub struct NodeResolveRelativeJoinError {
  pub path: String,
  pub base: Url,
  #[source]
  pub source: url::ParseError,
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Failed resolving specifier from data url referrer.")]
pub struct DataUrlReferrerError {
  #[source]
  pub source: url::ParseError,
}

#[derive(Debug, Boxed, JsError)]
pub struct NodeResolveError(pub Box<NodeResolveErrorKind>);

impl NodeResolveError {
  pub fn maybe_specifier(&self) -> Option<Cow<'_, UrlOrPath>> {
    match self.as_kind() {
      NodeResolveErrorKind::PathToUrl(err) => {
        Some(Cow::Owned(UrlOrPath::Path(err.0.clone())))
      }
      NodeResolveErrorKind::UrlToFilePath(err) => {
        Some(Cow::Owned(UrlOrPath::Url(err.0.clone())))
      }
      NodeResolveErrorKind::PackageImportsResolve(err) => err.maybe_specifier(),
      NodeResolveErrorKind::PackageResolve(err) => err.maybe_specifier(),
      NodeResolveErrorKind::TypesNotFound(err) => {
        Some(Cow::Borrowed(&err.0.code_specifier))
      }
      NodeResolveErrorKind::UnknownBuiltInNodeModule(err) => {
        err.maybe_specifier().map(|u| Cow::Owned(UrlOrPath::Url(u)))
      }
      NodeResolveErrorKind::FinalizeResolution(err) => err.maybe_specifier(),
      NodeResolveErrorKind::UnsupportedEsmUrlScheme(_)
      | NodeResolveErrorKind::DataUrlReferrer(_)
      | NodeResolveErrorKind::RelativeJoin(_) => None,
    }
  }
}

#[derive(Debug, Error, JsError)]
pub enum NodeResolveErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  RelativeJoin(#[from] NodeResolveRelativeJoinError),
  #[class(inherit)]
  #[error(transparent)]
  PathToUrl(#[from] deno_path_util::PathToUrlError),
  #[class(inherit)]
  #[error(transparent)]
  UrlToFilePath(#[from] deno_path_util::UrlToFilePathError),
  #[class(inherit)]
  #[error(transparent)]
  PackageImportsResolve(#[from] PackageImportsResolveError),
  #[class(inherit)]
  #[error(transparent)]
  UnsupportedEsmUrlScheme(#[from] UnsupportedEsmUrlSchemeError),
  #[class(inherit)]
  #[error(transparent)]
  DataUrlReferrer(#[from] DataUrlReferrerError),
  #[class(inherit)]
  #[error(transparent)]
  PackageResolve(#[from] PackageResolveError),
  #[class(inherit)]
  #[error(transparent)]
  TypesNotFound(#[from] TypesNotFoundError),
  #[class(inherit)]
  #[error(transparent)]
  UnknownBuiltInNodeModule(#[from] UnknownBuiltInNodeModuleError),
  #[class(inherit)]
  #[error(transparent)]
  FinalizeResolution(#[from] FinalizeResolutionError),
}

impl NodeResolveErrorKind {
  pub fn as_types_not_found(&self) -> Option<&TypesNotFoundError> {
    match self {
      NodeResolveErrorKind::TypesNotFound(not_found) => Some(not_found),
      NodeResolveErrorKind::PackageImportsResolve(err) => {
        err.as_kind().as_types_not_found()
      }
      NodeResolveErrorKind::PackageResolve(package_resolve_error) => {
        package_resolve_error.as_types_not_found()
      }
      NodeResolveErrorKind::UnsupportedEsmUrlScheme(_)
      | NodeResolveErrorKind::DataUrlReferrer(_)
      | NodeResolveErrorKind::FinalizeResolution(_)
      | NodeResolveErrorKind::RelativeJoin(_)
      | NodeResolveErrorKind::PathToUrl(_)
      | NodeResolveErrorKind::UnknownBuiltInNodeModule(_)
      | NodeResolveErrorKind::UrlToFilePath(_) => None,
    }
  }

  pub fn maybe_code(&self) -> Option<NodeJsErrorCode> {
    match self {
      NodeResolveErrorKind::RelativeJoin(_) => None,
      NodeResolveErrorKind::PathToUrl(_) => None,
      NodeResolveErrorKind::UrlToFilePath(_) => None,
      NodeResolveErrorKind::PackageImportsResolve(e) => Some(e.code()),
      NodeResolveErrorKind::UnsupportedEsmUrlScheme(e) => Some(e.code()),
      NodeResolveErrorKind::DataUrlReferrer(_) => None,
      NodeResolveErrorKind::PackageResolve(e) => Some(e.code()),
      NodeResolveErrorKind::TypesNotFound(e) => Some(e.code()),
      NodeResolveErrorKind::UnknownBuiltInNodeModule(e) => Some(e.code()),
      NodeResolveErrorKind::FinalizeResolution(e) => Some(e.code()),
    }
  }
}

#[derive(Debug, Boxed, JsError)]
pub struct FinalizeResolutionError(pub Box<FinalizeResolutionErrorKind>);

impl FinalizeResolutionError {
  pub fn maybe_specifier(&self) -> Option<Cow<'_, UrlOrPath>> {
    match self.as_kind() {
      FinalizeResolutionErrorKind::ModuleNotFound(err) => {
        Some(Cow::Borrowed(&err.specifier))
      }
      FinalizeResolutionErrorKind::TypesNotFound(err) => {
        Some(Cow::Borrowed(&err.0.code_specifier))
      }
      FinalizeResolutionErrorKind::UnsupportedDirImport(err) => {
        Some(Cow::Borrowed(&err.dir_url))
      }
      FinalizeResolutionErrorKind::PackageSubpathResolve(err) => {
        err.maybe_specifier()
      }
      FinalizeResolutionErrorKind::InvalidModuleSpecifierError(_)
      | FinalizeResolutionErrorKind::UrlToFilePath(_) => None,
    }
  }

  pub fn as_types_not_found(&self) -> Option<&TypesNotFoundError> {
    match self.as_kind() {
      FinalizeResolutionErrorKind::TypesNotFound(err) => Some(err),
      FinalizeResolutionErrorKind::PackageSubpathResolve(err) => {
        err.as_types_not_found()
      }
      FinalizeResolutionErrorKind::ModuleNotFound(_)
      | FinalizeResolutionErrorKind::UnsupportedDirImport(_)
      | FinalizeResolutionErrorKind::InvalidModuleSpecifierError(_)
      | FinalizeResolutionErrorKind::UrlToFilePath(_) => None,
    }
  }
}

#[derive(Debug, Error, JsError)]
pub enum FinalizeResolutionErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  InvalidModuleSpecifierError(#[from] InvalidModuleSpecifierError),
  #[class(inherit)]
  #[error(transparent)]
  ModuleNotFound(#[from] ModuleNotFoundError),
  #[class(inherit)]
  #[error(transparent)]
  TypesNotFound(#[from] TypesNotFoundError),
  #[class(inherit)]
  #[error(transparent)]
  PackageSubpathResolve(#[from] PackageSubpathResolveError),
  #[class(inherit)]
  #[error(transparent)]
  UnsupportedDirImport(#[from] UnsupportedDirImportError),
  #[class(inherit)]
  #[error(transparent)]
  #[property("code" = self.code())]
  UrlToFilePath(#[from] deno_path_util::UrlToFilePathError),
}

impl NodeJsErrorCoded for FinalizeResolutionErrorKind {
  fn code(&self) -> NodeJsErrorCode {
    match self {
      FinalizeResolutionErrorKind::InvalidModuleSpecifierError(e) => e.code(),
      FinalizeResolutionErrorKind::ModuleNotFound(e) => e.code(),
      FinalizeResolutionErrorKind::TypesNotFound(e) => e.code(),
      FinalizeResolutionErrorKind::PackageSubpathResolve(e) => e.code(),
      FinalizeResolutionErrorKind::UnsupportedDirImport(e) => e.code(),
      FinalizeResolutionErrorKind::UrlToFilePath(_) => {
        NodeJsErrorCode::ERR_INVALID_FILE_URL_PATH
      }
    }
  }
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error(
  "[{}] Cannot find module '{}'{}{}",
  self.code(),
  specifier,
  maybe_referrer.as_ref().map(|referrer| format!(" imported from '{}'", referrer)).unwrap_or_default(),
  suggested_ext.as_ref().map(|m| format!("\nDid you mean to import with the \".{}\" extension?", m)).unwrap_or_default()
)]
#[property("code" = self.code())]
pub struct ModuleNotFoundError {
  pub specifier: UrlOrPath,
  pub maybe_referrer: Option<UrlOrPath>,
  pub suggested_ext: Option<&'static str>,
}

impl NodeJsErrorCoded for ModuleNotFoundError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_MODULE_NOT_FOUND
  }
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error(
  "[{}] Directory import '{}' is not supported resolving ES modules{}{}",
  self.code(),
  dir_url,
  maybe_referrer.as_ref().map(|referrer| format!(" imported from '{}'", referrer)).unwrap_or_default(),
  suggestion.as_ref().map(|suggestion| format!("\nDid you mean to import '{suggestion}'?")).unwrap_or_default(),
)]
#[property("code" = self.code())]
pub struct UnsupportedDirImportError {
  pub dir_url: UrlOrPath,
  pub maybe_referrer: Option<UrlOrPath>,
  pub suggestion: Option<String>,
}

impl NodeJsErrorCoded for UnsupportedDirImportError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_UNSUPPORTED_DIR_IMPORT
  }
}

#[derive(Debug, JsError)]
#[class(generic)]
#[property("code" = self.code())]
pub struct InvalidPackageTargetError {
  pub pkg_json_path: PathBuf,
  pub sub_path: String,
  pub target: String,
  pub is_import: bool,
  pub maybe_referrer: Option<UrlOrPath>,
}

impl std::error::Error for InvalidPackageTargetError {}

impl std::fmt::Display for InvalidPackageTargetError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let rel_error = !self.is_import
      && !self.target.is_empty()
      && !self.target.starts_with("./");
    f.write_char('[')?;
    f.write_str(self.code().as_str())?;
    f.write_char(']')?;

    if self.sub_path == "." {
      assert!(!self.is_import);
      write!(
        f,
        " Invalid \"exports\" main target {} defined in the package config {}",
        self.target,
        self.pkg_json_path.display()
      )?;
    } else {
      let ie = if self.is_import { "imports" } else { "exports" };
      write!(
        f,
        " Invalid \"{}\" target {} defined for '{}' in the package config {}",
        ie,
        self.target,
        self.sub_path,
        self.pkg_json_path.display()
      )?;
    };

    if let Some(referrer) = &self.maybe_referrer {
      write!(f, " imported from '{}'", referrer)?;
    }
    if rel_error {
      write!(f, "; target must start with \"./\"")?;
    }
    Ok(())
  }
}

impl NodeJsErrorCoded for InvalidPackageTargetError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_INVALID_PACKAGE_TARGET
  }
}

#[derive(Debug, JsError)]
#[class(generic)]
#[property("code" = self.code())]
pub struct PackagePathNotExportedError {
  pub pkg_json_path: PathBuf,
  pub subpath: String,
  pub maybe_referrer: Option<UrlOrPath>,
  pub resolution_kind: NodeResolutionKind,
}

impl NodeJsErrorCoded for PackagePathNotExportedError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_PACKAGE_PATH_NOT_EXPORTED
  }
}

impl std::error::Error for PackagePathNotExportedError {}

impl std::fmt::Display for PackagePathNotExportedError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_char('[')?;
    f.write_str(self.code().as_str())?;
    f.write_char(']')?;

    let types_msg = match self.resolution_kind {
      NodeResolutionKind::Execution => String::new(),
      NodeResolutionKind::Types => " for types".to_string(),
    };
    if self.subpath == "." {
      write!(
        f,
        " No \"exports\" main defined{} in '{}'",
        types_msg,
        self.pkg_json_path.display()
      )?;
    } else {
      write!(
        f,
        " Package subpath '{}' is not defined{} by \"exports\" in '{}'",
        self.subpath,
        types_msg,
        self.pkg_json_path.display()
      )?;
    };

    if let Some(referrer) = &self.maybe_referrer {
      write!(f, " imported from '{}'", referrer)?;
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Error, JsError)]
#[class(type)]
#[error(
  "[{}] Only file and data URLs are supported by the default ESM loader.{} Received protocol '{}'",
  self.code(),
  if cfg!(windows) && url_scheme.len() == 2 { " On Windows, absolute path must be valid file:// URLS."} else { "" },
  url_scheme
)]
#[property("code" = self.code())]
pub struct UnsupportedEsmUrlSchemeError {
  pub url_scheme: String,
}

impl NodeJsErrorCoded for UnsupportedEsmUrlSchemeError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_UNSUPPORTED_ESM_URL_SCHEME
  }
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Failed resolving binary export. '{}' did not exist", pkg_json_path.display())]
pub struct MissingPkgJsonError {
  pub pkg_json_path: PathBuf,
}

#[derive(Debug, Error, JsError)]
pub enum ResolvePkgJsonBinExportError {
  #[class(inherit)]
  #[error(transparent)]
  ResolvePkgNpmBinaryCommands(#[from] ResolvePkgNpmBinaryCommandsError),
  #[class(generic)]
  #[error("Failed resolving binary export. {message}")]
  InvalidBinProperty { message: String },
}

#[derive(Debug, Error, JsError)]
pub enum ResolvePkgNpmBinaryCommandsError {
  #[class(inherit)]
  #[error(transparent)]
  PkgJsonLoad(#[from] PackageJsonLoadError),
  #[class(generic)]
  #[error(transparent)]
  MissingPkgJson(#[from] MissingPkgJsonError),
  #[class(inherit)]
  #[error(transparent)]
  MissingPkgJsonName(#[from] MissingPkgJsonNameError),
}

#[derive(Error, Debug, Clone, deno_error::JsError)]
#[class("NotFound")]
#[error("No such built-in module: node:{module_name}")]
#[property("code" = self.code())]
pub struct UnknownBuiltInNodeModuleError {
  /// Name of the invalid module.
  pub module_name: String,
}

impl UnknownBuiltInNodeModuleError {
  pub fn maybe_specifier(&self) -> Option<Url> {
    Url::parse(&format!("node:{}", self.module_name)).ok()
  }
}

impl NodeJsErrorCoded for UnknownBuiltInNodeModuleError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_UNKNOWN_BUILTIN_MODULE
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn types_resolution_package_path_not_exported() {
    let separator_char = if cfg!(windows) { '\\' } else { '/' };
    assert_eq!(
      PackagePathNotExportedError {
        pkg_json_path: PathBuf::from("test_path").join("package.json"),
        subpath: "./jsx-runtime".to_string(),
        maybe_referrer: None,
        resolution_kind: NodeResolutionKind::Types
      }
      .to_string(),
      format!(
        "[ERR_PACKAGE_PATH_NOT_EXPORTED] Package subpath './jsx-runtime' is not defined for types by \"exports\" in 'test_path{separator_char}package.json'"
      )
    );
    assert_eq!(
      PackagePathNotExportedError {
        pkg_json_path: PathBuf::from("test_path").join("package.json"),
        subpath: ".".to_string(),
        maybe_referrer: None,
        resolution_kind: NodeResolutionKind::Types
      }
      .to_string(),
      format!(
        "[ERR_PACKAGE_PATH_NOT_EXPORTED] No \"exports\" main defined for types in 'test_path{separator_char}package.json'"
      )
    );
  }
}
