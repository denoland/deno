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
pub fn generate_tsconfig(
  project_root: &Path,
  deno_compiler_options: Option<&Value>,
  deno_imports: Option<&Value>,
  files: &[String],
  http_modules: &BTreeMap<Url, String>,
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
fn build_tsconfig(
  project_root: &Path,
  deno_compiler_options: Option<&Value>,
  deno_imports: Option<&Value>,
  check_files: &[String],
  http_modules: &BTreeMap<Url, String>,
) -> Value {
  let mut compiler_options = base_compiler_options();

  // Merge user's deno.json compilerOptions (filtered to stock-tsc-compatible
  // options only)
  if let Some(user_opts) = deno_compiler_options {
    merge_deno_options(&mut compiler_options, user_opts);
  }

  // Generate "paths" for npm: and jsr: specifiers only
  let mut specifier_paths = generate_npm_paths(project_root, deno_imports);
  let jsr_paths = generate_jsr_paths(project_root, deno_imports);
  specifier_paths.extend(jsr_paths);
  let http_paths = generate_http_paths(http_modules);
  specifier_paths.extend(http_paths);

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
fn generate_npm_paths(
  _project_root: &Path,
  deno_imports: Option<&Value>,
) -> Map<String, Value> {
  let mut paths = Map::new();

  if let Some(imports) = deno_imports.and_then(|v| v.as_object()) {
    for (_alias, target) in imports {
      let target_str = match target.as_str() {
        Some(s) => s,
        None => continue,
      };

      if let Some(pkg_name) = parse_npm_specifier(target_str) {
        // Paths are relative to .deno/ directory
        let nm_path = format!("../node_modules/{pkg_name}");

        // Map the npm: specifier
        // e.g. "npm:express" -> ["../node_modules/express"]
        let npm_key = format!("npm:{pkg_name}");
        paths.entry(npm_key).or_insert_with(|| json!([&nm_path]));
      }
    }
  }

  paths
}

/// Parse an npm specifier like "npm:express@4", "npm:@scope/pkg@1.2.3",
/// or "npm:express/foo" and return just the package name (without version
/// or subpath). Returns `None` if `specifier` is not a valid npm: reference.
pub fn parse_npm_specifier(specifier: &str) -> Option<String> {
  deno_semver::npm::NpmPackageReqReference::from_str(specifier)
    .ok()
    .map(|r| r.req().name.to_string())
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
    for (_alias, target) in imports {
      let target_str = match target.as_str() {
        Some(s) => s,
        None => continue,
      };

      if let Some((scope, name, _version)) = parse_jsr_specifier(target_str) {
        // JSR npm compat uses @jsr/<scope>__<name>
        let jsr_npm_name =
          format!("{}__{}", scope.trim_start_matches('@'), name);
        let nm_path = format!("../node_modules/@jsr/{jsr_npm_name}");

        // Check if the package actually exists in node_modules
        let abs_path =
          project_root.join(format!("node_modules/@jsr/{jsr_npm_name}"));
        if !abs_path.exists() {
          continue;
        }

        // Resolve the types entry point from package.json exports
        let types_entry = resolve_jsr_types_entry(&abs_path)
          .unwrap_or_else(|| nm_path.to_string());

        // Map the jsr: specifier
        let jsr_key = format!("jsr:{scope}/{name}");
        paths
          .entry(jsr_key)
          .or_insert_with(|| json!([&types_entry]));
      }
    }
  }

  paths
}

/// Resolve the types entry point from a JSR package's package.json.
///
/// Reads the `"exports"` field and looks for `"."` -> `"types"` condition.
/// Returns a path relative to `.deno/` (e.g., `../node_modules/@jsr/std__assert/_dist/mod.d.ts`).
fn resolve_jsr_types_entry(pkg_dir: &Path) -> Option<String> {
  let pkg_json_path = pkg_dir.join("package.json");
  let content = std::fs::read_to_string(&pkg_json_path).ok()?;
  let pkg_json: Value = serde_json::from_str(&content).ok()?;

  // Try exports["."]["types"]
  let types_path = pkg_json
    .get("exports")
    .and_then(|e| e.get("."))
    .and_then(|dot| dot.get("types"))
    .and_then(|t| t.as_str())
    .or_else(|| {
      // Fallback: top-level "types" field
      pkg_json.get("types").and_then(|t| t.as_str())
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
  fn test_parse_npm_specifier_unscoped() {
    assert_eq!(
      parse_npm_specifier("npm:chalk@5"),
      Some("chalk".to_string())
    );
    assert_eq!(
      parse_npm_specifier("npm:express"),
      Some("express".to_string())
    );
    assert_eq!(
      parse_npm_specifier("npm:foo@1.2.3"),
      Some("foo".to_string())
    );
  }

  #[test]
  fn test_parse_npm_specifier_scoped() {
    assert_eq!(
      parse_npm_specifier("npm:@types/node@20"),
      Some("@types/node".to_string())
    );
    assert_eq!(
      parse_npm_specifier("npm:@scope/pkg@1.2.3"),
      Some("@scope/pkg".to_string())
    );
    assert_eq!(
      parse_npm_specifier("npm:@scope/pkg"),
      Some("@scope/pkg".to_string())
    );
  }

  #[test]
  fn test_parse_npm_specifier_with_subpath() {
    assert_eq!(
      parse_npm_specifier("npm:express/foo"),
      Some("express".to_string())
    );
    assert_eq!(
      parse_npm_specifier("npm:express@4/foo"),
      Some("express".to_string())
    );
    assert_eq!(
      parse_npm_specifier("npm:@scope/pkg/subpath"),
      Some("@scope/pkg".to_string())
    );
    assert_eq!(
      parse_npm_specifier("npm:@scope/pkg@1.0.0/subpath"),
      Some("@scope/pkg".to_string())
    );
  }

  #[test]
  fn test_parse_npm_specifier_not_npm() {
    assert_eq!(parse_npm_specifier("jsr:@std/assert@1"), None);
    assert_eq!(parse_npm_specifier("chalk@5"), None);
    assert_eq!(parse_npm_specifier("https://example.com"), None);
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
    // jsr specifiers should not appear in npm paths
    assert!(!paths.contains_key("jsr:@std/assert"));
    assert!(!paths.contains_key("@std/assert"));
    assert_eq!(paths.len(), 1);
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
    let tsconfig =
      build_tsconfig(project_root, None, None, &[], &BTreeMap::new());

    let include = tsconfig.get("include").unwrap().as_array().unwrap();
    assert_eq!(include, &vec![json!("../**/*")]);

    let exclude = tsconfig.get("exclude").unwrap().as_array().unwrap();
    assert_eq!(exclude, &vec![json!("../**/node_modules")]);
  }

  #[test]
  fn test_build_tsconfig_with_files() {
    let project_root = Path::new("/tmp/project");
    let files = vec!["main.ts".to_string(), "lib.ts".to_string()];
    let tsconfig =
      build_tsconfig(project_root, None, None, &files, &BTreeMap::new());

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
