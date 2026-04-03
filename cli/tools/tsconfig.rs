// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use deno_semver::Version;
use deno_semver::VersionReq;
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

  // Clean up the generated .deno/tsconfig.json (we've moved it to
  // deno.tsconfig.json), but keep .deno/types/ so the Deno type
  // definitions file referenced by the tsconfig remains available.
  let deno_tsconfig = project_root.join(".deno").join("tsconfig.json");
  if deno_tsconfig.exists() {
    let _ = std::fs::remove_file(&deno_tsconfig);
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

    // Resolve the matching version from npm.jsr.io
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

    // Resolve version: use the requested version/range from the specifier,
    // falling back to dist-tags.latest only when no version was specified.
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
      resolved_version,
    );
  }

  Ok(())
}

/// Resolve a JSR package version from npm.jsr.io registry metadata.
///
/// If `req_version` is provided (e.g. "1", "1.2", "^1.0.0"), finds the highest
/// published version that satisfies the requirement. If `None`, uses the
/// `dist-tags.latest` version.
fn resolve_jsr_version(
  metadata: &Value,
  req_version: Option<&str>,
  registry_name: &str,
) -> Result<String, AnyError> {
  match req_version {
    None => {
      // No version specified — use dist-tags.latest
      metadata
        .get("dist-tags")
        .and_then(|dt| dt.get("latest"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("No latest version for {registry_name}"))
    }
    Some(req_str) => {
      // Try to parse as an exact version first
      if let Ok(exact) = Version::parse_standard(req_str) {
        // Verify it exists in the registry
        if metadata
          .get("versions")
          .and_then(|vs| vs.get(exact.to_string()))
          .is_some()
        {
          return Ok(exact.to_string());
        }
      }

      // Parse as a version range and find the best matching version
      let version_req = VersionReq::parse_from_npm(req_str).map_err(|e| {
        anyhow!(
          "Failed to parse version requirement '{req_str}' for {registry_name}: {e}"
        )
      })?;

      let versions = metadata
        .get("versions")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow!("No versions found for {registry_name}"))?;

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
        anyhow!("No version of {registry_name} matches requirement '{req_str}'")
      })
    }
  }
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

      // Check if already configured to extend deno.tsconfig.json
      let already_extends = current_extends.is_some_and(|v| match v {
        Value::String(s) => {
          s == "deno.tsconfig.json" || s == "./deno.tsconfig.json"
        }
        Value::Array(arr) => arr.iter().any(|item| {
          item.as_str().is_some_and(|s| {
            s == "deno.tsconfig.json" || s == "./deno.tsconfig.json"
          })
        }),
        _ => false,
      });

      if already_extends {
        log::info!(
          "{} {} (already extends deno.tsconfig.json)",
          colors::green("Unchanged"),
          tsconfig_path.display()
        );
        return Ok(());
      }

      // Preserve existing extends chain by composing into an array
      match current_extends.cloned() {
        Some(existing) if !existing.is_null() => {
          // Compose: put deno.tsconfig.json first, then existing extends
          let mut chain = vec![json!("./deno.tsconfig.json")];
          match existing {
            Value::Array(arr) => chain.extend(arr),
            Value::String(_) => chain.push(existing),
            _ => {}
          }
          obj.insert("extends".to_string(), json!(chain));
        }
        _ => {
          obj.insert("extends".to_string(), json!("./deno.tsconfig.json"));
        }
      }

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
