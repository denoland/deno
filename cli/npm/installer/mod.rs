// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

pub use common::DenoTaskLifeCycleScriptsExecutor;
use deno_core::unsync::sync::AtomicFlag;
use deno_error::JsErrorBox;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;
use deno_npm::NpmSystemInfo;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_runtime::colors;
use deno_semver::package::PackageReq;
pub use local::SetupCache;
use rustc_hash::FxHashSet;

pub use self::common::lifecycle_scripts::LifecycleScriptsExecutor;
pub use self::common::lifecycle_scripts::NullLifecycleScriptsExecutor;
use self::common::NpmPackageExtraInfoProvider;
use self::common::NpmPackageFsInstaller;
use self::global::GlobalNpmPackageInstaller;
use self::local::LocalNpmPackageInstaller;
pub use self::resolution::AddPkgReqsResult;
pub use self::resolution::NpmResolutionInstaller;
use super::CliNpmCache;
use super::CliNpmTarballCache;
use super::NpmResolutionInitializer;
use super::WorkspaceNpmPatchPackages;
use crate::args::CliLockfile;
use crate::args::LifecycleScriptsConfig;
use crate::args::NpmInstallDepsProvider;
use crate::args::PackageJsonDepValueParseWithLocationError;
use crate::sys::CliSys;
use crate::util::progress_bar::ProgressBar;

mod common;
mod global;
mod local;
mod resolution;
