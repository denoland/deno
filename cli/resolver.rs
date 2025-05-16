// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use async_trait::async_trait;
use deno_error::JsErrorBox;
use deno_graph::NpmLoadError;
use deno_graph::NpmResolvePkgReqsResult;
use deno_npm::resolution::NpmResolutionError;
use deno_npm_installer::PackageCaching;
use deno_resolver::graph::FoundPackageJsonDepFlag;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_semver::package::PackageReq;
use node_resolver::DenoIsBuiltInNodeModuleChecker;

use crate::npm::CliNpmInstaller;
use crate::npm::CliNpmResolver;
use crate::sys::CliSys;

pub type CliCjsTracker =
  deno_resolver::cjs::CjsTracker<DenoInNpmPackageChecker, CliSys>;
pub type CliIsCjsResolver =
  deno_resolver::cjs::IsCjsResolver<DenoInNpmPackageChecker, CliSys>;
pub type CliNpmReqResolver = deno_resolver::npm::NpmReqResolver<
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  CliNpmResolver,
  CliSys,
>;
pub type CliResolver = deno_resolver::graph::DenoResolver<
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  CliNpmResolver,
  CliSys,
>;

pub fn on_resolve_diagnostic(
  diagnostic: deno_resolver::graph::MappedResolutionDiagnosticWithPosition,
) {
  log::warn!(
    "{} {}\n    at {}:{}",
    deno_runtime::colors::yellow("Warning"),
    diagnostic.diagnostic,
    diagnostic.referrer,
    diagnostic.start
  );
}
