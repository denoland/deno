// Copyright 2018-2026 the Deno authors. MIT license.

//! Post-install setup for stock TypeScript compatibility.
//!
//! After dependency resolution, this module:
//! 1. Installs jsr: compatibility packages via npm.jsr.io
//! 2. Mirrors http(s): modules into .deno/remote/<host><path>/...
//! 3. Generates .deno/tsconfig.json with paths mappings for npm:/jsr:/https:
//! 4. In global-cache mode, generates per-package referenced tsconfigs so
//!    stock TypeScript can preserve Deno's contextual npm resolution.
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
use crate::npm::CliNpmResolver;

/// Installed JSR package info for reporting.
pub struct InstalledJsrPackage {
  /// e.g. "@jsr/std__assert"
  pub name: String,
  /// e.g. "1.0.19"
  pub version: String,
}

#[derive(Default)]
struct NpmCacheProjects {
  /// Exact npm: specifier/import-map target to its resolved package folder.
  package_paths: BTreeMap<String, PathBuf>,
  /// Paths relative to `.deno/tsconfig.json` for the generated package
  /// projects.
  references: Vec<String>,
}

struct NodeTypesSetup {
  type_root: Option<String>,
  undici_types_dir: Option<PathBuf>,
}

fn path_for_typescript(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

fn collect_typescript_project_files(package_folder: &Path) -> Vec<String> {
  fn collect(dir: &Path, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
      return;
    };
    for entry in entries.flatten() {
      let path = entry.path();
      let Ok(file_type) = entry.file_type() else {
        continue;
      };
      if file_type.is_symlink() {
        continue;
      }
      if file_type.is_dir() {
        collect(&path, files);
        continue;
      }
      let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
      if file_name.ends_with(".d.ts")
        || file_name.ends_with(".d.mts")
        || file_name.ends_with(".d.cts")
      {
        files.push(path_for_typescript(&path));
      }
    }
  }

  let mut files = Vec::new();
  collect(package_folder, &mut files);
  files.sort();
  files
}

/// Build the `paths` entries for a package project's edge to one dependency.
///
/// The bare `alias -> [folder]` mapping is always correct: stock tsc reads the
/// folder's own `package.json` (`types` / `exports["."]`) to resolve the root
/// import, so it is emitted unconditionally.
///
/// For subpath imports (`dep/subpath`), a literal `alias/* -> folder/*`
/// wildcard performs a raw path substitution that ignores the dependency's
/// `exports` map, so a transitive `import "dep/subpath"` where the dependency
/// remaps `./subpath` via `exports` mis-resolves to a nonexistent path. When
/// the dependency declares an `exports` map we instead emit an exact
/// `alias/<subpath> -> [<resolved declaration>]` entry per export key,
/// preferring the generated declaration (mirroring how the top-level
/// `generate_npm_paths` handles subpaths). The `alias/*` wildcard is kept only
/// as a fallback for packages without an `exports` map.
fn dependency_project_paths(
  alias: &str,
  dependency_folder: &Path,
) -> serde_json::Map<String, Value> {
  let mut paths = serde_json::Map::new();
  let folder = path_for_typescript(dependency_folder);
  paths.insert(alias.to_string(), json!([folder]));

  let export_keys =
    crate::tsc::tsconfig_gen::package_export_keys(dependency_folder);
  if export_keys.is_empty() {
    // No subpath exports to consult (root-only or no `exports` map): fall back
    // to a literal subpath wildcard so `dep/subpath` still resolves to
    // `folder/subpath`.
    paths.insert(format!("{alias}/*"), json!([format!("{folder}/*")]));
    return paths;
  }

  // Emit one exact mapping per declared subpath export. Export keys may
  // themselves be wildcards (`"./features/*"`); the resolved value keeps the
  // `*` and the emitted key is a matching wildcard, so those resolve too.
  for exp_key in export_keys {
    let sub = exp_key.trim_start_matches("./");
    let Some(resolved) =
      crate::tsc::tsconfig_gen::resolve_package_types_entry_path(
        dependency_folder,
        &exp_key,
      )
      .filter(|p| p.exists())
      .or_else(|| {
        crate::tsc::tsconfig_gen::resolve_package_source_entry_path(
          dependency_folder,
          &exp_key,
        )
      })
    else {
      continue;
    };
    paths.insert(
      format!("{alias}/{sub}"),
      json!([path_for_typescript(&resolved)]),
    );
  }
  paths
}

/// Generate a referenced TypeScript project for every resolved package copy in
/// Deno's global npm cache.
///
/// A package project owns the cached package's type/source files and maps every
/// bare dependency name to the exact cache folder selected by the npm snapshot.
/// TypeScript uses these compiler options as a "project reference redirect"
/// while resolving imports from the package, which gives stock tsc the same
/// per-referrer dependency context as Deno's resolver without materializing a
/// node_modules tree.
fn generate_npm_cache_projects(
  project_root: &Path,
  deno_imports: Option<&Value>,
  npm_resolver: &CliNpmResolver,
) -> Result<NpmCacheProjects, AnyError> {
  let projects_root = project_root.join(".deno/npm");
  if projects_root.exists() {
    std::fs::remove_dir_all(&projects_root)?;
  }
  let Some(managed) = npm_resolver.as_managed() else {
    return Ok(Default::default());
  };
  if managed.root_node_modules_path().is_some() {
    return Ok(Default::default());
  }

  let snapshot = managed.resolution().snapshot();
  let mut packages = snapshot
    .all_packages_for_every_system()
    .filter_map(|package| {
      let folder = managed.resolve_pkg_folder_from_pkg_id(&package.id).ok()?;
      folder.exists().then_some((package, folder))
    })
    .collect::<Vec<_>>();
  packages.sort_by_key(|(package, _)| package.id.as_serialized());

  let mut result = NpmCacheProjects::default();
  for (package, package_folder) in packages {
    let files = collect_typescript_project_files(&package_folder);
    if files.is_empty() {
      continue;
    }
    let folder_name = deno_resolver::npm::get_package_folder_id_folder_name(
      &package.get_package_cache_folder_id(),
    );
    let project_dir = projects_root.join(&folder_name);
    std::fs::create_dir_all(&project_dir)?;

    let mut paths = serde_json::Map::new();
    let mut dependencies = package.dependencies.iter().collect::<Vec<_>>();
    dependencies.sort_by_key(|(alias, _)| *alias);
    for (alias, dependency_id) in dependencies {
      let Ok(dependency_folder) =
        managed.resolve_pkg_folder_from_pkg_id(dependency_id)
      else {
        continue;
      };
      if !dependency_folder.exists() {
        continue;
      }
      for (key, value) in dependency_project_paths(alias, &dependency_folder) {
        paths.insert(key, value);
      }
    }

    let config = json!({
      "_deno_generated": true,
      "compilerOptions": {
        "composite": true,
        "module": "esnext",
        "moduleResolution": "bundler",
        "paths": paths,
        "skipLibCheck": true,
        "types": [],
      },
      // `files` is intentional rather than an `include` glob. TypeScript only
      // applies a referenced project's resolver options to cache files it can
      // identify as explicit project files.
      "files": files,
    });
    std::fs::write(
      project_dir.join("tsconfig.json"),
      serde_json::to_string_pretty(&config)?,
    )?;
    result
      .references
      .push(format!("./npm/{folder_name}/tsconfig.json"));
  }

  if let Some(imports) = deno_imports.and_then(|v| v.as_object()) {
    for target in imports.values().filter_map(|v| v.as_str()) {
      if !target.starts_with("npm:") {
        continue;
      }
      let Ok(req_ref) =
        deno_semver::npm::NpmPackageReqReference::from_str(target)
      else {
        continue;
      };
      if let Ok(folder) =
        managed.resolve_pkg_folder_from_deno_module_req(req_ref.req())
        && folder.exists()
      {
        result.package_paths.insert(target.to_string(), folder);
      }
    }
  }

  Ok(result)
}

/// Run post-install setup: install jsr packages and generate tsconfig.
///
/// Called after the sync command builds the graph and initializes npm
/// resolution. Returns the list of newly installed JSR packages for reporting.
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
  npm_resolver: &CliNpmResolver,
) -> Result<Vec<InstalledJsrPackage>, AnyError> {
  let deno_json = read_deno_json(project_root)?;
  let deno_compiler_options = deno_json
    .as_ref()
    .and_then(|d| d.get("compilerOptions"))
    .cloned();

  // Workspace member aliases (`@std/assert` -> the local `./assert` member's
  // exports) shadow any published jsr mapping, so compute them up front and let
  // them win in the generated `paths`.
  let mut member_paths = deno_json
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
  // Snapshot the import-map alias -> target pairs (before adding graph specs)
  // so we can resolve bare specifiers against them.
  let alias_targets: Vec<(String, String)> = combined
    .iter()
    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
    .collect();

  // An alias that points at `jsr:`/`npm:<name>` where `<name>` is itself a
  // workspace member (e.g. `"fresh": "jsr:@fresh/core"` in a repo whose
  // `packages/fresh` member is named `@fresh/core`) must resolve to the LOCAL
  // member, not the published registry copy. Otherwise imports through the alias
  // get the registry package's types while local/relative imports get the
  // member's, and the two identical-looking types don't unify.
  add_member_alias_paths(&mut member_paths, &alias_targets);
  for spec in graph_specifiers {
    if is_special_specifier(spec) {
      // Direct scheme specifier: key it to itself.
      combined
        .entry(spec.clone())
        .or_insert_with(|| Value::String(spec.clone()));
    } else if let Some(resolved) =
      resolve_bare_against_import_map(spec, &alias_targets)
    {
      // Bare alias (+ maybe subpath): map it to the resolved scheme specifier
      // so the per-flavor path generators handle it (e.g. `@std/fmt/colors` ->
      // `jsr:@std/fmt@^1/colors`, `lume/foo` -> `https://.../lume@3/foo`).
      combined
        .entry(spec.clone())
        .or_insert(Value::String(resolved));
    }
  }

  // Always generate the base config: even a project with no external deps needs
  // the generated tsconfig + private `@types/deno` so stock tooling sees the
  // Deno globals. There's simply nothing to map into `paths` when there are no
  // external specifiers or workspace members.
  let combined_imports = Value::Object(combined);
  let deno_imports = Some(&combined_imports);
  let deno_compiler_options = deno_compiler_options.as_ref();

  // Stock TypeScript has no hook into Deno's contextual global-cache npm
  // resolver. Model that context with project references: every resolved npm
  // package copy gets a tiny tsconfig whose `paths` point at the exact package
  // copies selected for its dependency edges. TypeScript applies the referenced
  // project's compiler options while resolving imports from files owned by that
  // project, so duplicate versions and peer-dependency copies remain distinct.
  let npm_cache_projects =
    generate_npm_cache_projects(project_root, deno_imports, npm_resolver)?;
  let use_global_cache_layout = npm_resolver
    .as_managed()
    .is_some_and(|managed| managed.root_node_modules_path().is_none());

  let jsr_packages_dir = if use_global_cache_layout {
    project_root.join(".deno/npm-compat/@jsr")
  } else {
    project_root.join("node_modules/@jsr")
  };
  let installed =
    install_jsr_packages(&jsr_packages_dir, deno_imports, http_client).await?;

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

  // Warn about npm packages that could be resolved neither through a local
  // node_modules directory nor through the generated global-cache projects.
  let mut unmaterialized: Vec<String> = combined_imports
    .as_object()
    .into_iter()
    .flatten()
    .filter_map(|(_k, v)| v.as_str())
    .filter(|s| s.starts_with("npm:"))
    .filter(|s| !npm_cache_projects.package_paths.contains_key(*s))
    .filter_map(|s| deno_semver::npm::NpmPackageReqReference::from_str(s).ok())
    .map(|r| r.req().name.to_string())
    .filter(|name| !project_root.join(format!("node_modules/{name}")).exists())
    .collect();
  unmaterialized.sort();
  unmaterialized.dedup();
  if !unmaterialized.is_empty() {
    log::warn!(
      "sync-types: {} npm package(s) are not present under node_modules and \
       were left unmapped ({}). Enable a node_modules directory (e.g. set \
       \"nodeModulesDir\": \"auto\" in deno.json) so stock TypeScript can \
       resolve them.",
      unmaterialized.len(),
      unmaterialized.join(", "),
    );
  }

  // Ensure @types/node is available so Node globals (timers, node: builtins,
  // Buffer, URLPattern, ...) resolve under stock tooling.
  let node_types =
    ensure_types_node(project_root, http_client, use_global_cache_layout).await;
  if let Some(undici_types_dir) = &node_types.undici_types_dir {
    member_paths.insert(
      "undici-types".to_string(),
      json!([path_for_typescript(undici_types_dir)]),
    );
    member_paths.insert(
      "undici-types/*".to_string(),
      json!([format!("{}/*", path_for_typescript(undici_types_dir))]),
    );
  }

  // The project's own `exclude` (from deno.json) tells us which paths Deno
  // doesn't check (test fixtures, generated output); mirror it into the tsconfig
  // so we don't surface diagnostics for files Deno itself skips.
  let mut excludes: Vec<String> = deno_json
    .as_ref()
    .and_then(|d| d.get("exclude"))
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect()
    })
    .unwrap_or_default();

  // Exclude Deno's vendor directory. Vendored deps are third-party code Deno
  // doesn't type-check, and their `jsr:`/`npm:` imports resolve through Deno's
  // vendor map (not ours), so type-checking them here just surfaces the deps'
  // own errors. The vendor dir is `vendor/` at the project root when `vendor`
  // is enabled in deno.json or the directory is present.
  let vendor_enabled = deno_json
    .as_ref()
    .and_then(|d| d.get("vendor"))
    .and_then(|v| v.as_bool())
    .unwrap_or(false);
  if vendor_enabled || project_root.join("vendor").is_dir() {
    excludes.push("vendor".to_string());
  }

  // Generate .deno/tsconfig.json and ensure root tsconfig.json extends it
  generate_deno_tsconfig(
    project_root,
    deno_compiler_options,
    deno_imports,
    &http_modules,
    &member_paths,
    &jsr_packages_dir,
    &npm_cache_projects.package_paths,
    &npm_cache_projects.references,
    node_types.type_root.as_deref(),
    &excludes,
  )?;

  Ok(installed)
}

/// Resolve a bare specifier (import-map alias, possibly with a subpath) against
/// the import map by longest-prefix match, returning the composed scheme
/// specifier if the matched alias targets `npm:`/`jsr:`/`http(s):`.
///
/// - exact alias:            `@fresh/core`      + `@fresh/core` -> `jsr:@fresh/core@^2`
/// - alias + subpath:        `@std/fmt/colors`  + `@std/fmt`    -> `jsr:@std/fmt@^1/colors`
/// - trailing-slash prefix:  `lume/foo`         + `lume/`       -> `https://.../lume@3/foo`
fn resolve_bare_against_import_map(
  spec: &str,
  alias_targets: &[(String, String)],
) -> Option<String> {
  let mut best: Option<(&str, &str)> = None;
  for (alias, target) in alias_targets {
    let matches = spec == alias
      || (alias.ends_with('/') && spec.starts_with(alias.as_str()))
      || (!alias.ends_with('/') && spec.starts_with(&format!("{alias}/")));
    if matches && best.is_none_or(|(b, _)| alias.len() > b.len()) {
      best = Some((alias, target));
    }
  }
  let (alias, target) = best?;
  if !is_special_specifier(target) {
    return None;
  }
  // Append whatever of `spec` extends past the matched alias. For a trailing-
  // slash prefix that's `foo`; for a non-slash alias it's `/subpath` (or empty
  // for an exact match).
  Some(format!("{}{}", target, &spec[alias.len()..]))
}

/// Ensure a stock `@types/node` (and its `undici-types` dependency) is present
/// in the selected compatibility directory so the generated tsconfig can load
/// Node globals (timers, `node:` builtins, `Buffer`, `URLPattern`, ...). No-op
/// when the project already has `@types/node` installed.
///
/// Global-cache mode keeps these under `.deno/npm-compat`; local node_modules
/// mode keeps the existing `node_modules/@types` layout.
async fn ensure_types_node(
  project_root: &Path,
  http_client: &HttpClient,
  use_global_cache_layout: bool,
) -> NodeTypesSetup {
  let modules_dir = if use_global_cache_layout {
    project_root.join(".deno/npm-compat")
  } else {
    project_root.join("node_modules")
  };
  let type_root = if use_global_cache_layout {
    "./npm-compat/@types".to_string()
  } else {
    "../node_modules/@types".to_string()
  };
  let node_dir = modules_dir.join("@types/node");
  let undici_types_dir = modules_dir.join("undici-types");
  if node_dir.exists() {
    return NodeTypesSetup {
      type_root: Some(type_root),
      undici_types_dir: undici_types_dir.exists().then_some(undici_types_dir),
    };
  }
  match download_npm_package(&modules_dir, "@types/node", None, http_client)
    .await
  {
    Ok(Some((_version, deps))) => {
      if let Some(req) = deps.get("undici-types").and_then(|v| v.as_str()) {
        let _ = download_npm_package(
          &modules_dir,
          "undici-types",
          Some(req),
          http_client,
        )
        .await;
      }
      NodeTypesSetup {
        type_root: node_dir.exists().then_some(type_root),
        undici_types_dir: undici_types_dir.exists().then_some(undici_types_dir),
      }
    }
    _ => NodeTypesSetup {
      type_root: None,
      undici_types_dir: None,
    },
  }
}

/// Download an npm package tarball from registry.npmjs.org into
/// `node_modules/<pkg>` and return its resolved version + dependency map.
/// Resolves `req` (a semver range) if given, otherwise the `latest` dist-tag.
async fn download_npm_package(
  modules_dir: &Path,
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
  let dest = modules_dir.join(pkg);
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
  let member_patterns: Vec<&str> =
    members.iter().filter_map(|m| m.as_str()).collect();
  for member_rel in expand_member_patterns(project_root, &member_patterns) {
    let member_rel = member_rel.as_str();
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

/// Expand workspace member patterns into concrete member directories (relative
/// to the project root). A trailing `/*` (e.g. `./packages/*`) enumerates the
/// immediate subdirectories that contain a `deno.json(c)`; everything else is
/// treated as a literal member path. Deno's own workspace config supports this
/// glob form, and it's common in monorepos.
fn expand_member_patterns(
  project_root: &Path,
  patterns: &[&str],
) -> Vec<String> {
  let mut out = Vec::new();
  for pattern in patterns {
    let cleaned = pattern.trim_start_matches("./").trim_end_matches('/');
    if let Some(parent) = cleaned.strip_suffix("/*") {
      let Ok(entries) = std::fs::read_dir(project_root.join(parent)) else {
        continue;
      };
      for entry in entries.flatten() {
        if !entry.path().is_dir() {
          continue;
        }
        let dir = entry.path();
        if (dir.join("deno.json").exists() || dir.join("deno.jsonc").exists())
          && let Some(name) = entry.file_name().to_str()
        {
          out.push(format!("{parent}/{name}"));
        }
      }
    } else {
      out.push(cleaned.to_string());
    }
  }
  out.sort();
  out
}

/// Extract the bare package name from a `jsr:`/`npm:` specifier, dropping the
/// scheme and any version/subpath (`jsr:@fresh/core@^2` -> `@fresh/core`,
/// `npm:chalk@5` -> `chalk`, `npm:@scope/pkg@1` -> `@scope/pkg`).
fn scheme_package_name(spec: &str) -> Option<String> {
  let rest = spec
    .strip_prefix("jsr:")
    .or_else(|| spec.strip_prefix("npm:"))?;
  if let Some(scoped) = rest.strip_prefix('@') {
    let (scope, name_and_rest) = scoped.split_once('/')?;
    let name = name_and_rest.split('@').next()?;
    Some(format!("@{scope}/{name}"))
  } else {
    Some(rest.split('@').next()?.to_string())
  }
}

/// For each import-map alias whose target's package name matches a workspace
/// member, mirror that member's `paths` entries under the alias so imports
/// through the alias resolve to the local member's source rather than a
/// materialized registry copy.
fn add_member_alias_paths(
  member_paths: &mut serde_json::Map<String, Value>,
  alias_targets: &[(String, String)],
) {
  // Snapshot member entries so we can extend `member_paths` while iterating.
  let member_entries: Vec<(String, Value)> = member_paths
    .iter()
    .map(|(k, v)| (k.clone(), v.clone()))
    .collect();
  for (alias, target) in alias_targets {
    let Some(base) = scheme_package_name(target) else {
      continue;
    };
    let sub_prefix = format!("{base}/");
    for (member_key, member_val) in &member_entries {
      if member_key == &base {
        member_paths
          .entry(alias.clone())
          .or_insert_with(|| member_val.clone());
      } else if let Some(sub) = member_key.strip_prefix(&sub_prefix) {
        member_paths
          .entry(format!("{alias}/{sub}"))
          .or_insert_with(|| member_val.clone());
      }
    }
  }
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
#[allow(
  clippy::too_many_arguments,
  reason = "threads the independent inputs needed to generate a tsconfig"
)]
fn generate_deno_tsconfig(
  project_root: &Path,
  deno_compiler_options: Option<&Value>,
  deno_imports: Option<&Value>,
  http_modules: &BTreeMap<Url, String>,
  member_paths: &serde_json::Map<String, Value>,
  jsr_packages_dir: &Path,
  npm_package_paths: &BTreeMap<String, PathBuf>,
  npm_project_references: &[String],
  node_types_root: Option<&str>,
  excludes: &[String],
) -> Result<(), AnyError> {
  let generated = crate::tsc::tsconfig_gen::generate_tsconfig(
    project_root,
    deno_compiler_options,
    deno_imports,
    // Command-line roots scope graph/dependency discovery only. Keep the
    // generated project open so bundlers can consume its resolver mappings for
    // any entrypoint they are asked to bundle.
    &[],
    http_modules,
    member_paths,
    jsr_packages_dir,
    npm_package_paths,
    npm_project_references,
    node_types_root,
    excludes,
  )
  .map_err(|e| anyhow!("Failed to generate tsconfig: {e}"))?;

  log::debug!("Generated {}", generated.tsconfig_path.display());

  Ok(())
}

/// Install jsr: compatibility packages by downloading from npm.jsr.io.
///
/// Uses `HttpClient::download` directly (not `CliFileFetcher`) because we
/// don't want each registry/tarball URL surfacing as a user-visible
/// "Download …" line in `deno install` output — those are an
/// implementation detail of the stock-tsc compatibility setup. The
/// fetcher's caching and redirect handling are unnecessary for npm.jsr.io.
async fn install_jsr_packages(
  jsr_packages_dir: &Path,
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
    let pkg_dir = jsr_packages_dir.join(&npm_name);
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
    // Never materialize link entries. A symlink entry pointing outside `dest`,
    // followed by a regular-file entry nested "under" it, would let `unpack`
    // (via `create_dir_all` following the symlink) write outside `dest`. We only
    // need regular files/dirs for type extraction, so skip links entirely.
    let entry_type = entry.header().entry_type();
    if entry_type.is_symlink() || entry_type.is_hard_link() {
      continue;
    }
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
    // A trailing-slash URL is a directory/prefix specifier, not a module; the
    // files under it are discovered and mirrored per-subpath. Skip silently
    // (fetching the bare prefix just 404s).
    if requested_url.path().ends_with('/') {
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
  // Mark mirrored script modules `// @ts-nocheck`. Remote dependencies are not
  // the user's code and Deno itself doesn't re-type-check them; without this,
  // stock tsc/tsgo would report every type error inside the dependency (which
  // dwarfs the project's own diagnostics). `@ts-nocheck` suppresses errors in
  // the file while keeping its exported and ambient types available to
  // importers, and triple-slash directives (which may follow a leading comment)
  // still apply.
  let is_script = matches!(
    local_path.extension().and_then(|e| e.to_str()),
    Some("ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs")
  );
  if is_script {
    let mut content = Vec::with_capacity(bytes.len() + 16);
    content.extend_from_slice(b"// @ts-nocheck\n");
    content.extend_from_slice(bytes);
    std::fs::write(&local_path, &content)?;
  } else {
    std::fs::write(&local_path, bytes)?;
  }
  url_to_tsconfig_path(url)
    .ok_or_else(|| anyhow!("URL has no resolvable mirror path: {url}"))
}

/// Compute the mirror location for `url` relative to the remote root, e.g.
/// `https://example.com/x/foo.ts` → `example.com/x/foo.ts`. Includes a
/// non-default port and a hash of the query string so URLs that share a path
/// but differ by port or query (common on CDNs like esm.sh) don't collide on
/// disk or in the generated `paths`. The query hash goes into the last
/// segment's stem (before its first `.`), keeping the file in the same
/// directory and preserving its extension so relative imports still resolve.
/// Returns None for directory-like URLs (path ends in `/`), since we can't
/// infer an index filename without server cooperation.
fn url_to_mirror_rel(url: &Url) -> Option<String> {
  let host = url.host_str()?;
  let path = url.path();
  if path.ends_with('/') || path.is_empty() {
    return None;
  }
  let mut host_seg = host.to_string();
  if let Some(port) = url.port() {
    host_seg.push_str(&format!("__{port}"));
  }
  let rel = path.trim_start_matches('/');
  let rel = match url.query() {
    Some(query) => {
      let hash = short_hash(query);
      match rel.rsplit_once('/') {
        Some((dir, file)) => {
          format!("{dir}/{}", inject_stem_suffix(file, &hash))
        }
        None => inject_stem_suffix(rel, &hash),
      }
    }
    None => rel.to_string(),
  };
  Some(format!("{host_seg}/{rel}"))
}

/// Insert `suffix` into a filename before its first `.` (`mod.d.ts` ->
/// `mod.<suffix>.d.ts`), or append it when there's no extension.
fn inject_stem_suffix(file: &str, suffix: &str) -> String {
  match file.split_once('.') {
    Some((stem, ext)) => format!("{stem}.{suffix}.{ext}"),
    None => format!("{file}.{suffix}"),
  }
}

/// Deterministic short hex hash, used to disambiguate mirror paths by query.
fn short_hash(s: &str) -> String {
  use std::hash::Hash;
  use std::hash::Hasher;
  let mut hasher = std::collections::hash_map::DefaultHasher::new();
  s.hash(&mut hasher);
  format!("{:08x}", hasher.finish() as u32)
}

/// Compute the `paths`-relative mirror location for `url`, e.g.
/// `https://example.com/x/foo.ts` → `./remote/example.com/x/foo.ts`.
fn url_to_tsconfig_path(url: &Url) -> Option<String> {
  Some(format!("./remote/{}", url_to_mirror_rel(url)?))
}

/// Map a URL to its mirror file path under `<remote_root>/<mirror-rel>`.
fn url_to_local_path(remote_root: &Path, url: &Url) -> Option<PathBuf> {
  Some(remote_root.join(url_to_mirror_rel(url)?))
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_scheme_package_name() {
    assert_eq!(
      scheme_package_name("jsr:@fresh/core@^2.0.0").as_deref(),
      Some("@fresh/core")
    );
    assert_eq!(scheme_package_name("npm:chalk@5").as_deref(), Some("chalk"));
    assert_eq!(
      scheme_package_name("npm:@scope/pkg@1.2.3").as_deref(),
      Some("@scope/pkg")
    );
    assert_eq!(scheme_package_name("npm:chalk").as_deref(), Some("chalk"));
    assert_eq!(scheme_package_name("./local.ts"), None);
    assert_eq!(scheme_package_name("https://example.com/x.ts"), None);
  }

  #[test]
  fn test_add_member_alias_paths() {
    // `packages/fresh` (name `@fresh/core`) is a workspace member; the root
    // import map aliases `fresh` -> `jsr:@fresh/core`.
    let mut member_paths = serde_json::Map::new();
    member_paths.insert(
      "@fresh/core".into(),
      json!(["../packages/fresh/src/mod.ts"]),
    );
    member_paths.insert(
      "@fresh/core/runtime".into(),
      json!(["../packages/fresh/src/runtime.ts"]),
    );

    let alias_targets = vec![
      ("fresh".to_string(), "jsr:@fresh/core@^2.0.0".to_string()),
      // an alias to a non-member package is left alone
      ("chalk".to_string(), "npm:chalk@5".to_string()),
    ];

    add_member_alias_paths(&mut member_paths, &alias_targets);

    // alias and its subpath now point at the local member source
    assert_eq!(
      member_paths.get("fresh").unwrap(),
      &json!(["../packages/fresh/src/mod.ts"])
    );
    assert_eq!(
      member_paths.get("fresh/runtime").unwrap(),
      &json!(["../packages/fresh/src/runtime.ts"])
    );
    // non-member alias produced no new entry
    assert!(!member_paths.contains_key("chalk"));
  }

  #[test]
  fn test_dependency_project_paths_resolves_subpath_exports() {
    let dir = tempfile::tempdir().unwrap();
    let pkg_dir = dir.path().join("dep");
    std::fs::create_dir_all(pkg_dir.join("dist/nested")).unwrap();
    // The resolved declaration must exist on disk (dependency_project_paths
    // verifies existence before mapping, falling back to source otherwise).
    std::fs::write(pkg_dir.join("dist/index.d.ts"), "").unwrap();
    std::fs::write(pkg_dir.join("dist/nested/subpath.d.ts"), "").unwrap();
    std::fs::write(
      pkg_dir.join("package.json"),
      serde_json::to_string(&json!({
        "name": "dep",
        "exports": {
          ".": { "types": "./dist/index.d.ts", "default": "./dist/index.js" },
          // Remaps `dep/subpath` to a non-literal declaration path: the naive
          // `dep/* -> folder/*` wildcard would mis-resolve this.
          "./subpath": {
            "types": "./dist/nested/subpath.d.ts",
            "default": "./dist/nested/subpath.js",
          },
        }
      }))
      .unwrap(),
    )
    .unwrap();

    let folder = path_for_typescript(&pkg_dir);
    let paths = dependency_project_paths("dep", &pkg_dir);

    // Bare alias still maps to the folder (tsc reads its package.json).
    assert_eq!(paths.get("dep"), Some(&json!([folder])));
    // The subpath maps to the exact declaration, not `folder/subpath`.
    assert_eq!(
      paths.get("dep/subpath"),
      Some(&json!([format!("{folder}/dist/nested/subpath.d.ts")]))
    );
    // A package with an exports map does NOT get the literal wildcard fallback.
    assert!(!paths.contains_key("dep/*"));
  }

  #[test]
  fn test_dependency_project_paths_wildcard_fallback_without_exports() {
    let dir = tempfile::tempdir().unwrap();
    let pkg_dir = dir.path().join("dep");
    std::fs::create_dir_all(&pkg_dir).unwrap();
    std::fs::write(
      pkg_dir.join("package.json"),
      serde_json::to_string(&json!({
        "name": "dep",
        "types": "./index.d.ts",
      }))
      .unwrap(),
    )
    .unwrap();

    let folder = path_for_typescript(&pkg_dir);
    let paths = dependency_project_paths("dep", &pkg_dir);

    // Without an exports map, keep the literal subpath wildcard fallback.
    assert_eq!(paths.get("dep"), Some(&json!([folder])));
    assert_eq!(paths.get("dep/*"), Some(&json!([format!("{folder}/*")])));
  }

  #[test]
  fn test_url_to_mirror_rel_disambiguates_query_and_port() {
    let rel = |u: &str| url_to_mirror_rel(&Url::parse(u).unwrap()).unwrap();

    // Plain URL: host/path, unchanged.
    assert_eq!(rel("https://example.com/x/foo.ts"), "example.com/x/foo.ts");

    // Distinct queries must not collide, and the extension is preserved.
    let a = rel("https://esm.sh/react.ts?a=1");
    let b = rel("https://esm.sh/react.ts?a=2");
    assert_ne!(a, b);
    assert!(a.ends_with(".ts") && b.ends_with(".ts"));
    assert!(a.starts_with("esm.sh/") && b.starts_with("esm.sh/"));

    // Non-default port is part of the host segment.
    let p = rel("https://example.com:8443/x/foo.ts");
    assert!(p.starts_with("example.com__8443/"));
    // Same host, different port -> different mirror path.
    assert_ne!(p, rel("https://example.com:9443/x/foo.ts"));
  }
}
