// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::path::PathBuf;

use deno_config::deno_json::ConfigFile;
use deno_config::workspace::Workspace;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::MutexGuard;
use deno_core::serde_json;
use deno_error::JsErrorBox;
use deno_lockfile::Lockfile;
use deno_lockfile::NpmPackageInfoProvider;
use deno_lockfile::WorkspaceMemberConfig;
use deno_package_json::PackageJsonDepValue;
use deno_path_util::fs::atomic_write_file_with_retries;
use deno_runtime::deno_node::PackageJson;
use deno_semver::jsr::JsrDepPackageReq;
use indexmap::IndexMap;

use crate::args::deno_json::import_map_deps;
use crate::args::DenoSubcommand;
use crate::args::InstallFlags;
use crate::cache;
use crate::sys::CliSys;
use crate::Flags;
