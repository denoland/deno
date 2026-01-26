// Copyright 2018-2026 the Deno authors. MIT license.

//! Multi-environment support for the bundler.
//!
//! This module defines the different target environments (Server/Deno, Browser)
//! that the bundler can generate code for. It enables patterns like SSR + hydration
//! where server code references browser entrypoints.

use std::collections::HashMap;
use std::fmt;

use deno_ast::ModuleSpecifier;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;

/// The target environment for bundled code.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(crate = "deno_core::serde", rename_all = "lowercase")]
pub enum BundleEnvironment {
  /// Server-side / Deno runtime (default).
  Server,
  /// Browser runtime.
  Browser,
  /// Custom named environment.
  Custom(String),
}

impl Default for BundleEnvironment {
  fn default() -> Self {
    BundleEnvironment::Server
  }
}

impl fmt::Display for BundleEnvironment {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      BundleEnvironment::Server => write!(f, "server"),
      BundleEnvironment::Browser => write!(f, "browser"),
      BundleEnvironment::Custom(name) => write!(f, "{}", name),
    }
  }
}

impl BundleEnvironment {
  /// Parse an environment from a string.
  pub fn from_str(s: &str) -> Self {
    match s.to_lowercase().as_str() {
      "server" | "deno" | "node" => BundleEnvironment::Server,
      "browser" | "client" => BundleEnvironment::Browser,
      other => BundleEnvironment::Custom(other.to_string()),
    }
  }

  /// Check if this is a server-side environment.
  pub fn is_server(&self) -> bool {
    matches!(self, BundleEnvironment::Server)
  }

  /// Check if this is a browser environment.
  pub fn is_browser(&self) -> bool {
    matches!(self, BundleEnvironment::Browser)
  }
}

/// Configuration for a specific environment.
#[derive(Debug, Clone)]
pub struct EnvironmentConfig {
  /// The environment type.
  pub environment: BundleEnvironment,
  /// Entry points specific to this environment.
  pub entry_points: Vec<ModuleSpecifier>,
  /// Whether this environment should be bundled (vs. left as separate modules).
  pub bundle: bool,
  /// External modules that should not be bundled.
  pub external: Vec<String>,
  /// Conditions for package.json exports field resolution.
  pub conditions: Vec<String>,
  /// Module format for output.
  pub format: OutputFormat,
  /// Target platform features.
  pub target: EnvironmentTarget,
}

impl Default for EnvironmentConfig {
  fn default() -> Self {
    Self {
      environment: BundleEnvironment::Server,
      entry_points: Vec::new(),
      bundle: true,
      external: Vec::new(),
      conditions: vec!["import".to_string()],
      format: OutputFormat::Esm,
      target: EnvironmentTarget::default(),
    }
  }
}

impl EnvironmentConfig {
  /// Create a server environment config.
  pub fn server() -> Self {
    Self {
      environment: BundleEnvironment::Server,
      conditions: vec!["deno".to_string(), "import".to_string()],
      ..Default::default()
    }
  }

  /// Create a browser environment config.
  pub fn browser() -> Self {
    Self {
      environment: BundleEnvironment::Browser,
      conditions: vec!["browser".to_string(), "import".to_string()],
      target: EnvironmentTarget::browser(),
      ..Default::default()
    }
  }

  /// Add an entry point to this environment.
  pub fn with_entry(mut self, entry: ModuleSpecifier) -> Self {
    self.entry_points.push(entry);
    self
  }

  /// Add external modules.
  pub fn with_external(mut self, external: Vec<String>) -> Self {
    self.external = external;
    self
  }
}

/// Output module format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(crate = "deno_core::serde", rename_all = "lowercase")]
pub enum OutputFormat {
  /// ES modules (default).
  #[default]
  Esm,
  /// CommonJS modules.
  Cjs,
  /// IIFE (immediately invoked function expression).
  Iife,
}

impl fmt::Display for OutputFormat {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      OutputFormat::Esm => write!(f, "esm"),
      OutputFormat::Cjs => write!(f, "cjs"),
      OutputFormat::Iife => write!(f, "iife"),
    }
  }
}

/// Target platform features.
#[derive(Debug, Clone)]
pub struct EnvironmentTarget {
  /// ES version target (e.g., "es2022", "esnext").
  pub es_version: String,
  /// Supported features.
  pub features: TargetFeatures,
}

impl Default for EnvironmentTarget {
  fn default() -> Self {
    Self {
      es_version: "esnext".to_string(),
      features: TargetFeatures::deno(),
    }
  }
}

impl EnvironmentTarget {
  /// Create a browser target.
  pub fn browser() -> Self {
    Self {
      es_version: "es2022".to_string(),
      features: TargetFeatures::browser(),
    }
  }
}

/// Feature support flags for code generation.
#[derive(Debug, Clone, Default)]
pub struct TargetFeatures {
  /// Support for top-level await.
  pub top_level_await: bool,
  /// Support for import.meta.
  pub import_meta: bool,
  /// Support for dynamic import().
  pub dynamic_import: bool,
  /// Support for Deno namespace.
  pub deno_namespace: bool,
  /// Support for Web APIs (fetch, etc.).
  pub web_apis: bool,
}

impl TargetFeatures {
  /// Features available in Deno runtime.
  pub fn deno() -> Self {
    Self {
      top_level_await: true,
      import_meta: true,
      dynamic_import: true,
      deno_namespace: true,
      web_apis: true,
    }
  }

  /// Features available in modern browsers.
  pub fn browser() -> Self {
    Self {
      top_level_await: true,
      import_meta: true,
      dynamic_import: true,
      deno_namespace: false,
      web_apis: true,
    }
  }
}

/// Cross-environment reference tracking.
///
/// This allows server code to reference browser entrypoints, enabling
/// SSR + hydration patterns where the server knows about client bundles.
#[derive(Debug, Clone)]
pub struct CrossEnvRef {
  /// The source environment making the reference.
  pub from_env: BundleEnvironment,
  /// The target environment being referenced.
  pub to_env: BundleEnvironment,
  /// The specifier in the source environment.
  pub source_specifier: ModuleSpecifier,
  /// The specifier in the target environment.
  pub target_specifier: ModuleSpecifier,
  /// How this reference should be resolved at runtime.
  pub resolution: CrossEnvResolution,
}

/// How a cross-environment reference should be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossEnvResolution {
  /// Replace with a URL to the bundled asset.
  AssetUrl,
  /// Replace with an import of a manifest.
  Manifest,
  /// Keep as-is (for development).
  Passthrough,
}

/// Multi-environment build configuration.
#[derive(Debug, Clone, Default)]
pub struct MultiEnvConfig {
  /// Environment configurations by name.
  pub environments: HashMap<BundleEnvironment, EnvironmentConfig>,
  /// Cross-environment references.
  pub cross_refs: Vec<CrossEnvRef>,
}

impl MultiEnvConfig {
  /// Create a new multi-environment config.
  pub fn new() -> Self {
    Self::default()
  }

  /// Add an environment configuration.
  pub fn add_environment(&mut self, config: EnvironmentConfig) {
    self
      .environments
      .insert(config.environment.clone(), config);
  }

  /// Create a simple server-only config.
  pub fn server_only() -> Self {
    let mut config = Self::new();
    config.add_environment(EnvironmentConfig::server());
    config
  }

  /// Create a server + browser config for SSR apps.
  pub fn ssr() -> Self {
    let mut config = Self::new();
    config.add_environment(EnvironmentConfig::server());
    config.add_environment(EnvironmentConfig::browser());
    config
  }

  /// Get the environment config for a given environment.
  pub fn get(&self, env: &BundleEnvironment) -> Option<&EnvironmentConfig> {
    self.environments.get(env)
  }

  /// Check if an environment is configured.
  pub fn has_environment(&self, env: &BundleEnvironment) -> bool {
    self.environments.contains_key(env)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_environment_from_str() {
    assert_eq!(BundleEnvironment::from_str("server"), BundleEnvironment::Server);
    assert_eq!(BundleEnvironment::from_str("deno"), BundleEnvironment::Server);
    assert_eq!(BundleEnvironment::from_str("browser"), BundleEnvironment::Browser);
    assert_eq!(BundleEnvironment::from_str("client"), BundleEnvironment::Browser);
    assert_eq!(
      BundleEnvironment::from_str("edge"),
      BundleEnvironment::Custom("edge".to_string())
    );
  }

  #[test]
  fn test_multi_env_config() {
    let config = MultiEnvConfig::ssr();
    assert!(config.has_environment(&BundleEnvironment::Server));
    assert!(config.has_environment(&BundleEnvironment::Browser));
    assert!(!config.has_environment(&BundleEnvironment::Custom("edge".to_string())));
  }
}
