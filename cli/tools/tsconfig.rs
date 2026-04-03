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

  // Install jsr: packages to node_modules/@jsr/ via npm compatibility layer
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

/// Install jsr: packages to node_modules/@jsr/ by downloading from the npm
/// compatibility layer at npm.jsr.io. Uses curl + tar directly — no npm
/// dependency required.
fn install_jsr_packages(
  project_root: &Path,
  deno_imports: Option<&Value>,
) -> Result<(), AnyError> {
  let imports = match deno_imports.and_then(|v| v.as_object()) {
    Some(imports) => imports,
    None => return Ok(()),
  };

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

    let npm_name = format!("{}__{}", scope.trim_start_matches('@'), name);
    let pkg_dir = project_root
      .join("node_modules")
      .join("@jsr")
      .join(&npm_name);
    if pkg_dir.exists() {
      continue;
    }

    // Resolve the latest matching version from npm.jsr.io
    let registry_name = format!("@jsr/{npm_name}");
    let metadata_url =
      format!("https://npm.jsr.io/{}", registry_name.replace('/', "%2f"));

    log::info!(
      "{} {} from npm.jsr.io",
      colors::green("Installing"),
      registry_name,
    );

    // Fetch package metadata to get tarball URL
    let metadata_output = std::process::Command::new("curl")
      .args(["-fsSL", &metadata_url])
      .output()
      .map_err(|e| anyhow!("Failed to fetch jsr package metadata: {e}"))?;

    if !metadata_output.status.success() {
      log::warn!(
        "Failed to fetch metadata for {}: {}",
        registry_name,
        String::from_utf8_lossy(&metadata_output.stderr)
      );
      continue;
    }

    let metadata: Value = serde_json::from_slice(&metadata_output.stdout)
      .map_err(|e| {
        anyhow!("Failed to parse metadata for {registry_name}: {e}")
      })?;

    // Get the latest version's tarball URL
    let latest_version = metadata
      .get("dist-tags")
      .and_then(|dt| dt.get("latest"))
      .and_then(|v| v.as_str())
      .ok_or_else(|| anyhow!("No latest version for {registry_name}"))?;

    let tarball_url = metadata
      .get("versions")
      .and_then(|vs| vs.get(latest_version))
      .and_then(|v| v.get("dist"))
      .and_then(|d| d.get("tarball"))
      .and_then(|t| t.as_str())
      .ok_or_else(|| {
        anyhow!("No tarball URL for {registry_name}@{latest_version}")
      })?;

    // Download and extract
    let temp_dir = tempfile::tempdir()?;
    let tgz_path = temp_dir.path().join("package.tgz");

    let dl_status = std::process::Command::new("curl")
      .args(["-fsSL", "-o", &tgz_path.to_string_lossy(), tarball_url])
      .status()
      .map_err(|e| anyhow!("Failed to download {registry_name}: {e}"))?;

    if !dl_status.success() {
      log::warn!("Failed to download {}", registry_name);
      continue;
    }

    // Extract to node_modules/@jsr/<name>
    std::fs::create_dir_all(&pkg_dir)?;

    let extract_status = std::process::Command::new("tar")
      .args([
        "xzf",
        &tgz_path.to_string_lossy(),
        "-C",
        &pkg_dir.to_string_lossy(),
        "--strip-components=1",
      ])
      .status()
      .map_err(|e| anyhow!("Failed to extract {registry_name}: {e}"))?;

    if !extract_status.success() {
      log::warn!("Failed to extract {}", registry_name);
      let _ = std::fs::remove_dir_all(&pkg_dir);
      continue;
    }

    log::info!(
      "{} {}@{}",
      colors::green("Installed"),
      registry_name,
      latest_version,
    );
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
