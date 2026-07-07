// Copyright 2018-2026 the Deno authors. MIT license.

use deno_config::deno_json::NodeModulesDirMode;
use deno_core::error::AnyError;
use jsonc_parser::cst::CstInputValue;

use super::ConfigFix;
use super::DoctorCheck;
use super::DoctorContext;
use super::Finding;
use super::FindingSeverity;

/// Reports when a project combines `jsr:` dependencies with a node_modules
/// directory but has no explicit `jsrDepsInNodeModules` setting. Deno
/// currently defaults to keeping jsr packages out of node_modules; `--fix`
/// pins that current default so a future default flip cannot change the
/// project's install layout.
pub struct JsrDepsCheck;

impl DoctorCheck for JsrDepsCheck {
  fn name(&self) -> &'static str {
    "jsr-deps-in-node-modules"
  }

  fn run(&self, ctx: &DoctorContext) -> Result<Vec<Finding>, AnyError> {
    let Some(root_deno_json) = ctx.workspace.root_deno_json() else {
      return Ok(Vec::new());
    };
    if root_deno_json.json.jsr_deps_in_node_modules.is_some() {
      // Already pinned explicitly.
      return Ok(Vec::new());
    }
    if ctx.node_modules_dir_mode == NodeModulesDirMode::None {
      // The setting only has an effect when a node_modules directory is in
      // use.
      return Ok(Vec::new());
    }
    // Detect `jsr:` dependencies from the workspace's import maps (module
    // graph scanning is deliberately out of scope for doctor).
    let has_jsr_deps = ctx.workspace.deno_jsons().any(|deno_json| {
      deno_json
        .json
        .imports
        .as_ref()
        .and_then(|imports| imports.as_object())
        .is_some_and(|imports| {
          imports.values().any(|value| {
            value
              .as_str()
              .is_some_and(|value| value.starts_with("jsr:"))
          })
        })
    });
    if !has_jsr_deps {
      return Ok(Vec::new());
    }

    let file = ctx.config_file_path(&root_deno_json.specifier)?;
    let file_display = ctx.display_path(&file);
    Ok(vec![Finding {
      id: "jsr-deps-in-node-modules-not-pinned",
      severity: FindingSeverity::Warning,
      message: format!(
        "\"jsrDepsInNodeModules\" is not set in {file_display}, but this project uses jsr: dependencies (found in \"imports\") together with a node_modules directory (mode \"{mode}\"). Deno currently defaults to keeping jsr: packages out of node_modules.",
        mode = ctx.node_modules_dir_mode.as_str(),
      ),
      file,
      fix_description: Some(
        "set \"jsrDepsInNodeModules\": false (pins Deno's current default of not installing jsr: packages into node_modules)"
          .to_string(),
      ),
      fix: Some(ConfigFix::SetRootKey {
        key: "jsrDepsInNodeModules",
        value: CstInputValue::Bool(false),
      }),
    }])
  }
}
