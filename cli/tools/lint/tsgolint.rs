// Copyright 2018-2026 the Deno authors. MIT license.

//! Integration with `tsgolint` for type-aware lint rules.
//!
//! `tsgolint` is a Go binary (built on top of typescript-go) that runs
//! type-aware lint rules and emits diagnostics over a small "headless"
//! protocol. We drive it directly (NOT through oxlint) so that file
//! discovery, configuration, ignore directives (`// deno-lint-ignore`) and
//! output formatting all stay owned by Deno.
//!
//! Architecture: `tsgolint` builds one TypeScript program per `tsconfig.json`,
//! so it is run ONCE for the whole batch of files (building a program per file
//! would be catastrophically slow). The resulting diagnostics are stored
//! keyed by file path, and the per-file `external_linter` callback in
//! `linter.rs` looks them up. Routing the diagnostics back through deno_lint's
//! `ExternalLinterResult` means `// deno-lint-ignore` filtering,
//! `ban-unused-ignore` and diagnostic sorting are applied to them for free.
//!
//! NOTE(unstable): this is gated behind `DENO_UNSTABLE_TSGOLINT=1`. The binary
//! is downloaded automatically from npm on first use (see [`ensure_tsgolint`]),
//! the same way `deno bundle` fetches esbuild; `DENO_TSGOLINT_BIN` can override
//! the resolved path. Generating a `tsconfig.json` (so files aren't left
//! "unmatched") is a follow-up, tracked against the Deno 3 "no fork of tsgo"
//! plan.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use deno_ast::ModuleSpecifier;
use deno_ast::SourceRange;
use deno_ast::SourceTextInfo;
use deno_config::deno_json::LintRulesConfig;
use deno_config::workspace::WorkspaceDirectoryRc;
use deno_core::anyhow;
use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::diagnostic::LintDiagnosticDetails;
use deno_lint::diagnostic::LintDiagnosticRange;
use deno_lint::diagnostic::LintDocsUrl;
use deno_npm::registry::NpmRegistryApi;
use deno_semver::package::PackageNv;
use serde::Deserialize;

use crate::factory::CliFactory;

/// Version of the `tsgolint` binary (npm `oxlint-tsgolint`) to download. Kept in
/// sync with the type-aware rules Deno knows about in [`ALL_RULES`].
pub const TSGOLINT_VERSION: &str = "0.24.0";

/// Type-aware rules enabled by default when tsgolint is turned on. A
/// conservative, high-signal subset of what tsgolint implements. The full set
/// can be opted into per-rule via the lint config `include` list.
///
/// `require-await` is intentionally omitted because deno_lint already ships a
/// `require-await` rule and we don't want two rules sharing one code.
pub const DEFAULT_RULES: &[&str] = &[
  "await-thenable",
  "no-floating-promises",
  "no-for-in-array",
  "no-implied-eval",
  "no-misused-promises",
  "no-base-to-string",
  "no-duplicate-type-constituents",
  "no-redundant-type-constituents",
  "no-unnecessary-type-assertion",
  "only-throw-error",
  "prefer-promise-reject-errors",
  "restrict-plus-operands",
  "restrict-template-expressions",
  "unbound-method",
];

/// Every rule name tsgolint currently knows about. Used to validate
/// `include` entries so a typo doesn't silently do nothing.
pub const ALL_RULES: &[&str] = &[
  "await-thenable",
  "consistent-return",
  "consistent-type-exports",
  "dot-notation",
  "no-array-delete",
  "no-base-to-string",
  "no-confusing-void-expression",
  "no-deprecated",
  "no-duplicate-type-constituents",
  "no-floating-promises",
  "no-for-in-array",
  "no-implied-eval",
  "no-meaningless-void-operator",
  "no-misused-promises",
  "no-misused-spread",
  "no-mixed-enums",
  "no-redundant-type-constituents",
  "no-unnecessary-boolean-literal-compare",
  "no-unnecessary-condition",
  "no-unnecessary-qualifier",
  "no-unnecessary-template-expression",
  "no-unnecessary-type-arguments",
  "no-unnecessary-type-assertion",
  "no-unnecessary-type-conversion",
  "no-unnecessary-type-parameters",
  "no-unsafe-argument",
  "no-unsafe-assignment",
  "no-unsafe-call",
  "no-unsafe-enum-comparison",
  "no-unsafe-member-access",
  "no-unsafe-return",
  "no-unsafe-type-assertion",
  "no-unsafe-unary-minus",
  "non-nullable-type-assertion-style",
  "only-throw-error",
  "prefer-find",
  "prefer-includes",
  "prefer-nullish-coalescing",
  "prefer-optional-chain",
  "prefer-promise-reject-errors",
  "prefer-readonly-parameter-types",
  "prefer-readonly",
  "prefer-reduce-type-parameter",
  "prefer-regexp-exec",
  "prefer-return-this-type",
  "prefer-string-starts-ends-with",
  "promise-function-async",
  "related-getter-setter-pairs",
  "require-array-sort-compare",
  "require-await",
  "restrict-plus-operands",
  "restrict-template-expressions",
  "return-await",
  "strict-boolean-expressions",
  "strict-void-return",
  "switch-exhaustiveness-check",
  "unbound-method",
  "use-unknown-in-catch-callback-variable",
];

/// Whether type-aware linting via tsgolint is enabled. Unstable, opt-in.
pub fn is_enabled() -> bool {
  matches!(
    std::env::var("DENO_UNSTABLE_TSGOLINT").as_deref(),
    Ok("1") | Ok("true")
  )
}

/// Resolve the set of tsgolint rules to run from the lint config, starting
/// from [`DEFAULT_RULES`], adding any `include`d tsgolint rules and removing
/// any `exclude`d ones.
pub fn resolve_rules(config: &LintRulesConfig) -> Vec<String> {
  let exclude = config.exclude.clone().unwrap_or_default();
  let include = config.include.clone().unwrap_or_default();

  let mut rules: Vec<String> = DEFAULT_RULES
    .iter()
    .map(|s| s.to_string())
    .filter(|r| !exclude.contains(r))
    .collect();

  for name in include {
    if ALL_RULES.contains(&name.as_str())
      && !rules.contains(&name)
      && !exclude.contains(&name)
    {
      rules.push(name);
    }
  }

  rules
}

/// Precomputed tsgolint diagnostics for a batch of files, plus the set of
/// rules that were enabled (so deno_lint treats them as known, valid codes for
/// `// deno-lint-ignore` / `ban-unused-ignore`).
#[derive(Debug, Default)]
pub struct TsgolintResults {
  by_file: HashMap<String, Vec<TsgolintDiagnostic>>,
  enabled_rules: Vec<String>,
}

#[derive(Debug)]
struct TsgolintDiagnostic {
  code: String,
  message: String,
  hint: Option<String>,
  /// Byte offsets `(start, end)` into the file, or `None` for a whole-file
  /// diagnostic.
  range: Option<(usize, usize)>,
}

impl TsgolintResults {
  /// The enabled rule codes, for `ExternalLinterResult::rules`.
  pub fn rule_codes(&self) -> impl Iterator<Item = Cow<'static, str>> + '_ {
    self.enabled_rules.iter().map(|r| Cow::Owned(r.to_string()))
  }

  /// Convert the precomputed diagnostics for `file_path` into
  /// `LintDiagnostic`s anchored to the given parsed source.
  pub fn diagnostics_for(
    &self,
    file_path: &Path,
    specifier: &ModuleSpecifier,
    text_info: &SourceTextInfo,
  ) -> Vec<LintDiagnostic> {
    let Some(diags) = self.by_file.get(&normalize_key(file_path)) else {
      return Vec::new();
    };
    let start_pos = text_info.range().start;
    diags
      .iter()
      .map(|d| {
        let range = d.range.map(|(start, end)| LintDiagnosticRange {
          range: SourceRange::new(start_pos + start, start_pos + end),
          description: None,
          text_info: text_info.clone(),
        });
        LintDiagnostic {
          specifier: specifier.clone(),
          range,
          details: LintDiagnosticDetails {
            message: d.message.clone(),
            code: d.code.clone(),
            hint: d.hint.clone(),
            fixes: vec![],
            custom_docs_url: LintDocsUrl::None,
            info: vec![],
          },
        }
      })
      .collect()
  }
}

/// Run tsgolint once over `paths`, returning diagnostics keyed by file.
///
/// Files without a matching `tsconfig.json` are silently skipped by tsgolint
/// (it can't type-check them), so they simply produce no type-aware
/// diagnostics.
pub fn run(
  bin: &Path,
  member_dir: &WorkspaceDirectoryRc,
  paths: &[PathBuf],
  rules: Vec<String>,
) -> Result<TsgolintResults, AnyError> {
  if rules.is_empty() || paths.is_empty() {
    return Ok(TsgolintResults::default());
  }

  let cwd = deno_path_util::url_to_file_path(member_dir.dir_url())?;

  let file_paths: Vec<String> =
    paths.iter().map(|p| normalize_key(p)).collect();
  let rule_objs: Vec<serde_json::Value> = rules
    .iter()
    .map(|name| serde_json::json!({ "name": name }))
    .collect();
  let payload = serde_json::json!({
    "version": 2,
    "configs": [{
      "file_paths": file_paths,
      "rules": rule_objs,
    }],
    "report_syntactic": false,
    "report_semantic": false,
  });
  let payload_bytes = serde_json::to_vec(&payload)?;

  let mut child = Command::new(bin)
    .arg("headless")
    .current_dir(&cwd)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())
    .spawn()
    .map_err(|e| {
      deno_core::anyhow::anyhow!(
        "failed to spawn tsgolint ({}): {e}",
        bin.display()
      )
    })?;

  // tsgolint reads ALL of stdin before producing any output, so writing the
  // whole payload and then reading all of stdout cannot deadlock.
  child
    .stdin
    .take()
    .unwrap()
    .write_all(&payload_bytes)
    .map_err(|e| {
      deno_core::anyhow::anyhow!("failed to write tsgolint payload: {e}")
    })?;

  let output = child.wait_with_output()?;
  if !output.status.success() {
    bail!("tsgolint exited with status {}", output.status);
  }

  parse_frames(&output.stdout, &rules)
}

/// Parse the framed message stream from tsgolint stdout.
///
/// Each message is a 5-byte header (4-byte little-endian payload length +
/// 1-byte message type) followed by the JSON payload. Message types:
/// 0 = error, 1 = diagnostic, 2 = timing.
fn parse_frames(
  buf: &[u8],
  rules: &[String],
) -> Result<TsgolintResults, AnyError> {
  let mut by_file: HashMap<String, Vec<TsgolintDiagnostic>> = HashMap::new();
  let mut i = 0;
  while i + 5 <= buf.len() {
    let len =
      u32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]) as usize;
    let msg_type = buf[i + 4];
    i += 5;
    if i + len > buf.len() {
      break;
    }
    let body = &buf[i..i + len];
    i += len;

    match msg_type {
      0 => {
        let err: HeadlessError = serde_json::from_slice(body)?;
        bail!("tsgolint error: {}", err.error);
      }
      1 => {
        let d: HeadlessDiagnostic = serde_json::from_slice(body)?;
        // Only surface rule diagnostics (kind 0). Internal/tsconfig
        // diagnostics (kind 1) are program-level and out of scope here.
        if d.kind != 0 {
          continue;
        }
        let (Some(file_path), Some(rule)) = (d.file_path, d.rule) else {
          continue;
        };
        by_file.entry(normalize_str(&file_path)).or_default().push(
          TsgolintDiagnostic {
            code: rule,
            message: d.message.description,
            hint: d.message.help,
            range: d.range.map(|r| (r.pos, r.end)),
          },
        );
      }
      // timing (2) and anything else: ignore
      _ => {}
    }
  }

  Ok(TsgolintResults {
    by_file,
    enabled_rules: rules.to_vec(),
  })
}

/// npm platform target for the current host, matching the
/// `@oxlint-tsgolint/<target>` optional-dependency packages.
fn tsgolint_platform() -> Result<&'static str, AnyError> {
  Ok(match (std::env::consts::ARCH, std::env::consts::OS) {
    ("x86_64", "linux") => "linux-x64",
    ("aarch64", "linux") => "linux-arm64",
    ("x86_64", "macos") => "darwin-x64",
    ("aarch64", "macos") => "darwin-arm64",
    ("x86_64", "windows") => "win32-x64",
    ("aarch64", "windows") => "win32-arm64",
    (arch, os) => bail!(
      "type-aware linting (tsgolint) is not supported on this platform: {os} {arch}"
    ),
  })
}

/// Resolve the path to the tsgolint binary, downloading it from npm on first
/// use (into `DENO_DIR`, like `deno bundle` does for esbuild). Setting
/// `DENO_TSGOLINT_BIN` overrides this with an explicit path.
pub async fn ensure_tsgolint(
  factory: &CliFactory,
) -> Result<PathBuf, AnyError> {
  if let Ok(p) = std::env::var("DENO_TSGOLINT_BIN")
    && !p.is_empty()
  {
    return Ok(PathBuf::from(p));
  }

  let target = tsgolint_platform()?;
  let deno_dir = factory.deno_dir()?;
  let mut bin_path = deno_dir
    .dl_folder_path()
    .join(format!("tsgolint-{}", TSGOLINT_VERSION))
    .join(format!("tsgolint-{}", target));
  if cfg!(windows) {
    bin_path.set_extension("exe");
  }

  if bin_path.exists() {
    return Ok(bin_path);
  }

  let installer_factory = factory.npm_installer_factory()?;
  let npmrc = factory.npmrc()?;
  let api = installer_factory.registry_info_provider()?;
  let workspace_link_packages = factory
    .resolver_factory()?
    .workspace_factory()
    .workspace_npm_link_packages()?;
  let tarball_cache = installer_factory.tarball_cache()?;
  let npm_cache = factory.npm_cache()?;

  let pkg_name = format!("@oxlint-tsgolint/{}", target);
  let nv =
    PackageNv::from_str(&format!("{}@{}", pkg_name, TSGOLINT_VERSION)).unwrap();
  let mut info = api.package_info(&pkg_name).await?;
  let version_info = match info.version_info(&nv, &workspace_link_packages.0) {
    Ok(version_info) => version_info,
    Err(_) => {
      api.mark_force_reload();
      info = api.package_info(&pkg_name).await?;
      info.version_info(&nv, &workspace_link_packages.0)?
    }
  };
  let Some(dist) = &version_info.dist else {
    anyhow::bail!(
      "could not fetch tsgolint binary; download it manually and set \
       DENO_TSGOLINT_BIN to {}",
      bin_path.display()
    );
  };

  let registry_url = npmrc.get_registry_url(&nv.name);
  let package_folder =
    npm_cache.package_folder_for_nv_and_url(&nv, registry_url);
  let existed = package_folder.exists();
  if !existed {
    log::info!("Downloading tsgolint for type-aware linting...");
    tarball_cache
      .ensure_package(&nv, dist)
      .await
      .with_context(|| {
        format!(
          "failed to download tsgolint package tarball {} from {}",
          nv, dist.tarball
        )
      })?;
  }

  let src = if cfg!(windows) {
    package_folder.join("tsgolint.exe")
  } else {
    package_folder.join("tsgolint")
  };

  let bin_dir = bin_path.parent().unwrap();
  std::fs::create_dir_all(bin_dir).with_context(|| {
    format!("failed to create directory {}", bin_dir.display())
  })?;
  // Install the binary atomically: copy to a temporary file in the same
  // directory and then rename it into place. The rename is atomic, so a
  // concurrent `deno lint` process never observes (via the `exists()` check
  // above) and tries to execute a half-written binary, which would otherwise
  // fail with ETXTBSY ("text file busy") on Linux.
  let tmp_path = bin_dir.join(format!(".tsgolint-{}.tmp", std::process::id()));
  std::fs::copy(&src, &tmp_path).with_context(|| {
    format!(
      "failed to copy tsgolint binary from {} to {}",
      src.display(),
      tmp_path.display()
    )
  })?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(
      &tmp_path,
      std::fs::Permissions::from_mode(0o755),
    );
  }
  match std::fs::rename(&tmp_path, &bin_path) {
    Ok(()) => {}
    // Another process won the race and installed the binary already; our copy
    // is redundant, so just discard it.
    Err(_) if bin_path.exists() => {
      let _ = std::fs::remove_file(&tmp_path);
    }
    Err(err) => {
      let _ = std::fs::remove_file(&tmp_path);
      return Err(err).with_context(|| {
        format!(
          "failed to move tsgolint binary into place at {}",
          bin_path.display()
        )
      });
    }
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

  Ok(bin_path)
}

/// Normalize a path to the string form tsgolint reports (absolute, forward
/// slashes). Canonicalizes so symlinked roots match the program's file names.
fn normalize_key(path: &Path) -> String {
  let canonical = crate::util::fs::canonicalize_path(path)
    .unwrap_or_else(|_| path.to_path_buf());
  normalize_str(&canonical.to_string_lossy())
}

fn normalize_str(s: &str) -> String {
  s.replace('\\', "/")
}

#[derive(Deserialize)]
struct HeadlessError {
  error: String,
}

#[derive(Deserialize)]
struct HeadlessRange {
  pos: usize,
  end: usize,
}

#[derive(Deserialize)]
struct HeadlessMessage {
  #[allow(dead_code, reason = "deserialized from tsgolint output but unused")]
  id: String,
  description: String,
  #[serde(default)]
  help: Option<String>,
}

#[derive(Deserialize)]
struct HeadlessDiagnostic {
  kind: u8,
  #[serde(default)]
  range: Option<HeadlessRange>,
  message: HeadlessMessage,
  file_path: Option<String>,
  #[serde(default)]
  rule: Option<String>,
}

#[cfg(test)]
mod test {
  use super::*;

  fn frame(msg_type: u8, json: &str) -> Vec<u8> {
    let body = json.as_bytes();
    let mut v = Vec::new();
    v.extend_from_slice(&(body.len() as u32).to_le_bytes());
    v.push(msg_type);
    v.extend_from_slice(body);
    v
  }

  #[test]
  fn parse_frames_collects_rule_diagnostics() {
    let mut buf = Vec::new();
    // a rule diagnostic
    buf.extend(frame(
      1,
      r#"{"kind":0,"range":{"pos":10,"end":20},"message":{"id":"x","description":"Promise not awaited","help":"await it"},"file_path":"/a/b.ts","rule":"no-floating-promises"}"#,
    ));
    // a timing message (ignored)
    buf.extend(frame(2, r#"{"rules":[]}"#));
    // an internal/tsconfig diagnostic (kind 1, ignored)
    buf.extend(frame(
      1,
      r#"{"kind":1,"message":{"id":"TS","description":"tsconfig issue"},"file_path":"/a/b.ts"}"#,
    ));

    let results =
      parse_frames(&buf, &["no-floating-promises".to_string()]).unwrap();
    let diags = results.by_file.get("/a/b.ts").unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "no-floating-promises");
    assert_eq!(diags[0].message, "Promise not awaited");
    assert_eq!(diags[0].hint.as_deref(), Some("await it"));
    assert_eq!(diags[0].range, Some((10, 20)));
    assert_eq!(results.enabled_rules, vec!["no-floating-promises"]);
  }

  #[test]
  fn parse_frames_surfaces_error_message() {
    let buf = frame(0, r#"{"error":"boom"}"#);
    let err = parse_frames(&buf, &[]).unwrap_err();
    assert!(err.to_string().contains("boom"));
  }

  #[test]
  fn resolve_rules_applies_include_and_exclude() {
    let config = LintRulesConfig {
      tags: None,
      include: Some(vec!["no-deprecated".to_string()]),
      exclude: Some(vec!["no-floating-promises".to_string()]),
    };
    let rules = resolve_rules(&config);
    assert!(rules.contains(&"no-deprecated".to_string()));
    assert!(!rules.contains(&"no-floating-promises".to_string()));
    // a default rule that wasn't excluded is still present
    assert!(rules.contains(&"await-thenable".to_string()));
    // unknown include entries are ignored
    assert!(!rules.contains(&"totally-made-up".to_string()));
  }
}
