// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_graph::ModuleGraph;

/// Rewrites import/export specifiers in JS code for npm compatibility
pub fn rewrite_specifiers(
  content: &str,
  specifier: &ModuleSpecifier,
  graph: &ModuleGraph,
) -> Result<(String, HashMap<String, String>), AnyError> {
  let mut dependencies = HashMap::new();
  let mut result = content.to_string();

  // Collect dependencies from the graph
  if let Some(module) = graph.get(specifier) {
    if let deno_graph::Module::Js(js_module) = module {
      for (requested, dep) in &js_module.dependencies {
        // Use the requested specifier (what's in the source) not the resolved one
        let original_specifier = requested.as_str();

        if let Some(dep_specifier) = dep.maybe_code.ok() {
          // Extract package dependencies from resolved specifier
          if let Some(pkg_dep) = extract_package_dependency(&dep_specifier.specifier.as_str()) {
            dependencies.insert(pkg_dep.name.clone(), pkg_dep.version);
          }
        }

        // Rewrite the specifier in the content based on the original requested specifier
        let rewritten = rewrite_specifier(original_specifier)?;
        if rewritten != original_specifier {
          result = result.replace(&format!("\"{}\"", original_specifier), &format!("\"{}\"", rewritten));
          result = result.replace(&format!("'{}'", original_specifier), &format!("'{}'", rewritten));
        }
      }
    }
  }

  Ok((result, dependencies))
}

struct PackageDependency {
  name: String,
  version: String,
}

fn extract_package_dependency(specifier: &str) -> Option<PackageDependency> {
  if let Some(jsr_part) = specifier.strip_prefix("jsr:") {
    // jsr:@std/path@1.0.0 -> @std/path, ^1.0.0
    // jsr:@std/path@^1.0.0 -> @std/path, ^1.0.0
    let parts: Vec<&str> = jsr_part.split('@').collect();
    if parts.len() >= 2 {
      let name = if jsr_part.starts_with('@') {
        format!("@{}", parts[1])
      } else {
        parts[0].to_string()
      };
      let version = if parts.len() >= 3 {
        let ver = parts.last().unwrap();
        // Don't add ^ if it already starts with ^ or ~
        if ver.starts_with('^') || ver.starts_with('~') {
          ver.to_string()
        } else {
          format!("^{}", ver)
        }
      } else {
        "*".to_string()
      };
      return Some(PackageDependency { name, version });
    }
  } else if let Some(npm_part) = specifier.strip_prefix("npm:") {
    // npm:express@4.18.0 -> express, ^4.18.0
    let parts: Vec<&str> = npm_part.split('@').collect();
    if !parts.is_empty() {
      let name = if npm_part.starts_with('@') && parts.len() >= 2 {
        format!("@{}", parts[1])
      } else {
        parts[0].to_string()
      };
      let version = if parts.len() >= 2 {
        let ver = parts.last().unwrap();
        if ver.is_empty() {
          "*".to_string()
        } else {
          format!("^{}", ver)
        }
      } else {
        "*".to_string()
      };
      return Some(PackageDependency { name, version });
    }
  }

  None
}

fn rewrite_specifier(specifier: &str) -> Result<String, AnyError> {
  // Handle relative/absolute file paths
  if specifier.starts_with("./") || specifier.starts_with("../") || specifier.starts_with('/') {
    return Ok(rewrite_file_extension(specifier));
  }

  // Handle jsr: imports
  if let Some(jsr_part) = specifier.strip_prefix("jsr:") {
    // jsr:@std/path -> @std/path
    // jsr:@std/path@1.0.0 -> @std/path
    let parts: Vec<&str> = jsr_part.split('@').collect();
    if jsr_part.starts_with('@') && parts.len() >= 2 {
      return Ok(format!("@{}", parts[1]));
    } else if !parts.is_empty() {
      return Ok(parts[0].to_string());
    }
  }

  // Handle npm: imports
  if let Some(npm_part) = specifier.strip_prefix("npm:") {
    // npm:express -> express
    // npm:express@4.18.0 -> express
    let parts: Vec<&str> = npm_part.split('@').collect();
    if npm_part.starts_with('@') && parts.len() >= 2 {
      return Ok(format!("@{}", parts[1]));
    } else if !parts.is_empty() {
      return Ok(parts[0].to_string());
    }
  }

  // Handle node: builtin imports (keep as-is)
  if specifier.starts_with("node:") {
    return Ok(specifier.to_string());
  }

  // Handle file: URLs
  if specifier.starts_with("file:") {
    return Ok(rewrite_file_extension(specifier));
  }

  // Default: return as-is
  Ok(specifier.to_string())
}

fn rewrite_file_extension(path: &str) -> String {
  if path.ends_with(".ts") {
    path.replace(".ts", ".js")
  } else if path.ends_with(".tsx") {
    path.replace(".tsx", ".js")
  } else if path.ends_with(".mts") {
    path.replace(".mts", ".mjs")
  } else if path.ends_with(".cts") {
    path.replace(".cts", ".cjs")
  } else {
    path.to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_rewrite_file_extension() {
    assert_eq!(rewrite_file_extension("./mod.ts"), "./mod.js");
    assert_eq!(rewrite_file_extension("../utils.tsx"), "../utils.js");
    assert_eq!(rewrite_file_extension("./mod.mts"), "./mod.mjs");
    assert_eq!(rewrite_file_extension("./mod.js"), "./mod.js");
  }

  #[test]
  fn test_rewrite_specifier() {
    assert_eq!(
      rewrite_specifier("jsr:@std/path").unwrap(),
      "@std/path"
    );
    assert_eq!(
      rewrite_specifier("jsr:@std/path@1.0.0").unwrap(),
      "@std/path"
    );
    assert_eq!(
      rewrite_specifier("npm:express").unwrap(),
      "express"
    );
    assert_eq!(
      rewrite_specifier("npm:express@4.18.0").unwrap(),
      "express"
    );
    assert_eq!(
      rewrite_specifier("node:fs").unwrap(),
      "node:fs"
    );
    assert_eq!(
      rewrite_specifier("./utils.ts").unwrap(),
      "./utils.js"
    );
  }
}
