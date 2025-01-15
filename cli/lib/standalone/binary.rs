// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;

use deno_config::workspace::PackageJsonDepResolution;
use deno_runtime::deno_permissions::PermissionsOptions;
use deno_runtime::deno_telemetry::OtelConfig;
use deno_semver::Version;
use indexmap::IndexMap;
use serde::Deserialize;
use serde::Serialize;
use url::Url;

use super::virtual_fs::FileSystemCaseSensitivity;

pub const MAGIC_BYTES: &[u8; 8] = b"d3n0l4nd";

#[derive(Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnstableConfig {
  // TODO(bartlomieju): remove in Deno 2.5
  pub legacy_flag_enabled: bool, // --unstable
  pub bare_node_builtins: bool,
  pub detect_cjs: bool,
  pub sloppy_imports: bool,
  pub npm_lazy_caching: bool,
  pub features: Vec<String>, // --unstabe-kv --unstable-cron
}

#[derive(Deserialize, Serialize)]
pub enum NodeModules {
  Managed {
    /// Relative path for the node_modules directory in the vfs.
    node_modules_dir: Option<String>,
  },
  Byonm {
    root_node_modules_dir: Option<String>,
  },
}

#[derive(Deserialize, Serialize)]
pub struct SerializedWorkspaceResolverImportMap {
  pub specifier: String,
  pub json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerializedResolverWorkspaceJsrPackage {
  pub relative_base: String,
  pub name: String,
  pub version: Option<Version>,
  pub exports: IndexMap<String, String>,
}

#[derive(Deserialize, Serialize)]
pub struct SerializedWorkspaceResolver {
  pub import_map: Option<SerializedWorkspaceResolverImportMap>,
  pub jsr_pkgs: Vec<SerializedResolverWorkspaceJsrPackage>,
  pub package_jsons: BTreeMap<String, serde_json::Value>,
  pub pkg_json_resolution: PackageJsonDepResolution,
}

// Note: Don't use hashmaps/hashsets. Ensure the serialization
// is deterministic.
#[derive(Deserialize, Serialize)]
pub struct Metadata {
  pub argv: Vec<String>,
  pub seed: Option<u64>,
  pub code_cache_key: Option<u64>,
  pub permissions: PermissionsOptions,
  pub location: Option<Url>,
  pub v8_flags: Vec<String>,
  pub log_level: Option<log::Level>,
  pub ca_stores: Option<Vec<String>>,
  pub ca_data: Option<Vec<u8>>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub env_vars_from_env_file: IndexMap<String, String>,
  pub workspace_resolver: SerializedWorkspaceResolver,
  pub entrypoint_key: String,
  pub node_modules: Option<NodeModules>,
  pub unstable_config: UnstableConfig,
  pub otel_config: OtelConfig,
  pub vfs_case_sensitivity: FileSystemCaseSensitivity,
}

pub struct SourceMapStore {
  data: IndexMap<Cow<'static, str>, Cow<'static, [u8]>>,
}

impl SourceMapStore {
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      data: IndexMap::with_capacity(capacity),
    }
  }

  pub fn add(
    &mut self,
    specifier: Cow<'static, str>,
    source_map: Cow<'static, [u8]>,
  ) {
    self.data.insert(specifier, source_map);
  }

  pub fn get(&self, specifier: &str) -> Option<&[u8]> {
    self.data.get(specifier).map(|v| v.as_ref())
  }
}
