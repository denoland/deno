// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeMap;

use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_ast::TextChange;
use deno_graph::ModuleGraph;
use deno_graph::analysis::DependencyDescriptor;
use deno_graph::analysis::DynamicArgument;
use deno_graph::analysis::DynamicTemplatePart;
use deno_graph::analysis::TypeScriptReference;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;

use crate::tools::unfurl_utils::to_range;

/// Result of unfurling a module's specifiers for npm compatibility.
pub struct UnfurlResult {
  /// Text changes to apply to the source.
  pub text_changes: Vec<TextChange>,
  /// Extracted package dependencies (name -> version range).
  pub dependencies: BTreeMap<String, String>,
}

/// Unfurl specifiers in a module for npm compatibility.
///
/// Uses deno_graph's dependency descriptors (from AST analysis) to find all
/// import/export specifiers and rewrite them:
/// - `jsr:@scope/pkg@version/sub` → `@scope/pkg/sub`
/// - `npm:express@4.18.0/Router` → `express/Router`
/// - `./utils.ts` → `./utils.js`
/// - `node:fs` → `node:fs` (pass through)
///
/// Also extracts dependency info (package name → version range) for
/// package.json generation.
pub fn unfurl_specifiers(
  parsed_source: &ParsedSource,
  specifier: &ModuleSpecifier,
  graph: &ModuleGraph,
) -> UnfurlResult {
  let text_info = parsed_source.text_info_lazy();
  let module_info =
    deno_graph::ast::ParserModuleAnalyzer::module_info(parsed_source);

  let mut text_changes = Vec::new();
  let mut dependencies = BTreeMap::new();

  // Process all static and dynamic dependencies from the module info
  for dep in &module_info.dependencies {
    match dep {
      DependencyDescriptor::Static(dep) => {
        unfurl_static_specifier(
          &dep.specifier,
          &dep.specifier_range,
          text_info,
          specifier,
          graph,
          &mut text_changes,
          &mut dependencies,
        );
      }
      DependencyDescriptor::Dynamic(dep) => {
        unfurl_dynamic_specifier(
          dep,
          text_info,
          specifier,
          graph,
          &mut text_changes,
          &mut dependencies,
        );
      }
    }
  }

  // Process /// <reference path="..." /> and /// <reference types="..." />
  for ts_ref in &module_info.ts_references {
    let specifier_with_range = match ts_ref {
      TypeScriptReference::Path(s) => s,
      TypeScriptReference::Types { specifier, .. } => specifier,
    };
    unfurl_specifier_with_range(
      &specifier_with_range.text,
      &specifier_with_range.range,
      text_info,
      &mut text_changes,
      &mut dependencies,
    );
  }

  // Process /** @import {Type} from "./foo.ts" */
  for jsdoc in &module_info.jsdoc_imports {
    unfurl_specifier_with_range(
      &jsdoc.specifier.text,
      &jsdoc.specifier.range,
      text_info,
      &mut text_changes,
      &mut dependencies,
    );
  }

  // Process @jsxImportSource
  if let Some(specifier_with_range) = &module_info.jsx_import_source {
    unfurl_specifier_with_range(
      &specifier_with_range.text,
      &specifier_with_range.range,
      text_info,
      &mut text_changes,
      &mut dependencies,
    );
  }

  // Process @jsxImportSourceTypes
  if let Some(specifier_with_range) = &module_info.jsx_import_source_types {
    unfurl_specifier_with_range(
      &specifier_with_range.text,
      &specifier_with_range.range,
      text_info,
      &mut text_changes,
      &mut dependencies,
    );
  }

  // Process import.meta.resolve() calls
  {
    use deno_ast::swc::ecma_visit::VisitWith;

    use crate::tools::unfurl_utils::ImportMetaResolveCollector;

    let mut collector = ImportMetaResolveCollector::default();
    parsed_source.program_ref().visit_with(&mut collector);

    for (range, spec) in collector.specifiers {
      if let Some(rewritten) = rewrite_specifier(&spec) {
        // Extract dependencies from the specifier
        if let Some((name, version)) = extract_package_dependency(&spec) {
          dependencies.insert(name, version);
        }

        let byte_range = range.as_byte_range(text_info.range().start);
        text_changes.push(TextChange {
          range: byte_range,
          new_text: rewritten,
        });
      }
    }
  }

  UnfurlResult {
    text_changes,
    dependencies,
  }
}

/// Unfurl a specifier found via a SpecifierWithRange (ts_references, jsdoc, jsx).
fn unfurl_specifier_with_range(
  spec: &str,
  range: &deno_graph::PositionRange,
  text_info: &SourceTextInfo,
  text_changes: &mut Vec<TextChange>,
  dependencies: &mut BTreeMap<String, String>,
) {
  if let Some((name, version)) = extract_package_dependency(spec) {
    dependencies.insert(name, version);
  }

  if let Some(rewritten) = rewrite_specifier(spec) {
    let byte_range = to_range(text_info, range);
    text_changes.push(TextChange {
      range: byte_range,
      new_text: rewritten,
    });
  }
}

fn unfurl_static_specifier(
  spec: &str,
  range: &deno_graph::PositionRange,
  text_info: &SourceTextInfo,
  referrer: &ModuleSpecifier,
  graph: &ModuleGraph,
  text_changes: &mut Vec<TextChange>,
  dependencies: &mut BTreeMap<String, String>,
) {
  // Look up the resolved specifier from the graph to extract dependency info
  if let Some(deno_graph::Module::Js(js_module)) = graph.get(referrer)
    && let Some(dep) = js_module.dependencies.get(spec)
    && let Some(resolved) = dep.maybe_code.ok()
    && let Some((name, version)) =
      extract_package_dependency(resolved.specifier.as_str())
  {
    dependencies.insert(name, version);
  }

  if let Some(rewritten) = rewrite_specifier(spec) {
    let byte_range = to_range(text_info, range);
    text_changes.push(TextChange {
      range: byte_range,
      new_text: rewritten,
    });
  }
}

fn unfurl_dynamic_specifier(
  dep: &deno_graph::analysis::DynamicDependencyDescriptor,
  text_info: &SourceTextInfo,
  referrer: &ModuleSpecifier,
  graph: &ModuleGraph,
  text_changes: &mut Vec<TextChange>,
  dependencies: &mut BTreeMap<String, String>,
) {
  match &dep.argument {
    DynamicArgument::String(specifier) => {
      // Look up resolved specifier from graph for dependency extraction
      if let Some(deno_graph::Module::Js(js_module)) = graph.get(referrer)
        && let Some(graph_dep) = js_module.dependencies.get(specifier.as_str())
        && let Some(resolved) = graph_dep.maybe_code.ok()
        && let Some((name, version)) =
          extract_package_dependency(resolved.specifier.as_str())
      {
        dependencies.insert(name, version);
      }

      if let Some(rewritten) = rewrite_specifier(specifier) {
        let range = to_range(text_info, &dep.argument_range);
        // Find the specifier within the range (it may be surrounded by quotes)
        let text_in_range = &text_info.text_str()[range.clone()];
        if let Some(relative_index) = text_in_range.find(specifier.as_str()) {
          let start = range.start + relative_index;
          text_changes.push(TextChange {
            range: start..start + specifier.len(),
            new_text: rewritten,
          });
        }
      }
    }
    DynamicArgument::Template(parts) => {
      if let Some(DynamicTemplatePart::String { value: specifier }) =
        parts.first()
      {
        // Only handle template literals that start with a known prefix
        // ending in `/`, e.g. import(`npm:express/${path}`) is not common
        // for pack, but handle the simple string prefix case
        if !specifier.ends_with('/') && !specifier.is_empty() {
          // For complete string-like templates (no expressions), treat as string
          if parts.len() == 1
            && let Some(rewritten) = rewrite_specifier(specifier)
          {
            if let Some((name, version)) = extract_package_dependency(specifier)
            {
              dependencies.insert(name, version);
            }
            let range = to_range(text_info, &dep.argument_range);
            let text_in_range = &text_info.text_str()[range.clone()];
            if let Some(relative_index) = text_in_range.find(specifier.as_str())
            {
              let start = range.start + relative_index;
              text_changes.push(TextChange {
                range: start..start + specifier.len(),
                new_text: rewritten,
              });
            }
          }
          return;
        }

        if specifier.ends_with('/') {
          if let Some((name, version)) = extract_package_dependency(specifier) {
            dependencies.insert(name, version);
          }
          if let Some(rewritten) = rewrite_specifier(specifier) {
            let range = to_range(text_info, &dep.argument_range);
            let text_in_range = &text_info.text_str()[range.start..];
            if let Some(relative_index) = text_in_range.find(specifier.as_str())
            {
              let start = range.start + relative_index;
              text_changes.push(TextChange {
                range: start..start + specifier.len(),
                new_text: rewritten,
              });
            }
          }
        }
      }
    }
    DynamicArgument::Expr => {
      // Can't analyze arbitrary expressions, skip
    }
  }
}

/// Rewrite a jsr:/npm: specifier to a bare npm-compatible specifier.
/// Uses the parsed package name and optional sub-path.
fn rewrite_registry_specifier(name: &str, sub_path: Option<&str>) -> String {
  match sub_path {
    Some(sub) if !sub.is_empty() => format!("{}/{}", name, sub),
    _ => name.to_string(),
  }
}

/// Convert a JSR package name (e.g. `@std/path`) to its npm-published
/// counterpart (`@jsr/std__path`). JSR mirrors packages onto npm under the
/// `@jsr/` scope with `__` joining the original scope and name. If the
/// input doesn't look like a scoped JSR name we return it unchanged so the
/// caller can degrade gracefully on malformed specifiers.
fn jsr_name_to_npm(jsr_name: &str) -> String {
  let Some(stripped) = jsr_name.strip_prefix('@') else {
    return jsr_name.to_string();
  };
  match stripped.split_once('/') {
    Some((scope, name)) => format!("@jsr/{}__{}", scope, name),
    None => jsr_name.to_string(),
  }
}

/// Rewrite a specifier for npm compatibility.
/// Returns `Some(rewritten)` if the specifier needs rewriting, `None` if unchanged.
fn rewrite_specifier(specifier: &str) -> Option<String> {
  // Handle relative/absolute file paths — rewrite extensions
  if specifier.starts_with("./")
    || specifier.starts_with("../")
    || specifier.starts_with('/')
  {
    let rewritten = rewrite_file_extension(specifier);
    if rewritten != specifier {
      return Some(rewritten);
    }
    return None;
  }

  // Handle jsr: imports using deno_semver. JSR packages are mirrored to
  // the npm registry under the `@jsr/scope__name` scheme, so rewrite to
  // that form rather than the raw `@scope/name` (which won't resolve via
  // `npm install`).
  if specifier.starts_with("jsr:") {
    if let Ok(jsr_ref) = JsrPackageReqReference::from_str(specifier) {
      let npm_name = jsr_name_to_npm(&jsr_ref.req().name);
      return Some(rewrite_registry_specifier(&npm_name, jsr_ref.sub_path()));
    }
    // Fallback for malformed specifiers: strip prefix
    log::warn!("Failed to parse jsr specifier: {}", specifier);
    return Some(
      specifier
        .strip_prefix("jsr:")
        .unwrap_or(specifier)
        .to_string(),
    );
  }

  // Handle npm: imports using deno_semver
  if specifier.starts_with("npm:") {
    if let Ok(npm_ref) = NpmPackageReqReference::from_str(specifier) {
      return Some(rewrite_registry_specifier(
        &npm_ref.req().name,
        npm_ref.sub_path(),
      ));
    }
    // Fallback for malformed specifiers: strip prefix
    log::warn!("Failed to parse npm specifier: {}", specifier);
    return Some(
      specifier
        .strip_prefix("npm:")
        .unwrap_or(specifier)
        .to_string(),
    );
  }

  // Handle node: builtin imports (keep as-is)
  if specifier.starts_with("node:") {
    return None;
  }

  // file: URLs should not appear in npm package output.
  // We cannot reliably convert them to relative paths without the referrer.
  if specifier.starts_with("file:") {
    log::warn!(
      "file: URL found in specifier '{}' - this will not work in an npm package",
      specifier
    );
    return None;
  }

  // Default: unchanged
  None
}

/// Rewrite TypeScript file extensions to JavaScript equivalents.
/// Delegates to `extensions::ts_to_js_extension` for the actual swap,
/// preserving any directory prefix (e.g. `./`, `../`).
fn rewrite_file_extension(path: &str) -> String {
  // Declaration files should not have extensions rewritten
  if path.ends_with(".d.ts")
    || path.ends_with(".d.mts")
    || path.ends_with(".d.cts")
  {
    return path.to_string();
  }
  // Preserve the directory prefix, delegate extension swap
  if let Some(last_slash) = path.rfind('/') {
    let prefix = &path[..=last_slash];
    let filename = &path[last_slash + 1..];
    let converted = super::extensions::ts_to_js_extension(filename);
    format!("{}{}", prefix, converted)
  } else {
    super::extensions::ts_to_js_extension(path)
  }
}

/// Extract a package dependency (name, version range) from a specifier.
/// Uses `deno_semver` for safe parsing of jsr: and npm: specifiers.
fn extract_package_dependency(specifier: &str) -> Option<(String, String)> {
  // Trim trailing slash for template literal prefixes like "npm:express/"
  let specifier = specifier.trim_end_matches('/');

  if specifier.starts_with("jsr:") {
    let jsr_ref = JsrPackageReqReference::from_str(specifier).ok()?;
    // Use the @jsr/-scoped npm name so the dependency resolves on
    // `npm install`. The JSR scope/name pair is rewritten by
    // `jsr_name_to_npm`; see https://jsr.io/docs/npm-compatibility.
    let name = jsr_name_to_npm(&jsr_ref.req().name);
    let version = normalize_version(jsr_ref.req().version_req.version_text());
    return Some((name, version));
  }

  if specifier.starts_with("npm:") {
    let npm_ref = NpmPackageReqReference::from_str(specifier).ok()?;
    let name = npm_ref.req().name.to_string();
    let version = normalize_version(npm_ref.req().version_req.version_text());
    return Some((name, version));
  }

  None
}

/// Normalize a version string, adding `^` prefix if not already range-qualified.
fn normalize_version(version: &str) -> String {
  if version.starts_with('^')
    || version.starts_with('~')
    || version == "*"
    || version.is_empty()
  {
    if version.is_empty() {
      return "*".to_string();
    }
    version.to_string()
  } else {
    format!("^{}", version)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_rewrite_file_extension() {
    assert_eq!(rewrite_specifier("./mod.ts"), Some("./mod.js".to_string()));
    assert_eq!(
      rewrite_specifier("../utils.tsx"),
      Some("../utils.js".to_string())
    );
    assert_eq!(
      rewrite_specifier("./mod.mts"),
      Some("./mod.mjs".to_string())
    );
    assert_eq!(rewrite_specifier("./mod.js"), None);
    assert_eq!(rewrite_specifier("./types.d.ts"), None);
    assert_eq!(rewrite_specifier("./types.d.mts"), None);
    assert_eq!(rewrite_specifier("./types.d.cts"), None);
  }

  #[test]
  fn test_rewrite_jsr_specifier() {
    // JSR packages are mirrored to npm under `@jsr/scope__name`.
    assert_eq!(
      rewrite_specifier("jsr:@std/path"),
      Some("@jsr/std__path".to_string())
    );
    assert_eq!(
      rewrite_specifier("jsr:@std/path@1.0.0"),
      Some("@jsr/std__path".to_string())
    );
    assert_eq!(
      rewrite_specifier("jsr:@std/path@1.0.0/posix"),
      Some("@jsr/std__path/posix".to_string())
    );
  }

  #[test]
  fn test_rewrite_npm_specifier() {
    assert_eq!(
      rewrite_specifier("npm:express"),
      Some("express".to_string())
    );
    assert_eq!(
      rewrite_specifier("npm:express@4.18.0"),
      Some("express".to_string())
    );
    assert_eq!(
      rewrite_specifier("npm:express@4/Router"),
      Some("express/Router".to_string())
    );
    assert_eq!(
      rewrite_specifier("npm:@scope/pkg@1.0.0"),
      Some("@scope/pkg".to_string())
    );
    assert_eq!(
      rewrite_specifier("npm:@scope/pkg@1.0.0/sub"),
      Some("@scope/pkg/sub".to_string())
    );
  }

  #[test]
  fn test_rewrite_node_specifier() {
    assert_eq!(rewrite_specifier("node:fs"), None);
    assert_eq!(rewrite_specifier("node:path"), None);
  }

  #[test]
  fn test_extract_jsr_dependency() {
    assert_eq!(
      extract_package_dependency("jsr:@std/path@1.0.0"),
      Some(("@jsr/std__path".to_string(), "^1.0.0".to_string()))
    );
    assert_eq!(
      extract_package_dependency("jsr:@std/path@1.0.0/posix"),
      Some(("@jsr/std__path".to_string(), "^1.0.0".to_string()))
    );
    assert_eq!(
      extract_package_dependency("jsr:@std/path"),
      Some(("@jsr/std__path".to_string(), "*".to_string()))
    );
  }

  #[test]
  fn test_jsr_name_to_npm() {
    assert_eq!(jsr_name_to_npm("@std/path"), "@jsr/std__path");
    assert_eq!(jsr_name_to_npm("@scope/some-pkg"), "@jsr/scope__some-pkg");
    // malformed inputs pass through
    assert_eq!(jsr_name_to_npm("nopackage"), "nopackage");
    assert_eq!(jsr_name_to_npm("@noslash"), "@noslash");
  }

  #[test]
  fn test_extract_npm_dependency() {
    assert_eq!(
      extract_package_dependency("npm:express@4.18.0"),
      Some(("express".to_string(), "^4.18.0".to_string()))
    );
    assert_eq!(
      extract_package_dependency("npm:express"),
      Some(("express".to_string(), "*".to_string()))
    );
    assert_eq!(
      extract_package_dependency("npm:@scope/pkg@^1.0.0"),
      Some(("@scope/pkg".to_string(), "^1.0.0".to_string()))
    );
  }

  #[test]
  fn test_extract_dependency_with_trailing_slash() {
    assert_eq!(
      extract_package_dependency("npm:express/"),
      Some(("express".to_string(), "*".to_string()))
    );
    assert_eq!(
      extract_package_dependency("npm:@scope/pkg@1.0.0/"),
      Some(("@scope/pkg".to_string(), "^1.0.0".to_string()))
    );
  }

  #[test]
  fn test_extract_dependency_prerelease() {
    assert_eq!(
      extract_package_dependency("npm:express@4.18.0-beta.1"),
      Some(("express".to_string(), "^4.18.0-beta.1".to_string()))
    );
  }

  #[test]
  fn test_normalize_version() {
    assert_eq!(normalize_version("1.0.0"), "^1.0.0");
    assert_eq!(normalize_version("^1.0.0"), "^1.0.0");
    assert_eq!(normalize_version("~1.0.0"), "~1.0.0");
    assert_eq!(normalize_version("*"), "*");
    assert_eq!(normalize_version(""), "*");
  }
}
