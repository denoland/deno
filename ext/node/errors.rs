// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::PathBuf;

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use thiserror::Error;

use crate::NodeModuleKind;
use crate::NodeResolutionMode;

macro_rules! kinded_err {
  ($name:ident, $kind_name:ident) => {
    #[derive(Error, Debug)]
    #[error(transparent)]
    pub struct $name(pub Box<$kind_name>);

    impl $name {
      pub fn as_kind(&self) -> &$kind_name {
        &self.0
      }

      pub fn into_kind(self) -> $kind_name {
        *self.0
      }
    }

    impl<E> From<E> for $name
    where
      $kind_name: From<E>,
    {
      fn from(err: E) -> Self {
        $name(Box::new($kind_name::from(err)))
      }
    }
  };
}

kinded_err!(
  ResolvePkgSubpathFromDenoModuleError,
  ResolvePkgSubpathFromDenoModuleErrorKind
);

#[derive(Debug, Error)]
pub enum ResolvePkgSubpathFromDenoModuleErrorKind {
  #[error(transparent)]
  PackageSubpathResolve(#[from] PackageSubpathResolveError),
  #[error(transparent)]
  UrlToNodeResolution(#[from] UrlToNodeResolutionError),
}

// todo(THIS PR): how to make this a TypeError. Does it matter?
#[derive(Debug, Clone, Error)]
#[error(
  "[ERR_INVALID_MODULE_SPECIFIER] Invalid module '{}' {}{}",
  request,
  reason,
  maybe_referrer.as_ref().map(|referrer| format!(" imported from '{}'", referrer)).unwrap_or_default()
)]
pub struct InvalidModuleSpecifierError {
  pub request: String,
  pub reason: Cow<'static, str>,
  pub maybe_referrer: Option<String>,
}

#[derive(Debug, Error)]
pub enum LegacyMainResolveError {
  #[error(transparent)]
  PathToDeclarationUrl(PathToDeclarationUrlError),
}

kinded_err!(PackageFolderResolveError, PackageFolderResolveErrorKind);

#[derive(Debug, Error)]
pub enum PackageFolderResolveErrorKind {
  #[error(
    "Could not find package '{}' from referrer '{}'{}.",
    package_name,
    referrer,
    referrer_extra.as_ref().map(|r| format!(" ({})", r)).unwrap_or_default()
  )]
  NotFoundPackage {
    package_name: String,
    referrer: ModuleSpecifier,
    /// Extra information about the referrer.
    referrer_extra: Option<String>,
  },
  #[error(
    "Could not find referrer npm package '{}'{}.",
    referrer,
    referrer_extra.as_ref().map(|r| format!(" ({})", r)).unwrap_or_default()
  )]
  NotFoundReferrer {
    referrer: ModuleSpecifier,
    /// Extra information about the referrer.
    referrer_extra: Option<String>,
  },
  #[error("Failed resolving '{package_name}' from referrer '{referrer}'.")]
  Io {
    package_name: String,
    referrer: ModuleSpecifier,
    #[source]
    source: std::io::Error,
  },
}

kinded_err!(PackageSubpathResolveError, PackageSubpathResolveErrorKind);

#[derive(Debug, Error)]
pub enum PackageSubpathResolveErrorKind {
  #[error(transparent)]
  PkgJsonLoad(#[from] deno_config::package_json::PackageJsonLoadError),
  #[error(transparent)]
  PackageFolderResolve(#[from] PackageFolderResolveError),
  #[error(transparent)]
  DirNotFound(AnyError),
  #[error(transparent)]
  Exports(PackageExportsResolveError),
  #[error(transparent)]
  LegacyMain(LegacyMainResolveError),
  #[error(transparent)]
  LegacyExact(PathToDeclarationUrlError),
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
  pub maybe_referrer: Option<ModuleSpecifier>,
  pub referrer_kind: NodeModuleKind,
  pub mode: NodeResolutionMode,
}

kinded_err!(PackageTargetResolveError, PackageTargetResolveErrorKind);

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
  PathToDeclarationUrl(#[from] PathToDeclarationUrlError),
}

kinded_err!(PackageExportsResolveError, PackageExportsResolveErrorKind);

#[derive(Debug, Error)]
pub enum PackageExportsResolveErrorKind {
  #[error(transparent)]
  PackagePathNotExported(#[from] PackagePathNotExportedError),
  #[error(transparent)]
  PackageTargetResolve(#[from] PackageTargetResolveError),
}

#[derive(Debug, Error)]
pub enum PathToDeclarationUrlError {
  #[error(transparent)]
  SubPath(#[from] PackageSubpathResolveError),
}

kinded_err!(ClosestPkgJsonError, ClosestPkgJsonErrorKind);

#[derive(Debug, Error)]
pub enum ClosestPkgJsonErrorKind {
  #[error("Failed canonicalizing package.json directory '{dir_path}'.")]
  CanonicalizingDir {
    dir_path: PathBuf,
    #[source]
    source: std::io::Error,
  },
  #[error(transparent)]
  Load(#[from] deno_config::package_json::PackageJsonLoadError),
}

#[derive(Debug, Error)]
#[error("TypeScript files are not supported in npm packages: {specifier}")]
pub struct TypeScriptNotSupportedInNpmError {
  pub specifier: ModuleSpecifier,
}

kinded_err!(UrlToNodeResolutionError, UrlToNodeResolutionErrorKind);

#[derive(Debug, Error)]
pub enum UrlToNodeResolutionErrorKind {
  #[error(transparent)]
  TypeScriptNotSupported(#[from] TypeScriptNotSupportedInNpmError),
  #[error(transparent)]
  ClosestPkgJson(#[from] ClosestPkgJsonError),
}

// todo(THIS PR): this should be a TypeError
#[derive(Debug, Error)]
#[error(
  "[ERR_PACKAGE_IMPORT_NOT_DEFINED] Package import specifier \"{}\" is not defined{}{}",
  name,
  package_json_path.as_ref().map(|p| format!(" in package {}", p.display())).unwrap_or_default(),
  maybe_referrer.as_ref().map(|r| format!(" imported from '{}'", r)).unwrap_or_default(),
)]
pub struct PackageImportNotDefinedError {
  pub name: String,
  pub package_json_path: Option<PathBuf>,
  pub maybe_referrer: Option<ModuleSpecifier>,
}

kinded_err!(PackageImportsResolveError, PackageImportsResolveErrorKind);

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

kinded_err!(PackageResolveError, PackageResolveErrorKind);

#[derive(Debug, Error)]
pub enum PackageResolveErrorKind {
  #[error(transparent)]
  ClosestPkgJson(#[from] ClosestPkgJsonError),
  #[error(transparent)]
  InvalidModuleSpecifier(#[from] InvalidModuleSpecifierError),
  #[error(transparent)]
  ExportsResolve(#[from] PackageExportsResolveError),
  #[error(transparent)]
  SubpathResolve(#[from] PackageSubpathResolveError),
}

#[derive(Debug, Error)]
pub enum NodeResolveError {
  #[error("Failed joining '{path}' from '{base}'.")]
  RelativeJoinError {
    path: String,
    base: ModuleSpecifier,
    #[source]
    source: url::ParseError,
  },
  #[error(transparent)]
  PackageImportsResolve(#[from] PackageImportsResolveError),
  #[error(transparent)]
  UnsupportedEsmUrlScheme(#[from] UnsupportedEsmUrlSchemeError),
  #[error("Failed resolving specifier from data url referrer.")]
  DataUrlReferrerFailed {
    #[source]
    source: url::ParseError,
  },
  #[error(transparent)]
  PackageResolve(#[from] PackageResolveError),
  #[error(transparent)]
  PathToDeclarationUrl(#[from] PathToDeclarationUrlError),
  #[error(transparent)]
  UrlToNodeResolution(#[from] UrlToNodeResolutionError),
  #[error(transparent)]
  FinalizeResolution(#[from] FinalizeResolutionError),
}

kinded_err!(FinalizeResolutionError, FinalizeResolutionErrorKind);

#[derive(Debug, Error)]
pub enum FinalizeResolutionErrorKind {
  #[error(transparent)]
  InvalidModuleSpecifierError(#[from] InvalidModuleSpecifierError),
  #[error(transparent)]
  ModuleNotFound(#[from] ModuleNotFoundError),
  #[error(transparent)]
  UnsupportedDirImport(#[from] UnsupportedDirImportError),
}

#[derive(Debug, Error)]
#[error(
  "[ERR_MODULE_NOT_FOUND] Cannot find {} '{}'{}",
  typ,
  specifier,
  maybe_referrer.as_ref().map(|referrer| format!(" imported from '{}'", referrer)).unwrap_or_default()
)]
pub struct ModuleNotFoundError {
  pub specifier: ModuleSpecifier,
  pub maybe_referrer: Option<ModuleSpecifier>,
  pub typ: &'static str,
}

#[derive(Debug, Error)]
#[error(
  "[ERR_UNSUPPORTED_DIR_IMPORT] Directory import '{}' is not supported resolving ES modules{}",
  dir_url,
  maybe_referrer.as_ref().map(|referrer| format!(" imported from '{}'", referrer)).unwrap_or_default(),
)]
pub struct UnsupportedDirImportError {
  pub dir_url: ModuleSpecifier,
  pub maybe_referrer: Option<ModuleSpecifier>,
}

#[derive(Debug)]
pub struct InvalidPackageTargetError {
  pub pkg_json_path: PathBuf,
  pub sub_path: String,
  pub target: String,
  pub is_import: bool,
  pub maybe_referrer: Option<ModuleSpecifier>,
}

impl std::error::Error for InvalidPackageTargetError {}

impl std::fmt::Display for InvalidPackageTargetError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let rel_error = !self.is_import
      && !self.target.is_empty()
      && !self.target.starts_with("./");
    f.write_str("[ERR_INVALID_PACKAGE_TARGET]")?;

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

#[derive(Debug)]
pub struct PackagePathNotExportedError {
  pub pkg_json_path: PathBuf,
  pub subpath: String,
  pub maybe_referrer: Option<ModuleSpecifier>,
  pub mode: NodeResolutionMode,
}

impl std::error::Error for PackagePathNotExportedError {}

impl std::fmt::Display for PackagePathNotExportedError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("[ERR_PACKAGE_PATH_NOT_EXPORTED]")?;

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
  "[ERR_UNSUPPORTED_ESM_URL_SCHEME] Only file and data URLS are supported by the default ESM loader.{} Received protocol '{}'",
  if cfg!(windows) && url_scheme.len() == 2 { " On Windows, absolute path must be valid file:// URLS."} else { "" },
  url_scheme
)]
pub struct UnsupportedEsmUrlSchemeError {
  pub url_scheme: String,
}

#[derive(Debug, Error)]
pub enum ResolvePkgJsonBinExportError {
  #[error(transparent)]
  PkgJsonLoad(#[from] deno_config::package_json::PackageJsonLoadError),
  #[error("Failed resolving binary export. '{}' did not exist", pkg_json_path.display())]
  MissingPkgJson { pkg_json_path: PathBuf },
  #[error("Failed resolving binary export. {message}")]
  InvalidBinProperty { message: String },
  #[error(transparent)]
  UrlToNodeResolution(#[from] UrlToNodeResolutionError),
}

#[derive(Debug, Error)]
pub enum ResolveBinaryCommandsError {
  #[error(transparent)]
  PkgJsonLoad(#[from] deno_config::package_json::PackageJsonLoadError),
  #[error("'{}' did not have a name", pkg_json_path.display())]
  MissingPkgJsonName { pkg_json_path: PathBuf },
}

#[allow(unused)]
pub fn err_invalid_package_config(
  path: &str,
  maybe_base: Option<String>,
  maybe_message: Option<String>,
) -> AnyError {
  let mut msg =
    format!("[ERR_INVALID_PACKAGE_CONFIG] Invalid package config {path}");

  if let Some(base) = maybe_base {
    msg = format!("{msg} while importing {base}");
  }

  if let Some(message) = maybe_message {
    msg = format!("{msg}. {message}");
  }

  generic_error(msg)
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
