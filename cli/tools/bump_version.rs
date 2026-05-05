// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use deno_config::workspace::JsrPackageConfig;
use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_semver::SmallStackString;
use deno_semver::Version;
use jsonc_parser::cst::CstObject;
use jsonc_parser::cst::CstRootNode;
use jsonc_parser::json;
use log::info;

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::VersionFlags;
use crate::args::VersionIncrement;
use crate::factory::CliFactory;

struct ConfigUpdater {
  cst: CstRootNode,
  root_object: CstObject,
  path: PathBuf,
  modified: bool,
}

impl ConfigUpdater {
  fn new(config_file_path: PathBuf) -> Result<Self, AnyError> {
    let config_file_contents = std::fs::read_to_string(&config_file_path)
      .with_context(|| {
        format!("Reading config file '{}'", config_file_path.display())
      })?;
    let cst = CstRootNode::parse(&config_file_contents, &Default::default())
      .with_context(|| {
        format!("Parsing config file '{}'", config_file_path.display())
      })?;
    let root_object = cst.object_value_or_set();
    Ok(Self {
      cst,
      root_object,
      path: config_file_path,
      modified: false,
    })
  }

  fn display_path(&self) -> String {
    deno_path_util::url_from_file_path(&self.path)
      .map(|u| u.to_string())
      .unwrap_or_else(|_| self.path.display().to_string())
  }

  fn get_version(&self) -> Option<String> {
    self
      .root_object
      .get("version")?
      .value()?
      .as_string_lit()
      .and_then(|s| s.decoded_value().ok())
  }

  fn set_version(&mut self, version: &str) {
    let version_prop = self.root_object.get("version");
    match version_prop {
      Some(prop) => {
        prop.set_value(json!(version));
        self.modified = true;
      }
      None => {
        // Insert the version property at the beginning for better organization
        self.root_object.insert(0, "version", json!(version));
        self.modified = true;
      }
    }
  }

  fn commit(&self) -> Result<(), AnyError> {
    if !self.modified {
      return Ok(());
    }

    let new_text = self.cst.to_string();
    std::fs::write(&self.path, new_text).with_context(|| {
      format!("failed writing to '{}'", self.path.display())
    })?;
    Ok(())
  }
}

fn increment_version(
  current: &Version,
  increment: &VersionIncrement,
) -> Version {
  match increment {
    VersionIncrement::Major => Version {
      major: current.major + 1,
      minor: 0,
      patch: 0,
      pre: Default::default(),
      build: Default::default(),
    },
    VersionIncrement::Minor => Version {
      major: current.major,
      minor: current.minor + 1,
      patch: 0,
      pre: Default::default(),
      build: Default::default(),
    },
    VersionIncrement::Patch => Version {
      major: current.major,
      minor: current.minor,
      patch: current.patch + 1,
      pre: Default::default(),
      build: Default::default(),
    },
    VersionIncrement::Premajor => {
      let mut v = Version {
        major: current.major + 1,
        minor: 0,
        patch: 0,
        ..Default::default()
      };
      v.pre = vec![SmallStackString::from_static("0")].into();
      v
    }
    VersionIncrement::Preminor => {
      let mut v = Version {
        major: current.major,
        minor: current.minor + 1,
        patch: 0,
        ..Default::default()
      };
      v.pre = vec![SmallStackString::from_static("0")].into();
      v
    }
    VersionIncrement::Prepatch => {
      let mut v = Version {
        major: current.major,
        minor: current.minor,
        patch: current.patch + 1,
        ..Default::default()
      };
      v.pre = vec![SmallStackString::from_static("0")].into();
      v
    }
    VersionIncrement::Prerelease => {
      let mut v = current.clone();
      if v.pre.is_empty() {
        v.patch += 1;
        v.pre = vec![SmallStackString::from_static("0")].into();
      } else {
        let mut pre_vec = v.pre.iter().cloned().collect::<Vec<_>>();
        if let Some(last) = pre_vec.last_mut() {
          if let Ok(num) = last.parse::<u64>() {
            *last = SmallStackString::from_string((num + 1).to_string());
          } else {
            pre_vec.push(SmallStackString::from_static("0"));
          }
        }
        v.pre = pre_vec.into();
      }
      v
    }
  }
}

fn load_single_config(
  cli_options: &CliOptions,
) -> Result<ConfigUpdater, AnyError> {
  let start_dir = &cli_options.start_dir;

  // Check for deno.json first - it takes priority
  if let Some(deno_json) = start_dir.member_deno_json() {
    let config_path = deno_path_util::url_to_file_path(&deno_json.specifier)
      .context("Failed to convert deno.json URL to path")?;
    return ConfigUpdater::new(config_path);
  } else if let Some(pkg_json) = start_dir.member_pkg_json() {
    // Only fall back to package.json if deno.json doesn't exist
    return ConfigUpdater::new(pkg_json.path.clone());
  }

  bail!("No deno.json or package.json found in the current directory")
}

#[allow(clippy::print_stdout, reason = "user-facing output")]
pub fn bump_version_command(
  flags: Arc<Flags>,
  version_flags: VersionFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;

  let workspace = cli_options.workspace();
  let at_workspace_root =
    cli_options.start_dir.dir_url() == workspace.root_dir_url();
  let jsr_pkg_count = workspace.jsr_packages().count();
  let workspace_mode = match version_flags.workspace {
    Some(b) => b,
    None => at_workspace_root && jsr_pkg_count > 1,
  };

  if workspace_mode {
    bump_workspace(cli_options, &version_flags)
  } else {
    bump_single(cli_options, &version_flags)
  }
}

#[allow(clippy::print_stdout, reason = "user-facing output")]
fn bump_single(
  cli_options: &CliOptions,
  version_flags: &VersionFlags,
) -> Result<(), AnyError> {
  let mut config = load_single_config(cli_options)?;

  let current_version = if let Some(version_str) = config.get_version() {
    Version::parse_standard(&version_str).with_context(|| {
      format!(
        "Failed to parse version '{}' in {}",
        version_str,
        config.display_path()
      )
    })?
  } else {
    if version_flags.increment.is_none() {
      println!("No version found in configuration file");
      return Ok(());
    }
    // Default to 0.1.0 if no version is found but increment is specified
    info!("No version found, defaulting to 0.1.0");
    Version::parse_standard("0.1.0")
      .with_context(|| "Failed to create default version")?
  };

  let new_version = match &version_flags.increment {
    Some(increment) => increment_version(&current_version, increment),
    None => {
      println!("{}", current_version);
      return Ok(());
    }
  };

  if version_flags.dry_run {
    println!(
      "[dry-run] {}: {} -> {}",
      config.display_path(),
      current_version,
      new_version
    );
    return Ok(());
  }

  config.set_version(&new_version.to_string());
  config.commit()?;

  println!("{}", new_version);
  info!(
    "Version updated from {} to {} in {}",
    current_version,
    new_version,
    config.display_path()
  );
  Ok(())
}

// ---------------------------------------------------------------------------
// Workspace mode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BumpKind {
  Patch,
  Minor,
  Major,
}

impl BumpKind {
  fn label(self) -> &'static str {
    match self {
      BumpKind::Patch => "patch",
      BumpKind::Minor => "minor",
      BumpKind::Major => "major",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AppliedDiff {
  Bump(BumpKind),
  Prerelease,
}

impl AppliedDiff {
  fn label(&self) -> &'static str {
    match self {
      AppliedDiff::Bump(k) => k.label(),
      AppliedDiff::Prerelease => "prerelease",
    }
  }
}

struct WorkspacePackage {
  name: String,
  current_version: Version,
  config_path: PathBuf,
  /// Path relative to the repo root, for `git show <ref>:<path>`.
  config_path_relative: String,
}

#[allow(clippy::print_stdout, reason = "user-facing output")]
fn bump_workspace(
  cli_options: &CliOptions,
  version_flags: &VersionFlags,
) -> Result<(), AnyError> {
  let workspace = cli_options.workspace();
  let root_dir = workspace.root_dir_path();
  let pkgs =
    collect_workspace_packages(workspace.jsr_packages().collect(), &root_dir)?;

  if pkgs.is_empty() {
    bail!(
      "Workspace mode: no member packages with both `name` and `version` were found.\n\
       Workspace root: {}",
      workspace.root_dir_url()
    );
  }

  match &version_flags.increment {
    Some(increment) => {
      bump_workspace_explicit(cli_options, version_flags, &pkgs, increment)
    }
    None => bump_workspace_conventional(cli_options, version_flags, &pkgs),
  }
}

fn collect_workspace_packages(
  jsr_packages: Vec<JsrPackageConfig>,
  root_dir: &Path,
) -> Result<Vec<WorkspacePackage>, AnyError> {
  let mut out = Vec::new();
  for pkg in jsr_packages {
    let Some(version_str) = pkg.config_file.json.version.as_ref() else {
      continue;
    };
    let version = Version::parse_standard(version_str).with_context(|| {
      format!(
        "Failed to parse version '{}' for package '{}'",
        version_str, pkg.name
      )
    })?;
    let config_path =
      deno_path_util::url_to_file_path(&pkg.config_file.specifier)
        .context("Failed to convert deno.json URL to path")?;
    let config_path_relative = config_path
      .strip_prefix(root_dir)
      .unwrap_or(&config_path)
      .to_string_lossy()
      .replace('\\', "/");
    out.push(WorkspacePackage {
      name: pkg.name.clone(),
      current_version: version,
      config_path,
      config_path_relative,
    });
  }
  Ok(out)
}

#[allow(clippy::print_stdout, reason = "user-facing output")]
fn bump_workspace_explicit(
  cli_options: &CliOptions,
  version_flags: &VersionFlags,
  pkgs: &[WorkspacePackage],
  increment: &VersionIncrement,
) -> Result<(), AnyError> {
  let mut updates: Vec<(String, Version, Version, PathBuf)> = Vec::new();
  for pkg in pkgs {
    let new_version = increment_version(&pkg.current_version, increment);
    updates.push((
      pkg.name.clone(),
      pkg.current_version.clone(),
      new_version,
      pkg.config_path.clone(),
    ));
  }

  print_update_table(&updates);

  if version_flags.dry_run {
    println!("[dry-run] Skipping file writes.");
    return Ok(());
  }

  for (_, _, new_version, path) in &updates {
    let mut updater = ConfigUpdater::new(path.clone())?;
    updater.set_version(&new_version.to_string());
    updater.commit()?;
  }

  rewrite_import_map(cli_options, version_flags, &updates)?;

  println!("Bumped {} package(s).", updates.len());
  Ok(())
}

#[allow(clippy::print_stdout, reason = "user-facing output")]
fn bump_workspace_conventional(
  cli_options: &CliOptions,
  version_flags: &VersionFlags,
  pkgs: &[WorkspacePackage],
) -> Result<(), AnyError> {
  let workspace = cli_options.workspace();
  let root_dir = workspace.root_dir_path();

  let start = match &version_flags.start {
    Some(s) => s.clone(),
    None => {
      let out = run_git(
        &root_dir,
        &["describe", "--tags", "--abbrev=0"],
      )
      .context(
        "Failed to determine starting ref. Pass `--start <ref>` explicitly, or ensure the repository has at least one tag.",
      )?;
      out.trim().to_string()
    }
  };
  let base = match &version_flags.base {
    Some(b) => b.clone(),
    None => {
      let out = run_git(&root_dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .context("Failed to determine current branch")?;
      let trimmed = out.trim();
      if trimmed.is_empty() || trimmed == "HEAD" {
        bail!(
          "Could not determine the current branch. Pass `--base <ref>` explicitly."
        );
      }
      trimmed.to_string()
    }
  };

  println!(
    "Reading commits between {} and {} in {}",
    start,
    base,
    root_dir.display()
  );

  let pkg_names: BTreeSet<String> =
    pkgs.iter().map(|p| p.name.clone()).collect();
  let commits = read_commits(&root_dir, &start, &base)?;
  println!("Found {} commits.", commits.len());

  let mut bumps_by_pkg: BTreeMap<String, BumpKind> = BTreeMap::new();
  let mut commits_by_pkg: BTreeMap<String, Vec<CommitWithTag>> =
    BTreeMap::new();
  let mut diagnostics: Vec<Diagnostic> = Vec::new();

  for commit in &commits {
    if is_release_commit(&commit.subject) {
      continue;
    }
    match parse_commit(commit, &pkg_names) {
      ParsedCommit::Bumps(bumps) => {
        for b in bumps {
          let entry = bumps_by_pkg.entry(b.module.clone()).or_insert(b.bump);
          if b.bump > *entry {
            *entry = b.bump;
          }
          commits_by_pkg.entry(b.module.clone()).or_default().push(
            CommitWithTag {
              commit: commit.clone(),
              tag: b.tag,
            },
          );
        }
      }
      ParsedCommit::Diagnostic(d) => diagnostics.push(d),
    }
  }

  // Detect manual version changes by comparing the version at the start ref
  // with the current version on disk. If someone manually bumped a version
  // between releases, use that change as-is instead of computing from commits.
  let mut manual_changes: BTreeMap<String, (Version, Version)> =
    BTreeMap::new();
  for pkg in pkgs.iter() {
    if let Some(old_version) =
      read_version_at_ref(&root_dir, &start, &pkg.config_path_relative)
        .filter(|v| *v != pkg.current_version)
    {
      println!(
        "Detected manual version change for {}: {} -> {}",
        pkg.name, old_version, pkg.current_version
      );
      manual_changes
        .insert(pkg.name.clone(), (old_version, pkg.current_version.clone()));
    }
  }

  if bumps_by_pkg.is_empty() && manual_changes.is_empty() {
    println!("No version bumps inferred from commits.");
    return Ok(());
  }

  let mut updates: Vec<(String, Version, Version, PathBuf)> = Vec::new();
  let mut applied_diff_by_pkg: BTreeMap<String, AppliedDiff> = BTreeMap::new();
  // Track which packages were manually changed so we skip writing their version.
  let mut manually_bumped: BTreeSet<String> = BTreeSet::new();
  for pkg in pkgs {
    // Manual changes take precedence over commit-derived bumps.
    if let Some((old_version, new_version)) = manual_changes.get(&pkg.name) {
      let applied = diff_from_versions(old_version, new_version);
      applied_diff_by_pkg.insert(pkg.name.clone(), applied);
      updates.push((
        pkg.name.clone(),
        old_version.clone(),
        new_version.clone(),
        pkg.config_path.clone(),
      ));
      manually_bumped.insert(pkg.name.clone());
      continue;
    }
    let Some(intended_bump) = bumps_by_pkg.get(&pkg.name) else {
      continue;
    };
    let (new_version, applied) =
      apply_conventional_bump(&pkg.current_version, *intended_bump);
    applied_diff_by_pkg.insert(pkg.name.clone(), applied);
    updates.push((
      pkg.name.clone(),
      pkg.current_version.clone(),
      new_version,
      pkg.config_path.clone(),
    ));
  }

  // Sort updates alphabetically by package name for stable output.
  updates.sort_by(|a, b| a.0.cmp(&b.0));

  print_update_table_with_diff(&updates, &applied_diff_by_pkg);

  if !diagnostics.is_empty() {
    println!("Diagnostics ({}):", diagnostics.len());
    for d in &diagnostics {
      println!("  {} {}", d.kind.label(), d.commit.subject);
    }
  }

  let release_note = create_release_note(
    &updates,
    &applied_diff_by_pkg,
    &commits_by_pkg,
    chrono_today(),
  );

  if version_flags.dry_run {
    println!("\n[dry-run] Release note:\n{}", release_note);
    return Ok(());
  }

  for (name, _, new_version, path) in &updates {
    // Skip writing versions that were already manually changed on disk.
    if !manually_bumped.contains(name) {
      let mut updater = ConfigUpdater::new(path.clone())?;
      updater.set_version(&new_version.to_string());
      updater.commit()?;
    }
  }

  rewrite_import_map(cli_options, version_flags, &updates)?;

  let release_notes_path = version_flags
    .release_notes
    .clone()
    .unwrap_or_else(|| "Releases.md".to_string());
  let release_notes_full = if Path::new(&release_notes_path).is_absolute() {
    PathBuf::from(&release_notes_path)
  } else {
    root_dir.join(&release_notes_path)
  };
  prepend_release_notes(&release_notes_full, &release_note)?;

  println!(
    "Bumped {} package(s). Release note prepended to {}.",
    updates.len(),
    release_notes_full.display()
  );
  Ok(())
}

#[allow(clippy::print_stdout, reason = "user-facing output")]
fn print_update_table(updates: &[(String, Version, Version, PathBuf)]) {
  if updates.is_empty() {
    return;
  }
  let name_w = updates.iter().map(|u| u.0.len()).max().unwrap_or(8).max(8);
  let from_w = updates
    .iter()
    .map(|u| u.1.to_string().len())
    .max()
    .unwrap_or(7)
    .max(7);
  let to_w = updates
    .iter()
    .map(|u| u.2.to_string().len())
    .max()
    .unwrap_or(7)
    .max(7);
  println!(
    "{:<name_w$}  {:<from_w$}  {:<to_w$}",
    "package",
    "from",
    "to",
    name_w = name_w,
    from_w = from_w,
    to_w = to_w,
  );
  for (name, from, to, _) in updates {
    println!(
      "{:<name_w$}  {:<from_w$}  {:<to_w$}",
      name,
      from.to_string(),
      to.to_string(),
      name_w = name_w,
      from_w = from_w,
      to_w = to_w,
    );
  }
}

#[allow(clippy::print_stdout, reason = "user-facing output")]
fn print_update_table_with_diff(
  updates: &[(String, Version, Version, PathBuf)],
  diffs: &BTreeMap<String, AppliedDiff>,
) {
  if updates.is_empty() {
    return;
  }
  let name_w = updates.iter().map(|u| u.0.len()).max().unwrap_or(8).max(8);
  let from_w = updates
    .iter()
    .map(|u| u.1.to_string().len())
    .max()
    .unwrap_or(7)
    .max(7);
  let to_w = updates
    .iter()
    .map(|u| u.2.to_string().len())
    .max()
    .unwrap_or(7)
    .max(7);
  println!(
    "{:<name_w$}  {:<from_w$}  {:<to_w$}  type",
    "package",
    "from",
    "to",
    name_w = name_w,
    from_w = from_w,
    to_w = to_w,
  );
  for (name, from, to, _) in updates {
    let diff_label = diffs.get(name).map(|d| d.label()).unwrap_or("");
    println!(
      "{:<name_w$}  {:<from_w$}  {:<to_w$}  {}",
      name,
      from.to_string(),
      to.to_string(),
      diff_label,
      name_w = name_w,
      from_w = from_w,
      to_w = to_w,
    );
  }
}

// ---------------------------------------------------------------------------
// Conventional commits
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(dead_code, reason = "fields kept for future PR-body generation")]
struct Commit {
  hash: String,
  subject: String,
  body: String,
}

#[derive(Debug, Clone)]
struct CommitWithTag {
  commit: Commit,
  tag: String,
}

#[derive(Debug, Clone)]
struct ParsedBump {
  module: String,
  tag: String,
  bump: BumpKind,
}

#[derive(Debug, Clone)]
enum DiagnosticKind {
  UnknownCommit,
  UnknownModule,
  MissingScope,
  Skipped,
}

impl DiagnosticKind {
  fn label(&self) -> &'static str {
    match self {
      DiagnosticKind::UnknownCommit => "unknown_commit",
      DiagnosticKind::UnknownModule => "unknown_module",
      DiagnosticKind::MissingScope => "missing_scope",
      DiagnosticKind::Skipped => "skipped",
    }
  }
}

#[derive(Debug, Clone)]
struct Diagnostic {
  kind: DiagnosticKind,
  commit: Commit,
}

enum ParsedCommit {
  Bumps(Vec<ParsedBump>),
  Diagnostic(Diagnostic),
}

fn tag_to_bump(tag: &str) -> Option<BumpKind> {
  match tag {
    "BREAKING" => Some(BumpKind::Major),
    "feat" => Some(BumpKind::Minor),
    "fix" | "perf" | "docs" | "deprecation" | "refactor" | "test" | "style"
    | "chore" => Some(BumpKind::Patch),
    _ => None,
  }
}

const SCOPE_REQUIRED_TAGS: &[&str] =
  &["BREAKING", "feat", "fix", "perf", "deprecation"];

fn is_release_commit(subject: &str) -> bool {
  // Skip commits whose subject looks like a version bump or a release marker.
  let s = subject.trim_start();
  let release_re = regex::Regex::new(r"^v?\d+\.\d+\.\d+").unwrap();
  let release_word_re = regex::Regex::new(r"^Release \d+\.\d+\.\d+").unwrap();
  release_re.is_match(s) || release_word_re.is_match(s)
}

/// Parse a commit subject of the form `<tag>(<scopes,...>)<!>: <message>`.
fn parse_commit(commit: &Commit, pkg_names: &BTreeSet<String>) -> ParsedCommit {
  let re =
    regex::Regex::new(r"^([^:()]+)(?:\(([^)]+)\))?(\!)?: (.*)$").unwrap();
  let Some(caps) = re.captures(&commit.subject) else {
    return ParsedCommit::Diagnostic(Diagnostic {
      kind: DiagnosticKind::UnknownCommit,
      commit: commit.clone(),
    });
  };
  let tag = caps.get(1).unwrap().as_str().trim().to_string();
  let scopes_raw = caps.get(2).map(|m| m.as_str().to_string());
  let bang = caps.get(3).is_some();

  let scopes: Vec<String> = match scopes_raw {
    Some(s) if s.trim() == "*" => Vec::new(), // wildcard - resolved below
    Some(s) => s
      .split(',')
      .map(|p| p.trim().to_string())
      .filter(|p| !p.is_empty())
      .collect(),
    None => Vec::new(),
  };

  let wildcard = matches!(caps.get(2).map(|m| m.as_str().trim()), Some("*"));

  let base_bump = if bang {
    BumpKind::Major
  } else {
    match tag_to_bump(&tag) {
      Some(b) => b,
      None => {
        return ParsedCommit::Diagnostic(Diagnostic {
          kind: DiagnosticKind::UnknownCommit,
          commit: commit.clone(),
        });
      }
    }
  };

  let modules: Vec<String> = if wildcard {
    pkg_names.iter().cloned().collect()
  } else {
    scopes
  };

  if modules.is_empty() {
    let kind = if SCOPE_REQUIRED_TAGS.contains(&tag.as_str()) {
      DiagnosticKind::MissingScope
    } else {
      DiagnosticKind::Skipped
    };
    return ParsedCommit::Diagnostic(Diagnostic {
      kind,
      commit: commit.clone(),
    });
  }

  let unstable_re =
    regex::Regex::new(r"^(?:unstable/(.+)|(.+)/unstable)$").unwrap();

  let mut bumps = Vec::new();
  let mut had_unknown = false;
  for module in modules {
    let (resolved, force_patch) =
      if let Some(caps) = unstable_re.captures(&module) {
        let inner = caps
          .get(1)
          .or_else(|| caps.get(2))
          .map(|m| m.as_str().to_string())
          .unwrap_or_default();
        (inner, true)
      } else {
        (module.clone(), false)
      };

    let resolved_full = match resolve_module_name(&resolved, pkg_names) {
      Some(name) => name,
      None => {
        had_unknown = true;
        continue;
      }
    };

    let bump = if force_patch {
      BumpKind::Patch
    } else {
      base_bump
    };
    bumps.push(ParsedBump {
      module: resolved_full,
      tag: tag.clone(),
      bump,
    });
  }

  if bumps.is_empty() {
    return ParsedCommit::Diagnostic(Diagnostic {
      kind: if had_unknown {
        DiagnosticKind::UnknownModule
      } else {
        DiagnosticKind::Skipped
      },
      commit: commit.clone(),
    });
  }

  ParsedCommit::Bumps(bumps)
}

fn resolve_module_name(
  needle: &str,
  pkg_names: &BTreeSet<String>,
) -> Option<String> {
  if pkg_names.contains(needle) {
    return Some(needle.to_string());
  }
  let suffix = format!("/{}", needle);
  pkg_names
    .iter()
    .find(|name| name.ends_with(&suffix))
    .cloned()
}

/// Apply the bump to a current version, accounting for prerelease and 0.x.y.
fn apply_conventional_bump(
  current: &Version,
  intended: BumpKind,
) -> (Version, AppliedDiff) {
  if !current.pre.is_empty() {
    // Prerelease versions only ever bump the prerelease counter.
    let new_v = increment_version(current, &VersionIncrement::Prerelease);
    return (new_v, AppliedDiff::Prerelease);
  }
  let effective = if current.major == 0 {
    // 0.x.y semantics: major→minor, minor→patch.
    match intended {
      BumpKind::Major => BumpKind::Minor,
      BumpKind::Minor => BumpKind::Patch,
      BumpKind::Patch => BumpKind::Patch,
    }
  } else {
    intended
  };
  let new_v = match effective {
    BumpKind::Major => increment_version(current, &VersionIncrement::Major),
    BumpKind::Minor => increment_version(current, &VersionIncrement::Minor),
    BumpKind::Patch => increment_version(current, &VersionIncrement::Patch),
  };
  (new_v, AppliedDiff::Bump(effective))
}

// ---------------------------------------------------------------------------
// Git
// ---------------------------------------------------------------------------

fn run_git(cwd: &Path, args: &[&str]) -> Result<String, AnyError> {
  let output = Command::new("git").current_dir(cwd).args(args).output();
  let output = match output {
    Ok(o) => o,
    Err(e) => bail!("Failed to run git {:?}: {}", args, e),
  };
  if !output.status.success() {
    bail!(
      "git {:?} failed: {}",
      args,
      String::from_utf8_lossy(&output.stderr)
    );
  }
  Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Read the `"version"` field from a deno.json at a given git ref.
/// Returns `None` if the file doesn't exist at that ref or has no version.
fn read_version_at_ref(
  cwd: &Path,
  git_ref: &str,
  relative_path: &str,
) -> Option<Version> {
  let refspec = format!("{}:{}", git_ref, relative_path);
  let content = run_git(cwd, &["show", &refspec]).ok()?;
  // Parse as JSON/JSONC to extract the version field.
  let value: serde_json::Value = serde_json::from_str(&content).ok()?;
  let version_str = value.get("version")?.as_str()?;
  Version::parse_standard(version_str).ok()
}

/// Classify the type of change between two versions.
fn diff_from_versions(old: &Version, new: &Version) -> AppliedDiff {
  if !new.pre.is_empty() || !old.pre.is_empty() {
    return AppliedDiff::Prerelease;
  }
  if new.major != old.major {
    return AppliedDiff::Bump(BumpKind::Major);
  }
  if new.minor != old.minor {
    return AppliedDiff::Bump(BumpKind::Minor);
  }
  AppliedDiff::Bump(BumpKind::Patch)
}

const COMMIT_SEPARATOR: &str = "<!--bump-version-commit-separator-->";

fn read_commits(
  cwd: &Path,
  start: &str,
  base: &str,
) -> Result<Vec<Commit>, AnyError> {
  let format_arg = format!("--pretty=format:{}%H%B", COMMIT_SEPARATOR);
  let range = format!("{}..{}", start, base);
  let stdout = run_git(cwd, &["--no-pager", "log", &format_arg, &range])?;
  let mut commits = Vec::new();
  for chunk in stdout.split(COMMIT_SEPARATOR) {
    if chunk.is_empty() {
      continue;
    }
    if chunk.len() < 40 {
      continue;
    }
    let hash = chunk[..40].to_string();
    let rest = &chunk[40..];
    let (subject, body) = match rest.find('\n') {
      Some(i) => (
        rest[..i].trim().to_string(),
        rest[i + 1..].trim().to_string(),
      ),
      None => (rest.trim().to_string(), String::new()),
    };
    if subject.is_empty() {
      continue;
    }
    commits.push(Commit {
      hash,
      subject,
      body,
    });
  }
  Ok(commits)
}

// ---------------------------------------------------------------------------
// Import map rewriting
// ---------------------------------------------------------------------------

fn rewrite_import_map(
  cli_options: &CliOptions,
  version_flags: &VersionFlags,
  updates: &[(String, Version, Version, PathBuf)],
) -> Result<(), AnyError> {
  let workspace = cli_options.workspace();
  let root_dir = workspace.root_dir_path();

  // Collect every file we'd like to rewrite jsr: refs in.
  // - The explicit --import-map argument, if provided.
  // - Otherwise, the root deno.json's importMap target (if any) AND the root
  //   deno.json itself (since users sometimes inline imports there).
  // - Always also rewrite each member's deno.json so internal cross-package
  //   `jsr:` references stay consistent.
  let mut files: Vec<PathBuf> = Vec::new();

  if let Some(p) = &version_flags.import_map {
    let path = if Path::new(p).is_absolute() {
      PathBuf::from(p)
    } else {
      root_dir.join(p)
    };
    files.push(path);
  } else if let Some(root_deno_json) = workspace.root_deno_json() {
    let root_path = deno_path_util::url_to_file_path(&root_deno_json.specifier)
      .context("Failed to convert root deno.json URL to path")?;
    files.push(root_path.clone());
    if let Some(target) = root_deno_json.json.import_map.as_ref() {
      let parent = root_path.parent().unwrap_or(&root_dir);
      let mapped = parent.join(target);
      if mapped != root_path {
        files.push(mapped);
      }
    }
  }

  // Always rewrite each member deno.json.
  for (_, _, _, member_path) in updates {
    if !files.iter().any(|f| f == member_path) {
      files.push(member_path.clone());
    }
  }

  for file in files {
    if !file.exists() {
      continue;
    }
    let original = std::fs::read_to_string(&file)
      .with_context(|| format!("Reading {}", file.display()))?;
    let rewritten = rewrite_jsr_refs_in_text(&original, updates);
    if rewritten != original && !version_flags.dry_run {
      std::fs::write(&file, rewritten)
        .with_context(|| format!("Writing {}", file.display()))?;
    }
  }
  Ok(())
}

fn rewrite_jsr_refs_in_text(
  text: &str,
  updates: &[(String, Version, Version, PathBuf)],
) -> String {
  let mut out = text.to_string();
  for (name, from, to, _) in updates {
    if from == to {
      continue;
    }
    let from_str = from.to_string();
    let to_str = to.to_string();
    // Match `<name>@<prefix><from>` where prefix is one of ^, ~, =, <=, >=, <,
    // >, or empty. We rebuild the replacement preserving the prefix.
    let escaped_name = regex::escape(name);
    let escaped_from = regex::escape(&from_str);
    let pattern =
      format!(r"{}@(\^|~|=|<=|>=|<|>)?{}\b", escaped_name, escaped_from);
    let re = match regex::Regex::new(&pattern) {
      Ok(re) => re,
      Err(_) => continue,
    };
    let replacement = format!("{}@${{1}}{}", name, to_str);
    out = re.replace_all(&out, replacement.as_str()).into_owned();
  }
  out
}

// ---------------------------------------------------------------------------
// Release notes
// ---------------------------------------------------------------------------

/// Returns today's UTC date as `(year, month, day)` without pulling in a
/// date-time crate dependency.
fn chrono_today() -> (i32, u32, u32) {
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  // Days since 1970-01-01.
  let days = (now / 86_400) as i64;
  // Algorithm from "Howard Hinnant - chrono".
  let z = days + 719_468;
  let era = if z >= 0 {
    z / 146_097
  } else {
    (z - 146_096) / 146_097
  };
  let doe = z - era * 146_097; // [0, 146096]
  let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
  let y = yoe + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
  let mp = (5 * doy + 2) / 153; // [0, 11]
  let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
  let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
  let y = (y + (if m <= 2 { 1 } else { 0 })) as i32;
  (y, m, d)
}

fn release_title(date: (i32, u32, u32)) -> String {
  format!("{:04}.{:02}.{:02}", date.0, date.1, date.2)
}

fn create_release_note(
  updates: &[(String, Version, Version, PathBuf)],
  diffs: &BTreeMap<String, AppliedDiff>,
  commits_by_pkg: &BTreeMap<String, Vec<CommitWithTag>>,
  date: (i32, u32, u32),
) -> String {
  let mut out = String::new();
  out.push_str(&format!("### {}\n\n", release_title(date)));
  for (name, _from, to, _) in updates {
    let diff = diffs.get(name).map(|d| d.label()).unwrap_or("");
    out.push_str(&format!("#### {} {} ({})\n\n", name, to, diff));
    if let Some(commits) = commits_by_pkg.get(name) {
      let mut sorted = commits.clone();
      sorted.sort_by(|a, b| tag_priority(&a.tag).cmp(&tag_priority(&b.tag)));
      for c in sorted {
        out.push_str(&format!("- {}\n", c.commit.subject));
      }
    }
    out.push('\n');
  }
  out
}

fn tag_priority(tag: &str) -> u32 {
  match tag {
    "BREAKING" => 0,
    "feat" => 1,
    "deprecation" => 2,
    "fix" => 3,
    "perf" => 4,
    "docs" => 5,
    "style" => 6,
    "refactor" => 7,
    "test" => 8,
    "chore" => 9,
    _ => 100,
  }
}

fn prepend_release_notes(
  path: &Path,
  release_note: &str,
) -> Result<(), AnyError> {
  let existing = if path.exists() {
    std::fs::read_to_string(path).unwrap_or_default()
  } else {
    String::new()
  };
  let combined = if existing.is_empty() {
    release_note.to_string()
  } else {
    format!("{}\n{}", release_note.trim_end(), existing)
  };
  std::fs::write(path, combined)
    .with_context(|| format!("Writing {}", path.display()))?;
  Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;

  fn v(s: &str) -> Version {
    Version::parse_standard(s).unwrap()
  }

  #[test]
  fn apply_conventional_bump_for_one_x() {
    // 0.x.y: major intent → minor, minor intent → patch.
    let (new, diff) = apply_conventional_bump(&v("0.1.0"), BumpKind::Major);
    assert_eq!(new, v("0.2.0"));
    assert_eq!(diff, AppliedDiff::Bump(BumpKind::Minor));

    let (new, diff) = apply_conventional_bump(&v("0.1.0"), BumpKind::Minor);
    assert_eq!(new, v("0.1.1"));
    assert_eq!(diff, AppliedDiff::Bump(BumpKind::Patch));

    let (new, diff) = apply_conventional_bump(&v("0.1.0"), BumpKind::Patch);
    assert_eq!(new, v("0.1.1"));
    assert_eq!(diff, AppliedDiff::Bump(BumpKind::Patch));
  }

  #[test]
  fn apply_conventional_bump_for_stable() {
    let (new, diff) = apply_conventional_bump(&v("1.4.6"), BumpKind::Major);
    assert_eq!(new, v("2.0.0"));
    assert_eq!(diff, AppliedDiff::Bump(BumpKind::Major));

    let (new, diff) = apply_conventional_bump(&v("1.4.6"), BumpKind::Minor);
    assert_eq!(new, v("1.5.0"));
    assert_eq!(diff, AppliedDiff::Bump(BumpKind::Minor));
  }

  #[test]
  fn apply_conventional_bump_for_prerelease() {
    let (new, diff) =
      apply_conventional_bump(&v("1.0.0-rc.1"), BumpKind::Major);
    assert_eq!(diff, AppliedDiff::Prerelease);
    // Last numeric prerelease segment is incremented.
    assert_eq!(new.to_string(), "1.0.0-rc.2");
  }

  fn pkg_set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| s.to_string()).collect()
  }

  fn commit(subject: &str) -> Commit {
    Commit {
      hash: "0".repeat(40),
      subject: subject.to_string(),
      body: String::new(),
    }
  }

  #[test]
  fn parse_commit_basic() {
    let pkgs = pkg_set(&["@std/foo", "@std/bar"]);
    let parsed = parse_commit(&commit("fix(foo): something"), &pkgs);
    let bumps = match parsed {
      ParsedCommit::Bumps(b) => b,
      _ => panic!("expected bumps"),
    };
    assert_eq!(bumps.len(), 1);
    assert_eq!(bumps[0].module, "@std/foo");
    assert_eq!(bumps[0].bump, BumpKind::Patch);
  }

  #[test]
  fn parse_commit_breaking_bang() {
    let pkgs = pkg_set(&["@std/foo"]);
    let parsed = parse_commit(&commit("feat(foo)!: breaking"), &pkgs);
    let bumps = match parsed {
      ParsedCommit::Bumps(b) => b,
      _ => panic!("expected bumps"),
    };
    assert_eq!(bumps[0].bump, BumpKind::Major);
  }

  #[test]
  fn parse_commit_breaking_tag() {
    let pkgs = pkg_set(&["@std/foo"]);
    let parsed = parse_commit(&commit("BREAKING(foo): change"), &pkgs);
    let bumps = match parsed {
      ParsedCommit::Bumps(b) => b,
      _ => panic!("expected bumps"),
    };
    assert_eq!(bumps[0].bump, BumpKind::Major);
  }

  #[test]
  fn parse_commit_multi_scope() {
    let pkgs = pkg_set(&["@std/foo", "@std/bar", "@std/baz"]);
    let parsed = parse_commit(&commit("fix(foo,bar): a"), &pkgs);
    let bumps = match parsed {
      ParsedCommit::Bumps(b) => b,
      _ => panic!("expected bumps"),
    };
    assert_eq!(bumps.len(), 2);
  }

  #[test]
  fn parse_commit_wildcard() {
    let pkgs = pkg_set(&["@std/foo", "@std/bar"]);
    let parsed = parse_commit(&commit("refactor(*): clean up"), &pkgs);
    let bumps = match parsed {
      ParsedCommit::Bumps(b) => b,
      _ => panic!("expected bumps"),
    };
    assert_eq!(bumps.len(), 2);
  }

  #[test]
  fn parse_commit_unstable_scope() {
    let pkgs = pkg_set(&["@std/foo"]);
    let parsed = parse_commit(&commit("BREAKING(foo/unstable): x"), &pkgs);
    let bumps = match parsed {
      ParsedCommit::Bumps(b) => b,
      _ => panic!("expected bumps"),
    };
    assert_eq!(bumps[0].module, "@std/foo");
    // Even BREAKING degrades to patch under unstable scope.
    assert_eq!(bumps[0].bump, BumpKind::Patch);
  }

  #[test]
  fn parse_commit_missing_scope_required() {
    let pkgs = pkg_set(&["@std/foo"]);
    let parsed = parse_commit(&commit("fix: oops"), &pkgs);
    match parsed {
      ParsedCommit::Diagnostic(d) => {
        assert!(matches!(d.kind, DiagnosticKind::MissingScope))
      }
      _ => panic!("expected diagnostic"),
    }
  }

  #[test]
  fn parse_commit_chore_no_scope_skipped() {
    let pkgs = pkg_set(&["@std/foo"]);
    let parsed = parse_commit(&commit("chore: update deps"), &pkgs);
    match parsed {
      ParsedCommit::Diagnostic(d) => {
        assert!(matches!(d.kind, DiagnosticKind::Skipped))
      }
      _ => panic!("expected diagnostic"),
    }
  }

  #[test]
  fn parse_commit_unknown_module() {
    let pkgs = pkg_set(&["@std/foo"]);
    let parsed = parse_commit(&commit("fix(bogus): oops"), &pkgs);
    match parsed {
      ParsedCommit::Diagnostic(d) => {
        assert!(matches!(d.kind, DiagnosticKind::UnknownModule))
      }
      _ => panic!("expected diagnostic"),
    }
  }

  #[test]
  fn parse_commit_unknown_format() {
    let pkgs = pkg_set(&["@std/foo"]);
    let parsed = parse_commit(&commit("just some text without a colon"), &pkgs);
    match parsed {
      ParsedCommit::Diagnostic(d) => {
        assert!(matches!(d.kind, DiagnosticKind::UnknownCommit))
      }
      _ => panic!("expected diagnostic"),
    }
  }

  #[test]
  fn parse_commit_short_scope_resolves_to_full_name() {
    let pkgs = pkg_set(&["@std/foo"]);
    // "fix(foo): ..." should resolve to "@std/foo" via /-suffix matching.
    let parsed = parse_commit(&commit("fix(foo): ok"), &pkgs);
    let bumps = match parsed {
      ParsedCommit::Bumps(b) => b,
      _ => panic!("expected bumps"),
    };
    assert_eq!(bumps[0].module, "@std/foo");
  }

  #[test]
  fn release_title_format() {
    assert_eq!(release_title((2026, 4, 29)), "2026.04.29");
    assert_eq!(release_title((2024, 12, 1)), "2024.12.01");
  }

  #[test]
  fn rewrite_jsr_refs() {
    let updates = vec![(
      "@std/cli".to_string(),
      v("1.0.29"),
      v("1.0.30"),
      PathBuf::from("/x"),
    )];
    let input = r#"{
  "imports": {
    "@std/cli": "jsr:@std/cli@^1.0.29",
    "@std/cli/parse-args": "jsr:@std/cli@~1.0.29"
  }
}"#;
    let out = rewrite_jsr_refs_in_text(input, &updates);
    assert!(out.contains("jsr:@std/cli@^1.0.30"));
    assert!(out.contains("jsr:@std/cli@~1.0.30"));
    assert!(!out.contains("1.0.29"));
  }

  #[test]
  fn rewrite_jsr_refs_no_prefix() {
    let updates = vec![(
      "@std/foo".to_string(),
      v("1.0.0"),
      v("1.0.1"),
      PathBuf::from("/x"),
    )];
    let input = r#"{ "imports": { "@std/foo": "jsr:@std/foo@1.0.0" } }"#;
    let out = rewrite_jsr_refs_in_text(input, &updates);
    assert!(out.contains("jsr:@std/foo@1.0.1"));
  }

  #[test]
  fn rewrite_jsr_refs_does_not_touch_unrelated() {
    let updates = vec![(
      "@std/foo".to_string(),
      v("1.0.0"),
      v("1.0.1"),
      PathBuf::from("/x"),
    )];
    let input = r#"{ "imports": { "@std/foobar": "jsr:@std/foobar@^1.0.0" } }"#;
    let out = rewrite_jsr_refs_in_text(input, &updates);
    // `@std/foo` should not match `@std/foobar`.
    assert_eq!(out, input);
  }

  #[test]
  fn release_note_basic() {
    let updates = vec![(
      "@std/foo".to_string(),
      v("0.1.0"),
      v("0.1.1"),
      PathBuf::from("/x"),
    )];
    let mut diffs = BTreeMap::new();
    diffs.insert("@std/foo".to_string(), AppliedDiff::Bump(BumpKind::Patch));
    let mut commits = BTreeMap::new();
    commits.insert(
      "@std/foo".to_string(),
      vec![CommitWithTag {
        commit: commit("fix(foo): a"),
        tag: "fix".to_string(),
      }],
    );
    let note = create_release_note(&updates, &diffs, &commits, (2026, 4, 29));
    assert!(note.contains("### 2026.04.29"));
    assert!(note.contains("#### @std/foo 0.1.1 (patch)"));
    assert!(note.contains("- fix(foo): a"));
  }

  #[test]
  fn diff_from_versions_stable() {
    assert_eq!(
      diff_from_versions(&v("1.0.0"), &v("2.0.0")),
      AppliedDiff::Bump(BumpKind::Major)
    );
    assert_eq!(
      diff_from_versions(&v("1.0.0"), &v("1.1.0")),
      AppliedDiff::Bump(BumpKind::Minor)
    );
    assert_eq!(
      diff_from_versions(&v("1.0.0"), &v("1.0.1")),
      AppliedDiff::Bump(BumpKind::Patch)
    );
  }

  #[test]
  fn diff_from_versions_prerelease() {
    assert_eq!(
      diff_from_versions(&v("1.0.0-rc.1"), &v("1.0.0-rc.2")),
      AppliedDiff::Prerelease
    );
    // Promoting from prerelease to stable is also classified as prerelease
    // since the prerelease was involved.
    assert_eq!(
      diff_from_versions(&v("1.0.0-rc.1"), &v("1.0.0")),
      AppliedDiff::Prerelease
    );
  }

  #[test]
  fn is_release_commit_detects() {
    assert!(is_release_commit("v1.2.3"));
    assert!(is_release_commit("1.2.3"));
    assert!(is_release_commit("Release 1.2.3"));
    assert!(!is_release_commit("fix(foo): a"));
  }
}
