// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_graph::ModuleGraph;
use regex::Regex;

/// Rewrites import/export specifiers in JS code for npm compatibility
pub fn rewrite_specifiers(
  content: &str,
  specifier: &ModuleSpecifier,
  graph: &ModuleGraph,
) -> Result<(String, HashMap<String, String>), AnyError> {
  let mut dependencies = HashMap::new();

  // Collect list of specifiers to rewrite from the graph
  let mut specifier_rewrites: HashMap<String, String> = HashMap::new();

  if let Some(module) = graph.get(specifier) {
    if let deno_graph::Module::Js(js_module) = module {
      for (requested, dep) in &js_module.dependencies {
        let original_specifier = requested.as_str();

        if let Some(dep_specifier) = dep.maybe_code.ok() {
          // Extract package dependencies from resolved specifier
          if let Some(pkg_dep) = extract_package_dependency(&dep_specifier.specifier.as_str()) {
            dependencies.insert(pkg_dep.name.clone(), pkg_dep.version);
          }
        }

        // Determine if this specifier needs rewriting
        let rewritten = rewrite_specifier(original_specifier)?;
        if rewritten != original_specifier {
          specifier_rewrites.insert(original_specifier.to_string(), rewritten);
        }
      }
    }
  }

  // Use regex to safely replace specifiers only in import/export statements
  // This avoids corrupting comments, strings, etc.
  let mut result = content.to_string();

  for (original, rewritten) in specifier_rewrites {
    // Match import/export patterns with this specific specifier
    // Handles: from "spec", from 'spec', import("spec"), import('spec')
    let patterns = [
      (format!(r#"(from\s+)"{}""#, regex::escape(&original)), format!(r#"$1"{}""#, rewritten)),
      (format!(r#"(from\s+)'{}'"#, regex::escape(&original)), format!(r#"$1'{}'"#, rewritten)),
      (format!(r#"(import\s*\()\s*"{}"\s*\)"#, regex::escape(&original)), format!(r#"$1"{}")"#, rewritten)),
      (format!(r#"(import\s*\()\s*'{}'\s*\)"#, regex::escape(&original)), format!(r#"$1'{}')"#, rewritten)),
      // Also handle export ... from "spec"
      (format!(r#"(export\s+.*from\s+)"{}""#, regex::escape(&original)), format!(r#"$1"{}""#, rewritten)),
      (format!(r#"(export\s+.*from\s+)'{}'"#, regex::escape(&original)), format!(r#"$1'{}'"#, rewritten)),
    ];

    for (pattern, replacement) in patterns {
      let re = Regex::new(&pattern)?;
      result = re.replace_all(&result, replacement).to_string();
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
    // jsr:@std/path@1.0.0/posix -> @std/path, ^1.0.0 (strip subpath)
    // Parse scoped vs unscoped packages differently
    if jsr_part.starts_with('@') {
      // Scoped package: @scope/name@version/subpath
      let parts: Vec<&str> = jsr_part.splitn(3, '@').collect();
      if parts.len() >= 3 {
        let name = format!("@{}", parts[1]);
        // Strip subpath from version
        let version_part = parts[2].split('/').next().unwrap();
        let version = if version_part.starts_with('^') || version_part.starts_with('~') {
          version_part.to_string()
        } else {
          format!("^{}", version_part)
        };
        return Some(PackageDependency { name, version });
      } else if parts.len() == 2 {
        // No version specified
        let name = format!("@{}", parts[1].split('/').next().unwrap());
        return Some(PackageDependency { name, version: "*".to_string() });
      }
    } else {
      // Unscoped package: name@version/subpath
      let parts: Vec<&str> = jsr_part.splitn(2, '@').collect();
      let name = parts[0].split('/').next().unwrap().to_string();
      let version = if parts.len() == 2 {
        let version_part = parts[1].split('/').next().unwrap();
        if version_part.starts_with('^') || version_part.starts_with('~') {
          version_part.to_string()
        } else {
          format!("^{}", version_part)
        }
      } else {
        "*".to_string()
      };
      return Some(PackageDependency { name, version });
    }
  } else if let Some(npm_part) = specifier.strip_prefix("npm:") {
    // npm:express@4.18.0 -> express, ^4.18.0
    // npm:express@4.18.0/Router -> express, ^4.18.0 (strip subpath)
    // npm:@scope/pkg -> @scope/pkg, * (no version)
    if npm_part.starts_with('@') {
      // Scoped package: @scope/name@version/subpath
      let parts: Vec<&str> = npm_part.splitn(3, '@').collect();
      if parts.len() >= 3 {
        let name = format!("@{}", parts[1]);
        let version_part = parts[2].split('/').next().unwrap();
        let version = if version_part.is_empty() {
          "*".to_string()
        } else if version_part.starts_with('^') || version_part.starts_with('~') {
          version_part.to_string()
        } else {
          format!("^{}", version_part)
        };
        return Some(PackageDependency { name, version });
      } else if parts.len() == 2 {
        // No version: npm:@scope/pkg
        let name = format!("@{}", parts[1].split('/').next().unwrap());
        return Some(PackageDependency { name, version: "*".to_string() });
      }
    } else {
      // Unscoped package: name@version/subpath
      let parts: Vec<&str> = npm_part.splitn(2, '@').collect();
      let name = parts[0].split('/').next().unwrap().to_string();
      let version = if parts.len() == 2 {
        let version_part = parts[1].split('/').next().unwrap();
        if version_part.is_empty() {
          "*".to_string()
        } else if version_part.starts_with('^') || version_part.starts_with('~') {
          version_part.to_string()
        } else {
          format!("^{}", version_part)
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
    // jsr:@std/path@1.0.0/posix -> @std/path/posix (preserve subpath)
    if jsr_part.starts_with('@') {
      // Scoped package: @scope/name@version/subpath
      let parts: Vec<&str> = jsr_part.splitn(3, '@').collect();
      if parts.len() >= 3 {
        // Has version, extract name and subpath
        let name = format!("@{}", parts[1]);
        let rest = parts[2]; // version/subpath
        if let Some(slash_pos) = rest.find('/') {
          let subpath = &rest[slash_pos..]; // includes the /
          return Ok(format!("{}{}", name, subpath));
        } else {
          return Ok(name);
        }
      } else if parts.len() == 2 {
        // No version: jsr:@scope/name or jsr:@scope/name/subpath
        let name_and_subpath = parts[1];
        return Ok(format!("@{}", name_and_subpath));
      }
    } else {
      // Unscoped package: name@version/subpath
      let parts: Vec<&str> = jsr_part.splitn(2, '@').collect();
      let name = parts[0].split('/').next().unwrap().to_string();
      if parts.len() == 2 {
        // Has version
        let rest = parts[1]; // version/subpath
        if let Some(slash_pos) = rest.find('/') {
          let subpath = &rest[slash_pos..];
          return Ok(format!("{}{}", name, subpath));
        }
      }
      return Ok(name);
    }
  }

  // Handle npm: imports
  if let Some(npm_part) = specifier.strip_prefix("npm:") {
    // npm:express -> express
    // npm:express@4.18.0 -> express
    // npm:express@4/Router -> express/Router (preserve subpath)
    if npm_part.starts_with('@') {
      // Scoped package: @scope/name@version/subpath
      let parts: Vec<&str> = npm_part.splitn(3, '@').collect();
      if parts.len() >= 3 {
        let name = format!("@{}", parts[1]);
        let rest = parts[2]; // version/subpath
        if let Some(slash_pos) = rest.find('/') {
          let subpath = &rest[slash_pos..];
          return Ok(format!("{}{}", name, subpath));
        } else {
          return Ok(name);
        }
      } else if parts.len() == 2 {
        // No version: npm:@scope/name or npm:@scope/name/subpath
        let name_and_subpath = parts[1];
        return Ok(format!("@{}", name_and_subpath));
      }
    } else {
      // Unscoped package: name@version/subpath
      let parts: Vec<&str> = npm_part.splitn(2, '@').collect();
      let name = parts[0].split('/').next().unwrap().to_string();
      if parts.len() == 2 {
        let rest = parts[1]; // version/subpath
        if let Some(slash_pos) = rest.find('/') {
          let subpath = &rest[slash_pos..];
          return Ok(format!("{}{}", name, subpath));
        }
      }
      return Ok(name);
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
  replace_extension(path, ".tsx", ".js")
    .or_else(|| replace_extension(path, ".ts", ".js"))
    .or_else(|| replace_extension(path, ".mts", ".mjs"))
    .or_else(|| replace_extension(path, ".cts", ".cjs"))
    .unwrap_or_else(|| path.to_string())
}

fn replace_extension(path: &str, from: &str, to: &str) -> Option<String> {
  if path.ends_with(from) {
    Some(format!("{}{}", &path[..path.len() - from.len()], to))
  } else {
    None
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
      rewrite_specifier("jsr:@std/path@1.0.0/posix").unwrap(),
      "@std/path/posix"
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
      rewrite_specifier("npm:express@4/Router").unwrap(),
      "express/Router"
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
