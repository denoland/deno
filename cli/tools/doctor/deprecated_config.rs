// Copyright 2018-2026 the Deno authors. MIT license.

use deno_config::workspace::WorkspaceDiagnosticKind;
use deno_core::error::AnyError;
use jsonc_parser::cst::CstInputValue;

use super::ConfigFix;
use super::DoctorCheck;
use super::DoctorContext;
use super::Finding;
use super::FindingSeverity;

/// Reports deprecated config keys based on the workspace's own deprecation
/// diagnostics (the same ones printed as warnings by every command), so
/// detection cannot drift from what Deno actually deprecates. `--fix`
/// rewrites keys to their current form only when doing so is
/// behavior-preserving.
pub struct DeprecatedConfigCheck;

impl DoctorCheck for DeprecatedConfigCheck {
  fn name(&self) -> &'static str {
    "deprecated-config"
  }

  fn run(&self, ctx: &DoctorContext) -> Result<Vec<Finding>, AnyError> {
    let mut findings = Vec::new();
    for diagnostic in ctx.workspace.diagnostics() {
      let file = ctx.config_file_path(&diagnostic.config_url)?;
      let file_display = ctx.display_path(&file);
      match &diagnostic.kind {
        WorkspaceDiagnosticKind::DeprecatedNodeModulesDirOption {
          previous,
          suggestion,
        } => {
          findings.push(Finding {
            id: "deprecated-node-modules-dir-boolean",
            severity: FindingSeverity::Warning,
            message: format!(
              "{file_display} uses the boolean form `\"nodeModulesDir\": {previous}`, which is deprecated since Deno 2.0. Deno currently interprets it as \"{suggestion}\".",
              suggestion = suggestion.as_str(),
            ),
            file,
            fix_description: Some(format!(
              "replace `\"nodeModulesDir\": {previous}` with `\"nodeModulesDir\": \"{}\"` (the mode Deno currently derives from the boolean)",
              suggestion.as_str(),
            )),
            fix: Some(ConfigFix::SetRootKey {
              key: "nodeModulesDir",
              value: CstInputValue::String(suggestion.as_str().to_string()),
            }),
          });
        }
        WorkspaceDiagnosticKind::DeprecatedPatch => {
          findings.push(Finding {
            id: "deprecated-patch",
            severity: FindingSeverity::Warning,
            message: format!(
              "{file_display} uses \"patch\", which was renamed to \"links\". Deno currently still honors the old name.",
            ),
            file,
            fix_description: Some(
              "rename \"patch\" to \"links\" (the value is unchanged and Deno treats both the same today)".to_string(),
            ),
            fix: Some(ConfigFix::RenameRootKey {
              from: "patch",
              to: "links",
            }),
          });
        }
        WorkspaceDiagnosticKind::InvalidWorkspacesOption => {
          findings.push(Finding {
            id: "ignored-workspaces-option",
            severity: FindingSeverity::Warning,
            message: format!(
              "{file_display} has a \"workspaces\" field, which Deno ignores (the option is called \"workspace\"). This is not auto-fixed: renaming it would enable workspace resolution, which changes behavior. If a workspace is intended, rename it manually and verify the project still resolves as expected.",
            ),
            file,
            fix_description: None,
            fix: None,
          });
        }
        _ => {}
      }
    }
    Ok(findings)
  }
}
