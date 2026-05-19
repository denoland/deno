// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use deno_ast::ModuleSpecifier;
use deno_ast::SourceRange;
use deno_ast::SourceTextInfo;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::diagnostic::LintDiagnosticDetails;
use deno_lint::diagnostic::LintDiagnosticRange;
use deno_lint::diagnostic::LintDocsUrl;

use crate::util::fs::specifier_from_file_path;

pub const OXLINT_VERSION: &str = "1.51.0";
pub const TSGOLINT_VERSION: &str = "0.16.0";

// oxlint JSON output format
#[derive(Deserialize)]
struct OxlintOutput {
  diagnostics: Vec<OxlintDiagnostic>,
}

#[derive(Deserialize)]
struct OxlintDiagnostic {
  message: String,
  code: String,
  #[allow(dead_code)]
  severity: String,
  help: Option<String>,
  filename: String,
  labels: Vec<OxlintLabel>,
}

#[derive(Deserialize)]
struct OxlintLabel {
  span: OxlintSpan,
}

#[derive(Deserialize)]
struct OxlintSpan {
  offset: u32,
  length: u32,
}

/// Run oxlint on a batch of files and return diagnostics grouped by file path.
pub fn run_oxlint(
  files: &[PathBuf],
  cwd: &Path,
) -> Result<HashMap<PathBuf, Vec<LintDiagnostic>>, AnyError> {
  if files.is_empty() {
    return Ok(HashMap::new());
  }

  let deno_bin = std::env::current_exe()
    .context("failed to get current executable path")?;

  let mut cmd = Command::new(&deno_bin);
  cmd
    .arg("run")
    .arg("-A")
    .arg("--no-config")
    .arg(format!("npm:oxlint@{}", OXLINT_VERSION))
    .arg("--format")
    .arg("json");

  // Auto-detect oxlint config file by walking up from cwd, then from
  // the first file's directory (covers subdirectory invocations)
  let config = find_config_file(cwd, "oxlintrc.json").or_else(|| {
    files
      .first()
      .and_then(|f| f.parent())
      .and_then(|dir| find_config_file(dir, "oxlintrc.json"))
  });
  if let Some(config_path) = config {
    cmd.arg("-c").arg(&config_path);
  }

  // Enable plugins for broader rule coverage
  cmd.arg("--react-plugin");
  cmd.arg("--jsx-a11y-plugin");

  // Enable type-aware rules if tsconfig.json is found and tsgolint is available
  let tsconfig = find_config_file(cwd, "tsconfig.json").or_else(|| {
    files
      .first()
      .and_then(|f| f.parent())
      .and_then(|dir| find_config_file(dir, "tsconfig.json"))
  });
  if tsconfig.is_some() {
    // Resolve the tsgolint binary path by asking deno to locate the npm bin
    let tsgolint_resolve = Command::new(&deno_bin)
      .arg("eval")
      .arg("--no-config")
      .arg(format!(
        "import 'npm:oxlint-tsgolint@{}'; \
         // just triggers the download/cache",
        TSGOLINT_VERSION
      ))
      .output();

    // Find the tsgolint binary in the npm cache and add to PATH
    if let Ok(tsgolint_bin) = resolve_tsgolint_bin(&deno_bin) {
      if let Some(bin_dir) = tsgolint_bin.parent() {
        let current_path =
          std::env::var("PATH").unwrap_or_default();
        let new_path =
          format!("{}:{}", bin_dir.display(), current_path);
        cmd.env("PATH", new_path);
      }
      // Suppress the "not found" warning even if it ends up missing
      let _ = tsgolint_resolve;
      cmd.arg("--type-aware");
    }
  }

  cmd.args(files);

  let output = cmd.output().with_context(|| {
    format!("failed to execute oxlint via {}", deno_bin.display())
  })?;

  // oxlint exits with non-zero when it finds lint errors, which is expected.
  // We only care about parsing the JSON stdout.
  let result: OxlintOutput =
    deno_core::serde_json::from_slice(&output.stdout).with_context(|| {
      let stderr = String::from_utf8_lossy(&output.stderr);
      format!(
        "failed to parse oxlint JSON output (exit code: {:?}): {}",
        output.status.code(),
        stderr
      )
    })?;

  // Group diagnostics by file, reading source text for each unique file
  let mut source_cache: HashMap<PathBuf, (ModuleSpecifier, SourceTextInfo)> =
    HashMap::new();
  let mut map: HashMap<PathBuf, Vec<LintDiagnostic>> = HashMap::new();

  for diag in result.diagnostics {
    let raw_path = PathBuf::from(&diag.filename);
    // oxlint may return relative paths; canonicalize for HashMap lookup
    let path = if raw_path.is_absolute() {
      raw_path
    } else {
      std::env::current_dir()
        .map(|d| d.join(&raw_path))
        .unwrap_or(raw_path)
    };

    let entry = if let Some(entry) = source_cache.get(&path) {
      entry
    } else {
      let source_text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => continue,
      };
      let specifier = match specifier_from_file_path(&path) {
        Ok(s) => s,
        Err(_) => continue,
      };
      let text_info = SourceTextInfo::new(source_text.into());
      source_cache.insert(path.clone(), (specifier, text_info));
      source_cache.get(&path).unwrap()
    };

    let (specifier, text_info) = entry;
    if let Some(lint_diag) =
      map_to_lint_diagnostic(specifier, text_info, &diag)
    {
      map.entry(path).or_default().push(lint_diag);
    }
  }

  Ok(map)
}

fn map_to_lint_diagnostic(
  specifier: &ModuleSpecifier,
  text_info: &SourceTextInfo,
  diag: &OxlintDiagnostic,
) -> Option<LintDiagnostic> {
  // Strip the "eslint(...)" / "typescript-eslint(...)" / "react(...)" etc.
  // wrapper from code if present
  let code = diag
    .code
    .find('(')
    .and_then(|start| {
      diag.code.ends_with(')').then(|| {
        diag.code[start + 1..diag.code.len() - 1].to_string()
      })
    })
    .unwrap_or_else(|| diag.code.clone());

  // Use byte offset from span labels
  let range = if let Some(label) = diag.labels.first() {
    let base = text_info.range().start.as_source_pos();
    let start_pos = base + label.span.offset as usize;
    let end_pos = base + (label.span.offset + label.span.length) as usize;
    Some(LintDiagnosticRange {
      text_info: text_info.clone(),
      range: SourceRange::new(start_pos, end_pos),
      description: None,
    })
  } else {
    None
  };

  Some(LintDiagnostic {
    specifier: specifier.clone(),
    range,
    details: LintDiagnosticDetails {
      message: diag.message.clone(),
      code,
      hint: diag.help.clone(),
      fixes: vec![],
      custom_docs_url: LintDocsUrl::None,
      info: vec![],
    },
  })
}

/// Resolve the tsgolint native binary from the Deno npm cache.
/// First ensures the package is cached, then locates the platform binary.
fn resolve_tsgolint_bin(deno_bin: &Path) -> Result<PathBuf, AnyError> {
  // Ensure tsgolint is cached by running `deno cache`
  let _ = Command::new(deno_bin)
    .arg("cache")
    .arg("--no-config")
    .arg(format!("npm:oxlint-tsgolint@{}", TSGOLINT_VERSION))
    .output();

  // Get npm cache location from `deno info --json`
  let info_output = Command::new(deno_bin)
    .arg("info")
    .arg("--no-config")
    .arg("--json")
    .output()
    .context("failed to run deno info")?;

  #[derive(Deserialize)]
  struct DenoInfo {
    #[serde(rename = "npmCache")]
    npm_cache: Option<String>,
  }

  let info: DenoInfo =
    deno_core::serde_json::from_slice(&info_output.stdout)
      .context("failed to parse deno info output")?;

  let npm_cache = info
    .npm_cache
    .ok_or_else(|| deno_core::anyhow::anyhow!("npmCache not found in deno info"))?;

  let platform_pkg = tsgolint_platform_package_short();
  let bin_name = if cfg!(target_os = "windows") {
    "tsgolint.exe"
  } else {
    "tsgolint"
  };

  let candidate = PathBuf::from(&npm_cache)
    .join("registry.npmjs.org")
    .join(&platform_pkg)
    .join(TSGOLINT_VERSION)
    .join(bin_name);

  if candidate.exists() {
    Ok(candidate)
  } else {
    Err(deno_core::anyhow::anyhow!(
      "tsgolint binary not found at {}",
      candidate.display()
    ))
  }
}

fn tsgolint_platform_package_short() -> String {
  let os = if cfg!(target_os = "macos") {
    "darwin"
  } else if cfg!(target_os = "linux") {
    "linux"
  } else if cfg!(target_os = "windows") {
    "win32"
  } else {
    "unknown"
  };
  let arch = if cfg!(target_arch = "aarch64") {
    "arm64"
  } else if cfg!(target_arch = "x86_64") {
    "x64"
  } else {
    "unknown"
  };
  format!("@oxlint-tsgolint/{}-{}", os, arch)
}

/// Walk up from `start` looking for a file named `name`.
fn find_config_file(start: &Path, name: &str) -> Option<PathBuf> {
  let mut dir = start;
  loop {
    let candidate = dir.join(name);
    if candidate.exists() {
      return Some(candidate);
    }
    match dir.parent() {
      Some(parent) => dir = parent,
      None => return None,
    }
  }
}
