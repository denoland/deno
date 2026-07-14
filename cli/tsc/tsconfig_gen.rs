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

/// Whether Deno should honor a user's `tsconfig.json` at `project_root`.
///
/// Mirrors Deno's own config resolution (see #29925): a `tsconfig.json` is only
/// picked up when there's a sibling `deno.json`/`deno.jsonc`/`package.json` and
/// config discovery isn't disabled (`--no-config`). Otherwise the file is
/// ignored — `deno check` type-checks with Deno's own defaults and must not read
/// (or rewrite) the stray tsconfig.
pub fn should_honor_user_tsconfig(
  project_root: &Path,
  config_disabled: bool,
) -> bool {
  if config_disabled {
    return false;
  }
  ["deno.json", "deno.jsonc", "package.json"]
    .iter()
    .any(|f| project_root.join(f).exists())
}

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
#[allow(
  clippy::too_many_arguments,
  reason = "threads the independent inputs needed to generate a tsconfig"
)]
pub fn generate_tsconfig(
  project_root: &Path,
  deno_compiler_options: Option<&Value>,
  resolved_compiler_options: Option<&Value>,
  deno_imports: Option<&Value>,
  files: &[String],
  http_modules: &BTreeMap<Url, String>,
  member_paths: &Map<String, Value>,
  jsr_packages_dir: &Path,
  npm_package_paths: &BTreeMap<String, PathBuf>,
  npm_project_references: &[String],
  node_types_root: Option<&str>,
  excludes: &[String],
  manage_root_tsconfig: bool,
) -> Result<GeneratedTsConfig, std::io::Error> {
  // Write Deno type definitions to .deno/types/deno/ (private typeRoot).
  let types_dir = project_root.join(".deno/types/deno");
  std::fs::create_dir_all(&types_dir)?;
  let no_types_shims =
    no_types_npm_shims(project_root, deno_imports, npm_package_paths);
  write_deno_types(
    &types_dir.join("index.d.ts"),
    node_types_root.is_some(),
    &no_types_shims,
  )?;

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
    resolved_compiler_options,
    deno_imports,
    files,
    http_modules,
    member_paths,
    jsr_packages_dir,
    npm_package_paths,
    npm_project_references,
    node_types_root,
    excludes,
  );

  // Write to .deno/tsconfig.json
  let deno_dir = project_root.join(".deno");
  std::fs::create_dir_all(&deno_dir)?;
  let tsconfig_path = deno_dir.join("tsconfig.json");
  let content = serde_json::to_string_pretty(&tsconfig)
    .expect("failed to serialize tsconfig");
  std::fs::write(&tsconfig_path, &content)?;

  // Ensure root tsconfig.json exists and extends .deno/tsconfig.json. `deno
  // check` skips this when it will point tsc at `.deno/tsconfig.json` directly,
  // so it never rewrites a user's committed tsconfig that Deno isn't honoring.
  if manage_root_tsconfig {
    ensure_root_tsconfig(project_root, npm_project_references)?;
  }

  Ok(GeneratedTsConfig { tsconfig_path })
}

/// Ensure a root `tsconfig.json` exists that extends `.deno/tsconfig.json`.
///
/// - No existing tsconfig: create one with `extends: "./.deno/tsconfig.json"`.
/// - Existing `extends` is a single string (or absent): coerce into an array
///   with our generated path FIRST, then the original entry. TS 5.0+ resolves
///   array extends left-to-right with later entries overriding earlier ones, so
///   putting ours first makes our config the base and lets the user's own
///   config (e.g. a shared team config) override it — while our `paths`, which a
///   team config won't set, survive.
/// - Existing `extends` is already an array: prepend our path if missing.
///
/// The existing file is parsed as JSONC (tsconfig commonly has comments and
/// trailing commas); an unparseable file is a hard error rather than being
/// silently overwritten, which would drop the user's compiler options.
fn ensure_root_tsconfig(
  project_root: &Path,
  npm_project_references: &[String],
) -> Result<(), std::io::Error> {
  let root_tsconfig_path = project_root.join("tsconfig.json");
  let extends_path = "./.deno/tsconfig.json";

  if !root_tsconfig_path.exists() {
    let mut tsconfig =
      json!({ "_deno_generated": true, "extends": extends_path });
    set_root_npm_references(&mut tsconfig, npm_project_references);
    let content = serde_json::to_string_pretty(&tsconfig)
      .expect("failed to serialize tsconfig");
    return std::fs::write(&root_tsconfig_path, &content);
  }

  let content = std::fs::read_to_string(&root_tsconfig_path)?;

  // A `_deno_generated: true` sentinel marks a root tsconfig that WE created (as
  // opposed to a user-authored one). Ours is safe to regenerate wholesale; a
  // user's is only ever augmented (see the byte-splice below) so we never lose
  // their compiler options or comments. This also makes `deno check` idempotent:
  // on the second run the root tsconfig is always the one we wrote, and without
  // the sentinel we'd have no way to tell it apart from a committed config.
  if is_deno_generated_tsconfig(&content) {
    let mut tsconfig =
      json!({ "_deno_generated": true, "extends": extends_path });
    set_root_npm_references(&mut tsconfig, npm_project_references);
    let content = serde_json::to_string_pretty(&tsconfig)
      .expect("failed to serialize tsconfig");
    return std::fs::write(&root_tsconfig_path, &content);
  }

  // Parse as JSONC and edit the raw text by byte range (rather than
  // re-serializing) so the user's comments and formatting are preserved. An
  // unparseable file is a hard error rather than being silently overwritten.
  use jsonc_parser::ast::ObjectPropName;
  use jsonc_parser::ast::Value as JsoncValue;
  let ast = jsonc_parser::parse_to_ast(
    &content,
    &jsonc_parser::CollectOptions {
      comments: jsonc_parser::CommentCollectionStrategy::Off,
      tokens: false,
    },
    &jsonc_parser::ParseOptions::default(),
  )
  .map_err(|e| {
    std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      format!(
        "existing {} is not valid JSON/JSONC: {e}. Fix or remove it and re-run \
         `deno sync-types`.",
        root_tsconfig_path.display()
      ),
    )
  })?;

  let Some(JsoncValue::Object(obj)) = ast.value else {
    // Not a JSON object (empty file, array, ...) — leave it alone.
    return Ok(());
  };
  let extends = obj.properties.iter().find(|p| {
    let name = match &p.name {
      ObjectPropName::String(s) => s.value.as_ref(),
      ObjectPropName::Word(w) => w.value,
    };
    name == "extends"
  });

  let quoted = format!("\"{extends_path}\"");
  // Each arm yields Some((start, end, replacement)) as a byte-range splice, or
  // None when no change is needed.
  let edit: Option<(usize, usize, String)> = match extends.map(|p| &p.value) {
    // No `extends` yet: insert it as the first member, right after the `{`.
    None => {
      let at = obj.range.start + 1;
      let comma = if obj.properties.is_empty() { "" } else { "," };
      Some((at, at, format!("\n  \"extends\": {quoted}{comma}")))
    }
    // Already ours: nothing to do.
    Some(JsoncValue::StringLit(s)) if s.value.as_ref() == extends_path => None,
    // A single string: coerce to an array with ours first, original verbatim.
    Some(JsoncValue::StringLit(s)) => {
      let orig = &content[s.range.start..s.range.end];
      Some((s.range.start, s.range.end, format!("[{quoted}, {orig}]")))
    }
    // An array: prepend ours (right after `[`) unless it's already present.
    Some(JsoncValue::Array(arr)) => {
      let present = arr.elements.iter().any(|e| {
        matches!(e, JsoncValue::StringLit(s) if s.value.as_ref() == extends_path)
      });
      if present {
        None
      } else {
        let at = arr.range.start + 1;
        Some((at, at, format!("{quoted}, ")))
      }
    }
    // Non-string/array `extends` — leave the user's config alone.
    Some(_) => {
      log::warn!(
        "tsconfig.json has a non-string/array `extends`; not modifying. \
         Add \"{extends_path}\" to it manually for stock tsc support."
      );
      None
    }
  };

  if let Some((start, end, replacement)) = edit {
    let mut new_content =
      String::with_capacity(content.len() + replacement.len());
    new_content.push_str(&content[..start]);
    new_content.push_str(&replacement);
    new_content.push_str(&content[end..]);
    std::fs::write(&root_tsconfig_path, &new_content)?;
  }
  ensure_root_npm_references(&root_tsconfig_path, npm_project_references)
}

/// Whether a root `tsconfig.json`'s text carries the `_deno_generated: true`
/// sentinel, i.e. we created it (rather than the user). Parsed as JSONC since
/// tsconfig commonly has comments; an unparseable file is treated as not ours.
fn is_deno_generated_tsconfig(content: &str) -> bool {
  jsonc_parser::parse_to_serde_value::<Value>(
    content,
    &jsonc_parser::ParseOptions::default(),
  )
  .ok()
  .and_then(|v| v.get("_deno_generated").and_then(|s| s.as_bool()))
  .unwrap_or(false)
}

fn root_npm_reference_path(path: &str) -> String {
  format!("./.deno/{}", path.trim_start_matches("./"))
}

fn is_generated_npm_reference(value: &Value) -> bool {
  value
    .get("path")
    .and_then(|v| v.as_str())
    .is_some_and(|path| {
      path.starts_with("./.deno/npm/") || path.starts_with(".deno/npm/")
    })
}

fn set_root_npm_references(
  tsconfig: &mut Value,
  npm_project_references: &[String],
) {
  let Some(object) = tsconfig.as_object_mut() else {
    return;
  };
  let mut references = object
    .remove("references")
    .and_then(|v| v.as_array().cloned())
    .unwrap_or_default();
  references.retain(|value| !is_generated_npm_reference(value));
  references.extend(
    npm_project_references
      .iter()
      .map(|path| json!({ "path": root_npm_reference_path(path) })),
  );
  if !references.is_empty() {
    object.insert("references".to_string(), Value::Array(references));
  }
}

fn ensure_root_npm_references(
  root_tsconfig_path: &Path,
  npm_project_references: &[String],
) -> Result<(), std::io::Error> {
  let content = std::fs::read_to_string(root_tsconfig_path)?;
  let mut parsed = jsonc_parser::parse_to_serde_value::<Value>(
    &content,
    &jsonc_parser::ParseOptions::default(),
  )
  .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
  let old_references = parsed.get("references").cloned();
  set_root_npm_references(&mut parsed, npm_project_references);
  let new_references = parsed.get("references").cloned();
  if old_references == new_references {
    return Ok(());
  }

  use jsonc_parser::ast::ObjectPropName;
  use jsonc_parser::ast::Value as JsoncValue;
  use jsonc_parser::common::Ranged;
  let ast = jsonc_parser::parse_to_ast(
    &content,
    &jsonc_parser::CollectOptions {
      comments: jsonc_parser::CommentCollectionStrategy::Off,
      tokens: false,
    },
    &jsonc_parser::ParseOptions::default(),
  )
  .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
  let Some(JsoncValue::Object(object)) = ast.value else {
    return Ok(());
  };
  let property = object.properties.iter().find(|property| {
    let name = match &property.name {
      ObjectPropName::String(s) => s.value.as_ref(),
      ObjectPropName::Word(w) => w.value,
    };
    name == "references"
  });
  let replacement =
    serde_json::to_string_pretty(new_references.as_ref().unwrap_or(&json!([])))
      .expect("failed to serialize references");
  let (start, end, replacement) = match property {
    Some(property) => (
      property.value.range().start,
      property.value.range().end,
      replacement,
    ),
    None => {
      let at = object.range.start + 1;
      let comma = if object.properties.is_empty() {
        ""
      } else {
        ","
      };
      (at, at, format!("\n  \"references\": {replacement}{comma}"))
    }
  };
  let mut new_content =
    String::with_capacity(content.len() + replacement.len());
  new_content.push_str(&content[..start]);
  new_content.push_str(&replacement);
  new_content.push_str(&content[end..]);
  std::fs::write(root_tsconfig_path, new_content)
}

/// Web-global types that `@types/node` also declares (globally, via `node:url`)
/// and that must be owned by a single source. Deno ships the `URLPattern`
/// *interface* but lets `@types/node` own the *constructor var* (a second
/// unconditional `declare var URLPattern` would collide, TS2403). That split
/// means `new URLPattern()` yields Node's type while `x: URLPattern`
/// annotations use Deno's, and the shapes diverge (`@types/node`'s
/// `URLPatternResult.inputs` is `URLPatternInput[]`, Deno's/the DOM lib's is the
/// `[URLPatternInit] | [URLPatternInit, string]` tuple) -> TS2322.
///
/// This is a deliberate hack: when generating types we drop Deno's declarations
/// for the whole family so both the constructor and the instance type come from
/// `@types/node`. It assumes the `@types/node` we always install provides these
/// globals; a project pinning an `@types/node` that doesn't is on its own. The
/// proper fix is dual-globals reconciliation in Deno's core libs (which also
/// fixes `deno check`, broken on this today), not this generator.
const NODE_OWNED_GLOBAL_TYPES: &[&str] = &[
  "URLPattern",
  "URLPatternInit",
  "URLPatternInput",
  "URLPatternComponentResult",
  "URLPatternResult",
  "URLPatternOptions",
];

/// Write the Deno type declarations to a `.d.ts` file.
fn write_deno_types(
  path: &Path,
  has_node_types: bool,
  no_types_shims: &[String],
) -> Result<(), std::io::Error> {
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

  // Defer node-owned web globals to @types/node so the instance type and the
  // constructor agree (see NODE_OWNED_GLOBAL_TYPES).
  let filtered = if has_node_types {
    strip_top_level_type_decls(&filtered, NODE_OWNED_GLOBAL_TYPES)
  } else {
    filtered
  };

  // Ambient declaration so stock tsc resolves and types Deno's CSS module
  // imports. A side-effect `import "./x.css"` just needs the module to exist;
  // a `import sheet from "./x.css" with { type: "css" }` evaluates to a
  // constructable `CSSStyleSheet`, matching Deno's runtime. `CSSStyleSheet` is
  // provided by the deno.unstable lib included above. Without this, stock tsc
  // reports TS2882/TS2307 for every CSS import.
  let css_ambient = "\n\ndeclare module \"*.css\" {\n  \
     const stylesheet: CSSStyleSheet;\n  \
     export default stylesheet;\n\
     }\n";

  // Shorthand ambient declarations for imported npm packages that ship no
  // types, so tsc treats them as `any` (like Deno) instead of TS7016. Their
  // `paths` entries are omitted (see `generate_npm_paths`) so tsc falls through
  // to these ambients rather than loading the package's untyped `.js`.
  let no_types_ambient = if no_types_shims.is_empty() {
    String::new()
  } else {
    format!("\n\n{}\n", no_types_shims.join("\n"))
  };

  std::fs::write(
    path,
    format!(
      "// Auto-generated by Deno for stock TypeScript tooling.\n\
       // Do not edit — this file is regenerated as needed.\n\n\
       {filtered}{css_ambient}{no_types_ambient}"
    ),
  )
}

/// Remove top-level `interface NAME {...}` and `type NAME = ...;` declarations
/// (and their leading JSDoc block) for each NAME in `names`. Deno's lib types
/// are formatted with each top-level declaration's opening at column 0 and its
/// closing `}` at column 0, which this relies on.
fn strip_top_level_type_decls(text: &str, names: &[&str]) -> String {
  let lines: Vec<&str> = text.lines().collect();
  let mut out: Vec<&str> = Vec::with_capacity(lines.len());
  let mut i = 0;
  while i < lines.len() {
    let line = lines[i];
    let is_interface =
      names.iter().any(|n| line == format!("interface {n} {{"));
    let is_type_alias = names.iter().any(|n| {
      line.starts_with(&format!("type {n} =")) || line == format!("type {n}")
    });
    if is_interface || is_type_alias {
      // Drop the leading comment block (JSDoc etc.) immediately preceding this
      // declaration. Bounded to contiguous comment-looking / blank lines so we
      // never pop real code — a `*/` that closes a non-JSDoc `/* */` block would
      // otherwise send an unbounded "pop until /**" loop through earlier decls.
      while out.last().is_some_and(|l| {
        let t = l.trim_start();
        t.is_empty()
          || t.starts_with('*')
          || t.starts_with("/*")
          || t.starts_with("//")
      }) {
        out.pop();
      }
      if is_interface {
        // Skip to the matching top-level `}` (column 0).
        i += 1;
        while i < lines.len() && lines[i] != "}" {
          i += 1;
        }
        i += 1;
      } else {
        // Type alias. Skip until the statement terminates: a `;` at bracket
        // depth 0, so an interior `;` inside an object/union type doesn't end it
        // early. Bounded by end-of-input.
        let mut depth: i32 = 0;
        while i < lines.len() {
          let l = lines[i];
          for ch in l.chars() {
            match ch {
              '{' | '(' | '[' => depth += 1,
              '}' | ')' | ']' => depth -= 1,
              _ => {}
            }
          }
          let done = depth <= 0 && l.trim_end().ends_with(';');
          i += 1;
          if done {
            break;
          }
        }
      }
      continue;
    }
    out.push(line);
    i += 1;
  }
  out.join("\n")
}

/// Rebase a project-root-relative path onto `.deno/`, where the generated
/// tsconfig lives one level down: `.` -> `..`, `./src` -> `../src`,
/// `src` -> `../src`, `../x` -> `../../x`. Absolute paths are left untouched.
fn rebase_onto_deno_dir(path: &str) -> String {
  if path == "." {
    return "..".to_string();
  }
  if path.starts_with('/') {
    return path.to_string();
  }
  format!("../{}", path.trim_start_matches("./"))
}

/// Build the tsconfig JSON object.
///
/// The generated tsconfig lives at `.deno/tsconfig.json`, so all paths
/// are relative to that directory (e.g. `../node_modules/...` or an absolute
/// path into Deno's global npm cache).
#[allow(
  clippy::too_many_arguments,
  reason = "threads the independent inputs needed to generate a tsconfig"
)]
fn build_tsconfig(
  project_root: &Path,
  deno_compiler_options: Option<&Value>,
  resolved_compiler_options: Option<&Value>,
  deno_imports: Option<&Value>,
  check_files: &[String],
  http_modules: &BTreeMap<Url, String>,
  member_paths: &Map<String, Value>,
  jsr_packages_dir: &Path,
  npm_package_paths: &BTreeMap<String, PathBuf>,
  npm_project_references: &[String],
  node_types_root: Option<&str>,
  excludes: &[String],
) -> Value {
  let mut compiler_options = base_compiler_options();

  // When @types/node is available, load it alongside @types/deno so Node
  // globals (timers, node: builtins, Buffer, URLPattern, ...) resolve. Add the
  // selected local or npm-compat type root alongside the generated Deno types.
  if let Some(node_types_root) = node_types_root {
    compiler_options
      .insert("typeRoots".to_string(), json!(["./types", node_types_root]));
    compiler_options.insert("types".to_string(), json!(["deno", "node"]));
  }

  // Merge user's deno.json compilerOptions (filtered to stock-tsc-compatible
  // options only)
  if let Some(user_opts) = deno_compiler_options {
    merge_deno_options(&mut compiler_options, user_opts);
  }

  // Overlay Deno's *resolved* compiler options (when available). Unlike the raw
  // deno.json above, these fold in Deno's source-kind defaults (e.g. `strict`
  // and `noImplicitOverride` default off when a user `tsconfig.json` is in play,
  // but on for deno.json) and CLI overrides like `--check-js`, so they win.
  if let Some(resolved) = resolved_compiler_options {
    merge_resolved_options(&mut compiler_options, resolved);
  }

  // Generate "paths" for npm: and jsr: specifiers only
  let mut specifier_paths =
    generate_npm_paths(project_root, deno_imports, npm_package_paths);
  let jsr_paths =
    generate_jsr_paths(project_root, jsr_packages_dir, deno_imports);
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
  // priority over generated specifier mappings. The user's values are relative
  // to the project root, but the generated tsconfig lives in `.deno/`, so each
  // value is rebased one level up (like every generated mapping above).
  if let Some(user_paths) = deno_compiler_options
    .and_then(|co| co.get("paths"))
    .and_then(|p| p.as_object())
  {
    for (key, value) in user_paths {
      let rebased = match value.as_array() {
        Some(arr) => Value::Array(
          arr
            .iter()
            .map(|v| match v.as_str() {
              Some(s) => Value::String(rebase_onto_deno_dir(s)),
              None => v.clone(),
            })
            .collect(),
        ),
        None => value.clone(),
      };
      specifier_paths.insert(key.clone(), rebased);
    }
  }

  // Rebase the user's `baseUrl` onto `.deno/` too (`.` -> `..`, `./src` ->
  // `../src`), for the same reason.
  if let Some(base_url) = deno_compiler_options
    .and_then(|co| co.get("baseUrl"))
    .and_then(|b| b.as_str())
  {
    compiler_options
      .insert("baseUrl".to_string(), json!(rebase_onto_deno_dir(base_url)));
  }

  // Rebase the user's `rootDirs` onto `.deno/` too. Like `paths`/`baseUrl` these
  // hold project-root-relative directories that stock tsc would otherwise
  // resolve relative to the generated tsconfig (one level down), pointing them
  // at the wrong place.
  if let Some(root_dirs) = deno_compiler_options
    .and_then(|co| co.get("rootDirs"))
    .and_then(|r| r.as_array())
  {
    let rebased: Vec<Value> = root_dirs
      .iter()
      .map(|v| match v.as_str() {
        Some(s) => Value::String(rebase_onto_deno_dir(s)),
        None => v.clone(),
      })
      .collect();
    compiler_options.insert("rootDirs".to_string(), Value::Array(rebased));
  }

  // Merge the user's `compilerOptions.types`. Bare packages that resolve via
  // typeRoots (`deno`/`node`/`@types/*`) stay in the `types` array; entries
  // stock tsc can't resolve there (an imported npm package, a relative path) are
  // materialized as concrete `.d.ts` files added to the program below. See
  // `partition_user_types` / `merge_user_types`.
  let mut extra_type_files: Vec<String> = Vec::new();
  if let Some(user_types) = deno_compiler_options
    .and_then(|co| co.get("types"))
    .and_then(|t| t.as_array())
  {
    let (keep, files) = partition_user_types(
      project_root,
      user_types,
      deno_imports,
      npm_package_paths,
    );
    extra_type_files = files;
    if !keep.is_empty() {
      match compiler_options.get_mut("types") {
        Some(Value::Array(types)) => merge_user_types(types, &keep),
        _ => {
          compiler_options.insert("types".to_string(), Value::Array(keep));
        }
      }
    }
  }

  if !specifier_paths.is_empty() {
    compiler_options.insert("paths".to_string(), json!(specifier_paths));
  }

  // The `_deno_generated` sentinel lets Deno's own resolver identify this
  // tsconfig and exclude it from extends chains it processes — see
  // libs/resolver/deno_json.rs. Stock tsc/tsgo ignore unknown top-level
  // properties.
  let mut tsconfig = if check_files.is_empty() {
    // No specific files — check entire project. Mirror the project's own
    // `exclude` (from deno.json) so we don't type-check paths Deno itself skips
    // (test fixtures, generated output, etc.); the tsconfig lives in `.deno/`,
    // so a project-root pattern `x` is rebased to `../x`.
    let mut exclude = vec![json!("../**/node_modules")];
    for pattern in excludes {
      let rebased = format!("../{}", pattern.trim_start_matches("./"));
      exclude.push(json!(rebased));
    }
    json!({
      "_deno_generated": true,
      "compilerOptions": compiler_options,
      "include": ["../**/*"],
      "exclude": exclude,
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
  };
  // Add any declaration files materialized from `compilerOptions.types` (npm
  // packages / relative paths stock tsc can't resolve as `types`). tsc unions an
  // explicit `files` array with `include`, so this works in both branches.
  if !extra_type_files.is_empty()
    && let Some(object) = tsconfig.as_object_mut()
  {
    match object.get_mut("files") {
      Some(Value::Array(files)) => {
        files.extend(extra_type_files.iter().map(|f| json!(f)));
      }
      _ => {
        object.insert("files".to_string(), json!(extra_type_files));
      }
    }
  }
  if !npm_project_references.is_empty()
    && let Some(object) = tsconfig.as_object_mut()
  {
    object.insert(
      "references".to_string(),
      Value::Array(
        npm_project_references
          .iter()
          .map(|path| json!({ "path": path }))
          .collect(),
      ),
    );
  }
  tsconfig
}

/// Generate tsconfig "paths" entries for npm: specifiers.
///
/// Scans `deno.json` `"imports"` for entries like:
///   `"express": "npm:express@4"` -> `{ "npm:express": ["<resolved package>"] }`
///
/// With a local `node_modules`, bare aliases continue to resolve through normal
/// TypeScript package resolution. Without one, both aliases and npm specifiers
/// map to the exact resolved package folders in Deno's global cache.
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

/// Whether a materialized npm package ships no types at all, so stock tsc would
/// report TS7016 ("implicitly has an 'any' type") for it. Conservative: returns
/// false (assume typed) unless `package.json` is readable AND declares no
/// `types`/`typings`/`exports.types` AND has no `.d.ts` beside its main entry.
/// An unreadable/absent `package.json` returns false so we keep the default
/// path mapping (e.g. minimal fixtures, or generation running before install).
/// Whether a package.json `exports` value declares a `types` condition
/// anywhere in its (possibly deeply nested) conditions tree.
fn exports_declares_types(value: &Value) -> bool {
  match value {
    Value::Object(map) => map.iter().any(|(k, v)| {
      (k == "types" && v.is_string()) || exports_declares_types(v)
    }),
    Value::Array(arr) => arr.iter().any(exports_declares_types),
    _ => false,
  }
}

fn package_ships_no_types(pkg_dir: &Path) -> bool {
  let Ok(content) = std::fs::read_to_string(pkg_dir.join("package.json"))
  else {
    return false;
  };
  let Ok(pkg_json) = serde_json::from_str::<Value>(&content) else {
    return false;
  };
  // Explicit types via `exports.types` or top-level `types`/`typings`, or a
  // `types` condition nested anywhere in `exports` (e.g. conditions-only
  // `exports: { "node": { "types": "..." } }`, which has no "." key so
  // `resolve_package_types_entry_path` doesn't see it).
  if resolve_package_types_entry_path(pkg_dir, ".").is_some()
    || pkg_json.get("typings").and_then(|t| t.as_str()).is_some()
    || pkg_json.get("exports").is_some_and(exports_declares_types)
  {
    return false;
  }
  // Convention: a `.d.ts` beside the main entry (`index.js` -> `index.d.ts`).
  let main = pkg_json
    .get("main")
    .and_then(|m| m.as_str())
    .unwrap_or("index.js")
    .trim_start_matches("./");
  let main_dts = match main.rsplit_once('.') {
    Some((base, _)) => format!("{base}.d.ts"),
    None => format!("{main}.d.ts"),
  };
  if pkg_dir.join(&main_dts).exists() || pkg_dir.join("index.d.ts").exists() {
    return false;
  }
  true
}

/// Shorthand `declare module` lines for imported npm packages that ship no
/// resolvable types, so stock tsc treats them as `any` (like Deno) instead of
/// reporting TS7016. Only bare (non-subpath) imports of materialized packages
/// that `package_ships_no_types` confirms are untyped are shimmed; their `paths`
/// entries are skipped (see `generate_npm_paths`) so tsc uses these ambients.
fn no_types_npm_shims(
  project_root: &Path,
  deno_imports: Option<&Value>,
  npm_package_paths: &BTreeMap<String, PathBuf>,
) -> Vec<String> {
  let mut shims: Vec<String> = Vec::new();
  let Some(imports) = deno_imports.and_then(|v| v.as_object()) else {
    return shims;
  };
  for (alias, target) in imports {
    let Some(target_str) = target.as_str() else {
      continue;
    };
    if !target_str.starts_with("npm:") {
      continue;
    }
    let Ok(npm_ref) =
      deno_semver::npm::NpmPackageReqReference::from_str(target_str)
    else {
      continue;
    };
    if npm_ref.sub_path().is_some() {
      continue;
    }
    let pkg_name = npm_ref.req().name.to_string();
    let local_pkg_dir = project_root.join(format!("node_modules/{pkg_name}"));
    let pkg_dir = npm_package_paths
      .get(target_str)
      .cloned()
      .unwrap_or_else(|| local_pkg_dir.clone());
    if !package_ships_no_types(&pkg_dir) {
      continue;
    }
    let mut specs = vec![format!("npm:{pkg_name}"), target_str.to_string()];
    if alias != &pkg_name {
      specs.push(alias.clone());
    }
    for spec in specs {
      shims.push(format!("declare module {spec:?};"));
      shims.push(format!("declare module {:?};", format!("{spec}/*")));
    }
  }
  shims.sort();
  shims.dedup();
  shims
}

fn generate_npm_paths(
  project_root: &Path,
  deno_imports: Option<&Value>,
  npm_package_paths: &BTreeMap<String, PathBuf>,
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
      let local_pkg_dir = project_root.join(format!("node_modules/{pkg_name}"));
      let pkg_dir = npm_package_paths
        .get(target_str)
        .cloned()
        .unwrap_or_else(|| local_pkg_dir.clone());
      let is_global_cache = pkg_dir != local_pkg_dir;
      // Only map a package that is actually materialized under `node_modules`.
      // With Deno's default global-cache mode, `deno install` may resolve npm
      // deps without a local `node_modules/<pkg>`; emitting a path to a
      // nonexistent directory just makes stock tsc fail. The caller warns when
      // this happens so the user knows to enable a node_modules directory.
      if !pkg_dir.exists() {
        continue;
      }

      // A plain alias whose name matches the package (`"chalk": "npm:chalk"`,
      // and its subpaths like `chalk/foo`) resolves natively through
      // `node_modules` under `moduleResolution: bundler`, so an alias key would
      // be redundant. A *renamed* alias (`"$prism": "npm:prismjs"`, so source
      // writes `$prism/components/x`) has no `node_modules/$prism`, so it needs
      // an explicit mapping. `npm:`-scheme keys are always emitted since bundler
      // can't resolve the scheme on its own.
      let alias_needs_mapping = is_global_cache || alias != &pkg_name;
      match npm_ref.sub_path() {
        Some(sub) => {
          // `npm:preact/compat`: resolve through the package's `exports` to a
          // concrete .d.ts (relative, to avoid TS5090). Fall back to the naive
          // subpath under the package dir when exports can't be read (e.g. the
          // package isn't materialized yet). Key both the version-less scheme
          // (`npm:preact/compat`) and the exact specifier as written, plus the
          // source-written alias form when the alias is renamed.
          let rel = if is_global_cache {
            resolve_package_types_entry_path(&pkg_dir, &format!("./{sub}"))
              .unwrap_or_else(|| pkg_dir.join(sub))
              .to_string_lossy()
              .replace('\\', "/")
          } else {
            resolve_jsr_types_entry(&pkg_dir, &format!("./{sub}"))
              .unwrap_or_else(|| format!("../node_modules/{pkg_name}/{sub}"))
          };
          paths
            .entry(format!("npm:{pkg_name}/{sub}"))
            .or_insert_with(|| json!([&rel]));
          paths
            .entry(target_str.to_string())
            .or_insert_with(|| json!([&rel]));
          if alias_needs_mapping {
            paths.entry(alias.clone()).or_insert_with(|| json!([&rel]));
          }
        }
        None => {
          // A package that ships no types is shimmed as an ambient `any` module
          // in the generated Deno types (see `no_types_npm_shims`). Skip its
          // path mappings so tsc falls through to that ambient instead of
          // loading the untyped `.js` and reporting TS7016.
          if package_ships_no_types(&pkg_dir) {
            continue;
          }
          // Bare `npm:preact`: in global-cache mode map straight to the
          // resolved types entry (`.d.ts`) so tsc doesn't have to match the
          // package's `exports` conditions itself - its bundler conditions
          // exclude `node`/`deno`, which deno resolves, so a
          // `exports: { "node": { "types": ... } }` package would otherwise be
          // TS2307. Fall back to the package directory when no types entry
          // resolves. In local `node_modules` mode tsc resolves the package
          // natively, so map to the directory. Key both the version-less scheme
          // (`npm:preact`) and the exact specifier.
          let dir = if is_global_cache {
            resolve_package_types_entry_path(&pkg_dir, ".")
              .unwrap_or_else(|| pkg_dir.clone())
              .to_string_lossy()
              .replace('\\', "/")
          } else {
            format!("../node_modules/{pkg_name}")
          };
          paths
            .entry(format!("npm:{pkg_name}"))
            .or_insert_with(|| json!([&dir]));
          paths
            .entry(target_str.to_string())
            .or_insert_with(|| json!([&dir]));
          if alias_needs_mapping {
            paths.entry(alias.clone()).or_insert_with(|| json!([&dir]));
          }
          // Enumerate the package's own `exports` so subpaths written in source
          // map to their concrete .d.ts: the `npm:` scheme form always, and the
          // renamed-alias form (`$prism/components`) when applicable.
          for exp_key in package_export_keys(&pkg_dir) {
            let sub = exp_key.trim_start_matches("./");
            let Some(sub_rel) = (if is_global_cache {
              resolve_package_types_entry_path(&pkg_dir, &exp_key)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
            } else {
              resolve_jsr_types_entry(&pkg_dir, &exp_key)
            }) else {
              continue;
            };
            paths
              .entry(format!("npm:{pkg_name}/{sub}"))
              .or_insert_with(|| json!([&sub_rel]));
            if alias_needs_mapping {
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
/// into the selected compatibility directory. Maps `jsr:@scope/name` to that
/// path.
///
/// Only generates `jsr:<scope>/<name>` keys.
fn generate_jsr_paths(
  project_root: &Path,
  jsr_packages_dir: &Path,
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
        // JSR npm compat uses the flattened <scope>__<name> package name.
        let jsr_npm_name =
          format!("{}__{}", scope.trim_start_matches('@'), name);
        let pkg_dir = jsr_packages_dir.join(&jsr_npm_name);
        if !pkg_dir.exists() {
          continue;
        }

        // Recover the subpath the source actually imports (everything after
        // `jsr:@scope/name[@version]`), and resolve it through the installed
        // package's `exports`, preferring the generated `.d.ts` declaration so
        // stock tsc consumes it under `skipLibCheck` instead of type-checking
        // the dependency's `.ts` source. Falls back to the `.ts` source only
        // when the package ships no declaration for the export.
        let prefix = match &version {
          Some(v) => format!("jsr:{scope}/{name}@{v}"),
          None => format!("jsr:{scope}/{name}"),
        };
        // Normalize a `jsr:/…` (deno_graph resolved form) to `jsr:…` so the
        // prefix match below works; the emitted key keeps the original spelling.
        let norm_target = target_str
          .strip_prefix("jsr:/")
          .map(|r| format!("jsr:{r}"))
          .unwrap_or_else(|| target_str.to_string());
        let subpath = norm_target
          .strip_prefix(&prefix)
          .map(|s| s.trim_start_matches('/'))
          .unwrap_or("");
        let export_key = if subpath.is_empty() {
          ".".to_string()
        } else {
          format!("./{subpath}")
        };
        let Some(source_rel) = resolve_jsr_types_entry_for_config(
          project_root,
          &pkg_dir,
          &export_key,
        ) else {
          continue;
        };
        let compat_alias = format!("@jsr/{jsr_npm_name}");
        let compat_key = if subpath.is_empty() {
          compat_alias.clone()
        } else {
          format!("{compat_alias}/{subpath}")
        };

        // Key on the exact specifier as written in source. Also map the
        // import-map alias form when this came from an alias entry, including
        // aliases that point straight at a jsr subpath (e.g.
        // `"$std/fs/walk.ts": "jsr:@std/fs@1/walk"`), which the source imports
        // by the alias, not the scheme.
        paths
          .entry(target_str.to_string())
          .or_insert_with(|| json!([&source_rel]));
        if alias != target_str {
          paths
            .entry(alias.clone())
            .or_insert_with(|| json!([&source_rel]));
        }
        // The npm-compat runtime source rewrites JSR dependencies to their
        // flattened @jsr package names. There is no node_modules tree in global
        // cache mode, so make those internal bare imports resolve too.
        paths
          .entry(compat_key)
          .or_insert_with(|| json!([&source_rel]));

        // For a bare alias -> package (no subpath), enumerate the package's own
        // exports and map each under both the alias and the jsr: specifier, so
        // subpath imports like `fresh/runtime` resolve without depending on the
        // module graph having discovered them.
        if subpath.is_empty() {
          for exp_key in package_export_keys(&pkg_dir) {
            let sub = exp_key.trim_start_matches("./");
            let Some(sub_rel) = resolve_jsr_types_entry_for_config(
              project_root,
              &pkg_dir,
              &exp_key,
            ) else {
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
            paths
              .entry(format!("{compat_alias}/{sub}"))
              .or_insert_with(|| json!([&sub_rel]));
          }
        }
      }
    }
  }

  paths
}

/// Resolve a JSR export for the generated `paths` table, preferring the
/// generated `.d.ts` declaration (the `types` condition / declaration entry).
///
/// `skipLibCheck` only skips `.d.ts` files, so mapping a jsr dependency's
/// `paths` entry to its `.ts` *source* makes stock tsc type-check the
/// dependency's implementation — a regression. Prefer the declaration and fall
/// back to the `.ts` source only when the package ships no declaration for the
/// export (in which case tsc has nothing else to consume).
fn resolve_jsr_types_entry_for_config(
  project_root: &Path,
  pkg_dir: &Path,
  export_key: &str,
) -> Option<String> {
  let resolved = resolve_package_types_entry_path(pkg_dir, export_key)
    .filter(|p| p.exists())
    .or_else(|| resolve_package_source_entry_path(pkg_dir, export_key))?;
  if pkg_dir.starts_with(project_root.join("node_modules")) {
    path_relative_to_deno_dir(pkg_dir, &resolved)
  } else {
    Some(resolved.to_string_lossy().replace('\\', "/"))
  }
}

pub fn resolve_package_source_entry_path(
  pkg_dir: &Path,
  export_key: &str,
) -> Option<PathBuf> {
  let content = std::fs::read_to_string(pkg_dir.join("package.json")).ok()?;
  let pkg_json: Value = serde_json::from_str(&content).ok()?;
  let entry = pkg_json.get("exports").and_then(|e| e.get(export_key))?;
  let source_path = entry
    .get("default")
    .and_then(|v| v.as_str())
    .or_else(|| entry.as_str())?;
  let runtime_path =
    pkg_dir.join(source_path.strip_prefix("./").unwrap_or(source_path));

  // TypeScript extension substitution would find this sibling too, but making
  // it explicit keeps the generated mapping useful to non-TypeScript-aware
  // consumers that understand tsconfig paths.
  let source_path = match runtime_path.extension().and_then(|e| e.to_str()) {
    Some("js") => runtime_path.with_extension("ts"),
    Some("mjs") => runtime_path.with_extension("mts"),
    Some("cjs") => runtime_path.with_extension("cts"),
    _ => runtime_path.clone(),
  };
  if source_path.exists() {
    Some(source_path)
  } else if runtime_path.exists() {
    Some(runtime_path)
  } else {
    None
  }
}

fn path_relative_to_deno_dir(
  pkg_dir: &Path,
  resolved: &Path,
) -> Option<String> {
  let package_path = resolved.strip_prefix(pkg_dir).ok()?.to_string_lossy();
  let pkg_name = pkg_dir.file_name()?.to_string_lossy();
  let parent_name = pkg_dir
    .parent()
    .and_then(|p| p.file_name())
    .map(|f| f.to_string_lossy())
    .unwrap_or_default();

  if parent_name == "@jsr" {
    Some(format!("../node_modules/@jsr/{pkg_name}/{package_path}"))
  } else {
    Some(format!("../node_modules/{pkg_name}/{package_path}"))
  }
}

/// List a package's `exports` subpath keys (`"./foo"`), excluding the root
/// `"."`. Used to enumerate what an import-map alias can reach by subpath.
pub fn package_export_keys(pkg_dir: &Path) -> Vec<String> {
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

fn resolve_jsr_types_entry(pkg_dir: &Path, export_key: &str) -> Option<String> {
  let resolved = resolve_package_types_entry_path(pkg_dir, export_key)?;
  path_relative_to_deno_dir(pkg_dir, &resolved)
}

pub fn resolve_package_types_entry_path(
  pkg_dir: &Path,
  export_key: &str,
) -> Option<PathBuf> {
  let pkg_json_path = pkg_dir.join("package.json");
  let content = std::fs::read_to_string(&pkg_json_path).ok()?;
  let pkg_json: Value = serde_json::from_str(&content).ok()?;

  // Resolve `exports[export_key]` (e.g. "." or "./cookie_map"), preferring a
  // `types` condition (possibly nested inside condition objects like
  // `node`/`import`). The entry may be a conditions object or a bare string.
  let exports = pkg_json.get("exports");
  let entry = exports.and_then(|e| e.get(export_key)).or_else(|| {
    // A conditions-only `exports` (no subpath keys, e.g.
    // `exports: { "node": { "types": "..." } }`) is itself the "." target's
    // conditions.
    if export_key == "." && exports.is_some_and(is_conditions_only_exports) {
      exports
    } else {
      None
    }
  });
  let types_path = entry.and_then(export_types_target).or_else(|| {
    // Fallback: top-level "types" field, only for the root export.
    if export_key == "." {
      pkg_json.get("types").and_then(|t| t.as_str())
    } else {
      None
    }
  })?;

  let types_path = types_path.strip_prefix("./").unwrap_or(types_path);
  Some(pkg_dir.join(types_path))
}

/// Whether a package.json `exports` value is conditions-only: an object whose
/// keys are all conditions (none is a "." or "./"-prefixed subpath), in which
/// case the whole object is the root export's conditions.
fn is_conditions_only_exports(exports: &Value) -> bool {
  exports
    .as_object()
    .is_some_and(|m| !m.is_empty() && m.keys().all(|k| !k.starts_with('.')))
}

/// The types target of a resolved export entry: a `types` condition found
/// anywhere in the (possibly nested) conditions tree, or a bare string entry
/// (e.g. `exports: { ".": "./index.d.ts" }`). Does not fall through to
/// JS-only conditions (`import`/`default` without `types`), so a package that
/// declares no types resolves to `None`.
fn export_types_target(value: &Value) -> Option<&str> {
  match value {
    Value::String(s) => Some(s),
    Value::Object(map) => find_types_condition(map),
    Value::Array(arr) => arr.iter().find_map(export_types_target),
    _ => None,
  }
}

fn find_types_condition(map: &Map<String, Value>) -> Option<&str> {
  if let Some(Value::String(s)) = map.get("types") {
    return Some(s);
  }
  map
    .iter()
    .filter(|(k, _)| *k != "types")
    .find_map(|(_, v)| match v {
      Value::Object(m) => find_types_condition(m),
      Value::Array(a) => a.iter().find_map(|x| match x {
        Value::Object(m) => find_types_condition(m),
        _ => None,
      }),
      _ => None,
    })
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
  // deno_graph emits resolved jsr specifiers as `jsr:/@scope/...` (slash after
  // the scheme); accept that form too.
  let rest = rest.strip_prefix('/').unwrap_or(rest);
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
    // NOTE: `paths`, `baseUrl`, `rootDirs` are deliberately NOT passed through
    // here — they hold project-root-relative paths that must be rebased onto
    // `.deno/` (the generated tsconfig lives one level down). They're handled in
    // `build_tsconfig`.
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
    base.insert("lib".to_string(), filter_stock_libs(user_lib));
  }
}

/// Type-checking options taken from Deno's *resolved* compiler options rather
/// than the raw deno.json. These fold in Deno's source-kind defaults and CLI
/// overrides (e.g. `--check-js`), so they win over the base and the raw pass.
///
/// Deliberately excludes `skipLibCheck` (we always keep it on for speed) and the
/// path-like options (`paths`/`baseUrl`/`rootDirs`), which come from the raw
/// config so `build_tsconfig` can rebase them onto `.deno/`.
const RESOLVED_OPTION_KEYS: &[&str] = &[
  "allowUnreachableCode",
  "allowUnusedLabels",
  "checkJs",
  "emitDecoratorMetadata",
  "erasableSyntaxOnly",
  "exactOptionalPropertyTypes",
  "experimentalDecorators",
  "isolatedDeclarations",
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
  "strict",
  "strictBindCallApply",
  "strictBuiltinIteratorReturn",
  "strictFunctionTypes",
  "strictNullChecks",
  "strictPropertyInitialization",
  "useUnknownInCatchVariables",
  "verbatimModuleSyntax",
];

/// Overlay Deno's resolved compiler options (see `RESOLVED_OPTION_KEYS`) onto
/// the base. `jsx` and `lib` need translation for stock tsc, so they're handled
/// specially rather than copied verbatim.
fn merge_resolved_options(base: &mut Map<String, Value>, resolved: &Value) {
  let Some(map) = resolved.as_object() else {
    return;
  };
  for &key in RESOLVED_OPTION_KEYS {
    if let Some(value) = map.get(key) {
      base.insert(key.to_string(), value.clone());
    }
  }
  // stock tsc doesn't know Deno's `precompile`; treat it as `react-jsx`.
  match map.get("jsx").and_then(|v| v.as_str()) {
    Some("precompile") => {
      base.insert("jsx".to_string(), json!("react-jsx"));
    }
    Some(jsx) => {
      base.insert("jsx".to_string(), json!(jsx));
    }
    None => {}
  }
  if let Some(resolved_lib) = map.get("lib").and_then(|v| v.as_array()) {
    base.insert("lib".to_string(), filter_stock_libs(resolved_lib));
  }
}

/// Keep only libs stock tsc recognizes, always including `esnext`. Deno-specific
/// libs (`deno.*`) and the `node` pseudo-lib (provided instead via `@types/node`)
/// would make stock tsc error (TS6046), so they're dropped.
fn filter_stock_libs(libs: &[Value]) -> Value {
  let mut out: Vec<Value> = vec![json!("esnext")];
  for lib in libs {
    if let Some(s) = lib.as_str()
      && !s.starts_with("deno.")
      && s != "esnext"
      && s != "node"
    {
      out.push(lib.clone());
    }
  }
  Value::Array(out)
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
/// Resolve the cache/node_modules directory of a `compilerOptions.types` entry
/// that names an npm package the user imports (matched by import alias).
fn npm_types_pkg_dir(
  project_root: &Path,
  name: &str,
  deno_imports: Option<&Value>,
  npm_package_paths: &BTreeMap<String, PathBuf>,
) -> Option<PathBuf> {
  let target = deno_imports?.as_object()?.get(name)?.as_str()?;
  if !target.starts_with("npm:") {
    return None;
  }
  let npm_ref =
    deno_semver::npm::NpmPackageReqReference::from_str(target).ok()?;
  let pkg_name = npm_ref.req().name.to_string();
  let local = project_root.join(format!("node_modules/{pkg_name}"));
  let dir = npm_package_paths.get(target).cloned().unwrap_or(local);
  dir.exists().then_some(dir)
}

/// Partition a user's `compilerOptions.types` into entries stock tsc can resolve
/// via typeRoots (kept in the `types` array) and entries it cannot — a bare npm
/// package the user imports, or a relative path — which are materialized as
/// concrete `.d.ts` files added to the program instead.
///
/// Stock tsc resolves a `types` entry only as a package under
/// `typeRoots`/`node_modules/@types`; it never consults `paths`, and a plain
/// (non-`@types`) npm package or a relative path can't resolve that way at all,
/// which fails the whole build with TS2688 and masks every real diagnostic. Deno
/// accepts these because it loads them as modules; the stock-tooling equivalent
/// is to pull the actual declaration file into the program via `files`, which
/// carries its ambient/global declarations just the same.
fn partition_user_types(
  project_root: &Path,
  user_types: &[Value],
  deno_imports: Option<&Value>,
  npm_package_paths: &BTreeMap<String, PathBuf>,
) -> (Vec<Value>, Vec<String>) {
  let mut keep_in_types = Vec::new();
  let mut type_files = Vec::new();
  for entry in user_types {
    let Some(s) = entry.as_str() else {
      keep_in_types.push(entry.clone());
      continue;
    };
    // A relative/path-like entry (`./types.d.ts`) resolves relative to the
    // generated tsconfig in `.deno/`, pointing at the wrong place; materialize
    // it as an absolute file instead.
    let is_path_like =
      s.starts_with('.') || s.starts_with('/') || s.ends_with(".ts");
    if is_path_like {
      let abs = project_root.join(s.trim_start_matches("./"));
      type_files.push(abs.to_string_lossy().replace('\\', "/"));
      continue;
    }
    // A bare npm package the user imports: pull in its declaration file.
    if let Some(pkg_dir) =
      npm_types_pkg_dir(project_root, s, deno_imports, npm_package_paths)
      && let Some(dts) = resolve_package_types_entry_path(&pkg_dir, ".")
      && dts.exists()
    {
      type_files.push(dts.to_string_lossy().replace('\\', "/"));
      continue;
    }
    // `deno`, `node`, `@types/*`: resolvable via typeRoots, keep as-is.
    keep_in_types.push(entry.clone());
  }
  (keep_in_types, type_files)
}

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
  fn test_strip_top_level_type_decls() {
    let text = "\
interface Keep {
  a: number;
}

/** JSDoc for URLPattern. */
interface URLPattern {
  exec(): void;
}

type URLPatternInput = string | URLPatternInit;

interface AlsoKeep {
  b: string;
}";
    let out =
      strip_top_level_type_decls(text, &["URLPattern", "URLPatternInput"]);
    assert!(out.contains("interface Keep {"));
    assert!(out.contains("interface AlsoKeep {"));
    // stripped: the interface, the type alias, and the JSDoc block
    assert!(!out.contains("interface URLPattern {"));
    assert!(!out.contains("type URLPatternInput"));
    assert!(!out.contains("JSDoc for URLPattern"));
    // nested/other members untouched
    assert!(!out.contains("exec(): void"));
  }

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
  fn test_exports_declares_types() {
    // conditions-only exports with a nested `types` (no "." key) - the case
    // resolve_package_types_entry_path(".") misses.
    assert!(exports_declares_types(&json!({
      "node": { "types": "./dist/main.d.ts", "import": "./dist/main.mjs" }
    })));
    // subpath export whose condition carries types.
    assert!(exports_declares_types(&json!({
      ".": { "import": { "types": "./index.d.ts", "default": "./index.mjs" } }
    })));
    // array form.
    assert!(exports_declares_types(&json!({
      ".": [{ "types": "./a.d.ts" }, "./b.js"]
    })));
    // genuinely no types anywhere.
    assert!(!exports_declares_types(&json!({
      ".": { "import": "./index.mjs", "require": "./index.cjs" }
    })));
    // a subpath literally named "types" is not a types condition (value is not
    // a string pointing at a declaration - here it is an object).
    assert!(!exports_declares_types(&json!({
      "./types": { "import": "./types.mjs" }
    })));
  }

  #[test]
  fn test_export_types_target() {
    // conditions-only root export (no "." key) with nested types.
    assert!(is_conditions_only_exports(&json!({
      "node": { "types": "./dist/main.d.ts", "import": "./dist/main.mjs" }
    })));
    assert!(!is_conditions_only_exports(&json!({ ".": "./index.js" })));
    // `types` nested inside a condition object is found.
    assert_eq!(
      export_types_target(&json!({
        "node": { "types": "./dist/main.d.ts", "import": "./dist/main.mjs" }
      })),
      Some("./dist/main.d.ts")
    );
    // direct `types` condition.
    assert_eq!(
      export_types_target(&json!({ "types": "./i.d.ts", "import": "./i.mjs" })),
      Some("./i.d.ts")
    );
    // bare string entry (e.g. exports: { ".": "./index.d.ts" }).
    assert_eq!(
      export_types_target(&json!("./index.d.ts")),
      Some("./index.d.ts")
    );
    // no types anywhere -> None (do not fall through to JS-only conditions).
    assert_eq!(
      export_types_target(
        &json!({ "import": "./i.mjs", "require": "./i.cjs" })
      ),
      None
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
    // deno_graph resolved form: `jsr:/` with a slash after the scheme, plus a
    // subpath after the version.
    assert_eq!(
      parse_jsr_specifier("jsr:/@std/async@^1/deadline"),
      Some((
        "@std".to_string(),
        "async".to_string(),
        Some("^1".to_string())
      ))
    );
  }

  #[test]
  fn test_parse_jsr_specifier_not_jsr() {
    assert_eq!(parse_jsr_specifier("npm:chalk@5"), None);
    // jsr requires scoped packages
    assert_eq!(parse_jsr_specifier("jsr:assert@1"), None);
  }

  // Create empty `node_modules/<pkg>` dirs so generate_npm_paths (which only
  // maps materialized packages) has something to map.
  fn touch_node_modules(root: &Path, pkgs: &[&str]) {
    for pkg in pkgs {
      std::fs::create_dir_all(root.join("node_modules").join(pkg)).unwrap();
    }
  }

  #[test]
  fn test_generate_npm_paths_only_npm_keys() {
    let dir = tempfile::tempdir().unwrap();
    touch_node_modules(dir.path(), &["chalk", "express", "@mylib/foo"]);
    let imports = json!({
      "chalk": "npm:chalk@5",
      "express": "npm:express@4",
      "@mylib/foo": "npm:@mylib/foo@1",
    });
    let paths =
      generate_npm_paths(dir.path(), Some(&imports), &BTreeMap::new());

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
  fn test_generate_npm_paths_skips_unmaterialized() {
    // No node_modules on disk -> nothing to map (avoids dangling paths).
    let dir = tempfile::tempdir().unwrap();
    let imports = json!({ "chalk": "npm:chalk@5" });
    let paths =
      generate_npm_paths(dir.path(), Some(&imports), &BTreeMap::new());
    assert!(paths.is_empty());
  }

  #[test]
  fn test_generate_npm_paths_uses_global_cache() {
    let project = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let pkg_dir = cache.path().join("chalk/5.0.0");
    std::fs::create_dir_all(&pkg_dir).unwrap();
    let imports = json!({ "chalk": "npm:chalk@5" });
    let npm_package_paths =
      BTreeMap::from([("npm:chalk@5".to_string(), pkg_dir.clone())]);

    let paths =
      generate_npm_paths(project.path(), Some(&imports), &npm_package_paths);
    let expected = json!([pkg_dir.to_string_lossy().replace('\\', "/")]);
    assert_eq!(paths.get("chalk"), Some(&expected));
    assert_eq!(paths.get("npm:chalk"), Some(&expected));
    assert_eq!(paths.get("npm:chalk@5"), Some(&expected));
  }

  #[test]
  fn test_generate_npm_paths_skips_jsr() {
    let dir = tempfile::tempdir().unwrap();
    touch_node_modules(dir.path(), &["chalk"]);
    let imports = json!({
      "@std/assert": "jsr:@std/assert@1",
      "chalk": "npm:chalk@5",
    });
    let paths =
      generate_npm_paths(dir.path(), Some(&imports), &BTreeMap::new());

    assert!(paths.contains_key("npm:chalk"));
    // jsr specifiers should not appear in npm paths; every key is npm:-scheme
    assert!(!paths.contains_key("jsr:@std/assert"));
    assert!(!paths.contains_key("@std/assert"));
    assert!(paths.keys().all(|k| k.starts_with("npm:")));
  }

  #[test]
  fn test_generate_jsr_paths_prefers_declarations() {
    let project = tempfile::tempdir().unwrap();
    let jsr_dir = project.path().join(".deno/npm-compat/@jsr");
    let pkg_dir = jsr_dir.join("std__example");
    std::fs::create_dir_all(pkg_dir.join("_dist")).unwrap();
    std::fs::write(
      pkg_dir.join("package.json"),
      serde_json::to_string(&json!({
        "exports": {
          ".": {
            "types": "./_dist/mod.d.ts",
            "default": "./mod.js",
          },
          "./subpath": {
            "types": "./_dist/subpath.d.ts",
            "default": "./subpath.js",
          },
          // No `types` and no shipped declaration: must fall back to source.
          "./nodecl": {
            "default": "./nodecl.js",
          },
        }
      }))
      .unwrap(),
    )
    .unwrap();
    for file in [
      "mod.js",
      "mod.ts",
      "subpath.js",
      "subpath.ts",
      "nodecl.js",
      "nodecl.ts",
      "_dist/mod.d.ts",
      "_dist/subpath.d.ts",
    ] {
      std::fs::write(pkg_dir.join(file), "").unwrap();
    }

    let imports = json!({
      "example": "jsr:@std/example@1",
    });
    let paths = generate_jsr_paths(project.path(), &jsr_dir, Some(&imports));
    let rel =
      |p: &str| json!([pkg_dir.join(p).to_string_lossy().replace('\\', "/")]);
    // Root and declared subpath prefer the generated `.d.ts` declaration so
    // stock tsc consumes it under `skipLibCheck` rather than type-checking the
    // dependency's `.ts` source.
    let mod_dts = rel("_dist/mod.d.ts");
    let subpath_dts = rel("_dist/subpath.d.ts");
    // The export without a declaration falls back to the `.ts` source.
    let nodecl_ts = rel("nodecl.ts");

    assert_eq!(paths.get("jsr:@std/example@1"), Some(&mod_dts));
    assert_eq!(paths.get("example"), Some(&mod_dts));
    assert_eq!(paths.get("@jsr/std__example"), Some(&mod_dts));
    assert_eq!(paths.get("jsr:@std/example@1/subpath"), Some(&subpath_dts));
    assert_eq!(paths.get("@jsr/std__example/subpath"), Some(&subpath_dts));
    assert_eq!(paths.get("jsr:@std/example@1/nodecl"), Some(&nodecl_ts));
    assert_eq!(paths.get("@jsr/std__example/nodecl"), Some(&nodecl_ts));
  }

  #[test]
  fn test_generate_npm_paths_empty_imports() {
    let paths =
      generate_npm_paths(Path::new("/tmp/project"), None, &BTreeMap::new());
    assert!(paths.is_empty());

    let imports = json!({});
    let paths = generate_npm_paths(
      Path::new("/tmp/project"),
      Some(&imports),
      &BTreeMap::new(),
    );
    assert!(paths.is_empty());
  }

  #[test]
  fn test_build_tsconfig_includes_relative_to_deno_dir() {
    let project_root = Path::new("/tmp/project");
    let tsconfig = build_tsconfig(
      project_root,
      None,
      None,
      None,
      &[],
      &BTreeMap::new(),
      &Map::new(),
      Path::new("/tmp/project/node_modules/@jsr"),
      &BTreeMap::new(),
      &[],
      None,
      &[],
    );

    let include = tsconfig.get("include").unwrap().as_array().unwrap();
    assert_eq!(include, &vec![json!("../**/*")]);

    let exclude = tsconfig.get("exclude").unwrap().as_array().unwrap();
    assert_eq!(exclude, &vec![json!("../**/node_modules")]);
  }

  #[test]
  fn test_build_tsconfig_propagates_excludes() {
    let project_root = Path::new("/tmp/project");
    let excludes = vec!["jsonc/testdata".to_string(), "./_site".to_string()];
    let tsconfig = build_tsconfig(
      project_root,
      None,
      None,
      None,
      &[],
      &BTreeMap::new(),
      &Map::new(),
      Path::new("/tmp/project/node_modules/@jsr"),
      &BTreeMap::new(),
      &[],
      None,
      &excludes,
    );
    let exclude = tsconfig.get("exclude").unwrap().as_array().unwrap();
    // node_modules always excluded; project excludes are rebased onto `../`.
    assert_eq!(
      exclude,
      &vec![
        json!("../**/node_modules"),
        json!("../jsonc/testdata"),
        json!("../_site"),
      ]
    );
  }

  #[test]
  fn test_build_tsconfig_with_files() {
    let project_root = Path::new("/tmp/project");
    let files = vec!["main.ts".to_string(), "lib.ts".to_string()];
    let tsconfig = build_tsconfig(
      project_root,
      None,
      None,
      None,
      &files,
      &BTreeMap::new(),
      &Map::new(),
      Path::new("/tmp/project/node_modules/@jsr"),
      &BTreeMap::new(),
      &[],
      None,
      &[],
    );

    // Should use "files" instead of "include"/"exclude"
    assert!(tsconfig.get("include").is_none());
    assert!(tsconfig.get("exclude").is_none());
    let files_arr = tsconfig.get("files").unwrap().as_array().unwrap();
    assert_eq!(files_arr, &vec![json!("main.ts"), json!("lib.ts")]);
  }

  #[test]
  fn test_build_tsconfig_with_npm_project_references() {
    let references = vec![
      "./npm/a/tsconfig.json".to_string(),
      "./npm/b/tsconfig.json".to_string(),
    ];
    let tsconfig = build_tsconfig(
      Path::new("/tmp/project"),
      None,
      None,
      None,
      &[],
      &BTreeMap::new(),
      &Map::new(),
      Path::new("/tmp/project/node_modules/@jsr"),
      &BTreeMap::new(),
      &references,
      None,
      &[],
    );

    assert_eq!(
      tsconfig.get("references"),
      Some(&json!([
        { "path": "./npm/a/tsconfig.json" },
        { "path": "./npm/b/tsconfig.json" },
      ])),
    );
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
      None,
      Some(&imports),
      &[],
      &BTreeMap::new(),
      &Map::new(),
      Path::new("/tmp/project/node_modules/@jsr"),
      &BTreeMap::new(),
      &[],
      None,
      &[],
    );

    let paths = tsconfig
      .get("compilerOptions")
      .unwrap()
      .get("paths")
      .unwrap()
      .as_object()
      .unwrap();

    // User's custom path should override the generated one, rebased onto
    // `.deno/` (the generated tsconfig lives one level below the project root).
    assert_eq!(
      paths.get("npm:chalk").unwrap(),
      &json!(["../my-custom-chalk"])
    );
    // User's custom path alias is present and rebased.
    assert_eq!(paths.get("~/*").unwrap(), &json!(["../src/*"]));
  }

  #[test]
  fn test_rebase_onto_deno_dir() {
    assert_eq!(rebase_onto_deno_dir("."), "..");
    assert_eq!(rebase_onto_deno_dir("./src/*"), "../src/*");
    assert_eq!(rebase_onto_deno_dir("src/*"), "../src/*");
    assert_eq!(rebase_onto_deno_dir("../shared"), "../../shared");
    assert_eq!(rebase_onto_deno_dir("/abs/path"), "/abs/path");
  }

  #[test]
  fn test_ensure_root_tsconfig_creates_new() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    ensure_root_tsconfig(root, &[]).unwrap();

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

    ensure_root_tsconfig(root, &[]).unwrap();

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

    ensure_root_tsconfig(root, &[]).unwrap();

    let content = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    let tsconfig: Value = serde_json::from_str(&content).unwrap();
    // Ours goes first so the user's team config overrides our defaults.
    assert_eq!(
      tsconfig.get("extends").unwrap(),
      &json!(["./.deno/tsconfig.json", "./team-shared.json"])
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

    ensure_root_tsconfig(root, &[]).unwrap();

    let content = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    let tsconfig: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
      tsconfig.get("extends").unwrap(),
      &json!(["./.deno/tsconfig.json", "./a.json", "./b.json"])
    );
  }

  #[test]
  fn test_ensure_root_tsconfig_preserves_jsonc_options() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // JSONC: line comment + trailing comma + user compilerOptions.
    std::fs::write(
      root.join("tsconfig.json"),
      "{\n  // team config\n  \"compilerOptions\": { \"strict\": false, },\n}",
    )
    .unwrap();

    ensure_root_tsconfig(root, &[]).unwrap();

    let content = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    // The user's comment and trailing comma are PRESERVED (text is spliced, not
    // re-serialized), so it's still JSONC — parse it as such.
    assert!(content.contains("// team config"));
    assert!(content.contains("\"strict\": false,"));
    let parsed = jsonc_parser::parse_to_serde_value::<Value>(
      &content,
      &jsonc_parser::ParseOptions::default(),
    )
    .unwrap();
    // extends added, user's options not dropped.
    assert_eq!(
      parsed.get("extends").unwrap(),
      &json!("./.deno/tsconfig.json")
    );
    assert_eq!(
      parsed.get("compilerOptions").and_then(|c| c.get("strict")),
      Some(&json!(false))
    );
  }

  #[test]
  fn test_ensure_root_tsconfig_rejects_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("tsconfig.json"), "{ not valid").unwrap();
    // Fails loudly rather than silently overwriting the user's file.
    assert!(ensure_root_tsconfig(root, &[]).is_err());
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

    ensure_root_tsconfig(root, &[]).unwrap();

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

    ensure_root_tsconfig(root, &[]).unwrap();

    let content = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    let tsconfig: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
      tsconfig.get("extends").unwrap(),
      &json!("./.deno/tsconfig.json")
    );
  }

  #[test]
  fn test_ensure_root_tsconfig_updates_generated_npm_references() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
      root.join("tsconfig.json"),
      r#"{
  // keep the user's project reference
  "references": [
    { "path": "./packages/app" },
    { "path": "./.deno/npm/stale/tsconfig.json" }
  ]
}"#,
    )
    .unwrap();
    let references = vec![
      "./npm/a/tsconfig.json".to_string(),
      "./npm/b/tsconfig.json".to_string(),
    ];

    ensure_root_tsconfig(root, &references).unwrap();
    let once = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    ensure_root_tsconfig(root, &references).unwrap();
    let twice = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
    assert_eq!(once, twice);

    let parsed: Value = jsonc_parser::parse_to_serde_value(
      &once,
      &jsonc_parser::ParseOptions::default(),
    )
    .unwrap();
    assert_eq!(
      parsed.get("references"),
      Some(&json!([
        { "path": "./packages/app" },
        { "path": "./.deno/npm/a/tsconfig.json" },
        { "path": "./.deno/npm/b/tsconfig.json" },
      ])),
    );
  }
}
