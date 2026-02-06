// Copyright 2018-2026 the Deno authors. MIT license.

use deno_semver::StackString;
use deno_semver::jsr::JsrDepPackageReqParseError;
use deno_semver::package::PackageNv;
use thiserror::Error;

use crate::transforms::TransformError;

#[derive(Debug, Error)]
#[error("Failed reading lockfile at '{file_path}'")]
pub struct LockfileError {
  pub file_path: String,
  #[source]
  pub source: LockfileErrorReason,
}

#[derive(Debug, Error)]
pub enum LockfileErrorReason {
  #[error("Lockfile was empty")]
  Empty,
  #[error("Failed parsing. Lockfile may be corrupt")]
  ParseError(serde_json::Error),
  #[error("Failed deserializing. Lockfile may be corrupt")]
  DeserializationError(#[source] DeserializationError),
  #[error(
    "Unsupported lockfile version '{version}'. Try upgrading Deno or recreating the lockfile"
  )]
  UnsupportedVersion { version: String },
  #[error(
    "Failed upgrading lockfile to latest version. Lockfile may be corrupt"
  )]
  TransformError(#[source] TransformError),
}

impl From<TransformError> for LockfileErrorReason {
  fn from(e: TransformError) -> Self {
    LockfileErrorReason::TransformError(e)
  }
}

#[derive(Debug, Error)]
pub enum DeserializationError {
  #[error("Invalid {0} section: {1:#}")]
  FailedDeserializing(&'static str, serde_json::Error),
  #[error("Invalid npm package '{0}'")]
  InvalidNpmPackageId(StackString),
  #[error("Invalid npm package dependency '{0}'")]
  InvalidNpmPackageDependency(StackString),
  #[error(transparent)]
  InvalidPackageSpecifier(#[from] JsrDepPackageReqParseError),
  #[error("Invalid package specifier version '{version}' for '{specifier}'")]
  InvalidPackageSpecifierVersion {
    specifier: String,
    version: StackString,
  },
  #[error("Invalid jsr dependency '{dependency}' for '{package}'")]
  InvalidJsrDependency {
    package: PackageNv,
    dependency: StackString,
  },
  #[error(
    "npm package '{0}' was not found and could not have its version resolved"
  )]
  MissingPackage(StackString),
}
