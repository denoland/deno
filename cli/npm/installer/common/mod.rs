// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::parking_lot::RwLock;
use deno_error::JsErrorBox;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::NpmPackageExtraInfo;
use deno_npm::NpmResolutionPackage;
use deno_semver::package::PackageNv;
pub use deno_task_executor::DenoTaskLifeCycleScriptsExecutor;

use super::PackageCaching;
use crate::npm::CliNpmCache;
use crate::npm::WorkspaceNpmPatchPackages;

mod deno_task_executor;
