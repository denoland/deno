// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::error::AnyError;

use super::ConfigFix;
use super::DoctorCheck;
use super::DoctorContext;
use super::Finding;
use super::FindingSeverity;

/// Config-only unstable features that are valid in deno.json but are not
/// runtime features (kept in sync with the filtering in
/// `cli/args/mod.rs`).
const CONFIG_ONLY_UNSTABLE_FEATURES: &[&str] =
  &["fmt-component", "fmt-sql", "npm-lazy-caching"];

/// Reports entries in the deno.json `unstable` array that are not known
/// unstable features in this version of Deno — typically flags that have
/// been stabilized or removed. Such entries have no effect today, so
/// `--fix` deletes them without changing behavior.
pub struct UnstableFlagsCheck;

impl DoctorCheck for UnstableFlagsCheck {
  fn name(&self) -> &'static str {
    "unstable-flags"
  }

  fn run(&self, ctx: &DoctorContext) -> Result<Vec<Finding>, AnyError> {
    let mut findings = Vec::new();
    for deno_json in ctx.workspace.deno_jsons() {
      let stale_entries: Vec<String> = deno_json
        .json
        .unstable
        .iter()
        .filter(|entry| !is_known_unstable_feature(entry))
        .cloned()
        .collect();
      if stale_entries.is_empty() {
        continue;
      }
      let file = ctx.config_file_path(&deno_json.specifier)?;
      let file_display = ctx.display_path(&file);
      for entry in stale_entries {
        findings.push(Finding {
          id: "stale-unstable-flag",
          severity: FindingSeverity::Warning,
          message: format!(
            "\"unstable\" in {file_display} contains \"{entry}\", which is not a known unstable feature in this version of Deno (it may have been stabilized or removed). The entry has no effect.",
          ),
          file: file.clone(),
          fix_description: Some(format!(
            "remove \"{entry}\" from \"unstable\" (the entry is inert in the current version of Deno)",
          )),
          fix: Some(ConfigFix::RemoveUnstableEntries {
            entries: vec![entry],
          }),
        });
      }
    }
    Ok(findings)
  }
}

fn is_known_unstable_feature(name: &str) -> bool {
  deno_runtime::UNSTABLE_FEATURES
    .iter()
    .any(|feature| feature.name == name)
    || CONFIG_ONLY_UNSTABLE_FEATURES.contains(&name)
}
