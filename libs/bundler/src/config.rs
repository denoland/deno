// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;

use rustc_hash::FxHashMap;
use serde::Deserialize;
use serde::Serialize;

/// Whether an environment targets the browser or a server runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum EnvironmentTarget {
  /// Browser environment (default). Uses WebSocket for HMR.
  #[default]
  Browser,
  /// Server environment. Uses a module runner child process with IPC for HMR.
  Server,
}

/// The runtime that executes bundled code for an environment.
///
/// `Browser` implies browser target. `Node` and `Deno` imply server target.
#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize,
)]
pub enum EnvironmentRuntime {
  /// Browser environment (default). Code runs in a web browser.
  #[default]
  Browser,
  /// Node.js server runtime.
  Node,
  /// Deno server runtime.
  Deno,
}

impl EnvironmentRuntime {
  /// Derive the environment target (browser vs server) from the runtime.
  pub fn target(&self) -> EnvironmentTarget {
    match self {
      EnvironmentRuntime::Browser => EnvironmentTarget::Browser,
      EnvironmentRuntime::Node | EnvironmentRuntime::Deno => {
        EnvironmentTarget::Server
      }
    }
  }
}

/// Per-environment transform configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransformConfig {
  /// JSX runtime mode. If None, JSX files pass through unchanged.
  pub jsx: Option<JsxRuntime>,
  /// Global expression replacements applied at build time.
  ///
  /// Keys are dotted expressions (e.g. `"process.env.NODE_ENV"`) or
  /// typeof expressions (e.g. `"typeof window"`).
  /// Values are replacement code strings (e.g. `"\"production\""`, `"true"`).
  pub define: FxHashMap<String, String>,
}

/// Which JSX runtime to compile JSX syntax into.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JsxRuntime {
  /// Classic: `React.createElement("div", props, ...children)`
  Classic { factory: String, fragment: String },
  /// Automatic: `_jsx("div", {children: ...})` with auto-import from
  /// jsx-runtime
  Automatic { import_source: String },
  /// Precompile: static HTML template arrays + dynamic placeholders for SSR
  Precompile {
    import_source: String,
    skip_elements: Vec<String>,
    skip_props: Vec<String>,
  },
}

/// Output module format for emitted chunks.
#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize,
)]
pub enum OutputFormat {
  /// ES modules (import/export). Default.
  #[default]
  Esm,
  /// CommonJS (require/module.exports).
  Cjs,
}

impl OutputFormat {
  /// File extension for this format.
  pub fn extension(&self) -> &'static str {
    match self {
      OutputFormat::Esm => "js",
      OutputFormat::Cjs => "cjs",
    }
  }
}

impl std::fmt::Display for OutputFormat {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      OutputFormat::Esm => write!(f, "esm"),
      OutputFormat::Cjs => write!(f, "cjs"),
    }
  }
}

/// Source map output mode.
#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum SourceMapMode {
  /// No source maps generated.
  #[default]
  None,
  /// Separate `.map` file with `//# sourceMappingURL=<file>.map` comment.
  External,
  /// Base64 data URL inlined in the output via `//# sourceMappingURL=data:...`.
  Inline,
  /// Separate `.map` file generated but no `sourceMappingURL` comment added.
  Hidden,
}

/// Unique identifier for a build environment.
#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub struct EnvironmentId(u32);

impl EnvironmentId {
  pub fn new(id: u32) -> Self {
    Self(id)
  }

  pub fn value(&self) -> u32 {
    self.0
  }
}

/// Configuration for a single build environment.
///
/// An environment represents a distinct execution target (browser, server,
/// worker, etc.) with its own entries, conditions, output directory, and
/// external patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
  /// Unique identifier for this environment.
  pub id: EnvironmentId,
  /// Human-readable name (e.g., "browser", "server").
  pub name: String,
  /// The runtime for this environment (Browser, Node, Deno).
  pub runtime: EnvironmentRuntime,
  /// Whether this targets browser or server. Derived from `runtime`.
  pub target: EnvironmentTarget,
  /// Entry point specifiers.
  pub entries: Vec<String>,
  /// Package.json export conditions (e.g., \["browser", "import",
  /// "default"\]).
  pub conditions: Vec<String>,
  /// Output directory for this environment's chunks.
  pub output_dir: PathBuf,
  /// Glob patterns for modules to exclude from bundling (e.g., \["node:*"\]).
  pub external: Vec<String>,
  /// Per-environment transform settings (JSX, defines, etc.).
  pub transform: TransformConfig,
  /// Source map output mode.
  pub source_map: SourceMapMode,
  /// Names of other environments this one depends on.
  pub depends_on: Vec<String>,
}

/// Configuration for the bundler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundlerConfig {
  /// Project root directory.
  pub root: PathBuf,
  /// Build environments.
  pub environments: Vec<EnvironmentConfig>,
  /// Enable verbose debug logging.
  #[serde(skip, default)]
  pub debug: bool,
}
