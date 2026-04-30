// Copyright 2018-2026 the Deno authors. MIT license.

//! Framework detection for `deno compile .`.
//!
//! Detects web frameworks (Next.js, Astro, Remix, SvelteKit, Nuxt, Fresh,
//! SolidStart, TanStack Start, Vite SSR) and generates the appropriate
//! entrypoint and include paths so that `deno compile .` just works.

use std::path::Path;

use deno_core::error::AnyError;
use deno_core::serde_json;

/// Result of framework detection.
pub struct FrameworkDetection {
  /// Name of the detected framework (for display).
  pub name: &'static str,
  /// Generated entrypoint TypeScript/JavaScript code (production).
  pub entrypoint_code: String,
  /// Directories to include in the compiled binary.
  pub include_paths: Vec<String>,
  /// Optional build command to run before compilation (e.g. "next build").
  /// The command is run with the detected directory as cwd.
  pub build_command: Option<Vec<String>>,
}

/// Detect a web framework in the given directory.
///
/// Detection priority:
/// 1. Config-file based detection (highest priority)
/// 2. Package.json dependency-based detection
/// 3. deno.json import-based detection
pub fn detect_framework(
  dir: &Path,
) -> Result<Option<FrameworkDetection>, AnyError> {
  // --- Config-file based detection (highest priority) ---

  // Next.js: next.config.{js,mjs,ts}
  if has_config_file(dir, "next.config") {
    return Ok(Some(detect_nextjs(dir)?));
  }

  // Fresh: fresh.gen.ts or _fresh/
  if dir.join("fresh.gen.ts").exists() || dir.join("_fresh").is_dir() {
    return Ok(Some(detect_fresh(dir)));
  }

  // Astro: astro.config.{mjs,ts,js}
  if has_config_file(dir, "astro.config") {
    return Ok(Some(detect_astro(dir)));
  }

  // Nuxt: nuxt.config.{ts,js,mjs}
  if has_config_file(dir, "nuxt.config") {
    return Ok(Some(detect_nuxt(dir)));
  }

  // SvelteKit: svelte.config.{js,ts} — but only when there's positive
  // evidence of a Deno-targeted adapter or a recognized server output
  // shape. A bare svelte.config.* is not enough, since SvelteKit can be
  // built with many adapters (node, vercel, cloudflare, static, ...) that
  // do not produce `./.output/server/index.{ts,mjs}`.
  if has_config_file(dir, "svelte.config")
    && let Some(detection) = detect_sveltekit(dir)
  {
    return Ok(Some(detection));
  }

  // --- Package.json dependency-based detection ---
  if let Some(deps) = read_package_deps(dir) {
    // Remix
    if deps.has("@remix-run/react") || deps.has_dev("@remix-run/dev") {
      return Ok(Some(detect_remix()));
    }

    // SolidStart
    if deps.has("@solidjs/start") {
      return Ok(Some(detect_nitro_framework(dir, "SolidStart")));
    }

    // TanStack Start
    if deps.has("@tanstack/react-start") || deps.has("@tanstack/solid-start") {
      return Ok(Some(detect_nitro_framework(dir, "TanStack Start")));
    }
  }

  // --- Vite SSR (lower priority, needs a server.js) ---
  if has_config_file(dir, "vite.config")
    && let Some(detection) = detect_vite_ssr(dir)
  {
    return Ok(Some(detection));
  }

  // --- deno.json import-based detection ---
  if let Some(imports) = read_deno_json_imports(dir)
    && imports
      .iter()
      .any(|i| i.starts_with("fresh") || i.starts_with("@fresh/core"))
  {
    return Ok(Some(detect_fresh(dir)));
  }

  Ok(None)
}

// --- Framework-specific detection ---

fn deno_exe() -> String {
  std::env::current_exe()
    .map(|p| p.display().to_string())
    .unwrap_or_else(|_| "deno".into())
}

fn deno_task_build() -> Vec<String> {
  vec![deno_exe(), "task".into(), "build".into()]
}

fn detect_nextjs(dir: &Path) -> Result<FrameworkDetection, AnyError> {
  let version = detect_package_version(dir, "next").unwrap_or(15);
  let entrypoint = format!(
    r#"// @ts-nocheck
import {{ nextStart }} from "npm:next@^{version}/dist/cli/next-start.js";
globalThis.addEventListener("unhandledrejection", (e) => {{
  console.error("[entrypoint] Unhandled rejection:", e.reason);
  if (e.reason?.stack) console.error("[entrypoint] Stack:", e.reason.stack);
}});
// Guard: skip for forked workers (child_process.fork sets NODE_CHANNEL_FD).
// Workers use override_main_module to run their target script directly.
if (!Deno.env.get("NODE_CHANNEL_FD")) {{
  // Use import.meta.dirname so paths resolve against the VFS in the
  // compiled binary rather than the runtime CWD.
  await nextStart({{ hostname: "0.0.0.0" }}, import.meta.dirname);
}}
"#,
  );
  Ok(FrameworkDetection {
    name: "Next.js",
    entrypoint_code: entrypoint,
    include_paths: vec![".next".into()],
    build_command: Some(deno_task_build()),
  })
}

fn detect_astro(_dir: &Path) -> FrameworkDetection {
  FrameworkDetection {
    name: "Astro",
    entrypoint_code: "// @ts-nocheck\nimport \"./dist/server/entry.mjs\";\n"
      .into(),
    include_paths: vec!["dist".into()],
    build_command: Some(deno_task_build()),
  }
}

fn detect_fresh(dir: &Path) -> FrameworkDetection {
  // Fresh 2.x uses _fresh/server.js (build output) or imports @fresh/core
  // in deno.json. We intentionally do NOT use `fresh.gen.ts + deno.json`
  // as a heuristic because Fresh 1 projects also have both of those files.
  let is_fresh2 = dir.join("_fresh/server.js").exists()
    || read_deno_json_imports(dir)
      .map(|imports| imports.iter().any(|i| i.starts_with("@fresh/core")))
      .unwrap_or(false);
  if is_fresh2 {
    FrameworkDetection {
      name: "Fresh",
      entrypoint_code: r#"// @ts-nocheck
const mod = await import("./_fresh/server.js");
Deno.serve(mod.default.fetch);
"#
      .into(),
      include_paths: vec!["_fresh".into()],
      build_command: Some(vec![deno_exe(), "task".into(), "build".into()]),
    }
  } else {
    // Fresh 1.x — no build step needed, server-rendered
    FrameworkDetection {
      name: "Fresh",
      entrypoint_code: "// @ts-nocheck\nimport \"./main.ts\";\n".into(),
      include_paths: vec![],
      build_command: None,
    }
  }
}

fn detect_remix() -> FrameworkDetection {
  FrameworkDetection {
    name: "Remix",
    entrypoint_code:
      "// @ts-nocheck\nimport \"./node_modules/.bin/remix-serve\";\n".into(),
    include_paths: vec!["build".into()],
    build_command: Some(deno_task_build()),
  }
}

fn detect_nuxt(dir: &Path) -> FrameworkDetection {
  detect_nitro_framework(dir, "Nuxt")
}

fn detect_sveltekit(dir: &Path) -> Option<FrameworkDetection> {
  // Prefer post-build evidence of a supported output shape, since that
  // proves which adapter was actually used.
  if dir.join(".deno-deploy/server.ts").exists() {
    return Some(FrameworkDetection {
      name: "SvelteKit",
      entrypoint_code: "// @ts-nocheck\nimport \"./.deno-deploy/server.ts\";\n"
        .into(),
      include_paths: vec![".deno-deploy".into()],
      build_command: Some(deno_task_build()),
    });
  }
  if dir.join(".output/server/index.ts").exists()
    || dir.join(".output/server/index.mjs").exists()
  {
    let ext = if dir.join(".output/server/index.ts").exists() {
      "ts"
    } else {
      "mjs"
    };
    return Some(FrameworkDetection {
      name: "SvelteKit",
      entrypoint_code: format!(
        "// @ts-nocheck\nimport \"./.output/server/index.{ext}\";\n"
      ),
      include_paths: vec![".output".into()],
      build_command: Some(deno_task_build()),
    });
  }
  // No build artifacts yet — fall back to config inspection. We only
  // claim SvelteKit if the config references a supported adapter.
  let config_text =
    ["svelte.config.js", "svelte.config.ts", "svelte.config.mjs"]
      .iter()
      .find_map(|f| std::fs::read_to_string(dir.join(f)).ok())?;
  if config_text.contains("@deno/svelte-adapter") {
    return Some(FrameworkDetection {
      name: "SvelteKit",
      entrypoint_code: "// @ts-nocheck\nimport \"./.deno-deploy/server.ts\";\n"
        .into(),
      include_paths: vec![".deno-deploy".into()],
      build_command: Some(deno_task_build()),
    });
  }
  if config_text.contains("nitro")
    || config_text.contains("svelte-adapter-deno")
  {
    return Some(FrameworkDetection {
      name: "SvelteKit",
      entrypoint_code:
        "// @ts-nocheck\nimport \"./.output/server/index.mjs\";\n".into(),
      include_paths: vec![".output".into()],
      build_command: Some(deno_task_build()),
    });
  }
  None
}

/// Nuxt, SolidStart, TanStack Start all use Nitro with the `deno_server`
/// preset, outputting to `.output/server/index.{ts,mjs}`.
fn detect_nitro_framework(
  dir: &Path,
  name: &'static str,
) -> FrameworkDetection {
  let ext = if dir.join(".output/server/index.ts").exists() {
    "ts"
  } else {
    "mjs"
  };
  FrameworkDetection {
    name,
    entrypoint_code: format!(
      "// @ts-nocheck\nimport \"./.output/server/index.{ext}\";\n"
    ),
    include_paths: vec![".output".into()],
    build_command: Some(deno_task_build()),
  }
}

fn detect_vite_ssr(dir: &Path) -> Option<FrameworkDetection> {
  let server_file = ["server.js", "server.ts", "server.mjs"]
    .iter()
    .find(|f| dir.join(f).exists())?;
  Some(FrameworkDetection {
    name: "Vite",
    entrypoint_code: format!("// @ts-nocheck\nimport \"./{server_file}\";\n"),
    include_paths: vec!["dist".into()],
    build_command: Some(deno_task_build()),
  })
}

// --- Helpers ---

/// Check if a config file exists with any common extension.
fn has_config_file(dir: &Path, base_name: &str) -> bool {
  ["js", "mjs", "ts", "mts", "cjs"]
    .iter()
    .any(|ext| dir.join(format!("{base_name}.{ext}")).exists())
}

/// Read package.json dependencies.
fn read_package_deps(dir: &Path) -> Option<PackageDeps> {
  let content = std::fs::read_to_string(dir.join("package.json")).ok()?;
  let pkg: serde_json::Value = serde_json::from_str(&content).ok()?;
  Some(PackageDeps {
    deps: pkg
      .get("dependencies")
      .cloned()
      .unwrap_or(serde_json::Value::Object(Default::default())),
    dev_deps: pkg
      .get("devDependencies")
      .cloned()
      .unwrap_or(serde_json::Value::Object(Default::default())),
  })
}

struct PackageDeps {
  deps: serde_json::Value,
  dev_deps: serde_json::Value,
}

impl PackageDeps {
  fn has(&self, name: &str) -> bool {
    self.deps.get(name).is_some()
  }

  fn has_dev(&self, name: &str) -> bool {
    self.dev_deps.get(name).is_some()
  }
}

/// Extract the major version number from a package.json dependency.
fn detect_package_version(dir: &Path, package: &str) -> Option<u32> {
  let content = std::fs::read_to_string(dir.join("package.json")).ok()?;
  let pkg: serde_json::Value = serde_json::from_str(&content).ok()?;
  let ver_str = pkg
    .get("dependencies")
    .and_then(|d| d.get(package))
    .or_else(|| pkg.get("devDependencies").and_then(|d| d.get(package)))?
    .as_str()?;
  // Extract major version from "^16.1.6", "~15.0.0", "14.2.3", etc.
  ver_str
    .chars()
    .skip_while(|c: &char| !c.is_ascii_digit())
    .take_while(|c: &char| c.is_ascii_digit())
    .collect::<String>()
    .parse()
    .ok()
}

/// Read the `imports` keys from deno.json / deno.jsonc.
///
/// Uses JSONC-aware parsing so that commented deno.jsonc files are
/// handled correctly instead of silently failing detection.
fn read_deno_json_imports(dir: &Path) -> Option<Vec<String>> {
  let content = std::fs::read_to_string(dir.join("deno.json"))
    .or_else(|_| std::fs::read_to_string(dir.join("deno.jsonc")))
    .ok()?;
  let config: serde_json::Value =
    jsonc_parser::parse_to_serde_value(&content, &Default::default())
      .ok()
      .flatten()?;
  let imports = config.get("imports")?.as_object()?;
  Some(imports.keys().cloned().collect())
}

#[cfg(test)]
mod tests {
  use std::fs;

  use super::*;

  fn setup_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
  }

  #[test]
  fn no_framework_empty_dir() {
    let dir = setup_dir();
    let result = detect_framework(dir.path()).unwrap();
    assert!(result.is_none());
  }

  // --- Config-file based detection ---

  #[test]
  fn detects_nextjs_with_config_js() {
    let dir = setup_dir();
    fs::write(dir.path().join("next.config.js"), "").unwrap();
    fs::write(
      dir.path().join("package.json"),
      r#"{"dependencies":{"next":"^15.0.0"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Next.js");
    assert_eq!(det.include_paths, vec![".next"]);
    assert!(det.entrypoint_code.contains("next@^15"));
    // No .next dir => build_command is set
    assert!(det.build_command.is_some());
    let cmd = det.build_command.unwrap();
    assert_eq!(cmd[1..], vec!["task", "build"]);
  }

  #[test]
  fn nextjs_always_builds() {
    let dir = setup_dir();
    fs::write(dir.path().join("next.config.js"), "").unwrap();
    fs::create_dir(dir.path().join(".next")).unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Next.js");
    assert!(det.build_command.is_some());
  }

  #[test]
  fn detects_nextjs_with_config_mjs() {
    let dir = setup_dir();
    fs::write(dir.path().join("next.config.mjs"), "").unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Next.js");
  }

  #[test]
  fn detects_nextjs_with_config_ts() {
    let dir = setup_dir();
    fs::write(dir.path().join("next.config.ts"), "").unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Next.js");
  }

  #[test]
  fn nextjs_version_from_package_json() {
    let dir = setup_dir();
    fs::write(dir.path().join("next.config.js"), "").unwrap();
    fs::write(
      dir.path().join("package.json"),
      r#"{"dependencies":{"next":"^14.2.3"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert!(det.entrypoint_code.contains("next@^14"));
  }

  #[test]
  fn nextjs_defaults_to_v15() {
    let dir = setup_dir();
    fs::write(dir.path().join("next.config.js"), "").unwrap();
    // no package.json
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert!(det.entrypoint_code.contains("next@^15"));
  }

  #[test]
  fn detects_fresh_gen_ts() {
    let dir = setup_dir();
    fs::write(dir.path().join("fresh.gen.ts"), "").unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Fresh");
    assert!(det.include_paths.is_empty());
    // no _fresh/server.js => Fresh 1.x
    assert!(det.entrypoint_code.contains("main.ts"));
  }

  #[test]
  fn detects_fresh2_with_server_js() {
    let dir = setup_dir();
    fs::create_dir_all(dir.path().join("_fresh")).unwrap();
    fs::write(dir.path().join("_fresh/server.js"), "").unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Fresh");
    assert!(det.entrypoint_code.contains("_fresh/server.js"));
    let cmd = det.build_command.unwrap();
    assert_eq!(cmd[1..], vec!["task", "build"]);
  }

  #[test]
  fn fresh1_has_no_build_command() {
    let dir = setup_dir();
    fs::write(dir.path().join("fresh.gen.ts"), "").unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Fresh");
    assert!(det.build_command.is_none());
  }

  #[test]
  fn fresh1_with_deno_json_stays_fresh1() {
    // Regression: fresh.gen.ts + deno.json (without @fresh/core import)
    // should be treated as Fresh 1, not Fresh 2.
    let dir = setup_dir();
    fs::write(dir.path().join("fresh.gen.ts"), "").unwrap();
    fs::write(
      dir.path().join("deno.json"),
      r#"{"tasks":{"start":"deno run -A main.ts"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Fresh");
    // Fresh 1.x uses main.ts, not _fresh/server.js
    assert!(det.entrypoint_code.contains("main.ts"));
    assert!(det.build_command.is_none());
  }

  #[test]
  fn fresh2_detected_via_fresh_core_import() {
    let dir = setup_dir();
    fs::write(dir.path().join("fresh.gen.ts"), "").unwrap();
    fs::write(
      dir.path().join("deno.json"),
      r#"{"imports":{"@fresh/core":"jsr:@fresh/core@^2"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Fresh");
    // Should be Fresh 2 because @fresh/core is in imports
    assert!(det.entrypoint_code.contains("_fresh/server.js"));
    assert!(det.build_command.is_some());
  }

  #[test]
  fn detects_astro() {
    let dir = setup_dir();
    fs::write(dir.path().join("astro.config.mjs"), "").unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Astro");
    assert!(det.entrypoint_code.contains("dist/server/entry.mjs"));
    assert_eq!(det.include_paths, vec!["dist"]);
    assert!(det.build_command.is_some());
    let cmd = det.build_command.unwrap();
    assert_eq!(cmd[1..], vec!["task", "build"]);
  }

  #[test]
  fn detects_nuxt() {
    let dir = setup_dir();
    fs::write(dir.path().join("nuxt.config.ts"), "").unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Nuxt");
    assert_eq!(det.include_paths, vec![".output"]);
    let cmd = det.build_command.unwrap();
    assert_eq!(cmd[1..], vec!["task", "build"]);
  }

  #[test]
  fn detects_nuxt_with_ts_output() {
    let dir = setup_dir();
    fs::write(dir.path().join("nuxt.config.ts"), "").unwrap();
    fs::create_dir_all(dir.path().join(".output/server")).unwrap();
    fs::write(dir.path().join(".output/server/index.ts"), "").unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert!(det.entrypoint_code.contains("index.ts"));
  }

  #[test]
  fn detects_sveltekit_deno_deploy() {
    let dir = setup_dir();
    fs::write(dir.path().join("svelte.config.js"), "").unwrap();
    fs::create_dir_all(dir.path().join(".deno-deploy")).unwrap();
    fs::write(dir.path().join(".deno-deploy/server.ts"), "").unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "SvelteKit");
    assert!(det.entrypoint_code.contains(".deno-deploy/server.ts"));
    assert_eq!(det.include_paths, vec![".deno-deploy"]);
    let cmd = det.build_command.unwrap();
    assert_eq!(cmd[1..], vec!["task", "build"]);
  }

  #[test]
  fn detects_sveltekit_nitro_from_built_output() {
    let dir = setup_dir();
    fs::write(dir.path().join("svelte.config.ts"), "").unwrap();
    fs::create_dir_all(dir.path().join(".output/server")).unwrap();
    fs::write(dir.path().join(".output/server/index.mjs"), "").unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "SvelteKit");
    assert_eq!(det.include_paths, vec![".output"]);
    assert!(det.build_command.is_some());
  }

  #[test]
  fn detects_sveltekit_nitro_from_config() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("svelte.config.js"),
      "import adapter from 'svelte-adapter-deno';\nexport default { kit: { adapter: adapter() } };\n",
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "SvelteKit");
    assert_eq!(det.include_paths, vec![".output"]);
  }

  #[test]
  fn does_not_detect_sveltekit_with_unknown_adapter() {
    // Regression: a SvelteKit project using e.g. adapter-vercel should
    // NOT be claimed by our detector — it would generate a wrong
    // entrypoint that imports a path that doesn't exist.
    let dir = setup_dir();
    fs::write(
      dir.path().join("svelte.config.js"),
      "import adapter from '@sveltejs/adapter-vercel';\nexport default { kit: { adapter: adapter() } };\n",
    )
    .unwrap();
    assert!(detect_framework(dir.path()).unwrap().is_none());
  }

  // --- Package.json dependency-based detection ---

  #[test]
  fn detects_remix_from_deps() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("package.json"),
      r#"{"dependencies":{"@remix-run/react":"^2.0.0"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Remix");
    assert_eq!(det.include_paths, vec!["build"]);
    let cmd = det.build_command.unwrap();
    assert_eq!(cmd[1..], vec!["task", "build"]);
  }

  #[test]
  fn detects_remix_from_dev_deps() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("package.json"),
      r#"{"devDependencies":{"@remix-run/dev":"^2.0.0"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Remix");
  }

  #[test]
  fn detects_solidstart() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("package.json"),
      r#"{"dependencies":{"@solidjs/start":"^1.0.0"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "SolidStart");
    assert_eq!(det.include_paths, vec![".output"]);
    let cmd = det.build_command.unwrap();
    assert_eq!(cmd[1..], vec!["task", "build"]);
  }

  #[test]
  fn detects_tanstack_start_react() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("package.json"),
      r#"{"dependencies":{"@tanstack/react-start":"^1.0.0"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "TanStack Start");
  }

  #[test]
  fn detects_tanstack_start_solid() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("package.json"),
      r#"{"dependencies":{"@tanstack/solid-start":"^1.0.0"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "TanStack Start");
  }

  // --- Vite SSR ---

  #[test]
  fn detects_vite_ssr_with_server_js() {
    let dir = setup_dir();
    fs::write(dir.path().join("vite.config.js"), "").unwrap();
    fs::write(dir.path().join("server.js"), "").unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Vite");
    assert!(det.entrypoint_code.contains("server.js"));
    assert_eq!(det.include_paths, vec!["dist"]);
    let cmd = det.build_command.unwrap();
    assert_eq!(cmd[1..], vec!["task", "build"]);
  }

  #[test]
  fn detects_vite_ssr_with_server_ts() {
    let dir = setup_dir();
    fs::write(dir.path().join("vite.config.ts"), "").unwrap();
    fs::write(dir.path().join("server.ts"), "").unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Vite");
    assert!(det.entrypoint_code.contains("server.ts"));
  }

  #[test]
  fn vite_without_server_file_returns_none() {
    let dir = setup_dir();
    fs::write(dir.path().join("vite.config.js"), "").unwrap();
    // no server.js/ts/mjs
    let result = detect_framework(dir.path()).unwrap();
    assert!(result.is_none());
  }

  // --- deno.json import-based detection ---

  #[test]
  fn detects_fresh_from_deno_json_imports() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("deno.json"),
      r#"{"imports":{"fresh":"jsr:@fresh/core"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Fresh");
  }

  #[test]
  fn detects_fresh_from_deno_json_fresh_core() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("deno.json"),
      r#"{"imports":{"@fresh/core":"jsr:@fresh/core@^2"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Fresh");
  }

  #[test]
  fn detects_fresh_from_deno_jsonc() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("deno.jsonc"),
      r#"{"imports":{"fresh":"jsr:@fresh/core"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Fresh");
  }

  #[test]
  fn detects_fresh_from_commented_deno_jsonc() {
    // Regression: deno.jsonc with comments should still be parsed correctly.
    let dir = setup_dir();
    fs::write(
      dir.path().join("deno.jsonc"),
      "{\n  // This is a comment\n  \"imports\": {\n    \"fresh\": \"jsr:@fresh/core\"\n  }\n}\n",
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Fresh");
  }

  // --- Priority ---

  #[test]
  fn config_file_takes_priority_over_package_json() {
    let dir = setup_dir();
    // Has both next.config.js and remix in package.json
    fs::write(dir.path().join("next.config.js"), "").unwrap();
    fs::write(
      dir.path().join("package.json"),
      r#"{"dependencies":{"@remix-run/react":"^2.0.0"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Next.js");
  }

  #[test]
  fn package_json_takes_priority_over_vite_ssr() {
    let dir = setup_dir();
    fs::write(dir.path().join("vite.config.js"), "").unwrap();
    fs::write(dir.path().join("server.js"), "").unwrap();
    fs::write(
      dir.path().join("package.json"),
      r#"{"dependencies":{"@solidjs/start":"^1.0.0"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "SolidStart");
  }

  #[test]
  fn config_file_takes_priority_over_deno_json() {
    let dir = setup_dir();
    fs::write(dir.path().join("astro.config.mjs"), "").unwrap();
    fs::write(
      dir.path().join("deno.json"),
      r#"{"imports":{"fresh":"jsr:@fresh/core"}}"#,
    )
    .unwrap();
    let det = detect_framework(dir.path()).unwrap().unwrap();
    assert_eq!(det.name, "Astro");
  }

  // --- Helper unit tests ---

  #[test]
  fn has_config_file_various_extensions() {
    let dir = setup_dir();
    assert!(!has_config_file(dir.path(), "next.config"));
    fs::write(dir.path().join("next.config.mts"), "").unwrap();
    assert!(has_config_file(dir.path(), "next.config"));
  }

  #[test]
  fn detect_package_version_parses_caret() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("package.json"),
      r#"{"dependencies":{"next":"^16.1.6"}}"#,
    )
    .unwrap();
    assert_eq!(detect_package_version(dir.path(), "next"), Some(16));
  }

  #[test]
  fn detect_package_version_parses_tilde() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("package.json"),
      r#"{"dependencies":{"next":"~15.0.0"}}"#,
    )
    .unwrap();
    assert_eq!(detect_package_version(dir.path(), "next"), Some(15));
  }

  #[test]
  fn detect_package_version_parses_exact() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("package.json"),
      r#"{"dependencies":{"next":"14.2.3"}}"#,
    )
    .unwrap();
    assert_eq!(detect_package_version(dir.path(), "next"), Some(14));
  }

  #[test]
  fn detect_package_version_from_dev_deps() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("package.json"),
      r#"{"devDependencies":{"next":"^13.0.0"}}"#,
    )
    .unwrap();
    assert_eq!(detect_package_version(dir.path(), "next"), Some(13));
  }

  #[test]
  fn detect_package_version_missing_package() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("package.json"),
      r#"{"dependencies":{"react":"^18.0.0"}}"#,
    )
    .unwrap();
    assert_eq!(detect_package_version(dir.path(), "next"), None);
  }

  #[test]
  fn detect_package_version_no_package_json() {
    let dir = setup_dir();
    assert_eq!(detect_package_version(dir.path(), "next"), None);
  }

  #[test]
  fn read_deno_json_imports_returns_keys() {
    let dir = setup_dir();
    fs::write(
      dir.path().join("deno.json"),
      r#"{"imports":{"foo":"jsr:@foo/bar","baz":"npm:baz"}}"#,
    )
    .unwrap();
    let mut imports = read_deno_json_imports(dir.path()).unwrap();
    imports.sort();
    assert_eq!(imports, vec!["baz", "foo"]);
  }

  #[test]
  fn read_deno_json_imports_no_file() {
    let dir = setup_dir();
    assert!(read_deno_json_imports(dir.path()).is_none());
  }

  #[test]
  fn read_deno_json_imports_no_imports_key() {
    let dir = setup_dir();
    fs::write(dir.path().join("deno.json"), r#"{"tasks":{}}"#).unwrap();
    assert!(read_deno_json_imports(dir.path()).is_none());
  }
}
