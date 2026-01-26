// Copyright 2018-2026 the Deno authors. MIT license.

//! HMR (Hot Module Replacement) type definitions.
//!
//! This module contains types for Vite-compatible HMR support in vbundler.

use std::collections::HashMap;
use std::collections::HashSet;

use deno_ast::ModuleSpecifier;
use deno_core::serde_json;
use serde::Deserialize;
use serde::Serialize;

/// Configuration for HMR.
#[derive(Debug, Clone)]
pub struct HmrConfig {
  /// The WebSocket port for HMR connections.
  pub port: u16,
  /// The host for WebSocket connections.
  pub host: String,
  /// Whether to show an error overlay in the browser.
  pub overlay: bool,
  /// Timeout for HMR updates (in milliseconds).
  pub timeout: u32,
}

impl Default for HmrConfig {
  fn default() -> Self {
    Self {
      port: 24678,
      host: "localhost".to_string(),
      overlay: true,
      timeout: 30000,
    }
  }
}

impl HmrConfig {
  /// Create a new HMR config with a custom port.
  pub fn with_port(mut self, port: u16) -> Self {
    self.port = port;
    self
  }

  /// Create a new HMR config with a custom host.
  pub fn with_host(mut self, host: impl Into<String>) -> Self {
    self.host = host.into();
    self
  }

  /// Get the WebSocket URL for HMR connections.
  pub fn websocket_url(&self) -> String {
    format!("ws://{}:{}", self.host, self.port)
  }
}

/// HMR event types sent over WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum HmrEvent {
  /// Connection established.
  Connected,
  /// Module update available.
  Update(HmrUpdatePayload),
  /// Full page reload required.
  FullReload {
    /// Optional path that triggered the reload.
    path: Option<String>,
  },
  /// Module pruned (removed).
  Prune {
    /// Paths of modules being pruned.
    paths: Vec<String>,
  },
  /// Error occurred.
  Error(HmrErrorPayload),
  /// Custom event from server to client.
  Custom {
    /// Event name.
    event: String,
    /// Event data.
    data: Option<serde_json::Value>,
  },
}

/// Payload for HMR update events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HmrUpdatePayload {
  /// List of module updates.
  pub updates: Vec<HmrModuleUpdate>,
}

/// Information about a single module update.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HmrModuleUpdate {
  /// The type of update (js-update, css-update).
  #[serde(rename = "type")]
  pub update_type: String,
  /// The module path (URL).
  pub path: String,
  /// The accepting module path (boundary).
  #[serde(rename = "acceptedPath")]
  pub accepted_path: String,
  /// Timestamp for cache busting.
  pub timestamp: u64,
  /// Whether this is a self-accepting module.
  #[serde(rename = "isWithinCircularImport", default)]
  pub is_within_circular_import: bool,
}

impl HmrModuleUpdate {
  /// Create a new JS module update.
  pub fn js_update(path: impl Into<String>, accepted_path: impl Into<String>, timestamp: u64) -> Self {
    Self {
      update_type: "js-update".to_string(),
      path: path.into(),
      accepted_path: accepted_path.into(),
      timestamp,
      is_within_circular_import: false,
    }
  }

  /// Create a new CSS module update.
  pub fn css_update(path: impl Into<String>, timestamp: u64) -> Self {
    let path_str = path.into();
    Self {
      update_type: "css-update".to_string(),
      path: path_str.clone(),
      accepted_path: path_str,
      timestamp,
      is_within_circular_import: false,
    }
  }
}

/// Error payload for HMR.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HmrErrorPayload {
  /// Error message.
  pub message: String,
  /// Stack trace (if available).
  pub stack: Option<String>,
  /// Source file.
  pub file: Option<String>,
  /// Error location.
  pub loc: Option<HmrErrorLocation>,
  /// Plugin name (if error from plugin).
  pub plugin: Option<String>,
}

/// Error location within a file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HmrErrorLocation {
  /// Line number (1-indexed).
  pub line: u32,
  /// Column number (0-indexed).
  pub column: u32,
}

/// Information about a module for HMR tracking.
#[derive(Debug, Clone)]
pub struct HmrModuleInfo {
  /// The module specifier.
  pub specifier: ModuleSpecifier,
  /// Whether this module accepts its own updates.
  pub accept_self: bool,
  /// Dependencies that this module accepts updates from.
  pub accepted_deps: HashSet<ModuleSpecifier>,
  /// Whether this module has declined HMR.
  pub declined: bool,
  /// Whether this module has dispose callbacks.
  pub has_dispose: bool,
  /// Whether this module has prune callbacks.
  pub has_prune: bool,
}

impl HmrModuleInfo {
  /// Create a new HMR module info.
  pub fn new(specifier: ModuleSpecifier) -> Self {
    Self {
      specifier,
      accept_self: false,
      accepted_deps: HashSet::new(),
      declined: false,
      has_dispose: false,
      has_prune: false,
    }
  }

  /// Mark this module as self-accepting.
  pub fn with_accept_self(mut self) -> Self {
    self.accept_self = true;
    self
  }

  /// Add an accepted dependency.
  pub fn with_accepted_dep(mut self, dep: ModuleSpecifier) -> Self {
    self.accepted_deps.insert(dep);
    self
  }

  /// Mark this module as declined.
  pub fn with_declined(mut self) -> Self {
    self.declined = true;
    self
  }

  /// Check if this module can handle updates for a given dependency.
  pub fn can_accept(&self, dep: &ModuleSpecifier) -> bool {
    if self.declined {
      return false;
    }
    if self.accept_self && &self.specifier == dep {
      return true;
    }
    self.accepted_deps.contains(dep)
  }
}

/// Result of computing an HMR boundary.
#[derive(Debug, Clone)]
pub enum HmrBoundary {
  /// Module(s) found that can accept the update.
  Accepted {
    /// The modules that accept the update.
    boundaries: Vec<ModuleSpecifier>,
    /// All modules that need to be invalidated.
    invalidated: Vec<ModuleSpecifier>,
  },
  /// No accepting boundary found, full reload required.
  FullReload {
    /// The reason for requiring full reload.
    reason: String,
  },
  /// A module explicitly declined HMR.
  Declined {
    /// The module that declined.
    module: ModuleSpecifier,
  },
}

impl HmrBoundary {
  /// Create a boundary that accepts the update.
  pub fn accepted(boundaries: Vec<ModuleSpecifier>, invalidated: Vec<ModuleSpecifier>) -> Self {
    Self::Accepted {
      boundaries,
      invalidated,
    }
  }

  /// Create a boundary indicating full reload is needed.
  pub fn full_reload(reason: impl Into<String>) -> Self {
    Self::FullReload {
      reason: reason.into(),
    }
  }

  /// Create a boundary indicating a module declined HMR.
  pub fn declined(module: ModuleSpecifier) -> Self {
    Self::Declined { module }
  }

  /// Check if this boundary indicates the update can be applied.
  pub fn can_apply(&self) -> bool {
    matches!(self, Self::Accepted { .. })
  }
}

/// Message sent from client to server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum HmrClientMessage {
  /// Client is connected and ready.
  Connected,
  /// Custom event from client to server.
  Custom {
    /// Event name.
    event: String,
    /// Event data.
    data: Option<serde_json::Value>,
  },
}

/// State of HMR for a module.
#[derive(Debug, Clone, Default)]
pub struct ModuleHmrState {
  /// Data preserved across updates (import.meta.hot.data).
  pub data: HashMap<String, serde_json::Value>,
  /// The current version/timestamp of the module.
  pub version: u64,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_hmr_config_default() {
    let config = HmrConfig::default();
    assert_eq!(config.port, 24678);
    assert_eq!(config.host, "localhost");
    assert!(config.overlay);
    assert_eq!(config.websocket_url(), "ws://localhost:24678");
  }

  #[test]
  fn test_hmr_config_custom() {
    let config = HmrConfig::default()
      .with_port(3000)
      .with_host("127.0.0.1");
    assert_eq!(config.websocket_url(), "ws://127.0.0.1:3000");
  }

  #[test]
  fn test_hmr_event_serialization() {
    let event = HmrEvent::Update(HmrUpdatePayload {
      updates: vec![HmrModuleUpdate::js_update("/app.js", "/app.js", 1234567890)],
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("update"));
    assert!(json.contains("/app.js"));

    let parsed: HmrEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
  }

  #[test]
  fn test_hmr_module_info() {
    let spec = ModuleSpecifier::parse("file:///app/mod.ts").unwrap();
    let dep = ModuleSpecifier::parse("file:///app/dep.ts").unwrap();

    let info = HmrModuleInfo::new(spec.clone())
      .with_accept_self()
      .with_accepted_dep(dep.clone());

    assert!(info.can_accept(&spec));
    assert!(info.can_accept(&dep));
    assert!(!info.can_accept(&ModuleSpecifier::parse("file:///app/other.ts").unwrap()));
  }

  #[test]
  fn test_hmr_module_info_declined() {
    let spec = ModuleSpecifier::parse("file:///app/mod.ts").unwrap();
    let info = HmrModuleInfo::new(spec.clone())
      .with_accept_self()
      .with_declined();

    // Declined modules can't accept any updates
    assert!(!info.can_accept(&spec));
  }

  #[test]
  fn test_hmr_boundary() {
    let spec = ModuleSpecifier::parse("file:///app/mod.ts").unwrap();

    let accepted = HmrBoundary::accepted(vec![spec.clone()], vec![spec.clone()]);
    assert!(accepted.can_apply());

    let full_reload = HmrBoundary::full_reload("No accepting boundary");
    assert!(!full_reload.can_apply());

    let declined = HmrBoundary::declined(spec);
    assert!(!declined.can_apply());
  }
}
