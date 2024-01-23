// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::collections::VecDeque;

use deno_ast::ModuleSpecifier;
use deno_config::ConfigFile;
use deno_config::WorkspaceConfig;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_graph::FastCheckDiagnostic;
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
  packages: &[MemberRoots],
) -> Result<(), AnyError> {
  let mut diagnostic_count = 0;
  let mut seen_diagnostics = HashSet::new();
  let mut seen_modules = HashSet::with_capacity(graph.specifiers_count());
  for package in packages {
    let mut pending = VecDeque::new();
    for export in &package.exports {
      if seen_modules.insert(export.clone()) {
        pending.push_back(export.clone());
      }
    }

    'analyze_package: while let Some(specifier) = pending.pop_front() {
      let Ok(Some(module)) = graph.try_get_prefer_types(&specifier) else {
        continue;
      };
      let Some(esm_module) = module.esm() else {
        continue;
      };
      if let Some(diagnostic) = esm_module.fast_check_diagnostic() {
        for diagnostic in diagnostic.flatten_multiple() {
          if matches!(
            diagnostic,
            FastCheckDiagnostic::UnsupportedJavaScriptEntrypoint { .. }
          ) {
            // ignore JS packages for fast check
            log::warn!(
              concat!(
                "{} Package '{}' is a JavaScript package without a corresponding ",
                "declaration file. This may lead to a non-optimal experience for ",
                "users of your package. For performance reasons, it's recommended ",
                "to ship a corresponding TypeScript declaration file or to ",
                "convert to TypeScript.",
              ),
              deno_runtime::colors::yellow("Warning"),
              package.name,
            );
            break 'analyze_package; // no need to keep analyzing this package
          } else {
            let message = diagnostic.message_with_range_for_test();
            if !seen_diagnostics.insert(message.clone()) {
              continue;
            }

            log::error!("\n{}", message);
            diagnostic_count += 1;
          }
        }
      }

      // analyze the next dependencies
      for dep in esm_module.dependencies_prefer_fast_check().values() {
        let Some(specifier) = graph.resolve_dependency_from_dep(dep, true)
        else {
          continue;
        };

        let dep_in_same_package =
          specifier.as_str().starts_with(package.dir_url.as_str());
        if dep_in_same_package {
          let is_new = seen_modules.insert(specifier.clone());
          if is_new {
            pending.push_back(specifier.clone());
          }
        }
      }
    }
  }

  if diagnostic_count > 0 {
    // for the time being, tell the user why we have these errors and the benefit they bring
    log::error!(
      concat!(
        "\nFixing these fast check errors is required to make the code fast check compatible ",
        "which enables type checking your package's TypeScript code with the same ",
        "performance as if you had distributed declaration files. Do any of these ",
        "errors seem too restrictive or incorrect? Please open an issue if so to ",
        "help us improve: https://github.com/denoland/deno/issues\n",
      )
    );
    bail!(
      "Had {} fast check error{}.",
      diagnostic_count,
      if diagnostic_count == 1 { "" } else { "s" }
    )
  }
  Ok(())
}
