// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::error::AnyError;
use deno_core::serde_json;

use super::ConfigFix;
use super::DoctorCheck;
use super::DoctorContext;
use super::Finding;
use super::FindingSeverity;

/// The lockfile version the current version of Deno writes.
const CURRENT_LOCKFILE_VERSION: &str = "5";

/// Reports a deno.lock written at an older lockfile version. Deno migrates
/// old lockfiles transparently the next time the lockfile is written;
/// `--fix` performs that write now, through the regular lockfile writing
/// path, so the file no longer depends on the auto-migration.
pub struct LockfileVersionCheck;

impl DoctorCheck for LockfileVersionCheck {
  fn name(&self) -> &'static str {
    "lockfile-version"
  }

  fn run(&self, ctx: &DoctorContext) -> Result<Vec<Finding>, AnyError> {
    let Some(lockfile_path) = ctx.workspace.resolve_lockfile_path()? else {
      return Ok(Vec::new());
    };
    let Ok(text) = std::fs::read_to_string(&lockfile_path) else {
      // No lockfile (or unreadable); nothing to report.
      return Ok(Vec::new());
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
      // An unparseable lockfile errors on every command already; doctor
      // doesn't add anything on top of that.
      return Ok(Vec::new());
    };
    // Version 1 lockfiles had no "version" field.
    let version = value
      .get("version")
      .and_then(|v| v.as_str())
      .unwrap_or("1")
      .to_string();
    if version == CURRENT_LOCKFILE_VERSION {
      return Ok(Vec::new());
    }
    let file_display = ctx.display_path(&lockfile_path);
    let is_old_version = matches!(version.as_str(), "1" | "2" | "3" | "4");
    if !is_old_version {
      return Ok(vec![Finding {
        id: "lockfile-unknown-version",
        severity: FindingSeverity::Warning,
        message: format!(
          "{file_display} is lockfile version {version}, which this version of Deno does not know (it writes version {CURRENT_LOCKFILE_VERSION}). It was likely created by a newer version of Deno.",
        ),
        file: lockfile_path,
        fix_description: None,
        fix: None,
      }]);
    }
    Ok(vec![Finding {
      id: "lockfile-old-version",
      severity: FindingSeverity::Info,
      message: format!(
        "{file_display} is lockfile version {version} (the \"version\" field in that file). The current version of Deno writes version {CURRENT_LOCKFILE_VERSION} and migrates old lockfiles automatically the next time the lockfile is written.",
      ),
      file: lockfile_path,
      fix_description: Some(format!(
        "rewrite the lockfile at version {CURRENT_LOCKFILE_VERSION} using the same migration Deno applies automatically on the next lockfile write",
      )),
      fix: Some(ConfigFix::MigrateLockfile),
    }])
  }
}
