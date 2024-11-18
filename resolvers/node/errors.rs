// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::fmt::Write;
use std::path::PathBuf;

use boxed_error::Boxed;
use thiserror::Error;
use url::Url;

use crate::NodeModuleKind;
use crate::NodeResolutionMode;

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
  /// Deno specific since Node doesn't support TypeScript.
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
    }
  }
}

pub trait NodeJsErrorCoded {
  fn code(&self) -> NodeJsErrorCode;
}

// todo(https://github.com/denoland/deno_core/issues/810): make this a TypeError
#[derive(Debug, Clone, Error)]
#[error(
  "[{}] Invalid module '{}' {}{}",
  self.code(),
  request,
  reason,
  maybe_referrer.as_ref().map(|referrer| format!(" imported from '{}'", referrer)).unwrap_or_default()
)]
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

#[derive(Debug, Boxed)]
pub struct LegacyResolveError(pub Box<LegacyResolveErrorKind>);

#[derive(Debug, Error)]
pub enum LegacyResolveErrorKind {
  #[error(transparent)]
  TypesNotFound(#[from] TypesNotFoundError),
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

#[derive(Debug, Error)]
#[error(
  "Could not find package '{}' from referrer '{}'{}.",
  package_name,
  referrer,
  referrer_extra.as_ref().map(|r| format!(" ({})", r)).unwrap_or_default()
)]
pub struct PackageNotFoundError {
  pub package_name: String,
  pub referrer: Url,
  /// Extra information about the referrer.
  pub referrer_extra: Option<String>,
}

impl NodeJsErrorCoded for PackageNotFoundError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_MODULE_NOT_FOUND
  }
}

#[derive(Debug, Error)]
#[error(
  "Could not find referrer npm package '{}'{}.",
  referrer,
  referrer_extra.as_ref().map(|r| format!(" ({})", r)).unwrap_or_default()
)]
pub struct ReferrerNotFoundError {
  pub referrer: Url,
  /// Extra information about the referrer.
  pub referrer_extra: Option<String>,
}

impl NodeJsErrorCoded for ReferrerNotFoundError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_MODULE_NOT_FOUND
  }
}

#[derive(Debug, Error)]
#[error("Failed resolving '{package_name}' from referrer '{referrer}'.")]
pub struct PackageFolderResolveIoError {
  pub package_name: String,
  pub referrer: Url,
  #[source]
  pub source: std::io::Error,
}

impl NodeJsErrorCoded for PackageFolderResolveIoError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_MODULE_NOT_FOUND
  }
}

impl NodeJsErrorCoded for PackageFolderResolveError {
  fn code(&self) -> NodeJsErrorCode {
    match self.as_kind() {
      PackageFolderResolveErrorKind::PackageNotFound(e) => e.code(),
      PackageFolderResolveErrorKind::ReferrerNotFound(e) => e.code(),
      PackageFolderResolveErrorKind::Io(e) => e.code(),
    }
  }
}

#[derive(Debug, Boxed)]
pub struct PackageFolderResolveError(pub Box<PackageFolderResolveErrorKind>);

#[derive(Debug, Error)]
pub enum PackageFolderResolveErrorKind {
  #[error(transparent)]
  PackageNotFound(#[from] PackageNotFoundError),
  #[error(transparent)]
  ReferrerNotFound(#[from] ReferrerNotFoundError),
  #[error(transparent)]
  Io(#[from] PackageFolderResolveIoError),
}

impl NodeJsErrorCoded for PackageSubpathResolveError {
  fn code(&self) -> NodeJsErrorCode {
    match self.as_kind() {
      PackageSubpathResolveErrorKind::PkgJsonLoad(e) => e.code(),
      PackageSubpathResolveErrorKind::Exports(e) => e.code(),
      PackageSubpathResolveErrorKind::LegacyResolve(e) => e.code(),
    }
  }
}

#[derive(Debug, Boxed)]
pub struct PackageSubpathResolveError(pub Box<PackageSubpathResolveErrorKind>);

#[derive(Debug, Error)]
pub enum PackageSubpathResolveErrorKind {
  #[error(transparent)]
  PkgJsonLoad(#[from] PackageJsonLoadError),
  #[error(transparent)]
  Exports(PackageExportsResolveError),
  #[error(transparent)]
  LegacyResolve(LegacyResolveError),
}

#[derive(Debug, Error)]
#[error(
  "Target '{}' not found from '{}'{}{}.",
  target,
  pkg_json_path.display(),
  maybe_referrer.as_ref().map(|r|
    format!(
      " from{} referrer {}",
      match referrer_kind {
        NodeModuleKind::Esm => "",
        NodeModuleKind::Cjs => " cjs",
      },
      r
    )
  ).unwrap_or_default(),
  match mode {
    NodeResolutionMode::Execution => "",
    NodeResolutionMode::Types => " for types",
  }
)]
pub struct PackageTargetNotFoundError {
  pub pkg_json_path: PathBuf,
  pub target: String,
  pub maybe_referrer: Option<Url>,
  pub referrer_kind: NodeModuleKind,
  pub mode: NodeResolutionMode,
}

impl NodeJsErrorCoded for PackageTargetNotFoundError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_MODULE_NOT_FOUND
  }
}

impl NodeJsErrorCoded for PackageTargetResolveError {
  fn code(&self) -> NodeJsErrorCode {
    match self.as_kind() {
      PackageTargetResolveErrorKind::NotFound(e) => e.code(),
      PackageTargetResolveErrorKind::InvalidPackageTarget(e) => e.code(),
      PackageTargetResolveErrorKind::InvalidModuleSpecifier(e) => e.code(),
      PackageTargetResolveErrorKind::PackageResolve(e) => e.code(),
      PackageTargetResolveErrorKind::TypesNotFound(e) => e.code(),
    }
  }
}

#[derive(Debug, Boxed)]
pub struct PackageTargetResolveError(pub Box<PackageTargetResolveErrorKind>);

#[derive(Debug, Error)]
pub enum PackageTargetResolveErrorKind {
  #[error(transparent)]
  NotFound(#[from] PackageTargetNotFoundError),
  #[error(transparent)]
  InvalidPackageTarget(#[from] InvalidPackageTargetError),
  #[error(transparent)]
  InvalidModuleSpecifier(#[from] InvalidModuleSpecifierError),
  #[error(transparent)]
  PackageResolve(#[from] PackageResolveError),
  #[error(transparent)]
  TypesNotFound(#[from] TypesNotFoundError),
}

impl NodeJsErrorCoded for PackageExportsResolveError {
  fn code(&self) -> NodeJsErrorCode {
    match self.as_kind() {
      PackageExportsResolveErrorKind::PackagePathNotExported(e) => e.code(),
      PackageExportsResolveErrorKind::PackageTargetResolve(e) => e.code(),
    }
  }
}

#[derive(Debug, Boxed)]
pub struct PackageExportsResolveError(pub Box<PackageExportsResolveErrorKind>);

#[derive(Debug, Error)]
pub enum PackageExportsResolveErrorKind {
  #[error(transparent)]
  PackagePathNotExported(#[from] PackagePathNotExportedError),
  #[error(transparent)]
  PackageTargetResolve(#[from] PackageTargetResolveError),
}

#[derive(Debug, Error)]
#[error(
    "[{}] Could not find types for '{}'{}",
    self.code(),
    self.0.code_specifier,
    self.0.maybe_referrer.as_ref().map(|r| format!(" imported from '{}'", r)).unwrap_or_default(),
  )]
pub struct TypesNotFoundError(pub Box<TypesNotFoundErrorData>);

#[derive(Debug)]
pub struct TypesNotFoundErrorData {
  pub code_specifier: Url,
  pub maybe_referrer: Option<Url>,
}

impl NodeJsErrorCoded for TypesNotFoundError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_TYPES_NOT_FOUND
  }
}

#[derive(Debug, Error)]
#[error(
  "[{}] Invalid package config. {}",
  self.code(),
  self.0
)]
pub struct PackageJsonLoadError(
  #[source]
  #[from]
  pub deno_package_json::PackageJsonLoadError,
);

impl NodeJsErrorCoded for PackageJsonLoadError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_INVALID_PACKAGE_CONFIG
  }
}

impl NodeJsErrorCoded for ClosestPkgJsonError {
  fn code(&self) -> NodeJsErrorCode {
    match self.as_kind() {
      ClosestPkgJsonErrorKind::CanonicalizingDir(e) => e.code(),
      ClosestPkgJsonErrorKind::Load(e) => e.code(),
    }
  }
}

#[derive(Debug, Boxed)]
pub struct ClosestPkgJsonError(pub Box<ClosestPkgJsonErrorKind>);

#[derive(Debug, Error)]
pub enum ClosestPkgJsonErrorKind {
  #[error(transparent)]
  CanonicalizingDir(#[from] CanonicalizingPkgJsonDirError),
  #[error(transparent)]
  Load(#[from] PackageJsonLoadError),
}

#[derive(Debug, Error)]
#[error("[{}] Failed canonicalizing package.json directory '{}'.", self.code(), dir_path.display())]
pub struct CanonicalizingPkgJsonDirError {
  pub dir_path: PathBuf,
  #[source]
  pub source: std::io::Error,
}

impl NodeJsErrorCoded for CanonicalizingPkgJsonDirError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_MODULE_NOT_FOUND
  }
}

// todo(https://github.com/denoland/deno_core/issues/810): make this a TypeError
#[derive(Debug, Error)]
#[error(
  "[{}] Package import specifier \"{}\" is not defined{}{}",
  self.code(),
  name,
  package_json_path.as_ref().map(|p| format!(" in package {}", p.display())).unwrap_or_default(),
  maybe_referrer.as_ref().map(|r| format!(" imported from '{}'", r)).unwrap_or_default(),
)]
pub struct PackageImportNotDefinedError {
  pub name: String,
  pub package_json_path: Option<PathBuf>,
  pub maybe_referrer: Option<Url>,
}

impl NodeJsErrorCoded for PackageImportNotDefinedError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_PACKAGE_IMPORT_NOT_DEFINED
  }
}

#[derive(Debug, Boxed)]
pub struct PackageImportsResolveError(pub Box<PackageImportsResolveErrorKind>);

#[derive(Debug, Error)]
pub enum PackageImportsResolveErrorKind {
  #[error(transparent)]
  ClosestPkgJson(ClosestPkgJsonError),
  #[error(transparent)]
  InvalidModuleSpecifier(#[from] InvalidModuleSpecifierError),
  #[error(transparent)]
  NotDefined(#[from] PackageImportNotDefinedError),
  #[error(transparent)]
  Target(#[from] PackageTargetResolveError),
}

impl NodeJsErrorCoded for PackageImportsResolveErrorKind {
  fn code(&self) -> NodeJsErrorCode {
    match self {
      Self::ClosestPkgJson(e) => e.code(),
      Self::InvalidModuleSpecifier(e) => e.code(),
      Self::NotDefined(e) => e.code(),
      Self::Target(e) => e.code(),
    }
  }
}

impl NodeJsErrorCoded for PackageResolveError {
  fn code(&self) -> NodeJsErrorCode {
    match self.as_kind() {
      PackageResolveErrorKind::ClosestPkgJson(e) => e.code(),
      PackageResolveErrorKind::InvalidModuleSpecifier(e) => e.code(),
      PackageResolveErrorKind::PackageFolderResolve(e) => e.code(),
      PackageResolveErrorKind::ExportsResolve(e) => e.code(),
      PackageResolveErrorKind::SubpathResolve(e) => e.code(),
    }
  }
}

#[derive(Debug, Boxed)]
pub struct PackageResolveError(pub Box<PackageResolveErrorKind>);

#[derive(Debug, Error)]
pub enum PackageResolveErrorKind {
  #[error(transparent)]
  ClosestPkgJson(#[from] ClosestPkgJsonError),
  #[error(transparent)]
  InvalidModuleSpecifier(#[from] InvalidModuleSpecifierError),
  #[error(transparent)]
  PackageFolderResolve(#[from] PackageFolderResolveError),
  #[error(transparent)]
  ExportsResolve(#[from] PackageExportsResolveError),
  #[error(transparent)]
  SubpathResolve(#[from] PackageSubpathResolveError),
}

#[derive(Debug, Error)]
#[error("Failed joining '{path}' from '{base}'.")]
pub struct NodeResolveRelativeJoinError {
  pub path: String,
  pub base: Url,
  #[source]
  pub source: url::ParseError,
}

#[derive(Debug, Error)]
#[error("Failed resolving specifier from data url referrer.")]
pub struct DataUrlReferrerError {
  #[source]
  pub source: url::ParseError,
}

#[derive(Debug, Boxed)]
pub struct NodeResolveError(pub Box<NodeResolveErrorKind>);

#[derive(Debug, Error)]
pub enum NodeResolveErrorKind {
  #[error(transparent)]
  RelativeJoin(#[from] NodeResolveRelativeJoinError),
  #[error(transparent)]
  PackageImportsResolve(#[from] PackageImportsResolveError),
  #[error(transparent)]
  UnsupportedEsmUrlScheme(#[from] UnsupportedEsmUrlSchemeError),
  #[error(transparent)]
  DataUrlReferrer(#[from] DataUrlReferrerError),
  #[error(transparent)]
  PackageResolve(#[from] PackageResolveError),
  #[error(transparent)]
  TypesNotFound(#[from] TypesNotFoundError),
  #[error(transparent)]
  FinalizeResolution(#[from] FinalizeResolutionError),
}

#[derive(Debug, Boxed)]
pub struct FinalizeResolutionError(pub Box<FinalizeResolutionErrorKind>);

#[derive(Debug, Error)]
pub enum FinalizeResolutionErrorKind {
  #[error(transparent)]
  InvalidModuleSpecifierError(#[from] InvalidModuleSpecifierError),
  #[error(transparent)]
  ModuleNotFound(#[from] ModuleNotFoundError),
  #[error(transparent)]
  UnsupportedDirImport(#[from] UnsupportedDirImportError),
}

impl NodeJsErrorCoded for FinalizeResolutionError {
  fn code(&self) -> NodeJsErrorCode {
    match self.as_kind() {
      FinalizeResolutionErrorKind::InvalidModuleSpecifierError(e) => e.code(),
      FinalizeResolutionErrorKind::ModuleNotFound(e) => e.code(),
      FinalizeResolutionErrorKind::UnsupportedDirImport(e) => e.code(),
    }
  }
}

#[derive(Debug, Error)]
#[error(
  "[{}] Cannot find {} '{}'{}",
  self.code(),
  typ,
  specifier,
  maybe_referrer.as_ref().map(|referrer| format!(" imported from '{}'", referrer)).unwrap_or_default()
)]
pub struct ModuleNotFoundError {
  pub specifier: Url,
  pub maybe_referrer: Option<Url>,
  pub typ: &'static str,
}

impl NodeJsErrorCoded for ModuleNotFoundError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_MODULE_NOT_FOUND
  }
}

#[derive(Debug, Error)]
#[error(
  "[{}] Directory import '{}' is not supported resolving ES modules{}",
  self.code(),
  dir_url,
  maybe_referrer.as_ref().map(|referrer| format!(" imported from '{}'", referrer)).unwrap_or_default(),
)]
pub struct UnsupportedDirImportError {
  pub dir_url: Url,
  pub maybe_referrer: Option<Url>,
}

impl NodeJsErrorCoded for UnsupportedDirImportError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_UNSUPPORTED_DIR_IMPORT
  }
}

#[derive(Debug)]
pub struct InvalidPackageTargetError {
  pub pkg_json_path: PathBuf,
  pub sub_path: String,
  pub target: String,
  pub is_import: bool,
  pub maybe_referrer: Option<Url>,
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

#[derive(Debug)]
pub struct PackagePathNotExportedError {
  pub pkg_json_path: PathBuf,
  pub subpath: String,
  pub maybe_referrer: Option<Url>,
  pub mode: NodeResolutionMode,
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

    let types_msg = match self.mode {
      NodeResolutionMode::Execution => String::new(),
      NodeResolutionMode::Types => " for types".to_string(),
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

#[derive(Debug, Clone, Error)]
#[error(
  "[{}] Only file and data URLs are supported by the default ESM loader.{} Received protocol '{}'",
  self.code(),
  if cfg!(windows) && url_scheme.len() == 2 { " On Windows, absolute path must be valid file:// URLS."} else { "" },
  url_scheme
)]
pub struct UnsupportedEsmUrlSchemeError {
  pub url_scheme: String,
}

impl NodeJsErrorCoded for UnsupportedEsmUrlSchemeError {
  fn code(&self) -> NodeJsErrorCode {
    NodeJsErrorCode::ERR_UNSUPPORTED_ESM_URL_SCHEME
  }
}

#[derive(Debug, Error)]
pub enum ResolvePkgJsonBinExportError {
  #[error(transparent)]
  PkgJsonLoad(#[from] PackageJsonLoadError),
  #[error("Failed resolving binary export. '{}' did not exist", pkg_json_path.display())]
  MissingPkgJson { pkg_json_path: PathBuf },
  #[error("Failed resolving binary export. {message}")]
  InvalidBinProperty { message: String },
}

#[derive(Debug, Error)]
pub enum ResolveBinaryCommandsError {
  #[error(transparent)]
  PkgJsonLoad(#[from] PackageJsonLoadError),
  #[error("'{}' did not have a name", pkg_json_path.display())]
  MissingPkgJsonName { pkg_json_path: PathBuf },
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
        mode: NodeResolutionMode::Types
      }.to_string(),
      format!("[ERR_PACKAGE_PATH_NOT_EXPORTED] Package subpath './jsx-runtime' is not defined for types by \"exports\" in 'test_path{separator_char}package.json'")
    );
    assert_eq!(
      PackagePathNotExportedError {
        pkg_json_path: PathBuf::from("test_path").join("package.json"),
        subpath: ".".to_string(), 
        maybe_referrer: None,
        mode: NodeResolutionMode::Types
      }.to_string(),
      format!("[ERR_PACKAGE_PATH_NOT_EXPORTED] No \"exports\" main defined for types in 'test_path{separator_char}package.json'")
    );
  }
}
