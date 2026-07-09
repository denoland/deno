// Copyright 2018-2026 the Deno authors. MIT license.

//! Generate `.deno/tsconfig.json` for use with stock TypeScript tooling
//! (tsc, tsgo).
//!
//! This module translates Deno's compiler options (from `deno.json`) into a
//! standard `tsconfig.json` that stock TypeScript understands. It also injects
//! Deno type definitions so that the `Deno` namespace and other Deno-specific
//! globals are available.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use deno_core::serde_json;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use deno_core::url::Url;

use super::get_types_declaration_file_text;

/// Result of generating a tsconfig for stock TypeScript.
#[derive(Debug)]
pub struct GeneratedTsConfig {
  /// Path to the generated `.deno/tsconfig.json`.
  pub tsconfig_path: PathBuf,
}

/// Generate `.deno/tsconfig.json` and Deno type definitions for use with
/// stock TypeScript tooling.
///
/// Writes Deno types to `.deno/types/deno/index.d.ts` (a private typeRoot
/// only the generated tsconfig points at). They deliberately do NOT go to
/// `node_modules/@types/deno/`: TypeScript auto-discovers everything under
/// `node_modules/@types/*` whenever `compilerOptions.types` is unset, which
/// is exactly the state Deno's own type checker sees after the resolver
/// filters our generated tsconfig out of the extends chain. With @types/deno
/// auto-loaded into Deno's checker, every global also declared in
/// `lib.deno.shared_globals.d.ts` would duplicate (TS2403).
///
/// Generates `.deno/tsconfig.json` with compiler options and paths mappings
/// for npm:/jsr: specifiers. Also ensures a root `tsconfig.json` exists
/// that extends it.
#[allow(clippy::too_many_arguments)]
pub fn generate_tsconfig(
  project_root: &Path,
  deno_compiler_options: Option<&Value>,
  deno_imports: Option<&Value>,
  files: &[String],
  http_modules: &BTreeMap<Url, String>,
  member_paths: &Map<String, Value>,
  has_node_types: bool,
) -> Result<GeneratedTsConfig, std::io::Error> {
  // Write Deno type definitions to .deno/types/deno/ (private typeRoot).
  let types_dir = project_root.join(".deno/types/deno");
  std::fs::create_dir_all(&types_dir)?;
  write_deno_types(&types_dir.join("index.d.ts"))?;

  // Write a package.json for the @types/deno package so the typeRoots lookup
  // resolves the directory as a package.
  std::fs::write(
    types_dir.join("package.json"),
    serde_json::to_string_pretty(&json!({
      "name": "@types/deno",
      "version": "0.0.0",
      "types": "index.d.ts"
    }))
    .unwrap(),
  )?;

  // Build tsconfig
  let tsconfig = build_tsconfig(
    project_root,
    deno_compiler_options,
    deno_imports,
    files,
    http_modules,
    member_paths,
    has_node_types,
  );

  // Write to .deno/tsconfig.json
  let deno_dir = project_root.join(".deno");
  std::fs::create_dir_all(&deno_dir)?;
  let tsconfig_path = deno_dir.join("tsconfig.json");
  let content = serde_json::to_string_pretty(&tsconfig)
    .expect("failed to serialize tsconfig");
  std::fs::write(&tsconfig_path, &content)?;

  // Ensure root tsconfig.json exists and extends .deno/tsconfig.json
  ensure_root_tsconfig(project_root)?;

  Ok(GeneratedTsConfig { tsconfig_path })
}

/// Ensure a root `tsconfig.json` exists that extends `.deno/tsconfig.json`.
///
/// - No existing tsconfig: create one with `extends: "./.deno/tsconfig.json"`.
/// - Existing `extends` is a single string (or absent): coerce into an array
///   that includes the original entry followed by our generated path. TS 5.0+
///   resolves array extends left-to-right, with later entries overriding
///   earlier ones — putting ours last lets us add the URL `paths` mappings
///   without clobbering user-managed settings inherited from e.g. a shared
///   team config.
/// - Existing `extends` is already an array: append our path if missing,
///   otherwise leave it alone.
fn ensure_root_tsconfig(project_root: &Path) -> Result<(), std::io::Error> {
  let root_tsconfig_path = project_root.join("tsconfig.json");
  let extends_path = "./.deno/tsconfig.json";

  if !root_tsconfig_path.exists() {
    let tsconfig = json!({ "extends": extends_path });
    let content = serde_json::to_string_pretty(&tsconfig)
      .expect("failed to serialize tsconfig");
    return std::fs::write(&root_tsconfig_path, &content);
  }

  let content = std::fs::read_to_string(&root_tsconfig_path)?;
  let mut tsconfig: Value =
    serde_json::from_str(&content).unwrap_or_else(|_| json!({}));

  let Some(obj) = tsconfig.as_object_mut() else {
    return Ok(());
  };

  match obj.get("extends").cloned() {
    None => {
      obj.insert("extends".to_string(), json!(extends_path));
    }
    Some(Value::String(s)) if s == extends_path => {
      return Ok(());
    }
    Some(Value::String(existing)) => {
      obj.insert("extends".to_string(), json!([existing, extends_path]));
    }
    Some(Value::Array(arr)) => {
      let already = arr
        .iter()
        .any(|v| v.as_str().is_some_and(|s| s == extends_path));
      if already {
        return Ok(());
      }
      let mut new_arr = arr;
      new_arr.push(json!(extends_path));
      obj.insert("extends".to_string(), Value::Array(new_arr));
    }
    Some(_) => {
      // Non-string, non-array extends — leave the user's config alone rather
      // than guessing.
      log::warn!(
        "tsconfig.json has a non-string `extends`; not modifying. \
         Add \"{extends_path}\" to it manually for stock tsc support."
      );
      return Ok(());
    }
  }

  let content = serde_json::to_string_pretty(&tsconfig)
    .expect("failed to serialize tsconfig");
  std::fs::write(&root_tsconfig_path, &content)
}

/// Write the Deno type declarations to a `.d.ts` file.
fn write_deno_types(path: &Path) -> Result<(), std::io::Error> {
  let types_text = get_types_declaration_file_text();
  // Strip triple-slash reference directives that conflict with stock tsc
  let filtered: String = types_text
    .lines()
    .filter(|line| {
      let trimmed = line.trim();
      !(trimmed.starts_with("/// <reference no-default-lib")
        || trimmed.starts_with("/// <reference lib="))
    })
    .collect::<Vec<_>>()
    .join("\n");

  std::fs::write(
    path,
    format!(
      "// Auto-generated by Deno for stock TypeScript tooling.\n\
       // Do not edit — this file is regenerated as needed.\n\n\
       {filtered}"
    ),
  )
}

/// Build the tsconfig JSON object.
///
/// The generated tsconfig lives at `.deno/tsconfig.json`, so all paths
/// are relative to that directory (e.g. `../node_modules/...`).
#[allow(clippy::too_many_arguments)]
fn build_tsconfig(
  project_root: &Path,
  deno_compiler_options: Option<&Value>,
  deno_imports: Option<&Value>,
  check_files: &[String],
  http_modules: &BTreeMap<Url, String>,
  member_paths: &Map<String, Value>,
  has_node_types: bool,
) -> Value {
  let mut compiler_options = base_compiler_options();

  // When @types/node is available, load it alongside @types/deno so Node
  // globals (timers, node: builtins, Buffer, URLPattern, ...) resolve. It lives
  // in node_modules/@types, so add that typeRoot too.
  if has_node_types {
    compiler_options.insert(
      "typeRoots".to_string(),
      json!(["./types", "../node_modules/@types"]),
    );
    compiler_options.insert("types".to_string(), json!(["deno", "node"]));
  }

  // Merge user's deno.json compilerOptions (filtered to stock-tsc-compatible
  // options only)
  if let Some(user_opts) = deno_compiler_options {
    merge_deno_options(&mut compiler_options, user_opts);
  }

  // Generate "paths" for npm: and jsr: specifiers only
  let mut specifier_paths = generate_npm_paths(project_root, deno_imports);
  let jsr_paths = generate_jsr_paths(project_root, deno_imports);
  specifier_paths.extend(jsr_paths);
  let local_paths = generate_local_alias_paths(deno_imports);
  specifier_paths.extend(local_paths);
  let http_alias_paths = generate_http_alias_paths(deno_imports, http_modules);
  specifier_paths.extend(http_alias_paths);
  let http_paths = generate_http_paths(http_modules);
  specifier_paths.extend(http_paths);

  // Workspace-member aliases map to local source and shadow any published jsr
  // mapping for the same name, so apply them after the specifier paths.
  for (key, value) in member_paths {
    specifier_paths.insert(key.clone(), value.clone());
  }

  // Merge user-defined paths from deno.json compilerOptions — these take
  // priority over generated specifier mappings.
  if let Some(user_paths) = deno_compiler_options
    .and_then(|co| co.get("paths"))
    .and_then(|p| p.as_object())
  {
    for (key, value) in user_paths {
      specifier_paths.insert(key.clone(), value.clone());
    }
  }

  // Merge the user's bare-package `compilerOptions.types` with the deno/node
  // types we inject, rather than dropping them (see `merge_user_types` for why
  // subpath entries like `lume/types.ts` are handled via `include` instead).
  if let Some(user_types) = deno_compiler_options
    .and_then(|co| co.get("types"))
    .and_then(|t| t.as_array())
    && let Some(Value::Array(types)) = compiler_options.get_mut("types")
  {
    merge_user_types(types, user_types);
  }

  if !specifier_paths.is_empty() {
    compiler_options.insert("paths".to_string(), json!(specifier_paths));
  }

  // The `_deno_generated` sentinel lets Deno's own resolver identify this
  // tsconfig and exclude it from extends chains it processes — see
  // libs/resolver/deno_json.rs. Stock tsc/tsgo ignore unknown top-level
  // properties.
  if check_files.is_empty() {
    // No specific files — check entire project
    json!({
      "_deno_generated": true,
      "compilerOptions": compiler_options,
      "include": ["../**/*"],
      "exclude": ["../**/node_modules"],
    })
  } else {
    // Specific files requested — only check those
    let files_array: Vec<Value> =
      check_files.iter().map(|f| json!(f)).collect();
    json!({
      "_deno_generated": true,
      "compilerOptions": compiler_options,
      "files": files_array,
    })
  }
}

/// Generate tsconfig "paths" entries for npm: specifiers.
///
/// Scans `deno.json` `"imports"` for entries like:
///   `"express": "npm:express@4"` -> `{ "npm:express": ["../node_modules/express"] }`
///
/// Only generates `npm:<pkg>` keys -- bare aliases are resolved by
/// TypeScript via `node_modules` with `moduleResolution: "bundler"`.
/// Map import-map aliases that point at local files/directories (e.g.
/// `"@/": "./"`, `"utils": "./src/utils.ts"`) to project-relative paths. The
/// generated tsconfig lives in `.deno/`, so a project-root path `./x` becomes
/// `../x`. Trailing-slash prefix aliases become `<alias>*` -> `../<base>*`.
/// Map import-map aliases whose target is a mirrored http(s) URL to the local
/// mirror file, so code (and `jsxImportSource`) that imports the *alias* — e.g.
/// `lume/jsx-runtime` -> `https://.../jsx-runtime.ts` — resolves. Without this
/// only the raw URL is mapped, not the alias the source actually uses.
fn generate_http_alias_paths(
  deno_imports: Option<&Value>,
  http_modules: &BTreeMap<Url, String>,
) -> Map<String, Value> {
  let mut paths = Map::new();
  let Some(imports) = deno_imports.and_then(|v| v.as_object()) else {
    return paths;
  };
  for (alias, target) in imports {
    let Some(target) = target.as_str() else {
      continue;
    };
    if !(target.starts_with("http://") || target.starts_with("https://")) {
      continue;
    }
    if let Ok(url) = Url::parse(target)
      && let Some(mirrored) = http_modules.get(&url)
    {
      paths.insert(alias.clone(), json!([mirrored]));
    }
  }
  paths
}

fn generate_local_alias_paths(
  deno_imports: Option<&Value>,
) -> Map<String, Value> {
  let mut paths = Map::new();
  let Some(imports) = deno_imports.and_then(|v| v.as_object()) else {
    return paths;
  };
  for (alias, target) in imports {
    let Some(target) = target.as_str() else {
      continue;
    };
    if !(target.starts_with("./") || target.starts_with("../")) {
      continue;
    }
    // Re-base a project-root-relative path onto `.deno/` (one level down).
    let rebased = format!("../{}", target.trim_start_matches("./"));
    if let Some(prefix) = alias.strip_suffix('/') {
      // Prefix alias: `@/` -> `./` becomes `@/*` -> `../*`.
      paths.insert(format!("{prefix}/*"), json!([format!("{}*", rebased)]));
    } else {
      paths.insert(alias.clone(), json!([rebased]));
    }
  }
  paths
}

fn generate_npm_paths(
  project_root: &Path,
  deno_imports: Option<&Value>,
) -> Map<String, Value> {
  let mut paths = Map::new();

  if let Some(imports) = deno_imports.and_then(|v| v.as_object()) {
    for (alias, target) in imports {
      let target_str = match target.as_str() {
        Some(s) if s.starts_with("npm:") => s,
        _ => continue,
      };
      let Ok(npm_ref) =
        deno_semver::npm::NpmPackageReqReference::from_str(target_str)
      else {
        continue;
      };
      let pkg_name = npm_ref.req().name.to_string();
      let pkg_dir = project_root.join(format!("node_modules/{pkg_name}"));

      // A plain alias whose name matches the package (`"chalk": "npm:chalk"`,
      // and its subpaths like `chalk/foo`) resolves natively through
      // `node_modules` under `moduleResolution: bundler`, so an alias key would
      // be redundant. A *renamed* alias (`"$prism": "npm:prismjs"`, so source
      // writes `$prism/components/x`) has no `node_modules/$prism`, so it needs
      // an explicit mapping. `npm:`-scheme keys are always emitted since bundler
      // can't resolve the scheme on its own.
      let alias_renamed = alias != &pkg_name;
      match npm_ref.sub_path() {
        Some(sub) => {
          // `npm:preact/compat`: resolve through the package's `exports` to a
          // concrete .d.ts (relative, to avoid TS5090). Fall back to the naive
          // subpath under the package dir when exports can't be read (e.g. the
          // package isn't materialized yet). Key both the version-less scheme
          // (`npm:preact/compat`) and the exact specifier as written, plus the
          // source-written alias form when the alias is renamed.
          let rel = resolve_jsr_types_entry(&pkg_dir, &format!("./{sub}"))
            .unwrap_or_else(|| format!("../node_modules/{pkg_name}/{sub}"));
          paths
            .entry(format!("npm:{pkg_name}/{sub}"))
            .or_insert_with(|| json!([&rel]));
          paths
            .entry(target_str.to_string())
            .or_insert_with(|| json!([&rel]));
          if alias_renamed {
            paths.entry(alias.clone()).or_insert_with(|| json!([&rel]));
          }
        }
        None => {
          // Bare `npm:preact`: map to the package directory, which resolves via
          // its package.json `types`/`exports["."]`. Emitted unconditionally so
          // the mapping exists even if generation runs before install. Key both
          // the version-less scheme (`npm:preact`) and the exact specifier.
          let dir = format!("../node_modules/{pkg_name}");
          paths
            .entry(format!("npm:{pkg_name}"))
            .or_insert_with(|| json!([&dir]));
          paths
            .entry(target_str.to_string())
            .or_insert_with(|| json!([&dir]));
          if alias_renamed {
            paths.entry(alias.clone()).or_insert_with(|| json!([&dir]));
          }
          // Enumerate the package's own `exports` so subpaths written in source
          // map to their concrete .d.ts: the `npm:` scheme form always, and the
          // renamed-alias form (`$prism/components`) when applicable.
          for exp_key in package_export_keys(&pkg_dir) {
            let sub = exp_key.trim_start_matches("./");
            let Some(sub_rel) = resolve_jsr_types_entry(&pkg_dir, &exp_key)
            else {
              continue;
            };
            paths
              .entry(format!("npm:{pkg_name}/{sub}"))
              .or_insert_with(|| json!([&sub_rel]));
            if alias_renamed {
              paths
                .entry(format!("{alias}/{sub}"))
                .or_insert_with(|| json!([&sub_rel]));
            }
          }
        }
      }
    }
  }

  paths
}

/// Generate tsconfig "paths" entries for jsr: specifiers.
///
/// JSR packages are available via npm compatibility at `npm.jsr.io` and install
/// to `node_modules/@jsr/<scope>__<name>`. Maps `jsr:@scope/name` to that path.
///
/// Only generates `jsr:<scope>/<name>` keys.
fn generate_jsr_paths(
  project_root: &Path,
  deno_imports: Option<&Value>,
) -> Map<String, Value> {
  let mut paths = Map::new();

  if let Some(imports) = deno_imports.and_then(|v| v.as_object()) {
    for (alias, target) in imports {
      let target_str = match target.as_str() {
        Some(s) => s,
        None => continue,
      };

      if let Some((scope, name, version)) = parse_jsr_specifier(target_str) {
        // JSR npm compat installs to node_modules/@jsr/<scope>__<name>.
        let jsr_npm_name =
          format!("{}__{}", scope.trim_start_matches('@'), name);
        let pkg_dir =
          project_root.join(format!("node_modules/@jsr/{jsr_npm_name}"));
        if !pkg_dir.exists() {
          continue;
        }

        // Recover the subpath the source actually imports (everything after
        // `jsr:@scope/name[@version]`), and resolve it through the installed
        // package's `exports` to a concrete .d.ts. We map to that *relative*
        // file rather than a non-relative `@jsr/*` wildcard so tsc/tsgo doesn't
        // emit TS5090 ("non-relative paths are not allowed") - verified.
        let prefix = match &version {
          Some(v) => format!("jsr:{scope}/{name}@{v}"),
          None => format!("jsr:{scope}/{name}"),
        };
        let subpath = target_str
          .strip_prefix(&prefix)
          .map(|s| s.trim_start_matches('/'))
          .unwrap_or("");
        let export_key = if subpath.is_empty() {
          ".".to_string()
        } else {
          format!("./{subpath}")
        };
        let Some(types_rel) = resolve_jsr_types_entry(&pkg_dir, &export_key)
        else {
          continue;
        };

        // Key on the exact specifier as written in source. Also map the
        // import-map alias form (bare) when this came from an alias entry.
        paths
          .entry(target_str.to_string())
          .or_insert_with(|| json!([&types_rel]));
        if alias != target_str && subpath.is_empty() {
          paths
            .entry(alias.clone())
            .or_insert_with(|| json!([&types_rel]));
        }

        // For a bare alias -> package (no subpath), enumerate the package's own
        // exports and map each under both the alias and the jsr: specifier, so
        // subpath imports like `fresh/runtime` resolve without depending on the
        // module graph having discovered them.
        if subpath.is_empty() {
          for exp_key in package_export_keys(&pkg_dir) {
            let sub = exp_key.trim_start_matches("./");
            let Some(sub_rel) = resolve_jsr_types_entry(&pkg_dir, &exp_key)
            else {
              continue;
            };
            paths
              .entry(format!("{prefix}/{sub}"))
              .or_insert_with(|| json!([&sub_rel]));
            if alias != target_str {
              paths
                .entry(format!("{alias}/{sub}"))
                .or_insert_with(|| json!([&sub_rel]));
            }
          }
        }
      }
    }
  }

  paths
}

/// List a package's `exports` subpath keys (`"./foo"`), excluding the root
/// `"."`. Used to enumerate what an import-map alias can reach by subpath.
fn package_export_keys(pkg_dir: &Path) -> Vec<String> {
  let Ok(content) = std::fs::read_to_string(pkg_dir.join("package.json"))
  else {
    return vec![];
  };
  let Ok(pkg) = serde_json::from_str::<Value>(&content) else {
    return vec![];
  };
  pkg
    .get("exports")
    .and_then(|e| e.as_object())
    .map(|m| m.keys().filter(|k| k.starts_with("./")).cloned().collect())
    .unwrap_or_default()
}

/// Resolve the types entry point from a JSR package's package.json.
///
/// Reads the `"exports"` field and looks for `"."` -> `"types"` condition.
/// Returns a path relative to `.deno/` (e.g., `../node_modules/@jsr/std__assert/_dist/mod.d.ts`).
fn resolve_jsr_types_entry(pkg_dir: &Path, export_key: &str) -> Option<String> {
  let pkg_json_path = pkg_dir.join("package.json");
  let content = std::fs::read_to_string(&pkg_json_path).ok()?;
  let pkg_json: Value = serde_json::from_str(&content).ok()?;

  // Resolve `exports[export_key]` (e.g. "." or "./cookie_map"), preferring the
  // "types" condition; the entry may be a conditions object or a bare string.
  let entry = pkg_json.get("exports").and_then(|e| e.get(export_key));
  let types_path = entry
    .and_then(|v| {
      v.get("types")
        .and_then(|t| t.as_str())
        .or_else(|| v.as_str())
    })
    .or_else(|| {
      // Fallback: top-level "types" field, only for the root export.
      if export_key == "." {
        pkg_json.get("types").and_then(|t| t.as_str())
      } else {
        None
      }
    })?;

  // Convert package-relative path to .deno/-relative path
  let pkg_name = pkg_dir.file_name()?.to_string_lossy();
  let parent_name = pkg_dir
    .parent()
    .and_then(|p| p.file_name())
    .map(|f| f.to_string_lossy())
    .unwrap_or_default();

  let types_path = types_path.strip_prefix("./").unwrap_or(types_path);

  if parent_name == "@jsr" {
    Some(format!("../node_modules/@jsr/{pkg_name}/{types_path}"))
  } else {
    Some(format!("../node_modules/{pkg_name}/{types_path}"))
  }
}

/// Generate tsconfig "paths" entries for http(s): specifiers.
///
/// The installer returns a map from user-facing URL → local mirror path
/// (already relative to `.deno/`). For X-TypeScript-Types-bearing modules
/// the local path points at the `.d.ts` rather than the JS source.
///
/// With `moduleResolution: "bundler"`, stock tsc accepts colon-containing
/// keys like `https://...`, which lets us redirect URL imports to local
/// files.
fn generate_http_paths(
  http_modules: &BTreeMap<Url, String>,
) -> Map<String, Value> {
  let mut paths = Map::new();
  for (url, local) in http_modules {
    paths.insert(url.as_str().to_string(), json!([local]));
  }
  paths
}

/// Parse a jsr specifier like "jsr:@std/assert@1" or "jsr:@scope/name@1.2.3"
/// and return (scope, name, optional_version). E.g. ("@std", "assert", Some("1")).
pub fn parse_jsr_specifier(
  specifier: &str,
) -> Option<(String, String, Option<String>)> {
  let rest = specifier.strip_prefix("jsr:")?;
  // JSR specifiers are always scoped: @scope/name@version
  if !rest.starts_with('@') {
    return None;
  }
  let slash_pos = rest.find('/')?;
  let scope = &rest[..slash_pos];
  let after_slash = &rest[slash_pos + 1..];
  // `after_slash` is `name`, `name@version`, `name/subpath`, or
  // `name@version/subpath`. Split off the version (if any), and drop any
  // trailing `/subpath` from both the version and the bare name so we never
  // feed a subpath into a semver requirement (e.g. `jsr:@std/x@1/walk`).
  let (name, version) = if let Some(at_pos) = after_slash.find('@') {
    let name = &after_slash[..at_pos];
    let version_and_subpath = &after_slash[at_pos + 1..];
    let version = version_and_subpath
      .split_once('/')
      .map(|(v, _subpath)| v)
      .unwrap_or(version_and_subpath);
    (name, Some(version.to_string()))
  } else {
    let name = after_slash
      .split_once('/')
      .map(|(n, _subpath)| n)
      .unwrap_or(after_slash);
    (name, None)
  };
  Some((scope.to_string(), name.to_string(), version))
}

/// Base compiler options for stock tsc that approximate Deno's defaults.
fn base_compiler_options() -> Map<String, Value> {
  let obj = json!({
    // Deno defaults
    "strict": true,
    "noImplicitOverride": true,
    "allowJs": true,
    "checkJs": false,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,

    // Module settings for stock tsc
    "target": "esnext",
    "module": "esnext",
    "moduleResolution": "bundler",
    "moduleDetection": "force",

    // Allow .ts extensions in imports (TS 5.0+)
    "allowImportingTsExtensions": true,

    // Standard libs (Deno-specific libs like deno.window are replaced by
    // the @types/deno package under .deno/types/)
    "lib": ["esnext"],

    // typeRoots points at our private .deno/types/ dir; "types: [deno]"
    // tells tsc to load exactly that and nothing else. Without this, tsc
    // would auto-include every @types/* in node_modules, which would
    // collide with Deno's runtime types when this file is read by Deno's
    // own checker.
    "typeRoots": ["./types"],
    "types": ["deno"],

    // Skip checking node_modules types for speed
    "skipLibCheck": true,
  });

  match obj {
    Value::Object(map) => map,
    _ => unreachable!(),
  }
}

/// Merge user's deno.json compilerOptions into the base, filtering to only
/// options that stock tsc understands.
fn merge_deno_options(base: &mut Map<String, Value>, user_opts: &Value) {
  let Some(user_map) = user_opts.as_object() else {
    return;
  };

  // Options from deno.json that map directly to tsconfig.json
  const PASSTHROUGH_OPTIONS: &[&str] = &[
    "allowUnreachableCode",
    "allowUnusedLabels",
    "checkJs",
    "emitDecoratorMetadata",
    "exactOptionalPropertyTypes",
    "experimentalDecorators",
    "isolatedDeclarations",
    "jsx",
    "jsxFactory",
    "jsxFragmentFactory",
    "jsxImportSource",
    "noErrorTruncation",
    "noFallthroughCasesInSwitch",
    "noImplicitAny",
    "noImplicitOverride",
    "noImplicitReturns",
    "noImplicitThis",
    "noPropertyAccessFromIndexSignature",
    "noUncheckedIndexedAccess",
    "noUnusedLocals",
    "noUnusedParameters",
    "paths",
    "baseUrl",
    "rootDirs",
    "skipLibCheck",
    "strict",
    "strictBindCallApply",
    "strictBuiltinIteratorReturn",
    "strictFunctionTypes",
    "strictNullChecks",
    "strictPropertyInitialization",
    "useUnknownInCatchVariables",
    "verbatimModuleSyntax",
  ];

  for &key in PASSTHROUGH_OPTIONS {
    if let Some(value) = user_map.get(key) {
      base.insert(key.to_string(), value.clone());
    }
  }

  // Handle jsx: "precompile" -> "react-jsx" (stock tsc doesn't know precompile)
  if let Some(jsx) = user_map.get("jsx").and_then(|v| v.as_str())
    && jsx == "precompile"
  {
    base.insert("jsx".to_string(), json!("react-jsx"));
  }

  // Handle lib: merge with our base lib
  if let Some(user_lib) = user_map.get("lib").and_then(|v| v.as_array()) {
    let mut libs: Vec<Value> = vec![json!("esnext")];
    for lib in user_lib {
      if let Some(s) = lib.as_str() {
        // Skip Deno-specific libs that stock tsc doesn't know
        if !s.starts_with("deno.") && s != "esnext" {
          libs.push(lib.clone());
        }
      }
    }
    base.insert("lib".to_string(), Value::Array(libs));
  }
}

/// Merge the user's `compilerOptions.types` into `base_types`, keeping only the
/// entries that actually resolve as `types` entries.
///
/// `compilerOptions.types` resolves entries as type *packages* via
/// typeRoots/node_modules; it does NOT consult `paths`. So only a bare package
/// name belongs here: unscoped (`node`) or scoped (`@types/react`). A subpath
/// entry (`lume/types.ts`) can't resolve this way and would make stock tsc/tsgo
/// fail the whole program build (TS2688), masking every real diagnostic, so it
/// is dropped. Deno accepts such entries because it loads them as modules; the
/// stock-tooling equivalent is that the (materialized) file is pulled into the
/// program by the tsconfig `include` glob, which carries its ambient
/// declarations and `/// <reference lib=... />` directives just the same.
fn merge_user_types(base_types: &mut Vec<Value>, user_types: &[Value]) {
  for entry in user_types {
    let Some(s) = entry.as_str() else { continue };
    let is_bare_package = match s.strip_prefix('@') {
      Some(rest) => rest.matches('/').count() == 1,
      None => !s.contains('/'),
    };
    if !is_bare_package {
      log::debug!(
        "sync-types: dropping non-package `types` entry {s:?} \
         (resolved via `include` instead)"
      );
      continue;
    }
    if !base_types.iter().any(|e| e == entry) {
      base_types.push(entry.clone());
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_base_compiler_options() {
    let opts = base_compiler_options();
    assert_eq!(opts.get("strict").unwrap(), &json!(true));
    assert_eq!(opts.get("target").unwrap(), &json!("esnext"));
    assert_eq!(opts.get("module").unwrap(), &json!("esnext"));
    assert_eq!(opts.get("noEmit").unwrap(), &json!(true));
  }

  #[test]
  fn test_merge_deno_options_passthrough() {
    let mut base = base_compiler_options();
    let user = json!({
      "strict": false,
      "jsx": "react-jsx",
      "jsxImportSource": "preact",
    });
    merge_deno_options(&mut base, &user);
    assert_eq!(base.get("strict").unwrap(), &json!(false));
    assert_eq!(base.get("jsx").unwrap(), &json!("react-jsx"));
    assert_eq!(base.get("jsxImportSource").unwrap(), &json!("preact"));
  }

  #[test]
  fn test_merge_deno_options_precompile_jsx() {
    let mut base = base_compiler_options();
    let user = json!({ "jsx": "precompile" });
    merge_deno_options(&mut base, &user);
    assert_eq!(base.get("jsx").unwrap(), &json!("react-jsx"));
  }

  #[test]
  fn test_merge_deno_options_filters_deno_libs() {
    let mut base = base_compiler_options();
    let user = json!({ "lib": ["deno.window", "dom", "esnext"] });
    merge_deno_options(&mut base, &user);
    let lib = base.get("lib").unwrap().as_array().unwrap();
    assert!(lib.contains(&json!("esnext")));
    assert!(lib.contains(&json!("dom")));
    assert!(!lib.iter().any(|v| v.as_str() == Some("deno.window")));
  }

  #[test]
  fn test_merge_deno_options_ignores_unknown() {
    let mut base = base_compiler_options();
    let user = json!({
      "strict": false,
      "unknownOption": true,
      "target": "es2020",
    });
    merge_deno_options(&mut base, &user);
    assert_eq!(base.get("strict").unwrap(), &json!(false));
    // unknownOption and target should not pass through
    assert!(base.get("unknownOption").is_none());
    // target stays at the base value
    assert_eq!(base.get("target").unwrap(), &json!("esnext"));
  }

  #[test]
  fn test_merge_user_types() {
    let mut base = vec![json!("deno"), json!("node")];
    let user = vec![
      // unscoped bare package -> resolves via typeRoots/node_modules, kept
      json!("react"),
      // scoped bare package -> kept
      json!("@types/react"),
      // subpath entry -> `types` can't resolve it (would TS2688); dropped and
      // instead covered by the `include` glob pulling in the mirrored file
      json!("lume/types.ts"),
      // scoped subpath entry -> also dropped
      json!("@scope/pkg/sub"),
      // duplicate of an injected type -> not added twice
      json!("node"),
    ];

    merge_user_types(&mut base, &user);

    assert_eq!(
      base,
      vec![
        json!("deno"),
        json!("node"),
        json!("react"),
        json!("@types/react"),
      ]
    );
  }

  #[test]
  fn test_parse_jsr_specifier() {
    assert_eq!(
      parse_jsr_specifier("jsr:@std/assert@1"),
      Some((
        "@std".to_string(),
        "assert".to_string(),
        Some("1".to_string())
      ))
    );
    assert_eq!(
      parse_jsr_specifier("jsr:@std/path"),
      Some(("@std".to_string(), "path".to_string(), None))
    );
    assert_eq!(
      parse_jsr_specifier("jsr:@scope/name@1.2.3"),
      Some((
        "@scope".to_string(),
        "name".to_string(),
        Some("1.2.3".to_string())
      ))
    );
  }

  #[test]
  fn test_parse_jsr_specifier_not_jsr() {
    assert_eq!(parse_jsr_specifier("npm:chalk@5"), None);
    // jsr requires scoped packages
    assert_eq!(parse_jsr_specifier("jsr:assert@1"), None);
  }

  #[test]
  fn test_generate_npm_paths_only_npm_keys() {
    let imports = json!({
      "chalk": "npm:chalk@5",
      "express": "npm:express@4",
      "@mylib/foo": "npm:@mylib/foo@1",
    });
    let paths = generate_npm_paths(Path::new("/tmp/project"), Some(&imports));

    // Should have npm: prefixed keys only
    assert!(paths.contains_key("npm:chalk"));
    assert!(paths.contains_key("npm:express"));
    assert!(paths.contains_key("npm:@mylib/foo"));

    // Should NOT have bare alias keys
    assert!(!paths.contains_key("chalk"));
    assert!(!paths.contains_key("express"));
    assert!(!paths.contains_key("@mylib/foo"));

    // Should NOT have /* glob keys
    assert!(!paths.contains_key("npm:chalk/*"));
    assert!(!paths.contains_key("chalk/*"));

    // Paths should be relative to .deno/
    assert_eq!(
      paths.get("npm:chalk").unwrap(),
      &json!(["../node_modules/chalk"])
    );
    assert_eq!(
      paths.get("npm:@mylib/foo").unwrap(),
      &json!(["../node_modules/@mylib/foo"])
    );
  }

  #[test]
  fn test_generate_npm_paths_skips_jsr() {
    let imports = json!({
      "@std/assert": "jsr:@std/assert@1",
      "chalk": "npm:chalk@5",
    });
    let paths = generate_npm_paths(Path::new("/tmp/project"), Some(&imports));

    assert!(paths.contains_key("npm:chalk"));
    // jsr specifiers should not appear in npm paths; every key is npm:-scheme
    assert!(!paths.contains_key("jsr:@std/assert"));
    assert!(!paths.contains_key("@std/assert"));
    assert!(paths.keys().all(|k| k.starts_with("npm:")));
  }

  #[test]
  fn test_generate_npm_paths_empty_imports() {
    let paths = generate_npm_paths(Path::new("/tmp/project"), None);
    assert!(paths.is_empty());

    let imports = json!({});
    let paths = generate_npm_paths(Path::new("/tmp/project"), Some(&imports));
    assert!(paths.is_empty());
  }

  #[test]
  fn test_build_tsconfig_includes_relative_to_deno_dir() {
    let project_root = Path::new("/tmp/project");
    let tsconfig = build_tsconfig(
      project_root,
      None,
      None,
      &[],
      &BTreeMap::new(),
      &Map::new(),
      false,
    );

    let include = tsconfig.get("include").unwrap().as_array().unwrap();
    assert_eq!(include, &vec![json!("../**/*")]);

    let exclude = tsconfig.get("exclude").unwrap().as_array().unwrap();
    assert_eq!(exclude, &vec![json!("../**/node_modules")]);
  }

  #[test]
  fn test_build_tsconfig_with_files() {
    let project_root = Path::new("/tmp/project");
    let files = vec!["main.ts".to_string(), "lib.ts".to_string()];
    let tsconfig = build_tsconfig(
      project_root,
      None,
      None,
      &files,
      &BTreeMap::new(),
      &Map::new(),
      false,
    );

    // Should use "files" instead of "include"/"exclude"
    assert!(tsconfig.get("include").is_none());
    assert!(tsconfig.get("exclude").is_none());
    let files_arr = tsconfig.get("files").unwrap().as_array().unwrap();
    assert_eq!(files_arr, &vec![json!("main.ts"), json!("lib.ts")]);
  }

  #[test]
  fn test_build_tsconfig_user_paths_override() {
    let project_root = Path::new("/tmp/project");
    let imports = json!({
      "chalk": "npm:chalk@5",
    });
    let compiler_options = json!({
      "paths": {
        "npm:chalk": ["./my-custom-chalk"],
        "~/*": ["./src/*"],
      },
    });
    let tsconfig = build_tsconfig(
      project_root,
      Some(&compiler_options),
      Some(&imports),
      &[],
      &BTreeMap::new(),
      &Map::new(),
      false,
    );

    let paths = tsconfig
      .get("compilerOptions")
      .unwrap()
      .get("paths")
      .unwrap()
      .as_object()
      .unwrap();

    // User's custom path should override generated one
    assert_eq!(
      paths.get("npm:chalk").unwrap(),
      &json!(["./my-custom-chalk"])
    );
    // User's custom path alias should be present
    assert_eq!(paths.get("~/*").unwrap(), &json!(["./src/*"]));
  }

  #[test]
  fn test_ensure_root_tsconfig_creates_new() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    ensure_root_tsconfig(root).unwrap();

    let content = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    let tsconfig: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
      tsconfig.get("extends").unwrap(),
      &json!("./.deno/tsconfig.json")
    );
  }

  #[test]
  fn test_ensure_root_tsconfig_updates_existing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Create an existing tsconfig.json with some settings
    std::fs::write(
      root.join("tsconfig.json"),
      r#"{ "compilerOptions": { "strict": true } }"#,
    )
    .unwrap();

    ensure_root_tsconfig(root).unwrap();

    let content = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    let tsconfig: Value = serde_json::from_str(&content).unwrap();
    // Should add extends
    assert_eq!(
      tsconfig.get("extends").unwrap(),
      &json!("./.deno/tsconfig.json")
    );
    // Should preserve existing options
    assert_eq!(
      tsconfig
        .get("compilerOptions")
        .unwrap()
        .get("strict")
        .unwrap(),
      &json!(true)
    );
  }

  #[test]
  fn test_ensure_root_tsconfig_preserves_string_extends() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
      root.join("tsconfig.json"),
      r#"{ "extends": "./team-shared.json" }"#,
    )
    .unwrap();

    ensure_root_tsconfig(root).unwrap();

    let content = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    let tsconfig: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
      tsconfig.get("extends").unwrap(),
      &json!(["./team-shared.json", "./.deno/tsconfig.json"])
    );
  }

  #[test]
  fn test_ensure_root_tsconfig_preserves_array_extends() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
      root.join("tsconfig.json"),
      r#"{ "extends": ["./a.json", "./b.json"] }"#,
    )
    .unwrap();

    ensure_root_tsconfig(root).unwrap();

    let content = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    let tsconfig: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
      tsconfig.get("extends").unwrap(),
      &json!(["./a.json", "./b.json", "./.deno/tsconfig.json"])
    );
  }

  #[test]
  fn test_ensure_root_tsconfig_array_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
      root.join("tsconfig.json"),
      r#"{ "extends": ["./a.json", "./.deno/tsconfig.json"] }"#,
    )
    .unwrap();

    ensure_root_tsconfig(root).unwrap();

    let content = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    let tsconfig: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
      tsconfig.get("extends").unwrap(),
      &json!(["./a.json", "./.deno/tsconfig.json"])
    );
  }

  #[test]
  fn test_ensure_root_tsconfig_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Create tsconfig with correct extends already set
    std::fs::write(
      root.join("tsconfig.json"),
      r#"{ "extends": "./.deno/tsconfig.json", "compilerOptions": {} }"#,
    )
    .unwrap();

    ensure_root_tsconfig(root).unwrap();

    let content = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    let tsconfig: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
      tsconfig.get("extends").unwrap(),
      &json!("./.deno/tsconfig.json")
    );
  }
}
