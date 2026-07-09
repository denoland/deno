// Copyright 2018-2026 the Deno authors. MIT license.

//! Post-install setup for stock TypeScript compatibility.
//!
//! After `deno install` sets up node_modules/, this module:
//! 1. Installs jsr: packages to node_modules/@jsr/ via npm.jsr.io
//! 2. Mirrors http(s): modules into .deno/remote/<host><path>/...
//! 3. Generates .deno/tsconfig.json with paths mappings for npm:/jsr:/https:
//!
//! This enables stock TypeScript tooling (tsc, tsserver, VS Code) to work
//! with Deno projects that use jsr:, npm:, and http(s): specifiers.

use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_semver::Version;
use deno_semver::VersionReq;
use flate2::read::GzDecoder;

use crate::file_fetcher::CliFileFetcher;
use crate::http_util::HttpClient;

/// Installed JSR package info for reporting.
pub struct InstalledJsrPackage {
  /// e.g. "@jsr/std__assert"
  pub name: String,
  /// e.g. "1.0.19"
  pub version: String,
}

/// Run post-install setup: install jsr packages and generate tsconfig.
///
/// Called after `deno install` completes npm resolution and node_modules setup.
/// Returns the list of newly installed JSR packages for reporting.
fn is_special_specifier(s: &str) -> bool {
  s.starts_with("jsr:")
    || s.starts_with("npm:")
    || s.starts_with("http://")
    || s.starts_with("https://")
}

pub async fn setup_npm_compat(
  project_root: &Path,
  file_fetcher: &CliFileFetcher,
  http_client: &HttpClient,
  permissions: &PermissionsContainer,
  graph_specifiers: &[String],
) -> Result<Vec<InstalledJsrPackage>, AnyError> {
  let deno_json = read_deno_json(project_root)?;
  let deno_compiler_options = deno_json
    .as_ref()
    .and_then(|d| d.get("compilerOptions"))
    .cloned();

  // Workspace member aliases (`@std/assert` -> the local `./assert` member's
  // exports) shadow any published jsr mapping, so compute them up front and let
  // them win in the generated `paths`.
  let member_paths = deno_json
    .as_ref()
    .map(|d| workspace_member_paths(project_root, d))
    .unwrap_or_default();

  // Combine the root import map (inline `imports`, or an `importMap` file) with
  // the specifiers discovered in the module graph. Graph specifiers (e.g.
  // `jsr:@std/path` written directly in source) are keyed by themselves so the
  // import-map-driven generation maps them like an alias.
  let mut combined = deno_json
    .as_ref()
    .and_then(|d| d.get("imports"))
    .and_then(|v| v.as_object())
    .cloned()
    .unwrap_or_default();
  if let Some(map_imports) = deno_json
    .as_ref()
    .and_then(|d| read_referenced_import_map(project_root, d))
  {
    for (k, v) in map_imports {
      combined.entry(k).or_insert(v);
    }
  }
  for spec in graph_specifiers {
    combined
      .entry(spec.clone())
      .or_insert_with(|| Value::String(spec.clone()));
  }

  let has_special_specifiers = combined.iter().any(|(k, v)| {
    is_special_specifier(k) || v.as_str().is_some_and(is_special_specifier)
  });
  // Generate if there's anything to map: external specifiers, or a workspace
  // whose members need local-path aliases (e.g. std).
  if !has_special_specifiers && member_paths.is_empty() {
    return Ok(vec![]);
  }

  let combined_imports = Value::Object(combined);
  let deno_imports = Some(&combined_imports);
  let deno_compiler_options = deno_compiler_options.as_ref();

  // Install jsr: packages to node_modules/@jsr/
  let installed =
    install_jsr_packages(project_root, deno_imports, http_client).await?;

  // Mirror http(s): modules (and their transitive remote/relative imports)
  // into .deno/remote/<host><path>/...
  let http_modules = match install_http_modules(
    project_root,
    deno_imports,
    file_fetcher,
    permissions,
  )
  .await
  {
    Ok(map) => map,
    Err(e) => {
      log::warn!("Failed to materialize remote modules: {e}");
      BTreeMap::new()
    }
  };

  // Ensure @types/node is available so Node globals (timers, node: builtins,
  // Buffer, URLPattern, ...) resolve under stock tooling.
  let has_node_types = ensure_types_node(project_root, http_client).await;

  // Generate .deno/tsconfig.json and ensure root tsconfig.json extends it
  generate_deno_tsconfig(
    project_root,
    deno_compiler_options,
    deno_imports,
    &http_modules,
    &member_paths,
    has_node_types,
  )?;

  Ok(installed)
}

/// Ensure a stock `@types/node` (and its `undici-types` dependency) is present
/// in `node_modules/@types` so the generated tsconfig can load Node globals
/// (timers, `node:` builtins, `Buffer`, `URLPattern`, ...). No-op when the
/// project already has `@types/node` installed. Returns whether it's available.
///
/// This is the interim: once #35889 lands, node types come from the global
/// cache instead of being materialized per-project here.
async fn ensure_types_node(
  project_root: &Path,
  http_client: &HttpClient,
) -> bool {
  if project_root.join("node_modules/@types/node").exists() {
    return true;
  }
  match download_npm_package(project_root, "@types/node", None, http_client)
    .await
  {
    Ok(Some((_version, deps))) => {
      if let Some(req) = deps.get("undici-types").and_then(|v| v.as_str()) {
        let _ = download_npm_package(
          project_root,
          "undici-types",
          Some(req),
          http_client,
        )
        .await;
      }
      project_root.join("node_modules/@types/node").exists()
    }
    _ => false,
  }
}

/// Download an npm package tarball from registry.npmjs.org into
/// `node_modules/<pkg>` and return its resolved version + dependency map.
/// Resolves `req` (a semver range) if given, otherwise the `latest` dist-tag.
async fn download_npm_package(
  project_root: &Path,
  pkg: &str,
  req: Option<&str>,
  http_client: &HttpClient,
) -> Result<Option<(String, serde_json::Map<String, Value>)>, AnyError> {
  let meta_url =
    format!("https://registry.npmjs.org/{}", pkg.replace('/', "%2f"));
  let bytes = match http_client.download(Url::parse(&meta_url)?).await {
    Ok(b) => b,
    Err(e) => {
      log::debug!("Failed to fetch metadata for {pkg}: {e}");
      return Ok(None);
    }
  };
  let meta: Value = serde_json::from_slice(&bytes)?;
  let version = match req {
    Some(r) => resolve_version_req(&meta, r),
    None => meta
      .get("dist-tags")
      .and_then(|t| t.get("latest"))
      .and_then(|v| v.as_str())
      .map(String::from),
  };
  let Some(version) = version else {
    return Ok(None);
  };
  let vinfo = meta.get("versions").and_then(|vs| vs.get(&version));
  let Some(tarball) = vinfo
    .and_then(|v| v.get("dist"))
    .and_then(|d| d.get("tarball"))
    .and_then(|t| t.as_str())
  else {
    return Ok(None);
  };
  let tb = http_client.download(Url::parse(tarball)?).await?;
  let dest = project_root.join("node_modules").join(pkg);
  if let Err(e) = extract_tarball_gz(&tb, &dest) {
    log::debug!("Failed to extract {pkg}: {e}");
    let _ = std::fs::remove_dir_all(&dest);
    return Ok(None);
  }
  let deps = vinfo
    .and_then(|v| v.get("dependencies"))
    .and_then(|d| d.as_object())
    .cloned()
    .unwrap_or_default();
  Ok(Some((version, deps)))
}

/// Highest version in the packument satisfying the npm semver range `req`.
fn resolve_version_req(meta: &Value, req: &str) -> Option<String> {
  let vr = VersionReq::parse_from_npm(req).ok()?;
  let versions = meta.get("versions")?.as_object()?;
  versions
    .keys()
    .filter_map(|v| Version::parse_from_npm(v).ok())
    .filter(|v| vr.matches(v))
    .max()
    .map(|v| v.to_string())
}

/// If `deno_json` references an external import map via `importMap`, read that
/// file and return its `imports` object.
fn read_referenced_import_map(
  project_root: &Path,
  deno_json: &Value,
) -> Option<serde_json::Map<String, Value>> {
  let rel = deno_json.get("importMap").and_then(|v| v.as_str())?;
  let content = std::fs::read_to_string(project_root.join(rel)).ok()?;
  let parsed: Value = serde_json::from_str(&content).ok()?;
  parsed.get("imports").and_then(|v| v.as_object()).cloned()
}

/// Build tsconfig `paths` entries that map each workspace member's package name
/// (and subpath exports) to its local source files, so stock tooling resolves
/// e.g. `@std/assert` and `@std/assert/equals` to `../assert/mod.ts` and
/// `../assert/equals.ts` rather than a published copy. Paths are relative to
/// the generated `.deno/tsconfig.json`.
fn workspace_member_paths(
  project_root: &Path,
  deno_json: &Value,
) -> serde_json::Map<String, Value> {
  let mut paths = serde_json::Map::new();
  let Some(members) = deno_json.get("workspace").and_then(|w| w.as_array())
  else {
    return paths;
  };
  for member in members.iter().filter_map(|m| m.as_str()) {
    let member_rel = member.trim_start_matches("./").trim_end_matches('/');
    let Some(member_json) = read_deno_json(&project_root.join(member_rel))
      .ok()
      .flatten()
    else {
      continue;
    };
    let Some(name) = member_json.get("name").and_then(|n| n.as_str()) else {
      continue;
    };
    let Some(exports) = member_json.get("exports") else {
      continue;
    };
    let mut add = |sub: &str, file: &str| {
      let file = file.trim_start_matches("./");
      let target = format!("../{member_rel}/{file}");
      let key = if sub == "." {
        name.to_string()
      } else {
        format!("{name}/{}", sub.trim_start_matches("./"))
      };
      paths.insert(key, json!([target]));
    };
    match exports {
      // `"exports": "./mod.ts"`
      Value::String(file) => add(".", file),
      // `"exports": { ".": "./mod.ts", "./x": "./x.ts" }`
      Value::Object(map) => {
        for (sub, target) in map {
          if let Some(file) = target.as_str() {
            add(sub, file);
          }
        }
      }
      _ => {}
    }
  }
  paths
}

fn read_deno_json(project_root: &Path) -> Result<Option<Value>, AnyError> {
  let deno_json_path = project_root.join("deno.json");
  let deno_jsonc_path = project_root.join("deno.jsonc");

  if deno_json_path.exists() {
    let content = std::fs::read_to_string(&deno_json_path)?;
    Ok(Some(serde_json::from_str(&content)?))
  } else if deno_jsonc_path.exists() {
    let content = std::fs::read_to_string(&deno_jsonc_path)?;
    let parsed: Option<Value> = jsonc_parser::parse_to_serde_value(
      &content,
      &jsonc_parser::ParseOptions::default(),
    )?;
    Ok(Some(parsed.unwrap_or(json!({}))))
  } else {
    Ok(None)
  }
}

/// Generate tsconfig.deno.json at the project root with paths mappings.
#[allow(clippy::too_many_arguments)]
fn generate_deno_tsconfig(
  project_root: &Path,
  deno_compiler_options: Option<&Value>,
  deno_imports: Option<&Value>,
  http_modules: &BTreeMap<Url, String>,
  member_paths: &serde_json::Map<String, Value>,
  has_node_types: bool,
) -> Result<(), AnyError> {
  let generated = crate::tsc::tsconfig_gen::generate_tsconfig(
    project_root,
    deno_compiler_options,
    deno_imports,
    &[],
    http_modules,
    member_paths,
    has_node_types,
  )
  .map_err(|e| anyhow!("Failed to generate tsconfig: {e}"))?;

  log::debug!("Generated {}", generated.tsconfig_path.display());

  Ok(())
}

/// Install jsr: packages to node_modules/@jsr/ by downloading from npm.jsr.io.
///
/// Uses `HttpClient::download` directly (not `CliFileFetcher`) because we
/// don't want each registry/tarball URL surfacing as a user-visible
/// "Download …" line in `deno install` output — those are an
/// implementation detail of the stock-tsc compatibility setup. The
/// fetcher's caching and redirect handling are unnecessary for npm.jsr.io.
async fn install_jsr_packages(
  project_root: &Path,
  deno_imports: Option<&Value>,
  http_client: &HttpClient,
) -> Result<Vec<InstalledJsrPackage>, AnyError> {
  let mut installed = Vec::new();
  let imports = match deno_imports.and_then(|v| v.as_object()) {
    Some(imports) => imports,
    None => return Ok(installed),
  };

  for (_alias, target) in imports {
    let target_str = match target.as_str() {
      Some(s) if s.starts_with("jsr:") => s,
      _ => continue,
    };

    let Some((scope, name, req_version)) =
      crate::tsc::tsconfig_gen::parse_jsr_specifier(target_str)
    else {
      continue;
    };

    let npm_name = format!("{}__{}", scope.trim_start_matches('@'), name);
    let pkg_dir = project_root
      .join("node_modules")
      .join("@jsr")
      .join(&npm_name);
    if pkg_dir.exists() {
      continue;
    }

    let registry_name = format!("@jsr/{npm_name}");
    let npm_jsr_registry = std::env::var("DENO_NPM_JSR_REGISTRY")
      .unwrap_or_else(|_| "https://npm.jsr.io".to_string());
    let metadata_url = format!(
      "{}/{}",
      npm_jsr_registry.trim_end_matches('/'),
      registry_name.replace('/', "%2f")
    );
    let metadata_url = match Url::parse(&metadata_url) {
      Ok(u) => u,
      Err(e) => {
        log::debug!("Invalid jsr registry URL {metadata_url}: {e}");
        continue;
      }
    };

    log::debug!("Installing {} from {}", registry_name, npm_jsr_registry);

    let metadata_bytes = match http_client.download(metadata_url).await {
      Ok(b) => b,
      Err(e) => {
        log::debug!("Failed to fetch metadata for {registry_name}: {e}");
        continue;
      }
    };

    let metadata: Value =
      serde_json::from_slice(&metadata_bytes).map_err(|e| {
        anyhow!("Failed to parse metadata for {registry_name}: {e}")
      })?;

    let resolved_version =
      resolve_jsr_version(&metadata, req_version.as_deref(), &registry_name)?;

    let tarball_url = metadata
      .get("versions")
      .and_then(|vs| vs.get(&resolved_version))
      .and_then(|v| v.get("dist"))
      .and_then(|d| d.get("tarball"))
      .and_then(|t| t.as_str())
      .ok_or_else(|| {
        anyhow!("No tarball URL for {registry_name}@{resolved_version}")
      })?;
    let tarball_url = match Url::parse(tarball_url) {
      Ok(u) => u,
      Err(e) => {
        log::debug!(
          "Invalid tarball URL {tarball_url} for {registry_name}: {e}"
        );
        continue;
      }
    };

    let tarball_bytes = match http_client.download(tarball_url).await {
      Ok(b) => b,
      Err(e) => {
        log::debug!("Failed to download {registry_name}: {e}");
        continue;
      }
    };

    if let Err(e) = extract_tarball_gz(&tarball_bytes, &pkg_dir) {
      log::debug!("Failed to extract {registry_name}: {e}");
      let _ = std::fs::remove_dir_all(&pkg_dir);
      continue;
    }

    installed.push(InstalledJsrPackage {
      name: registry_name,
      version: resolved_version,
    });
  }

  Ok(installed)
}

/// Extract a gzipped npm-style tarball into `dest`, stripping the leading
/// `package/` directory the way `tar --strip-components=1` does. Replaces
/// the previous `tar` shell-out so the install works the same on Linux,
/// macOS (BSD tar), Windows, and minimal containers without `tar`/`curl`.
fn extract_tarball_gz(gz_bytes: &[u8], dest: &Path) -> Result<(), AnyError> {
  std::fs::create_dir_all(dest)?;
  let mut archive = tar::Archive::new(GzDecoder::new(gz_bytes));
  for entry in archive.entries()? {
    let mut entry = entry?;
    let path = entry.path()?.into_owned();
    // Skip the leading "package/" (or whatever the single root dir is named).
    let stripped: PathBuf = path.components().skip(1).collect();
    if stripped.as_os_str().is_empty() {
      continue;
    }
    if stripped.is_absolute()
      || stripped.components().any(|c| {
        matches!(
          c,
          std::path::Component::ParentDir | std::path::Component::RootDir
        )
      })
    {
      return Err(anyhow!(
        "Refusing to extract tar entry outside dest: {}",
        path.display()
      ));
    }
    let out_path = dest.join(stripped);
    // tar entries aren't guaranteed to list a directory before the files under
    // it, and `Entry::unpack` won't create missing parents — so ensure the
    // parent exists first (otherwise nested files like `_dist/mod.d.ts` fail).
    if let Some(parent) = out_path.parent() {
      std::fs::create_dir_all(parent)?;
    }
    entry.unpack(&out_path)?;
  }
  Ok(())
}

/// A resolved entry for the tsconfig `paths` table: maps a user-facing URL
/// (whatever appears in source as `import "..."`) to the local mirror file
/// path (relative to `.deno/`) that tsc should resolve it to.
pub type HttpModulePaths = BTreeMap<Url, String>;

/// Materialize http(s): modules referenced from `deno.json` `imports` (and
/// their transitive remote/relative imports) into `.deno/remote/<host><path>`.
///
/// Uses `CliFileFetcher`, which transparently reuses `DENO_DIR` cache, follows
/// redirects, and exposes response headers. When a module advertises types via
/// the `X-TypeScript-Types` header (esm.sh and friends), the types URL is
/// fetched too and the paths entry for the original URL is pointed at the
/// types file so tsc gets real declarations.
///
/// Module specifier discovery within source still uses a regex scanner — this
/// is acknowledged tech debt; a deno_graph walk is the production answer.
///
/// Returns a map from user-facing URL to the local mirror path (relative to
/// `.deno/`), suitable for direct use as tsconfig `paths` values.
pub async fn install_http_modules(
  project_root: &Path,
  deno_imports: Option<&Value>,
  file_fetcher: &CliFileFetcher,
  permissions: &PermissionsContainer,
) -> Result<HttpModulePaths, AnyError> {
  let mut paths: HttpModulePaths = BTreeMap::new();
  let mut queue: VecDeque<Url> = VecDeque::new();

  let Some(imports) = deno_imports.and_then(|v| v.as_object()) else {
    return Ok(paths);
  };

  for (_alias, target) in imports {
    let Some(s) = target.as_str() else {
      continue;
    };
    if !(s.starts_with("http://") || s.starts_with("https://")) {
      continue;
    }
    if let Ok(url) = Url::parse(s) {
      queue.push_back(url);
    }
  }

  if queue.is_empty() {
    return Ok(paths);
  }

  let remote_root = project_root.join(".deno").join("remote");

  while let Some(requested_url) = queue.pop_front() {
    if paths.contains_key(&requested_url) {
      continue;
    }

    // Fetch (cached) source + headers via the standard file fetcher, gated
    // by --allow-import the same way runtime http imports are. Follows
    // redirects; `file.url` is the canonical post-redirect URL.
    let file = match file_fetcher.fetch(&requested_url, permissions).await {
      Ok(f) => f,
      Err(e) => {
        log::warn!(
          "Skipping {requested_url}: {e} (try running `deno install --allow-import` to grant access)"
        );
        continue;
      }
    };
    let final_url = file.url.clone();

    // If the module advertises a separate types file (X-TypeScript-Types,
    // common on esm.sh), fetch *that* and use it as the source-of-truth for
    // both mirroring and import scanning. Otherwise use the module itself.
    let types_url = file
      .maybe_headers
      .as_ref()
      .and_then(|h| {
        h.get("x-typescript-types")
          .or_else(|| h.get("types"))
          .or_else(|| h.get("typescript-types"))
      })
      .and_then(|v| Url::parse(v).ok().or_else(|| final_url.join(v).ok()));

    let (effective_url, effective_bytes) = if let Some(t) = types_url.as_ref() {
      match file_fetcher.fetch(t, permissions).await {
        Ok(f) => (f.url.clone(), f.source.clone()),
        Err(e) => {
          log::debug!(
            "X-TypeScript-Types fetch failed for {t} ({e}), falling back to source"
          );
          (final_url.clone(), file.source.clone())
        }
      }
    } else {
      (final_url.clone(), file.source.clone())
    };

    // Mirror the chosen content. We deliberately don't mirror both the JS
    // and the .d.ts: their URL paths often collide on disk (e.g.
    // `cowsay@1.6.0` as file vs `cowsay@1.6.0/index.d.ts` under a dir).
    let local = match write_mirror(
      &remote_root,
      &effective_url,
      effective_bytes.as_ref(),
    ) {
      Ok(p) => p,
      Err(e) => {
        log::debug!("Failed to mirror {effective_url} ({e})");
        continue;
      }
    };
    // Emit paths entries for every URL form the user (or transitive imports)
    // might reference for this module — all pointing at the same mirror.
    paths.insert(requested_url.clone(), local.clone());
    if final_url != requested_url {
      paths.insert(final_url.clone(), local.clone());
    }
    if effective_url != final_url {
      paths.insert(effective_url.clone(), local.clone());
    }

    // Scan the effective (type-bearing) source for transitive imports.
    let scan_source =
      String::from_utf8_lossy(effective_bytes.as_ref()).into_owned();
    for spec in scan_import_specifiers(&effective_url, &scan_source) {
      let resolved =
        if spec.starts_with("http://") || spec.starts_with("https://") {
          Url::parse(&spec).ok()
        } else if spec.starts_with("./")
          || spec.starts_with("../")
          || spec.starts_with('/')
        {
          // `./`, `../`, and same-host absolute paths (e.g. `/src/foo.ts`,
          // common on deno.land/x and esm.sh CDNs) all resolve against the
          // effective URL via Url::join.
          effective_url.join(&spec).ok()
        } else {
          None
        };
      if let Some(child) = resolved
        && (child.scheme() == "http" || child.scheme() == "https")
        && !paths.contains_key(&child)
      {
        queue.push_back(child);
      }
    }
  }

  Ok(paths)
}

/// Write `bytes` to the local mirror file for `url`, returning the path
/// relative to `.deno/` (suitable for tsconfig `paths`).
fn write_mirror(
  remote_root: &Path,
  url: &Url,
  bytes: &[u8],
) -> Result<String, AnyError> {
  let local_path = url_to_local_path(remote_root, url)
    .ok_or_else(|| anyhow!("URL has no resolvable mirror path: {url}"))?;
  if let Some(parent) = local_path.parent() {
    std::fs::create_dir_all(parent)?;
  }
  std::fs::write(&local_path, bytes)?;
  url_to_tsconfig_path(url)
    .ok_or_else(|| anyhow!("URL has no resolvable mirror path: {url}"))
}

/// Compute the `paths`-relative mirror location for `url`, e.g.
/// `https://example.com/x/foo.ts` → `./remote/example.com/x/foo.ts`.
fn url_to_tsconfig_path(url: &Url) -> Option<String> {
  let host = url.host_str()?;
  let path = url.path();
  if path.ends_with('/') || path.is_empty() {
    return None;
  }
  let rel = path.trim_start_matches('/');
  Some(format!("./remote/{host}/{rel}"))
}

/// Map a URL to its mirror file path under `<remote_root>/<host><path>`.
/// Returns None for URLs that don't yield a sensible file path (e.g. ending
/// in `/`, since we can't infer an index filename without server cooperation).
fn url_to_local_path(remote_root: &Path, url: &Url) -> Option<PathBuf> {
  let host = url.host_str()?;
  let path = url.path();
  if path.ends_with('/') || path.is_empty() {
    // Directory-like URL — would need server to tell us the actual filename.
    // Skip for the prototype; full deno_graph handles this via redirects.
    return None;
  }
  // Strip leading '/', percent-decoding left as-is for the prototype.
  let rel = path.trim_start_matches('/');
  Some(remote_root.join(host).join(rel))
}

/// Extract module specifier string literals from a JS/TS source via
/// `deno_ast` + `deno_graph` analysis. Returns the raw specifier strings for
/// static imports/exports and string-literal dynamic imports. Template/expr
/// dynamic imports are skipped — they aren't statically analyzable.
///
/// Returns an empty vec if the source can't be parsed; the caller will simply
/// stop walking that branch.
fn scan_import_specifiers(specifier: &Url, source: &str) -> Vec<String> {
  use deno_ast::MediaType;
  use deno_graph::analysis::DependencyDescriptor;
  use deno_graph::analysis::DynamicArgument;

  let media_type = MediaType::from_specifier(specifier);
  if !matches!(
    media_type,
    MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::Mjs
      | MediaType::Cjs
      | MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Tsx
  ) {
    return Vec::new();
  }

  let parsed = match deno_ast::parse_module(deno_ast::ParseParams {
    specifier: specifier.clone(),
    text: source.into(),
    media_type,
    capture_tokens: false,
    maybe_syntax: None,
    scope_analysis: false,
  }) {
    Ok(p) => p,
    Err(e) => {
      log::debug!("Failed to parse {specifier} for import scan: {e}");
      return Vec::new();
    }
  };

  let module_info = deno_graph::ast::ParserModuleAnalyzer::module_info(&parsed);
  let mut out = Vec::new();
  for dep in &module_info.dependencies {
    match dep {
      DependencyDescriptor::Static(d) => out.push(d.specifier.to_string()),
      DependencyDescriptor::Dynamic(d) => {
        if let DynamicArgument::String(s) = &d.argument {
          out.push(s.clone());
        }
      }
    }
  }
  out
}

fn resolve_jsr_version(
  metadata: &Value,
  req_version: Option<&str>,
  registry_name: &str,
) -> Result<String, AnyError> {
  match req_version {
    None => metadata
      .get("dist-tags")
      .and_then(|dt| dt.get("latest"))
      .and_then(|v| v.as_str())
      .map(|s| s.to_string())
      .ok_or_else(|| anyhow!("No latest version for {registry_name}")),
    Some(req_str) => {
      if let Ok(exact) = Version::parse_standard(req_str)
        && metadata
          .get("versions")
          .and_then(|vs| vs.get(exact.to_string()))
          .is_some()
      {
        return Ok(exact.to_string());
      }

      let version_req = VersionReq::parse_from_npm(req_str)
        .map_err(|e| anyhow!("Invalid version req '{req_str}': {e}"))?;

      let versions = metadata
        .get("versions")
        .and_then(|vs| vs.as_object())
        .ok_or_else(|| anyhow!("No versions for {registry_name}"))?;

      let mut best: Option<Version> = None;
      for key in versions.keys() {
        if let Ok(v) = Version::parse_standard(key)
          && version_req.matches(&v)
          && best.as_ref().is_none_or(|b| v > *b)
        {
          best = Some(v);
        }
      }

      best.map(|v| v.to_string()).ok_or_else(|| {
        anyhow!("No version matching '{req_str}' for {registry_name}")
      })
    }
  }
}
