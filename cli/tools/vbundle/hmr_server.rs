// Copyright 2018-2026 the Deno authors. MIT license.

//! HMR (Hot Module Replacement) Server.
//!
//! This module implements a WebSocket server for HMR updates. It coordinates
//! file watching with connected clients to provide real-time module updates.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_core::futures::channel::mpsc;
use deno_core::futures::channel::mpsc::UnboundedReceiver;
use deno_core::futures::channel::mpsc::UnboundedSender;
use deno_core::parking_lot::RwLock;
use deno_core::serde_json;
use tokio::sync::broadcast;

use super::hmr_types::HmrBoundary;
use super::hmr_types::HmrConfig;
use super::hmr_types::HmrEvent;
use super::hmr_types::HmrModuleInfo;
use super::hmr_types::HmrModuleUpdate;
use super::hmr_types::HmrUpdatePayload;
use super::source_graph::SharedSourceGraph;

/// HMR Module Graph for tracking dependencies and computing update boundaries.
#[derive(Debug, Default)]
pub struct HmrModuleGraph {
  /// Module HMR info keyed by specifier.
  modules: HashMap<ModuleSpecifier, HmrModuleInfo>,
  /// Reverse dependency map: module -> modules that import it.
  importers: HashMap<ModuleSpecifier, HashSet<ModuleSpecifier>>,
}

impl HmrModuleGraph {
  /// Create a new empty HMR module graph.
  pub fn new() -> Self {
    Self::default()
  }

  /// Build the HMR graph from a source graph.
  pub fn from_source_graph(source_graph: &SharedSourceGraph) -> Self {
    let mut hmr_graph = Self::new();
    let graph = source_graph.read();

    for module in graph.modules() {
      // Add module info
      hmr_graph.modules.insert(
        module.specifier.clone(),
        HmrModuleInfo::new(module.specifier.clone()),
      );

      // Build reverse dependency map
      for import in &module.imports {
        hmr_graph
          .importers
          .entry(import.specifier.clone())
          .or_default()
          .insert(module.specifier.clone());
      }
      for import in &module.dynamic_imports {
        hmr_graph
          .importers
          .entry(import.specifier.clone())
          .or_default()
          .insert(module.specifier.clone());
      }
    }

    hmr_graph
  }

  /// Get modules that import a given module (reverse dependencies).
  pub fn get_importers(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Vec<ModuleSpecifier> {
    self
      .importers
      .get(specifier)
      .map(|set| set.iter().cloned().collect())
      .unwrap_or_default()
  }

  /// Get HMR info for a module.
  pub fn get_module_info(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&HmrModuleInfo> {
    self.modules.get(specifier)
  }

  /// Get mutable HMR info for a module.
  pub fn get_module_info_mut(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<&mut HmrModuleInfo> {
    self.modules.get_mut(specifier)
  }

  /// Update module HMR info (e.g., when accept/decline is called).
  pub fn update_module_info(&mut self, info: HmrModuleInfo) {
    self.modules.insert(info.specifier.clone(), info);
  }

  /// Find the accepting boundary for an HMR update.
  ///
  /// Starting from the changed module, traverse up the dependency graph
  /// until we find modules that accept the update or reach entry points.
  pub fn find_accepting_boundary(
    &self,
    changed: &ModuleSpecifier,
  ) -> HmrBoundary {
    let mut visited = HashSet::new();
    let mut to_process = vec![changed.clone()];
    let mut boundaries = Vec::new();
    let mut invalidated = vec![changed.clone()];

    while let Some(current) = to_process.pop() {
      if visited.contains(&current) {
        continue;
      }
      visited.insert(current.clone());

      // Check if this module has HMR info
      if let Some(info) = self.modules.get(&current) {
        // Check if module declined HMR
        if info.declined {
          return HmrBoundary::declined(current);
        }

        // Check if module accepts the changed module
        if info.can_accept(changed) {
          boundaries.push(current.clone());
          continue;
        }

        // Check if module accepts itself (self-accepting modules)
        if info.accept_self && &current == changed {
          boundaries.push(current.clone());
          continue;
        }
      }

      // Get importers to propagate up
      let importers = self.get_importers(&current);

      if importers.is_empty() {
        // Reached an entry point without finding an accepting boundary
        return HmrBoundary::full_reload(format!(
          "No accepting boundary found for module: {}",
          changed
        ));
      }

      // Add importers to process
      for importer in importers {
        if !visited.contains(&importer) {
          to_process.push(importer.clone());
          invalidated.push(importer.clone());
        }
      }
    }

    if boundaries.is_empty() {
      HmrBoundary::full_reload(format!(
        "No accepting boundary found for module: {}",
        changed
      ))
    } else {
      HmrBoundary::accepted(boundaries, invalidated)
    }
  }
}

/// Shared HMR module graph.
pub type SharedHmrGraph = Arc<RwLock<HmrModuleGraph>>;

/// Create a new shared HMR graph.
pub fn new_shared_hmr_graph() -> SharedHmrGraph {
  Arc::new(RwLock::new(HmrModuleGraph::new()))
}

/// Message to send to HMR clients.
#[derive(Debug, Clone)]
pub enum HmrServerMessage {
  /// Send an HMR event to all clients.
  Broadcast(HmrEvent),
  /// Shutdown the server.
  Shutdown,
}

/// HMR Server state.
pub struct HmrServer {
  /// Server configuration.
  pub config: HmrConfig,
  /// The HMR module graph.
  hmr_graph: SharedHmrGraph,
  /// Channel to send messages to connected clients.
  broadcast_tx: broadcast::Sender<HmrServerMessage>,
  /// Channel to receive file change notifications.
  file_change_rx: Option<UnboundedReceiver<Vec<PathBuf>>>,
  /// The source graph (for looking up module info).
  source_graph: SharedSourceGraph,
}

impl HmrServer {
  /// Create a new HMR server.
  pub fn new(
    config: HmrConfig,
    source_graph: SharedSourceGraph,
    file_change_rx: UnboundedReceiver<Vec<PathBuf>>,
  ) -> Self {
    let hmr_graph = Arc::new(RwLock::new(HmrModuleGraph::from_source_graph(
      &source_graph,
    )));
    let (broadcast_tx, _) = broadcast::channel(256);

    Self {
      config,
      hmr_graph,
      broadcast_tx,
      file_change_rx: Some(file_change_rx),
      source_graph,
    }
  }

  /// Get a broadcast receiver for HMR messages.
  pub fn subscribe(&self) -> broadcast::Receiver<HmrServerMessage> {
    self.broadcast_tx.subscribe()
  }

  /// Get the broadcast sender (for external use).
  pub fn get_sender(&self) -> broadcast::Sender<HmrServerMessage> {
    self.broadcast_tx.clone()
  }

  /// Get the HMR graph.
  pub fn get_hmr_graph(&self) -> SharedHmrGraph {
    self.hmr_graph.clone()
  }

  /// Handle a file change notification.
  pub fn handle_file_change(&self, paths: &[PathBuf]) -> Vec<HmrEvent> {
    let specifiers: Vec<ModuleSpecifier> =
      paths.iter().filter_map(path_to_specifier).collect();

    self.handle_specifier_change(&specifiers)
  }

  /// Handle a specifier change notification.
  ///
  /// This is the core logic for handling module changes. It finds the
  /// accepting boundary for each changed module and generates the
  /// appropriate HMR events.
  pub fn handle_specifier_change(
    &self,
    specifiers: &[ModuleSpecifier],
  ) -> Vec<HmrEvent> {
    let mut events = Vec::new();
    let timestamp = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_millis() as u64;

    let hmr_graph = self.hmr_graph.read();

    for specifier in specifiers {
      // Check if this module is in our graph
      let graph = self.source_graph.read();
      if !graph.has_module(specifier) {
        continue;
      }
      drop(graph);

      // Find accepting boundary
      let boundary = hmr_graph.find_accepting_boundary(specifier);

      match boundary {
        HmrBoundary::Accepted {
          boundaries,
          invalidated: _,
        } => {
          let updates: Vec<HmrModuleUpdate> = boundaries
            .into_iter()
            .map(|boundary| {
              HmrModuleUpdate::js_update(
                specifier.to_string(),
                boundary.to_string(),
                timestamp,
              )
            })
            .collect();

          if !updates.is_empty() {
            events.push(HmrEvent::Update(HmrUpdatePayload { updates }));
          }
        }
        HmrBoundary::FullReload { reason } => {
          log::info!("HMR full reload required: {}", reason);
          events.push(HmrEvent::FullReload {
            path: Some(specifier.to_string()),
          });
        }
        HmrBoundary::Declined { module } => {
          log::info!("HMR declined by module: {}", module);
          events.push(HmrEvent::FullReload {
            path: Some(specifier.to_string()),
          });
        }
      }
    }

    events
  }

  /// Broadcast an event to all connected clients.
  pub fn broadcast(&self, event: HmrEvent) {
    let _ = self.broadcast_tx.send(HmrServerMessage::Broadcast(event));
  }

  /// Shutdown the server.
  pub fn shutdown(&self) {
    let _ = self.broadcast_tx.send(HmrServerMessage::Shutdown);
  }

  /// Run the HMR server event loop.
  ///
  /// This watches for file changes and broadcasts updates to clients.
  pub async fn run(&mut self) -> Result<(), AnyError> {
    let mut file_change_rx = self.file_change_rx.take().unwrap();

    // Broadcast initial connected event
    self.broadcast(HmrEvent::Connected);

    loop {
      tokio::select! {
        Some(paths) = file_change_rx.next() => {
          log::debug!("HMR server received file changes: {:?}", paths);

          // Handle the file changes
          let events = self.handle_file_change(&paths);

          // Broadcast all events
          for event in events {
            self.broadcast(event);
          }
        }
        else => {
          // Channel closed, shutdown
          break;
        }
      }
    }

    Ok(())
  }
}

/// Convert a file path to a module specifier.
fn path_to_specifier(path: &PathBuf) -> Option<ModuleSpecifier> {
  let path = path.canonicalize().ok()?;
  ModuleSpecifier::from_file_path(path).ok()
}

/// HMR WebSocket handler.
///
/// This handles individual WebSocket connections for HMR.
pub struct HmrWebSocketHandler {
  /// Broadcast receiver for HMR messages.
  broadcast_rx: broadcast::Receiver<HmrServerMessage>,
  /// Sender for client messages.
  #[allow(dead_code)]
  client_tx: UnboundedSender<String>,
}

impl HmrWebSocketHandler {
  /// Create a new WebSocket handler.
  pub fn new(
    broadcast_rx: broadcast::Receiver<HmrServerMessage>,
    client_tx: UnboundedSender<String>,
  ) -> Self {
    Self {
      broadcast_rx,
      client_tx,
    }
  }

  /// Run the WebSocket handler, forwarding messages to the client.
  pub async fn run(&mut self) -> Result<(), AnyError> {
    loop {
      match self.broadcast_rx.recv().await {
        Ok(HmrServerMessage::Broadcast(event)) => {
          let json = serde_json::to_string(&event)?;
          log::debug!("Sending HMR event to client: {}", json);
          // The actual WebSocket send would happen here
          // For now, we just log it
        }
        Ok(HmrServerMessage::Shutdown) => {
          break;
        }
        Err(broadcast::error::RecvError::Lagged(_)) => {
          // Missed some messages, continue
          continue;
        }
        Err(broadcast::error::RecvError::Closed) => {
          break;
        }
      }
    }

    Ok(())
  }
}

/// Create file change sender and receiver pair.
pub fn create_file_change_channel() -> (
  UnboundedSender<Vec<PathBuf>>,
  UnboundedReceiver<Vec<PathBuf>>,
) {
  mpsc::unbounded()
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use deno_ast::MediaType;

  use super::*;
  use crate::tools::vbundle::environment::BundleEnvironment;
  use crate::tools::vbundle::source_graph::ImportInfo;
  use crate::tools::vbundle::source_graph::SourceModule;

  fn create_test_specifier(path: &str) -> ModuleSpecifier {
    ModuleSpecifier::parse(&format!("file:///{}", path)).unwrap()
  }

  // ===========================================================================
  // Test Infrastructure
  // ===========================================================================

  /// Helper to construct SharedSourceGraph with known dependency structures.
  struct TestSourceGraphBuilder {
    modules: Vec<(String, Vec<String>)>,
    entrypoints: Vec<String>,
  }

  impl TestSourceGraphBuilder {
    fn new() -> Self {
      Self {
        modules: Vec::new(),
        entrypoints: Vec::new(),
      }
    }

    /// Add a module with its imports.
    fn add_module(mut self, specifier: &str, imports: &[&str]) -> Self {
      self.modules.push((
        specifier.to_string(),
        imports.iter().map(|s| s.to_string()).collect(),
      ));
      self
    }

    /// Mark a module as an entry point.
    fn add_entrypoint(mut self, specifier: &str) -> Self {
      self.entrypoints.push(specifier.to_string());
      self
    }

    /// Build the SharedSourceGraph.
    fn build(self) -> SharedSourceGraph {
      let graph = SharedSourceGraph::new();
      {
        let mut g = graph.write();

        // Add all modules
        for (specifier, imports) in &self.modules {
          let spec = ModuleSpecifier::parse(specifier).unwrap();
          let mut module =
            SourceModule::new(spec.clone(), "".into(), MediaType::TypeScript);
          module.add_environment(BundleEnvironment::Server);

          // Add imports
          for import_spec in imports {
            let import_specifier = ModuleSpecifier::parse(import_spec).unwrap();
            module.imports.push(ImportInfo {
              specifier: import_specifier,
              original: import_spec.clone(),
              named: vec![],
              default_import: None,
              namespace_import: None,
              is_type_only: false,
              range: (0, 0),
            });
          }

          // Mark as entry if it's in the entrypoints list
          if self.entrypoints.contains(specifier) {
            module.is_entry = true;
          }

          g.add_module(module);
        }

        // Register entrypoints
        for entry in &self.entrypoints {
          let spec = ModuleSpecifier::parse(entry).unwrap();
          g.add_entrypoint(BundleEnvironment::Server, spec);
        }
      }

      graph
    }
  }

  /// Context wrapper for end-to-end HMR testing.
  struct TestHmrContext {
    source_graph: SharedSourceGraph,
    hmr_graph: SharedHmrGraph,
    #[allow(dead_code)]
    file_change_tx: UnboundedSender<Vec<PathBuf>>,
    file_change_rx: Option<UnboundedReceiver<Vec<PathBuf>>>,
  }

  impl TestHmrContext {
    /// Create a new test context from a source graph.
    fn new(source_graph: SharedSourceGraph) -> Self {
      let hmr_graph = Arc::new(RwLock::new(HmrModuleGraph::from_source_graph(
        &source_graph,
      )));
      let (file_change_tx, file_change_rx) = create_file_change_channel();

      Self {
        source_graph,
        hmr_graph,
        file_change_tx,
        file_change_rx: Some(file_change_rx),
      }
    }

    /// Configure a module to accept itself (has import.meta.hot.accept()).
    fn with_self_accept(self, specifier: &str) -> Self {
      let spec = ModuleSpecifier::parse(specifier).unwrap();
      let info = HmrModuleInfo::new(spec.clone()).with_accept_self();
      self.hmr_graph.write().update_module_info(info);
      self
    }

    /// Configure a module to accept a specific dependency.
    fn with_dep_accept(self, acceptor: &str, dep: &str) -> Self {
      let acceptor_spec = ModuleSpecifier::parse(acceptor).unwrap();
      let dep_spec = ModuleSpecifier::parse(dep).unwrap();

      {
        let mut graph = self.hmr_graph.write();
        let info = if let Some(existing) = graph.get_module_info(&acceptor_spec)
        {
          let mut info = existing.clone();
          info.accepted_deps.insert(dep_spec);
          info
        } else {
          HmrModuleInfo::new(acceptor_spec.clone()).with_accepted_dep(dep_spec)
        };
        graph.update_module_info(info);
      }
      self
    }

    /// Configure a module to decline HMR.
    #[allow(dead_code)]
    fn with_declined(self, specifier: &str) -> Self {
      let spec = ModuleSpecifier::parse(specifier).unwrap();
      let info = HmrModuleInfo::new(spec.clone()).with_declined();
      self.hmr_graph.write().update_module_info(info);
      self
    }

    /// Simulate a file change and get resulting events.
    ///
    /// Paths should be specifier strings like "file:///app/mod.ts".
    fn simulate_change(&self, specifiers: &[&str]) -> Vec<HmrEvent> {
      // Create a temporary HmrServer to handle the file change
      let (_, file_rx) = create_file_change_channel();
      let server = HmrServer {
        config: HmrConfig::default(),
        hmr_graph: self.hmr_graph.clone(),
        broadcast_tx: broadcast::channel(16).0,
        file_change_rx: Some(file_rx),
        source_graph: self.source_graph.clone(),
      };

      // Convert specifier strings to ModuleSpecifiers
      let specs: Vec<ModuleSpecifier> = specifiers
        .iter()
        .map(|s| ModuleSpecifier::parse(s).unwrap())
        .collect();

      server.handle_specifier_change(&specs)
    }

    /// Build an HmrServer for async tests.
    fn build_server(mut self) -> (HmrServer, UnboundedSender<Vec<PathBuf>>) {
      let file_rx = self.file_change_rx.take().unwrap();
      let server = HmrServer {
        config: HmrConfig::default(),
        hmr_graph: self.hmr_graph.clone(),
        broadcast_tx: broadcast::channel(16).0,
        file_change_rx: Some(file_rx),
        source_graph: self.source_graph.clone(),
      };
      (server, self.file_change_tx.clone())
    }
  }

  // ===========================================================================
  // End-to-End Tests
  // ===========================================================================

  #[test]
  fn test_e2e_self_accepting_module() {
    let graph = TestSourceGraphBuilder::new()
      .add_module("file:///app/mod.ts", &[])
      .add_entrypoint("file:///app/mod.ts")
      .build();

    let ctx = TestHmrContext::new(graph).with_self_accept("file:///app/mod.ts");

    let events = ctx.simulate_change(&["file:///app/mod.ts"]);

    assert_eq!(events.len(), 1);
    match &events[0] {
      HmrEvent::Update(payload) => {
        assert_eq!(payload.updates.len(), 1);
        assert!(payload.updates[0].path.contains("mod.ts"));
        assert!(payload.updates[0].accepted_path.contains("mod.ts"));
      }
      _ => panic!("Expected Update event, got {:?}", events[0]),
    }
  }

  #[test]
  fn test_e2e_bubble_up_no_hot_in_child() {
    // child.ts has NO import.meta.hot
    // parent.ts imports child and has import.meta.hot.accept("./child")
    let graph = TestSourceGraphBuilder::new()
      .add_module("file:///app/child.ts", &[])
      .add_module("file:///app/parent.ts", &["file:///app/child.ts"])
      .add_entrypoint("file:///app/parent.ts")
      .build();

    let ctx = TestHmrContext::new(graph)
      .with_dep_accept("file:///app/parent.ts", "file:///app/child.ts");

    // Change child - should bubble to parent
    let events = ctx.simulate_change(&["file:///app/child.ts"]);

    assert_eq!(events.len(), 1);
    match &events[0] {
      HmrEvent::Update(payload) => {
        assert_eq!(payload.updates.len(), 1);
        assert!(payload.updates[0].path.contains("child.ts"));
        assert!(payload.updates[0].accepted_path.contains("parent.ts"));
      }
      _ => panic!("Expected Update event, got {:?}", events[0]),
    }
  }

  #[test]
  fn test_e2e_full_bubble_chain() {
    // entry -> middle -> leaf
    // Each module accepts its direct dependency:
    // - entry accepts middle
    // - middle accepts leaf
    // This creates a full bubble-up chain.
    let graph = TestSourceGraphBuilder::new()
      .add_module("file:///app/leaf.ts", &[])
      .add_module("file:///app/middle.ts", &["file:///app/leaf.ts"])
      .add_module("file:///app/entry.ts", &["file:///app/middle.ts"])
      .add_entrypoint("file:///app/entry.ts")
      .build();

    let ctx = TestHmrContext::new(graph)
      .with_dep_accept("file:///app/middle.ts", "file:///app/leaf.ts");

    // Change leaf - should bubble to middle (which accepts it)
    let events = ctx.simulate_change(&["file:///app/leaf.ts"]);

    assert_eq!(events.len(), 1);
    match &events[0] {
      HmrEvent::Update(payload) => {
        assert_eq!(payload.updates.len(), 1);
        // leaf.ts change is accepted by middle.ts
        assert!(payload.updates[0].path.contains("leaf.ts"));
        assert!(payload.updates[0].accepted_path.contains("middle.ts"));
      }
      _ => panic!("Expected Update event, got {:?}", events[0]),
    }
  }

  #[test]
  fn test_e2e_self_accept_no_bubble_needed() {
    // When a self-accepting module changes, it handles its own update
    let graph = TestSourceGraphBuilder::new()
      .add_module("file:///app/child.ts", &[])
      .add_module("file:///app/parent.ts", &["file:///app/child.ts"])
      .add_entrypoint("file:///app/parent.ts")
      .build();

    // child.ts is self-accepting
    let ctx =
      TestHmrContext::new(graph).with_self_accept("file:///app/child.ts");

    // Change child.ts - it accepts itself
    let events = ctx.simulate_change(&["file:///app/child.ts"]);

    assert_eq!(events.len(), 1);
    match &events[0] {
      HmrEvent::Update(payload) => {
        assert_eq!(payload.updates.len(), 1);
        assert!(payload.updates[0].path.contains("child.ts"));
        assert!(payload.updates[0].accepted_path.contains("child.ts"));
      }
      _ => panic!("Expected Update event, got {:?}", events[0]),
    }
  }

  #[test]
  fn test_e2e_no_acceptor_full_reload() {
    // Neither module has import.meta.hot
    let graph = TestSourceGraphBuilder::new()
      .add_module("file:///app/dep.ts", &[])
      .add_module("file:///app/entry.ts", &["file:///app/dep.ts"])
      .add_entrypoint("file:///app/entry.ts")
      .build();

    let ctx = TestHmrContext::new(graph);
    // No HMR configuration

    let events = ctx.simulate_change(&["file:///app/dep.ts"]);

    assert_eq!(events.len(), 1);
    assert!(
      matches!(&events[0], HmrEvent::FullReload { .. }),
      "Expected FullReload event, got {:?}",
      events[0]
    );
  }

  #[test]
  fn test_e2e_diamond_multiple_acceptors() {
    //     entry
    //    /     \
    //  left   right  (both accept shared)
    //    \     /
    //    shared
    let graph = TestSourceGraphBuilder::new()
      .add_module("file:///app/shared.ts", &[])
      .add_module("file:///app/left.ts", &["file:///app/shared.ts"])
      .add_module("file:///app/right.ts", &["file:///app/shared.ts"])
      .add_module(
        "file:///app/entry.ts",
        &["file:///app/left.ts", "file:///app/right.ts"],
      )
      .add_entrypoint("file:///app/entry.ts")
      .build();

    let ctx = TestHmrContext::new(graph)
      .with_dep_accept("file:///app/left.ts", "file:///app/shared.ts")
      .with_dep_accept("file:///app/right.ts", "file:///app/shared.ts");

    let events = ctx.simulate_change(&["file:///app/shared.ts"]);

    // Should have updates for both left and right
    assert_eq!(events.len(), 1);
    if let HmrEvent::Update(payload) = &events[0] {
      assert_eq!(
        payload.updates.len(),
        2,
        "Expected 2 updates (left and right), got {}",
        payload.updates.len()
      );

      let accepted_paths: Vec<&str> = payload
        .updates
        .iter()
        .map(|u| u.accepted_path.as_str())
        .collect();

      assert!(
        accepted_paths.iter().any(|p| p.contains("left.ts")),
        "Expected left.ts in accepted paths"
      );
      assert!(
        accepted_paths.iter().any(|p| p.contains("right.ts")),
        "Expected right.ts in accepted paths"
      );
    } else {
      panic!("Expected Update event, got {:?}", events[0]);
    }
  }

  #[test]
  fn test_e2e_declined_module_triggers_full_reload() {
    let graph = TestSourceGraphBuilder::new()
      .add_module("file:///app/child.ts", &[])
      .add_module("file:///app/parent.ts", &["file:///app/child.ts"])
      .add_entrypoint("file:///app/parent.ts")
      .build();

    let ctx = TestHmrContext::new(graph).with_declined("file:///app/parent.ts");

    let events = ctx.simulate_change(&["file:///app/child.ts"]);

    assert_eq!(events.len(), 1);
    assert!(
      matches!(&events[0], HmrEvent::FullReload { .. }),
      "Expected FullReload when module declines HMR, got {:?}",
      events[0]
    );
  }

  #[test]
  fn test_e2e_multiple_file_changes() {
    let graph = TestSourceGraphBuilder::new()
      .add_module("file:///app/a.ts", &[])
      .add_module("file:///app/b.ts", &[])
      .add_module(
        "file:///app/entry.ts",
        &["file:///app/a.ts", "file:///app/b.ts"],
      )
      .add_entrypoint("file:///app/entry.ts")
      .build();

    let ctx = TestHmrContext::new(graph)
      .with_dep_accept("file:///app/entry.ts", "file:///app/a.ts")
      .with_dep_accept("file:///app/entry.ts", "file:///app/b.ts");

    // Change both files at once
    let events = ctx.simulate_change(&["file:///app/a.ts", "file:///app/b.ts"]);

    // Should have two Update events
    assert_eq!(events.len(), 2, "Expected 2 events for 2 file changes");

    for event in &events {
      assert!(
        matches!(event, HmrEvent::Update(_)),
        "Expected Update events, got {:?}",
        event
      );
    }
  }

  #[test]
  fn test_e2e_change_unknown_file() {
    let graph = TestSourceGraphBuilder::new()
      .add_module("file:///app/mod.ts", &[])
      .add_entrypoint("file:///app/mod.ts")
      .build();

    let ctx = TestHmrContext::new(graph).with_self_accept("file:///app/mod.ts");

    // Change a file that's not in the graph
    let events = ctx.simulate_change(&["file:///app/unknown.ts"]);

    // Should produce no events
    assert!(
      events.is_empty(),
      "Expected no events for unknown file, got {:?}",
      events
    );
  }

  #[tokio::test]
  async fn test_e2e_async_server_startup() {
    // This test verifies the async server event flow:
    // 1. Server broadcasts Connected event on startup
    // 2. Channel shutdown causes server to exit cleanly
    let graph = TestSourceGraphBuilder::new()
      .add_module("file:///app/mod.ts", &[])
      .add_entrypoint("file:///app/mod.ts")
      .build();

    let ctx = TestHmrContext::new(graph).with_self_accept("file:///app/mod.ts");

    let (mut server, file_tx) = ctx.build_server();
    let mut event_rx = server.subscribe();

    // Spawn server
    let handle = tokio::spawn(async move { server.run().await });

    // Receive Connected event (sent on startup)
    let msg1 =
      tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .expect("Timeout waiting for Connected event")
        .expect("Channel closed");

    assert!(
      matches!(msg1, HmrServerMessage::Broadcast(HmrEvent::Connected)),
      "Expected Connected event on startup, got {:?}",
      msg1
    );

    // Clean up - dropping the sender closes the channel and causes run() to exit
    drop(file_tx);
    let result = tokio::time::timeout(Duration::from_millis(100), handle).await;
    assert!(result.is_ok(), "Server should have shut down cleanly");
  }

  #[tokio::test]
  async fn test_e2e_async_broadcast_channel() {
    // Test that the broadcast channel works for multiple subscribers
    let graph = TestSourceGraphBuilder::new()
      .add_module("file:///app/mod.ts", &[])
      .add_entrypoint("file:///app/mod.ts")
      .build();

    let ctx = TestHmrContext::new(graph).with_self_accept("file:///app/mod.ts");

    let (server, _file_tx) = ctx.build_server();

    // Create multiple subscribers
    let mut rx1 = server.subscribe();
    let mut rx2 = server.subscribe();

    // Broadcast an event directly
    server.broadcast(HmrEvent::FullReload { path: None });

    // Both subscribers should receive it
    let msg1 = tokio::time::timeout(Duration::from_millis(50), rx1.recv())
      .await
      .expect("Timeout waiting for rx1")
      .expect("Channel closed");
    let msg2 = tokio::time::timeout(Duration::from_millis(50), rx2.recv())
      .await
      .expect("Timeout waiting for rx2")
      .expect("Channel closed");

    assert!(matches!(
      msg1,
      HmrServerMessage::Broadcast(HmrEvent::FullReload { .. })
    ));
    assert!(matches!(
      msg2,
      HmrServerMessage::Broadcast(HmrEvent::FullReload { .. })
    ));
  }

  #[test]
  fn test_hmr_graph_importers() {
    let mut graph = HmrModuleGraph::new();

    let main = create_test_specifier("app/main.ts");
    let dep = create_test_specifier("app/dep.ts");

    // Add modules
    graph
      .modules
      .insert(main.clone(), HmrModuleInfo::new(main.clone()));
    graph
      .modules
      .insert(dep.clone(), HmrModuleInfo::new(dep.clone()));

    // Add reverse dependency (main imports dep)
    graph
      .importers
      .entry(dep.clone())
      .or_default()
      .insert(main.clone());

    let importers = graph.get_importers(&dep);
    assert_eq!(importers.len(), 1);
    assert_eq!(importers[0], main);
  }

  #[test]
  fn test_hmr_boundary_self_accepting() {
    let mut graph = HmrModuleGraph::new();

    let module = create_test_specifier("app/self_accept.ts");
    let info = HmrModuleInfo::new(module.clone()).with_accept_self();
    graph.modules.insert(module.clone(), info);

    let boundary = graph.find_accepting_boundary(&module);
    assert!(boundary.can_apply());

    if let HmrBoundary::Accepted { boundaries, .. } = boundary {
      assert_eq!(boundaries.len(), 1);
      assert_eq!(boundaries[0], module);
    } else {
      panic!("Expected accepted boundary");
    }
  }

  #[test]
  fn test_hmr_boundary_parent_accepting() {
    let mut graph = HmrModuleGraph::new();

    let parent = create_test_specifier("app/parent.ts");
    let child = create_test_specifier("app/child.ts");

    // Parent accepts updates from child
    let parent_info =
      HmrModuleInfo::new(parent.clone()).with_accepted_dep(child.clone());
    graph.modules.insert(parent.clone(), parent_info);
    graph
      .modules
      .insert(child.clone(), HmrModuleInfo::new(child.clone()));

    // Parent imports child
    graph
      .importers
      .entry(child.clone())
      .or_default()
      .insert(parent.clone());

    let boundary = graph.find_accepting_boundary(&child);
    assert!(boundary.can_apply());

    if let HmrBoundary::Accepted { boundaries, .. } = boundary {
      assert_eq!(boundaries.len(), 1);
      assert_eq!(boundaries[0], parent);
    } else {
      panic!("Expected accepted boundary");
    }
  }

  #[test]
  fn test_hmr_boundary_declined() {
    let mut graph = HmrModuleGraph::new();

    let parent = create_test_specifier("app/parent.ts");
    let child = create_test_specifier("app/child.ts");

    // Parent declines HMR
    let parent_info = HmrModuleInfo::new(parent.clone()).with_declined();
    graph.modules.insert(parent.clone(), parent_info);
    graph
      .modules
      .insert(child.clone(), HmrModuleInfo::new(child.clone()));

    // Parent imports child
    graph
      .importers
      .entry(child.clone())
      .or_default()
      .insert(parent.clone());

    let boundary = graph.find_accepting_boundary(&child);
    assert!(!boundary.can_apply());

    if let HmrBoundary::Declined { module } = boundary {
      assert_eq!(module, parent);
    } else {
      panic!("Expected declined boundary");
    }
  }

  #[test]
  fn test_hmr_boundary_full_reload_no_acceptor() {
    let mut graph = HmrModuleGraph::new();

    let entry = create_test_specifier("app/entry.ts");
    let child = create_test_specifier("app/child.ts");

    // Neither module accepts HMR
    graph
      .modules
      .insert(entry.clone(), HmrModuleInfo::new(entry.clone()));
    graph
      .modules
      .insert(child.clone(), HmrModuleInfo::new(child.clone()));

    // Entry imports child (entry is the root)
    graph
      .importers
      .entry(child.clone())
      .or_default()
      .insert(entry.clone());

    // Entry has no importers (it's the entry point)

    let boundary = graph.find_accepting_boundary(&child);
    assert!(!boundary.can_apply());

    if let HmrBoundary::FullReload { reason } = boundary {
      assert!(reason.contains("No accepting boundary"));
    } else {
      panic!("Expected full reload");
    }
  }
}
