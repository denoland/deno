// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;

use deno_ast::ModuleSpecifier;
use deno_config::ConfigFile;
use deno_config::WorkspaceConfig;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_graph::ModuleGraph;

#[derive(Debug)]
pub struct MemberRoots {
  pub name: String,
  pub dir_url: ModuleSpecifier,
  pub exports: Vec<ModuleSpecifier>,
}

pub fn get_workspace_member_roots(
  config: &WorkspaceConfig,
) -> Result<Vec<MemberRoots>, AnyError> {
  let mut members = Vec::with_capacity(config.members.len());
  let mut seen_names = HashSet::with_capacity(config.members.len());
  for member in &config.members {
    if !seen_names.insert(&member.package_name) {
      bail!(
        "Cannot have two workspace packages with the same name ('{}' at {})",
        member.package_name,
        member.path.display(),
      );
    }
    members.push(MemberRoots {
      name: member.package_name.clone(),
      dir_url: member.config_file.specifier.join("./").unwrap().clone(),
      exports: resolve_config_file_roots_from_exports(&member.config_file)?,
    });
  }
  Ok(members)
}

pub fn resolve_config_file_roots_from_exports(
  config_file: &ConfigFile,
) -> Result<Vec<ModuleSpecifier>, AnyError> {
  let exports_config = config_file
    .to_exports_config()
    .with_context(|| {
      format!("Failed to parse exports at {}", config_file.specifier)
    })?
    .into_map();
  let mut exports = Vec::with_capacity(exports_config.len());
  for (_, value) in exports_config {
    let entry_point =
      config_file.specifier.join(&value).with_context(|| {
        format!("Failed to join {} with {}", config_file.specifier, value)
      })?;
    exports.push(entry_point);
  }
  Ok(exports)
}

pub fn surface_fast_check_type_graph_errors(
  graph: &ModuleGraph,
) -> Result<(), AnyError> {
  let mut diagnostic_count = 0;
  let mut seen_diagnostics = HashSet::new();
  for module in graph.modules() {
    if module.specifier().scheme() != "file" {
      continue;
    }
    let Some(module) = module.esm() else {
      continue;
    };
    if let Some(diagnostic) = module.fast_check_diagnostic() {
      for diagnostic in diagnostic.flatten_multiple() {
        let message = diagnostic.message_with_range();
        if !seen_diagnostics.insert(message.clone()) {
          continue;
        }
        log::error!("{}", message);
        diagnostic_count += 1;
      }
    }
  }
  if diagnostic_count > 0 {
    bail!(
      "Had {} fast check error{}.",
      diagnostic_count,
      if diagnostic_count == 1 { "" } else { "s" }
    )
  }
  Ok(())
}
