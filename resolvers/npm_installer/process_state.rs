// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;

use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NpmProcessStateKind {
  Snapshot(deno_npm::resolution::SerializedNpmResolutionSnapshot),
  Byonm,
}

/// The serialized npm process state which can be written to a file and then
/// the FD or path can be passed to a spawned deno process via the
/// `DENO_DONT_USE_INTERNAL_NODE_COMPAT_STATE_FD` env var.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NpmProcessState {
  pub kind: NpmProcessStateKind,
  pub local_node_modules_path: Option<String>,
}

impl NpmProcessState {
  pub fn new_managed(
    snapshot: ValidSerializedNpmResolutionSnapshot,
    node_modules_path: Option<&Path>,
  ) -> Self {
    NpmProcessState {
      kind: NpmProcessStateKind::Snapshot(snapshot.into_serialized()),
      local_node_modules_path: node_modules_path
        .map(|p| p.to_string_lossy().to_string()),
    }
  }

  pub fn new_local(
    snapshot: ValidSerializedNpmResolutionSnapshot,
    node_modules_path: &Path,
  ) -> Self {
    NpmProcessState::new_managed(snapshot, Some(node_modules_path))
  }

  pub fn as_serialized(&self) -> String {
    serde_json::to_string(self).unwrap()
  }
}
