// Copyright 2018-2026 the Deno authors. MIT license.

//! Implementation of the `deno doctor` subcommand.
//!
//! Doctor reports project configuration that relies on implicit defaults
//! which change in Deno 3 and, with `--fix`, pins the current behavior
//! explicitly in the config file so that upgrading to Deno 3 does not
//! change how the project runs. Fixes are conservative: they only ever
//! write out what the current version of Deno already does implicitly and
//! never emit configuration that changes behavior.

mod compiler_options;
mod deprecated_config;
mod heads_up;
mod jsr_deps;
mod lockfile;
mod node_modules_dir;
mod unstable_flags;

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_config::deno_json::NodeModulesDirMode;
use deno_config::workspace::Workspace;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_path_util::url_to_file_path;
use jsonc_parser::cst::CstInputValue;
use jsonc_parser::cst::CstObject;
use jsonc_parser::cst::CstRootNode;

use crate::args::DoctorFlags;
use crate::args::Flags;
use crate::colors;
use crate::factory::CliFactory;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingSeverity {
  Warning,
  Info,
}

/// A single edit to apply when `--fix` is passed. Every fix writes out
/// behavior the current version of Deno already exhibits.
#[derive(Debug)]
pub enum ConfigFix {
  /// Insert or overwrite a top-level key in a deno.json/deno.jsonc file.
  SetRootKey {
    key: &'static str,
    value: CstInputValue,
  },
  /// Insert or overwrite a key nested inside a top-level object (e.g.
  /// `compilerOptions.strict`), creating the parent object if needed.
  SetNestedKey {
    parent: &'static str,
    key: &'static str,
    value: CstInputValue,
  },
  /// Rename a top-level key, keeping its value and formatting.
  RenameRootKey {
    from: &'static str,
    to: &'static str,
  },
  /// Remove string entries from the top-level `unstable` array, removing
  /// the array itself when it becomes empty.
  RemoveUnstableEntries { entries: Vec<String> },
  /// Rewrite the lockfile through the regular lockfile writing path, which
  /// serializes it at the current lockfile version.
  MigrateLockfile,
}

#[derive(Debug)]
pub struct Finding {
  /// Stable machine-readable identifier of the kind of finding.
  pub id: &'static str,
  pub severity: FindingSeverity,
  /// Human-readable description, including the heuristic the conclusion
  /// was derived from so users can verify the reasoning.
  pub message: String,
  /// The file the finding derives from.
  pub file: PathBuf,
  /// Human-readable description of what `--fix` writes. `None` when the
  /// finding is not automatically fixable.
  pub fix_description: Option<String>,
  pub fix: Option<ConfigFix>,
}

/// A report-only note about a Deno 3 change that has no automatic fix and
/// no per-project detection. Rendered in a separate "heads-up" section and
/// never affects the `--check` exit code.
#[derive(Debug, serde::Serialize)]
pub struct Note {
  pub id: &'static str,
  pub message: String,
}

/// Everything a check provider may look at. Providers must not re-parse
/// config files by hand; detection goes through the resolved workspace.
pub struct DoctorContext {
  pub workspace: Arc<Workspace>,
  /// The node modules dir mode this project resolves to right now,
  /// including the implicit heuristics that apply when the config key is
  /// absent.
  pub node_modules_dir_mode: NodeModulesDirMode,
  /// Initial cwd, used to display paths relative to the invocation.
  pub initial_cwd: PathBuf,
}

impl DoctorContext {
  pub fn display_path(&self, path: &Path) -> String {
    match path.strip_prefix(&self.initial_cwd) {
      Ok(suffix) if suffix.components().next().is_some() => {
        suffix.display().to_string()
      }
      Ok(_) => path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string()),
      Err(_) => path.display().to_string(),
    }
  }

  pub fn config_file_path(
    &self,
    specifier: &deno_core::url::Url,
  ) -> Result<PathBuf, AnyError> {
    url_to_file_path(specifier)
      .with_context(|| format!("Invalid config file specifier: {specifier}"))
  }
}

/// A single isolated check. Checks only analyze and describe; all file
/// modification happens in the orchestrator's fix stage.
pub trait DoctorCheck {
  fn name(&self) -> &'static str;
  fn run(&self, ctx: &DoctorContext) -> Result<Vec<Finding>, AnyError>;
}

pub async fn doctor(
  flags: Arc<Flags>,
  doctor_flags: DoctorFlags,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let workspace = cli_options.workspace().clone();

  if workspace.root_deno_json().is_none() && workspace.root_pkg_json().is_none()
  {
    if doctor_flags.json {
      crate::display::write_json_to_stdout(&serde_json::json!({
        "findings": [],
        "notes": [],
      }))?;
    } else {
      log::info!(
        "No deno.json or package.json found in \"{}\". Nothing to check.",
        cli_options.initial_cwd().display()
      );
    }
    return Ok(0);
  }

  let ctx = DoctorContext {
    workspace: workspace.clone(),
    node_modules_dir_mode: factory
      .workspace_factory()?
      .node_modules_dir_mode()?,
    initial_cwd: cli_options.initial_cwd().to_path_buf(),
  };

  let checks: Vec<Box<dyn DoctorCheck>> = vec![
    Box::new(node_modules_dir::NodeModulesDirCheck),
    Box::new(compiler_options::CompilerOptionsCheck),
    Box::new(deprecated_config::DeprecatedConfigCheck),
    Box::new(unstable_flags::UnstableFlagsCheck),
    Box::new(jsr_deps::JsrDepsCheck),
    Box::new(lockfile::LockfileVersionCheck),
  ];

  let mut findings = Vec::new();
  for check in &checks {
    let mut check_findings = check
      .run(&ctx)
      .with_context(|| format!("Failed to run check \"{}\"", check.name()))?;
    findings.append(&mut check_findings);
  }
  let notes = heads_up::notes();

  let mut fixed = vec![false; findings.len()];
  if doctor_flags.fix {
    apply_fixes(&factory, &mut findings, &mut fixed).await?;
  }

  let unfixed_count = fixed.iter().filter(|fixed| !**fixed).count();

  if doctor_flags.json {
    print_json_report(&ctx, &findings, &fixed, &notes)?;
  } else if doctor_flags.fix {
    print_fix_report(&ctx, &findings, &fixed, &notes);
  } else {
    print_report(&ctx, &findings, &notes);
  }

  if doctor_flags.check && unfixed_count > 0 {
    return Ok(1);
  }
  Ok(0)
}

async fn apply_fixes(
  factory: &CliFactory,
  findings: &mut [Finding],
  fixed: &mut [bool],
) -> Result<(), AnyError> {
  // Group config-file edits by file so each file is parsed and written
  // exactly once, preserving formatting and comments via CST editing.
  let mut by_file: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
  for (i, finding) in findings.iter().enumerate() {
    match &finding.fix {
      Some(ConfigFix::MigrateLockfile) => {
        // The lockfile is not a config file; run it through the regular
        // lockfile writing path so migration behaves exactly like any
        // other command that writes the lockfile.
        if let Some(lockfile) = factory.maybe_lockfile().await? {
          lockfile.lock().has_content_changed = true;
          lockfile.write_if_changed()?;
          fixed[i] = true;
        }
      }
      Some(_) => {
        by_file.entry(finding.file.clone()).or_default().push(i);
      }
      None => {}
    }
  }

  for (path, indexes) in by_file {
    let text = std::fs::read_to_string(&path)
      .with_context(|| format!("Failed to read {}", path.display()))?;
    let cst = CstRootNode::parse(&text, &Default::default())
      .with_context(|| format!("Failed to parse {}", path.display()))?;
    let root = cst.object_value_or_set();
    let root_was_empty = root.properties().is_empty();
    for i in indexes {
      let fix = findings[i].fix.as_ref().unwrap();
      apply_config_fix(&root, fix)?;
      fixed[i] = true;
    }
    if root_was_empty {
      root.ensure_multiline();
    }
    let new_text = cst.to_string();
    if new_text != text {
      std::fs::write(&path, new_text)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    }
  }
  Ok(())
}

fn apply_config_fix(root: &CstObject, fix: &ConfigFix) -> Result<(), AnyError> {
  match fix {
    ConfigFix::SetRootKey { key, value } => {
      set_object_key(root, key, value.clone());
    }
    ConfigFix::SetNestedKey { parent, key, value } => {
      let parent_obj = root.object_value_or_set(parent);
      set_object_key(&parent_obj, key, value.clone());
    }
    ConfigFix::RenameRootKey { from, to } => {
      if let Some(prop) = root.get(from)
        && let Some(name) = prop.name()
        && let Some(string_lit) = name.as_string_lit()
      {
        string_lit.set_raw_value(format!("\"{to}\""));
      }
    }
    ConfigFix::RemoveUnstableEntries { entries } => {
      if let Some(array) = root.array_value("unstable") {
        for element in array.elements() {
          let is_stale = element
            .as_string_lit()
            .and_then(|s| s.decoded_value().ok())
            .is_some_and(|value| entries.contains(&value));
          if is_stale {
            element.remove();
          }
        }
        if array.elements().is_empty()
          && let Some(prop) = root.get("unstable")
        {
          prop.remove();
        }
      }
    }
    ConfigFix::MigrateLockfile => {
      // handled in apply_fixes
    }
  }
  Ok(())
}

fn set_object_key(object: &CstObject, key: &str, value: CstInputValue) {
  match object.get(key) {
    Some(prop) => {
      prop.set_value(value);
    }
    None => {
      object.append(key, value);
    }
  }
}

fn print_report(ctx: &DoctorContext, findings: &[Finding], notes: &[Note]) {
  if findings.is_empty() {
    log::info!(
      "No findings. This project does not rely on implicit configuration that changes in Deno 3."
    );
  } else {
    let mut by_file: BTreeMap<String, Vec<&Finding>> = BTreeMap::new();
    for finding in findings {
      by_file
        .entry(ctx.display_path(&finding.file))
        .or_default()
        .push(finding);
    }
    for (file, file_findings) in by_file {
      log::info!("{}", colors::cyan(&file));
      for finding in file_findings {
        let severity = match finding.severity {
          FindingSeverity::Warning => colors::yellow("warning"),
          FindingSeverity::Info => colors::intense_blue("info"),
        };
        log::info!("  {} [{}] {}", severity, finding.id, finding.message);
        if let Some(fix_description) = &finding.fix_description {
          log::info!("    fix: {}", fix_description);
        }
      }
    }
  }

  print_notes(notes);

  if !findings.is_empty() {
    let fixable = findings.iter().filter(|f| f.fix.is_some()).count();
    log::info!(
      "\nFound {} {} ({} fixable with {}).",
      findings.len(),
      if findings.len() == 1 {
        "finding"
      } else {
        "findings"
      },
      fixable,
      colors::bold("deno doctor --fix"),
    );
  }
}

fn print_fix_report(
  ctx: &DoctorContext,
  findings: &[Finding],
  fixed: &[bool],
  notes: &[Note],
) {
  let mut by_file: BTreeMap<String, Vec<&Finding>> = BTreeMap::new();
  for (finding, fixed) in findings.iter().zip(fixed) {
    if *fixed {
      by_file
        .entry(ctx.display_path(&finding.file))
        .or_default()
        .push(finding);
    }
  }
  let fixed_count = fixed.iter().filter(|f| **f).count();
  for (file, file_findings) in &by_file {
    log::info!("{}", colors::cyan(file));
    for finding in file_findings {
      let marker = match finding.fix.as_ref().unwrap() {
        ConfigFix::SetRootKey { .. } | ConfigFix::SetNestedKey { .. } => "+",
        ConfigFix::RenameRootKey { .. } | ConfigFix::MigrateLockfile => "~",
        ConfigFix::RemoveUnstableEntries { .. } => "-",
      };
      log::info!(
        "  {} {}",
        colors::green(marker),
        finding.fix_description.as_deref().unwrap_or(finding.id),
      );
    }
  }

  let unfixed: Vec<&Finding> = findings
    .iter()
    .zip(fixed)
    .filter(|(_, fixed)| !**fixed)
    .map(|(f, _)| f)
    .collect();
  if !unfixed.is_empty() {
    log::info!("\nNot fixable automatically:");
    for finding in unfixed {
      log::info!(
        "  [{}] {} ({})",
        finding.id,
        finding.message,
        ctx.display_path(&finding.file)
      );
    }
  }

  print_notes(notes);

  if fixed_count == 0 {
    log::info!("\nNo fixes to apply.");
  } else {
    log::info!(
      "\nApplied {} {} to {} {}.",
      fixed_count,
      if fixed_count == 1 { "fix" } else { "fixes" },
      by_file.len(),
      if by_file.len() == 1 { "file" } else { "files" },
    );
  }
}

fn print_notes(notes: &[Note]) {
  if notes.is_empty() {
    return;
  }
  log::info!("\nDeno 3 heads-up (report only, no fixes available yet):");
  for note in notes {
    log::info!("  - {}", note.message);
  }
}

fn print_json_report(
  ctx: &DoctorContext,
  findings: &[Finding],
  fixed: &[bool],
  notes: &[Note],
) -> Result<(), AnyError> {
  let findings_json: Vec<serde_json::Value> = findings
    .iter()
    .zip(fixed)
    .map(|(finding, fixed)| {
      serde_json::json!({
        "id": finding.id,
        "severity": finding.severity,
        "message": finding.message,
        "file": ctx.display_path(&finding.file),
        "fixable": finding.fix.is_some(),
        "fix_description": finding.fix_description,
        "fixed": fixed,
      })
    })
    .collect();
  crate::display::write_json_to_stdout(&serde_json::json!({
    "findings": findings_json,
    "notes": notes,
  }))
}
