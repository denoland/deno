// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_ast::SourceRange;
use deno_ast::SourceTextInfo;
use deno_core::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::diagnostic::LintDiagnosticDetails;
use deno_lint::diagnostic::LintDiagnosticRange;
use deno_lint::diagnostic::LintDocsUrl;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmRegistryApi;
use deno_npm_cache::TarballCache;
use deno_resolver::workspace::WorkspaceNpmLinkPackagesRc;
use deno_semver::package::PackageNv;

use crate::cache::DenoDir;
use crate::npm::CliNpmCache;
use crate::npm::CliNpmCacheHttpClient;
use crate::npm::CliNpmRegistryInfoProvider;
use crate::sys::CliSys;
use crate::util::fs::specifier_from_file_path;

pub const OXLINT_VERSION: &str = "1.2.0";

fn oxlint_platform() -> &'static str {
  match (std::env::consts::ARCH, std::env::consts::OS) {
    ("x86_64", "linux") => "linux-x64-gnu",
    ("aarch64", "linux") => "linux-arm64-gnu",
    ("x86_64", "macos" | "apple") => "darwin-x64",
    ("aarch64", "macos" | "apple") => "darwin-arm64",
    ("x86_64", "windows") => "win32-x64",
    ("aarch64", "windows") => "win32-arm64",
    _ => panic!(
      "Unsupported platform for oxlint: {} {}",
      std::env::consts::ARCH,
      std::env::consts::OS
    ),
  }
}

pub async fn ensure_oxlint(
  deno_dir: &DenoDir,
  npmrc: &ResolvedNpmRc,
  api: &Arc<CliNpmRegistryInfoProvider>,
  workspace_link_packages: &WorkspaceNpmLinkPackagesRc,
  tarball_cache: &Arc<TarballCache<CliNpmCacheHttpClient, CliSys>>,
  npm_cache: &CliNpmCache,
) -> Result<PathBuf, AnyError> {
  let target = oxlint_platform();
  let mut oxlint_path = deno_dir
    .dl_folder_path()
    .join(format!("oxlint-{}", OXLINT_VERSION))
    .join(format!("oxlint-{}", target));
  if cfg!(windows) {
    oxlint_path.set_extension("exe");
  }

  if oxlint_path.exists() {
    return Ok(oxlint_path);
  }

  let pkg_name = format!("@oxlint/{}", target);
  let nv =
    PackageNv::from_str(&format!("{}@{}", pkg_name, OXLINT_VERSION)).unwrap();
  let mut info = api.package_info(&pkg_name).await?;
  let version_info = match info.version_info(&nv, &workspace_link_packages.0) {
    Ok(version_info) => version_info,
    Err(_) => {
      api.mark_force_reload();
      info = api.package_info(&pkg_name).await?;
      info.version_info(&nv, &workspace_link_packages.0)?
    }
  };
  if let Some(dist) = &version_info.dist {
    let registry_url = npmrc.get_registry_url(&nv.name);
    let package_folder =
      npm_cache.package_folder_for_nv_and_url(&nv, registry_url);
    let existed = package_folder.exists();

    if !existed {
      tarball_cache
        .ensure_package(&nv, dist)
        .await
        .with_context(|| {
          format!(
            "failed to download oxlint package tarball {} from {}",
            nv, dist.tarball
          )
        })?;
    }

    let path = if cfg!(windows) {
      package_folder.join("oxlint.exe")
    } else {
      package_folder.join("oxlint")
    };

    std::fs::create_dir_all(oxlint_path.parent().unwrap()).with_context(
      || {
        format!(
          "failed to create directory {}",
          oxlint_path.parent().unwrap().display()
        )
      },
    )?;
    std::fs::copy(&path, &oxlint_path).with_context(|| {
      format!(
        "failed to copy oxlint binary from {} to {}",
        path.display(),
        oxlint_path.display()
      )
    })?;

    // Make executable on unix
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      std::fs::set_permissions(
        &oxlint_path,
        std::fs::Permissions::from_mode(0o755),
      )?;
    }

    if !existed {
      let _ = std::fs::remove_dir_all(&package_folder).inspect_err(|e| {
        log::warn!(
          "failed to remove directory {}: {}",
          package_folder.display(),
          e
        );
      });
    }
    Ok(oxlint_path)
  } else {
    anyhow::bail!(
      "could not fetch oxlint binary; download it manually and copy it to {}",
      oxlint_path.display()
    );
  }
}

// oxlint JSON output format
#[derive(Deserialize)]
struct OxlintOutput {
  diagnostics: Vec<OxlintDiagnostic>,
}

#[derive(Deserialize)]
struct OxlintDiagnostic {
  message: String,
  code: String,
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
  oxlint_bin: &Path,
  files: &[PathBuf],
) -> Result<HashMap<PathBuf, Vec<LintDiagnostic>>, AnyError> {
  if files.is_empty() {
    return Ok(HashMap::new());
  }

  let output = Command::new(oxlint_bin)
    .arg("--format")
    .arg("json")
    .args(files)
    .output()
    .with_context(|| {
      format!("failed to execute oxlint at {}", oxlint_bin.display())
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
    let path = PathBuf::from(&diag.filename);

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
  // Strip the "eslint(...)" wrapper from code if present
  let code = diag
    .code
    .strip_prefix("eslint(")
    .and_then(|s| s.strip_suffix(')'))
    .or_else(|| {
      diag
        .code
        .strip_prefix("typescript-eslint(")
        .and_then(|s| s.strip_suffix(')'))
    })
    .unwrap_or(&diag.code)
    .to_string();

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
