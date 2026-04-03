// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use deno_terminal::colors;

/// Generate `deno.tsconfig.json` and update `tsconfig.json` to extend it.
///
/// This enables stock TypeScript tooling (tsc, tsserver, VS Code) to work
/// with Deno projects by:
/// - Injecting Deno type definitions
/// - Mapping npm: and jsr: specifiers via tsconfig "paths"
/// - Setting compiler options that match Deno's defaults
pub fn generate(project_root: &Path) -> Result<(), AnyError> {
  // Read deno.json if it exists
  let deno_json_path = project_root.join("deno.json");
  let deno_jsonc_path = project_root.join("deno.jsonc");
  let deno_json: Option<Value> = if deno_json_path.exists() {
    let content = std::fs::read_to_string(&deno_json_path)?;
    Some(serde_json::from_str(&content)?)
  } else if deno_jsonc_path.exists() {
    let content = std::fs::read_to_string(&deno_jsonc_path)?;
    {
      let parsed: Option<Value> = jsonc_parser::parse_to_serde_value(
        &content,
        &jsonc_parser::ParseOptions::default(),
      )?;
      Some(parsed.unwrap_or(json!({})))
    }
  } else {
    None
  };

  let deno_compiler_options =
    deno_json.as_ref().and_then(|j| j.get("compilerOptions"));
  let deno_imports = deno_json.as_ref().and_then(|j| j.get("imports"));

  // Install npm: and jsr: packages to node_modules/ so stock tsc can find them
  install_npm_packages(project_root, deno_imports)?;
  install_jsr_packages(project_root, deno_imports)?;

  // Generate deno.tsconfig.json using tsconfig_gen
  let generated = crate::tsc::tsconfig_gen::generate_tsconfig(
    project_root,
    deno_compiler_options,
    deno_imports,
    &[], // no specific files — include all
  )
  .map_err(|e| anyhow!("Failed to generate tsconfig: {e}"))?;

  // Now rename .deno/tsconfig.json to deno.tsconfig.json at project root
  let deno_tsconfig_path = project_root.join("deno.tsconfig.json");
  let generated_content = std::fs::read_to_string(&generated.tsconfig_path)?;

  // Rewrite paths to be relative to project root instead of .deno/
  let mut tsconfig: Value = serde_json::from_str(&generated_content)?;
  rewrite_paths_for_project_root(&mut tsconfig, project_root);
  let content =
    serde_json::to_string_pretty(&tsconfig).expect("failed to serialize");
  std::fs::write(&deno_tsconfig_path, &content)?;

  log::info!(
    "{} {}",
    colors::green("Generated"),
    deno_tsconfig_path.display()
  );

  // Create or update tsconfig.json to extend deno.tsconfig.json
  update_user_tsconfig(project_root)?;

  // Clean up .deno/ temp directory
  let deno_dir = project_root.join(".deno");
  if deno_dir.exists() {
    let _ = std::fs::remove_dir_all(&deno_dir);
  }

  Ok(())
}

/// Rewrite paths in the tsconfig from .deno/-relative to project-root-relative.
fn rewrite_paths_for_project_root(tsconfig: &mut Value, _project_root: &Path) {
  // Fix "files" array — types path
  if let Some(files) = tsconfig.get_mut("files").and_then(|f| f.as_array_mut())
  {
    for file in files.iter_mut() {
      if let Some(s) = file.as_str() {
        // .deno/types/deno.d.ts → .deno/types/deno.d.ts (keep as-is, it's
        // relative to tsconfig location which will be project root)
        *file = json!(format!(".deno/{s}"));
      }
    }
  }

  // Fix "include" array
  if let Some(include) =
    tsconfig.get_mut("include").and_then(|i| i.as_array_mut())
  {
    for item in include.iter_mut() {
      if let Some(s) = item.as_str() {
        // "../**/*" → "./**/*"  (was relative to .deno/, now relative to root)
        let fixed = s.strip_prefix("../").unwrap_or(s);
        *item = json!(format!("./{fixed}"));
      }
    }
  }

  // Fix "exclude" array
  if let Some(exclude) =
    tsconfig.get_mut("exclude").and_then(|e| e.as_array_mut())
  {
    for item in exclude.iter_mut() {
      if let Some(s) = item.as_str() {
        let fixed = s.strip_prefix("../").unwrap_or(s);
        *item = json!(format!("./{fixed}"));
      }
    }
  }

  // Fix "paths" in compilerOptions
  if let Some(paths) = tsconfig
    .get_mut("compilerOptions")
    .and_then(|co| co.get_mut("paths"))
    .and_then(|p| p.as_object_mut())
  {
    for (_key, targets) in paths.iter_mut() {
      if let Some(arr) = targets.as_array_mut() {
        for target in arr.iter_mut() {
          if let Some(s) = target.as_str() {
            // "../node_modules/..." → "./node_modules/..."
            let fixed = s.strip_prefix("../").unwrap_or(s);
            *target = json!(format!("./{fixed}"));
          }
        }
      }
    }
  }

  // Remove "extends" if present (it was for .deno/ context)
  tsconfig.as_object_mut().map(|m| m.remove("extends"));
}

/// Install npm: packages from deno.json imports to node_modules/.
fn install_npm_packages(
  project_root: &Path,
  deno_imports: Option<&Value>,
) -> Result<(), AnyError> {
  let imports = match deno_imports.and_then(|v| v.as_object()) {
    Some(imports) => imports,
    None => return Ok(()),
  };

  let mut to_install: Vec<String> = Vec::new();
  for (_alias, target) in imports {
    let target_str = match target.as_str() {
      Some(s) if s.starts_with("npm:") => s,
      _ => continue,
    };

    let Some(pkg_name) =
      crate::tsc::tsconfig_gen::parse_npm_specifier(target_str)
    else {
      continue;
    };

    // Check if already installed
    let pkg_dir = project_root.join("node_modules").join(&pkg_name);
    if pkg_dir.exists() {
      continue;
    }

    // Use the full specifier (with version) for install
    let install_name = target_str.strip_prefix("npm:").unwrap_or(target_str);
    to_install.push(install_name.to_string());
  }

  if to_install.is_empty() {
    return Ok(());
  }

  log::info!(
    "{} npm packages: {}",
    colors::green("Installing"),
    to_install.join(", ")
  );

  let status = std::process::Command::new("npm")
    .arg("install")
    .arg("--no-save")
    .args(&to_install)
    .current_dir(project_root)
    .status()
    .map_err(|e| anyhow!("Failed to run npm install: {e}"))?;

  if !status.success() {
    return Err(anyhow!(
      "npm install failed for packages: {}",
      to_install.join(", ")
    ));
  }

  Ok(())
}

/// Install jsr: packages to node_modules/@jsr/ using the npm compatibility
/// layer at npm.jsr.io. This makes jsr packages available for stock tsc
/// resolution via tsconfig "paths".
fn install_jsr_packages(
  project_root: &Path,
  deno_imports: Option<&Value>,
) -> Result<(), AnyError> {
  let imports = match deno_imports.and_then(|v| v.as_object()) {
    Some(imports) => imports,
    None => return Ok(()),
  };

  // Collect jsr: specifiers that need installing
  let mut to_install: Vec<String> = Vec::new();
  for (_alias, target) in imports {
    let target_str = match target.as_str() {
      Some(s) if s.starts_with("jsr:") => s,
      _ => continue,
    };

    let Some((scope, name)) =
      crate::tsc::tsconfig_gen::parse_jsr_specifier(target_str)
    else {
      continue;
    };

    // Convert jsr scope/name to npm compat format: @jsr/scope__name
    let npm_name = format!("@jsr/{}__{}", scope.trim_start_matches('@'), name);

    // Check if already installed
    let pkg_dir = project_root.join("node_modules").join(&npm_name);
    if pkg_dir.exists() {
      continue;
    }

    to_install.push(npm_name);
  }

  if to_install.is_empty() {
    return Ok(());
  }

  log::info!(
    "{} jsr packages via npm.jsr.io: {}",
    colors::green("Installing"),
    to_install.join(", ")
  );

  let status = std::process::Command::new("npm")
    .arg("install")
    .arg("--no-save")
    .arg("--registry=https://npm.jsr.io")
    .args(&to_install)
    .current_dir(project_root)
    .status()
    .map_err(|e| anyhow!("Failed to run npm install: {e}"))?;

  if !status.success() {
    return Err(anyhow!(
      "npm install failed for jsr packages: {}",
      to_install.join(", ")
    ));
  }

  Ok(())
}

/// Create or update tsconfig.json to extend deno.tsconfig.json.
fn update_user_tsconfig(project_root: &Path) -> Result<(), AnyError> {
  let tsconfig_path = project_root.join("tsconfig.json");

  if tsconfig_path.exists() {
    // Read existing tsconfig.json and add/update "extends"
    let content = std::fs::read_to_string(&tsconfig_path)?;
    let mut tsconfig: Value = serde_json::from_str(&content).or_else(|_| {
      jsonc_parser::parse_to_serde_value(
        &content,
        &jsonc_parser::ParseOptions::default(),
      )
      .map(|v: Option<Value>| v.unwrap_or(json!({})))
      .map_err(|e| anyhow!("Failed to parse tsconfig.json: {e}"))
    })?;

    if let Some(obj) = tsconfig.as_object_mut() {
      let current_extends = obj.get("extends");
      if current_extends.is_some_and(|v| {
        v == "deno.tsconfig.json" || v == "./deno.tsconfig.json"
      }) {
        log::info!(
          "{} {} (already extends deno.tsconfig.json)",
          colors::green("Unchanged"),
          tsconfig_path.display()
        );
        return Ok(());
      }

      obj.insert("extends".to_string(), json!("./deno.tsconfig.json"));
      let updated =
        serde_json::to_string_pretty(&tsconfig).expect("failed to serialize");
      std::fs::write(&tsconfig_path, updated)?;
      log::info!(
        "{} {} (added extends)",
        colors::green("Updated"),
        tsconfig_path.display()
      );
    }
  } else {
    // Create a minimal tsconfig.json
    let tsconfig = json!({
      "extends": "./deno.tsconfig.json"
    });
    let content =
      serde_json::to_string_pretty(&tsconfig).expect("failed to serialize");
    std::fs::write(&tsconfig_path, content)?;
    log::info!("{} {}", colors::green("Created"), tsconfig_path.display());
  }

  Ok(())
}
