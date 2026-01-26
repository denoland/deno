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
use deno_core::futures::channel::mpsc;
use deno_core::futures::channel::mpsc::UnboundedReceiver;
use deno_core::futures::channel::mpsc::UnboundedSender;
use deno_core::futures::StreamExt;
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
  pub fn get_importers(&self, specifier: &ModuleSpecifier) -> Vec<ModuleSpecifier> {
    self
      .importers
      .get(specifier)
      .map(|set| set.iter().cloned().collect())
      .unwrap_or_default()
  }

  /// Get HMR info for a module.
  pub fn get_module_info(&self, specifier: &ModuleSpecifier) -> Option<&HmrModuleInfo> {
    self.modules.get(specifier)
  }

  /// Get mutable HMR info for a module.
  pub fn get_module_info_mut(&mut self, specifier: &ModuleSpecifier) -> Option<&mut HmrModuleInfo> {
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
  pub fn find_accepting_boundary(&self, changed: &ModuleSpecifier) -> HmrBoundary {
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
    let hmr_graph = Arc::new(RwLock::new(HmrModuleGraph::from_source_graph(&source_graph)));
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
    let mut events = Vec::new();
    let timestamp = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_millis() as u64;

    let hmr_graph = self.hmr_graph.read();

    for path in paths {
      // Convert path to module specifier
      let specifier = match path_to_specifier(path) {
        Some(s) => s,
        None => continue,
      };

      // Check if this module is in our graph
      let graph = self.source_graph.read();
      if !graph.has_module(&specifier) {
        continue;
      }

      // Find accepting boundary
      let boundary = hmr_graph.find_accepting_boundary(&specifier);

      match boundary {
        HmrBoundary::Accepted { boundaries, invalidated: _ } => {
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
pub fn create_file_change_channel() -> (UnboundedSender<Vec<PathBuf>>, UnboundedReceiver<Vec<PathBuf>>) {
  mpsc::unbounded()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn create_test_specifier(path: &str) -> ModuleSpecifier {
    ModuleSpecifier::parse(&format!("file:///{}", path)).unwrap()
  }

  #[test]
  fn test_hmr_graph_importers() {
    let mut graph = HmrModuleGraph::new();

    let main = create_test_specifier("app/main.ts");
    let dep = create_test_specifier("app/dep.ts");

    // Add modules
    graph.modules.insert(main.clone(), HmrModuleInfo::new(main.clone()));
    graph.modules.insert(dep.clone(), HmrModuleInfo::new(dep.clone()));

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
    let parent_info = HmrModuleInfo::new(parent.clone()).with_accepted_dep(child.clone());
    graph.modules.insert(parent.clone(), parent_info);
    graph.modules.insert(child.clone(), HmrModuleInfo::new(child.clone()));

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
    graph.modules.insert(child.clone(), HmrModuleInfo::new(child.clone()));

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
    graph.modules.insert(entry.clone(), HmrModuleInfo::new(entry.clone()));
    graph.modules.insert(child.clone(), HmrModuleInfo::new(child.clone()));

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
