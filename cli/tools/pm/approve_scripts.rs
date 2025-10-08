// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_npm_installer::lifecycle_scripts::LifecycleScriptsState;
use serde::Deserialize;
use serde::Serialize;

use crate::args::ApproveScriptsFlags;
use crate::args::Flags;

/// Find the closest node_modules directory by walking up the directory tree
fn find_node_modules_dir(start_dir: &Path) -> Option<PathBuf> {
  let mut current_dir = start_dir;

  loop {
    let node_modules = current_dir.join("node_modules");
    if node_modules.exists() && node_modules.is_dir() {
      return Some(node_modules);
    }

    match current_dir.parent() {
      Some(parent) => current_dir = parent,
      None => return None,
    }
  }
}

/// Read the .state.toml file from the node_modules/.deno/ directory
fn read_state_json(
  node_modules_dir: &Path,
) -> Result<LifecycleScriptsState, AnyError> {
  let state_file_path = node_modules_dir.join(".deno").join(".state.json");

  if !state_file_path.exists() {
    // If the file doesn't exist, return default state
    return Ok(LifecycleScriptsState::default());
  }

  let content =
    std::fs::read_to_string(&state_file_path).with_context(|| {
      format!("Failed to read state file: {}", state_file_path.display())
    })?;

  let state: LifecycleScriptsState = serde_json::from_str(&content)
    .with_context(|| {
      format!(
        "Failed to deserialize state file: {}",
        state_file_path.display()
      )
    })?;

  Ok(state)
}

pub async fn approve_scripts(
  flags: Arc<Flags>,
  approve_scripts_flags: ApproveScriptsFlags,
) -> Result<(), AnyError> {
  // Get the current working directory to start searching from
  let current_dir =
    std::env::current_dir().context("Failed to get current directory")?;

  // Find the closest node_modules directory
  let node_modules_dir = find_node_modules_dir(&current_dir)
    .ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "No node_modules directory found. Please run this command from a project with a node_modules directory."
      )
    })?;

  println!(
    "Found node_modules directory at: {}",
    node_modules_dir.display()
  );

  // TODO(bartlomieju): this will error out if there are no scripts to approve (ie. no .state.json on disk)
  // Read the existing .state.json file
  let mut state = read_state_json(&node_modules_dir)
    .context("Failed to read .state.json file")?;

  if state.ignored_scripts.is_empty() {
    println!("No packages with lifecycle scripts found in .state.json");
    return Ok(());
  }

  eprintln!(
    "WARNING: interactive mode is not available yet, please `deno approve-scripts <value>`"
  );
  println!("Current packages with lifecycle scripts:");
  for package in &state.ignored_scripts {
    println!("  - {}", package);
  }

  // Display the packages that would be approved
  if !approve_scripts_flags.packages.is_empty() {
    println!("Packages to approve for lifecycle scripts:");
    for package in &approve_scripts_flags.packages {
      println!("  - {}", package);
    }
  }

  // TODO: Implement the actual approval logic:
  // 1. Validate that the provided packages exist in the ignored_scripts list
  // 2. Remove approved packages from the ignored_scripts list
  // 3. Write back the updated .state.toml file

  Ok(())
}
