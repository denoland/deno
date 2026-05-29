// Copyright 2018-2026 the Deno authors. MIT license.

//! MVP support for `deno publish --npm`.
//!
//! Reads `package.json` from the current directory, builds an
//! npm-compatible tarball, and publishes it to the npm registry via the
//! standard `PUT /:pkg` upload endpoint.
//!
//! Out of scope for the MVP: provenance attestation, 2FA OTP retry,
//! workspaces, and Deno-style projects (deno.json -> npm transpilation).
//! See follow-up issues.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_npmrc::NPM_DEFAULT_REGISTRY;
use deno_npmrc::NpmRc;
use deno_terminal::colors;
use flate2::Compression;
use flate2::write::GzEncoder;
use http_body_util::BodyExt;
use sha1::Digest as _;
use sha1::Sha1;
use sha2::Sha512;

use crate::args::Flags;
use crate::args::PublishFlags;
use crate::factory::CliFactory;
use crate::http_util::HttpClient;
use crate::util::display::human_size;
use crate::util::git::check_if_git_repo_dirty;

pub async fn publish(
  flags: Arc<Flags>,
  publish_flags: PublishFlags,
) -> Result<(), AnyError> {
  let cli_factory = CliFactory::from_flags(flags);
  let cli_options = cli_factory.cli_options()?;
  let cwd = cli_options.initial_cwd().to_path_buf();

  let pkg_json_path = cwd.join("package.json");
  let pkg_json_text =
    std::fs::read_to_string(&pkg_json_path).with_context(|| {
      format!(
        "Failed to read package.json at {}. `deno publish --npm` currently \
         requires a package.json in the working directory.",
        pkg_json_path.display()
      )
    })?;
  let pkg_json: Value = serde_json::from_str(&pkg_json_text)
    .with_context(|| format!("Invalid JSON in {}", pkg_json_path.display()))?;

  let name = pkg_json
    .get("name")
    .and_then(Value::as_str)
    .with_context(|| {
      format!("Missing 'name' field in {}", pkg_json_path.display())
    })?
    .to_string();

  let version = match publish_flags.set_version.as_ref() {
    Some(v) => v.clone(),
    None => pkg_json
      .get("version")
      .and_then(Value::as_str)
      .with_context(|| {
        format!(
          "Missing 'version' field in {}. Add a version or pass --set-version.",
          pkg_json_path.display()
        )
      })?
      .to_string(),
  };

  validate_npm_name(&name)?;
  validate_semver(&version)?;

  // Apply --set-version by rewriting the version inside the in-memory package.json
  // text that we will ship inside the tarball + manifest.
  let (effective_pkg_json_text, effective_pkg_json_value) =
    if publish_flags.set_version.is_some() {
      let mut v = pkg_json.clone();
      v["version"] = Value::String(version.clone());
      (serde_json::to_string_pretty(&v)? + "\n", v)
    } else {
      (pkg_json_text.clone(), pkg_json.clone())
    };

  if std::env::var("DENO_TESTING_DISABLE_GIT_CHECK").is_err()
    && !publish_flags.allow_dirty
    && let Some(dirty_text) = check_if_git_repo_dirty(&cwd).await
  {
    log::error!("\nUncommitted changes:\n\n{}\n", dirty_text);
    bail!(
      "Aborting due to uncommitted changes. Check in source code or run with --allow-dirty"
    );
  }

  let files = collect_files(&cwd, &effective_pkg_json_value)?;

  log::info!(
    "{} {}@{} to npm",
    colors::green_bold("Preparing"),
    colors::gray(&name),
    colors::gray(&version),
  );
  for (rel, size) in &files {
    log::info!("   {} ({})", rel, human_size(*size as f64));
  }

  let tarball = build_tarball(&cwd, &files, &effective_pkg_json_text)
    .with_context(|| {
      format!("Failed to build npm tarball for {}@{}", name, version)
    })?;

  let sha1_hex = {
    let mut h = Sha1::new();
    h.update(&tarball);
    let digest = h.finalize();
    digest
      .iter()
      .map(|b| format!("{:02x}", b))
      .collect::<String>()
  };
  let sha512_b64 = {
    let mut h = Sha512::new();
    h.update(&tarball);
    BASE64_STANDARD.encode(h.finalize())
  };
  let integrity = format!("sha512-{}", sha512_b64);

  log::info!(
    "Tarball: {} ({})",
    tarball_filename(&name, &version),
    human_size(tarball.len() as f64)
  );
  log::info!("Integrity: {}", integrity);

  if publish_flags.dry_run {
    log::warn!("{} Dry run complete", colors::green("Success"));
    return Ok(());
  }

  if publish_flags.no_provenance {
    // No-op for MVP: npm provenance is a follow-up. Accepting the flag
    // keeps the surface consistent with the JSR path.
  }

  let token = resolve_npm_token(publish_flags.token.as_deref(), &cwd)?;
  let registry = NPM_DEFAULT_REGISTRY;

  let manifest = build_publish_manifest(
    &name,
    &version,
    &effective_pkg_json_value,
    &tarball,
    &sha1_hex,
    &integrity,
    registry,
  );

  let http_client = cli_factory.http_client_provider().get_or_create()?;
  upload(&http_client, registry, &name, &token, &manifest).await?;

  log::info!(
    "{} {}@{} to npm",
    colors::green_bold("Successfully published"),
    name,
    version,
  );
  Ok(())
}

fn validate_npm_name(name: &str) -> Result<(), AnyError> {
  // Minimal sanity check. npm has a fuller spec (validate-npm-package-name)
  // but the registry will reject invalid names; we just block obvious
  // path-traversal and emptiness here.
  if name.is_empty() {
    bail!("package name is empty");
  }
  if name.contains("..") || name.starts_with('.') || name.starts_with('_') {
    bail!("invalid npm package name: '{}'", name);
  }
  Ok(())
}

fn validate_semver(version: &str) -> Result<(), AnyError> {
  if deno_semver::Version::parse_standard(version).is_err() {
    bail!(
      "invalid semver version '{}'. Provide a standard semver like 1.2.3",
      version
    );
  }
  Ok(())
}

/// Collect files to ship in the tarball.
///
/// MVP rules:
/// - If package.json has a `files` array, include only entries matching those
///   patterns plus package.json/README/LICENSE.
/// - Otherwise include everything in the package directory minus a hardcoded
///   ignore list (node_modules, .git, .env*, .DS_Store, *.log).
///
/// Always excluded: node_modules, .git, .env*, .DS_Store, *-debug.log,
/// package-lock.json's siblings stay (npm itself keeps them).
fn collect_files(
  root: &Path,
  pkg_json: &Value,
) -> Result<Vec<(String, u64)>, AnyError> {
  let files_field: Option<Vec<String>> =
    pkg_json.get("files").and_then(Value::as_array).map(|arr| {
      arr
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
    });

  let mut out: Vec<(String, u64)> = Vec::new();
  for entry in walkdir::WalkDir::new(root)
    .follow_links(false)
    .sort_by_file_name()
  {
    let entry = entry?;
    if !entry.file_type().is_file() {
      continue;
    }
    let abs = entry.path();
    let rel = abs.strip_prefix(root).unwrap();
    let rel_str = rel.to_string_lossy().replace('\\', "/");

    if is_default_ignored(&rel_str) {
      continue;
    }
    if let Some(ref patterns) = files_field
      && !is_always_included(&rel_str)
      && !files_pattern_matches(patterns, &rel_str)
    {
      continue;
    }

    let size = entry.metadata()?.len();
    out.push((rel_str, size));
  }

  if !out.iter().any(|(p, _)| p == "package.json") {
    bail!("package.json was not found after collection (this is a bug)");
  }

  Ok(out)
}

fn is_default_ignored(rel: &str) -> bool {
  let first = rel.split('/').next().unwrap_or("");
  if first == "node_modules" || first == ".git" || first == ".svn" {
    return true;
  }
  let name = rel.rsplit('/').next().unwrap_or(rel);
  if name == ".DS_Store" || name == ".npmrc" || name == ".npmignore" {
    return true;
  }
  if name.starts_with(".env") || name.ends_with("-debug.log") {
    return true;
  }
  false
}

fn is_always_included(rel: &str) -> bool {
  if rel == "package.json" {
    return true;
  }
  let lower = rel.to_ascii_lowercase();
  matches!(
    lower.as_str(),
    "readme"
      | "readme.md"
      | "readme.markdown"
      | "license"
      | "license.md"
      | "license.txt"
      | "licence"
      | "licence.md"
      | "licence.txt"
  )
}

/// Match a relative path against a `files` pattern. Supports a literal
/// prefix and a single trailing `*` glob - good enough for MVP. For full
/// glob support we will likely defer to `deno_config::glob` in a follow-up.
fn files_pattern_matches(patterns: &[String], rel: &str) -> bool {
  for p in patterns {
    let p = p.trim_start_matches("./");
    if p == rel {
      return true;
    }
    if let Some(prefix) = p.strip_suffix("/*")
      && let Some(rest) = rel.strip_prefix(prefix)
      && rest.starts_with('/')
      && !rest[1..].contains('/')
    {
      return true;
    }
    if let Some(prefix) = p.strip_suffix("/**")
      && (rel == prefix || rel.starts_with(&format!("{}/", prefix)))
    {
      return true;
    }
    // Bare directory entry like "lib" => include "lib/..."
    if rel.starts_with(&format!("{}/", p)) {
      return true;
    }
  }
  false
}

fn tarball_filename(name: &str, version: &str) -> String {
  let normalized = name.trim_start_matches('@').replace('/', "-");
  format!("{}-{}.tgz", normalized, version)
}

fn build_tarball(
  root: &Path,
  files: &[(String, u64)],
  pkg_json_text: &str,
) -> Result<Vec<u8>, AnyError> {
  let buf = Vec::with_capacity(1024 * 16);
  let enc = GzEncoder::new(buf, Compression::default());
  let mut tar = tar::Builder::new(enc);

  // Always write package.json first from the in-memory text so that
  // --set-version takes effect even if the on-disk file is different.
  append_reproducible(
    &mut tar,
    "package/package.json",
    pkg_json_text.as_bytes(),
  )?;

  for (rel, _) in files {
    if rel == "package.json" {
      continue;
    }
    let abs = root.join(rel);
    let bytes = std::fs::read(&abs)
      .with_context(|| format!("Failed to read {}", abs.display()))?;
    append_reproducible(&mut tar, &format!("package/{}", rel), &bytes)?;
  }

  let enc = tar.into_inner()?;
  Ok(enc.finish()?)
}

fn append_reproducible(
  tar: &mut tar::Builder<impl std::io::Write>,
  path: &str,
  bytes: &[u8],
) -> std::io::Result<()> {
  let mut header = tar::Header::new_gnu();
  header.set_path(path)?;
  header.set_size(bytes.len() as u64);
  header.set_mode(0o644);
  header.set_mtime(0);
  header.set_uid(0);
  header.set_gid(0);
  header.set_cksum();
  tar.append(&header, bytes)
}

/// Resolve an npm bearer token in priority order:
/// 1. `--token` flag
/// 2. `NODE_AUTH_TOKEN` / `NPM_TOKEN` env vars
/// 3. `~/.npmrc` `//registry.npmjs.org/:_authToken=...`
/// 4. `<cwd>/.npmrc` (project-local override)
fn resolve_npm_token(
  flag_token: Option<&str>,
  cwd: &Path,
) -> Result<String, AnyError> {
  if let Some(t) = flag_token {
    return Ok(t.to_string());
  }
  if let Ok(t) = std::env::var("NODE_AUTH_TOKEN")
    && !t.is_empty()
  {
    return Ok(t);
  }
  if let Ok(t) = std::env::var("NPM_TOKEN")
    && !t.is_empty()
  {
    return Ok(t);
  }
  // Local .npmrc (project) takes precedence over user-level .npmrc.
  let candidates = [Some(cwd.join(".npmrc")), home_npmrc()];
  for npmrc_path in candidates.iter().flatten() {
    if let Some(token) = read_npmrc_token(npmrc_path)? {
      return Ok(token);
    }
  }
  bail!(
    "No npm auth token found. Pass --token, set NPM_TOKEN, or run `npm login` to populate ~/.npmrc."
  )
}

fn home_npmrc() -> Option<PathBuf> {
  // The npm CLI honors `$HOME/.npmrc` on POSIX and `%USERPROFILE%\.npmrc` on
  // Windows. `std::env::home_dir` was undeprecated in Rust 1.86; we use the
  // env var directly to avoid the platform divergence dance.
  #[cfg(windows)]
  let home = std::env::var_os("USERPROFILE").map(PathBuf::from);
  #[cfg(not(windows))]
  let home = std::env::var_os("HOME").map(PathBuf::from);
  home.map(|h| h.join(".npmrc"))
}

fn read_npmrc_token(path: &Path) -> Result<Option<String>, AnyError> {
  let Ok(text) = std::fs::read_to_string(path) else {
    return Ok(None);
  };
  let rc = NpmRc::parse(&sys_traits::impls::RealSys, &text)
    .with_context(|| format!("Failed to parse {}", path.display()))?;
  // Look for the default npm registry config first.
  let host_and_path = "registry.npmjs.org/";
  if let Some(cfg) = rc.registry_configs.get(host_and_path)
    && let Some(token) = &cfg.auth_token
  {
    return Ok(Some(token.clone()));
  }
  Ok(None)
}

fn build_publish_manifest(
  name: &str,
  version: &str,
  pkg_json: &Value,
  tarball: &[u8],
  sha1_hex: &str,
  integrity: &str,
  registry: &str,
) -> Value {
  let tar_name = tarball_filename(name, version);
  let tarball_url =
    format!("{}/{}/-/{}", registry.trim_end_matches('/'), name, tar_name);

  let mut version_meta = pkg_json.clone();
  version_meta["_id"] = json!(format!("{}@{}", name, version));
  version_meta["dist"] = json!({
    "shasum": sha1_hex,
    "integrity": integrity,
    "tarball": tarball_url,
  });

  json!({
    "_id": name,
    "name": name,
    "description": pkg_json.get("description").cloned().unwrap_or(Value::Null),
    "dist-tags": { "latest": version },
    "versions": { version: version_meta },
    "access": "public",
    "_attachments": {
      tar_name: {
        "content_type": "application/octet-stream",
        "data": BASE64_STANDARD.encode(tarball),
        "length": tarball.len(),
      }
    }
  })
}

async fn upload(
  client: &HttpClient,
  registry: &str,
  name: &str,
  token: &str,
  manifest: &Value,
) -> Result<(), AnyError> {
  let url_str = format!(
    "{}/{}",
    registry.trim_end_matches('/'),
    url_encode_pkg(name)
  );
  let url = Url::parse(&url_str)
    .with_context(|| format!("Invalid registry URL: {}", url_str))?;

  let response = client
    .put_json(url, manifest)?
    .header(
      http::header::AUTHORIZATION,
      format!("Bearer {}", token).parse()?,
    )
    .header(http::header::ACCEPT, "application/json".parse()?)
    .send()
    .await?;

  let status = response.status();
  if !status.is_success() {
    let body = response
      .into_body()
      .collect()
      .await
      .map(|b| String::from_utf8_lossy(&b.to_bytes()).to_string())
      .unwrap_or_default();
    bail!(
      "npm publish failed: HTTP {}\n{}",
      status,
      body.chars().take(2000).collect::<String>()
    );
  }
  Ok(())
}

/// URL-encode the scope in `@scope/name` (`@` -> `%40`, `/` -> `%2f`) per
/// npm's publish endpoint convention. Unscoped names pass through unchanged.
fn url_encode_pkg(name: &str) -> String {
  if let Some(rest) = name.strip_prefix('@')
    && let Some((scope, pkg)) = rest.split_once('/')
  {
    format!("@{}%2f{}", scope, pkg)
  } else {
    name.to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn url_encode_scoped() {
    assert_eq!(url_encode_pkg("@scope/pkg"), "@scope%2fpkg");
    assert_eq!(url_encode_pkg("pkg"), "pkg");
  }

  #[test]
  fn tarball_naming() {
    assert_eq!(
      tarball_filename("@scope/pkg", "1.2.3"),
      "scope-pkg-1.2.3.tgz"
    );
    assert_eq!(tarball_filename("pkg", "0.0.1"), "pkg-0.0.1.tgz");
  }

  #[test]
  fn files_pattern_matching() {
    let patterns = vec!["lib".to_string(), "dist/*".to_string()];
    assert!(files_pattern_matches(&patterns, "lib/a.js"));
    assert!(files_pattern_matches(&patterns, "lib/sub/a.js"));
    assert!(files_pattern_matches(&patterns, "dist/a.js"));
    assert!(!files_pattern_matches(&patterns, "dist/sub/a.js"));
    assert!(!files_pattern_matches(&patterns, "src/a.js"));
  }

  #[test]
  fn default_ignores() {
    assert!(is_default_ignored("node_modules/foo/bar.js"));
    assert!(is_default_ignored(".git/HEAD"));
    assert!(is_default_ignored(".env.local"));
    assert!(is_default_ignored("npm-debug.log"));
    assert!(!is_default_ignored("src/index.ts"));
  }
}
