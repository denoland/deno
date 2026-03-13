// Copyright 2018-2026 the Deno authors. MIT license.

//! Framework detection and desktop entrypoint generation for `deno compile --desktop`.
//!
//! Detects web frameworks (Next.js, Astro, Remix, SvelteKit, Nuxt, Fresh, Vite SSR)
//! and generates the appropriate entrypoint and include paths so that
//! `deno compile --desktop .` just works.

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
  /// Dev server entrypoint code for HMR mode.
  /// When set, `deno compile --desktop --hmr .` will use this entrypoint
  /// instead of the production one, running the framework's dev server
  /// inside the Deno desktop runtime so that `Deno.desktop` APIs work.
  pub dev_entrypoint_code: Option<String>,
}

/// Detect a web framework in the given directory.
///
/// Detection priority (matches deployng):
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

  // SvelteKit: svelte.config.{js,ts}
  if has_config_file(dir, "svelte.config") {
    return Ok(Some(detect_sveltekit(dir)));
  }

  // --- Package.json dependency-based detection ---
  if let Some(deps) = read_package_deps(dir) {
    // Remix
    if deps.has("@remix-run/react") || deps.has_dev("@remix-run/dev") {
      return Ok(Some(detect_remix(dir)));
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
  if has_config_file(dir, "vite.config") {
    if let Some(detection) = detect_vite_ssr(dir) {
      return Ok(Some(detection));
    }
  }

  // --- deno.json import-based detection ---
  if let Some(imports) = read_deno_json_imports(dir) {
    if imports
      .iter()
      .any(|i| i.starts_with("fresh") || i.starts_with("@fresh/core"))
    {
      return Ok(Some(detect_fresh(dir)));
    }
  }

  Ok(None)
}

// --- Framework-specific detection ---

fn detect_nextjs(dir: &Path) -> Result<FrameworkDetection, AnyError> {
  let version = detect_package_version(dir, "next").unwrap_or(15);
  let entrypoint = format!(
    r#"// @ts-nocheck
import {{ nextStart }} from "npm:next@^{version}/dist/cli/next-start.js";
import {{ nextDev }} from "npm:next@^{version}/dist/cli/next-dev.js";
globalThis.addEventListener("unhandledrejection", (e) => {{
  console.error("[desktop-entrypoint] Unhandled rejection:", e.reason);
  if (e.reason?.stack) console.error("[desktop-entrypoint] Stack:", e.reason.stack);
}});
// Guard: skip for forked workers (child_process.fork sets NODE_CHANNEL_FD).
// Workers use override_main_module to run their target script directly.
if (!Deno.env.get("NODE_CHANNEL_FD")) {{
  if (Deno.env.get("DENO_DESKTOP_DEV")) {{
    await nextDev({{ port: 41520, hostname: "127.0.0.1" }}, "default", ".");
    // Keep alive after dev server starts — nextDev resolves once the
    // worker is ready, but we need the parent to stay alive for the
    // child process IPC and restart handling.
    await new Promise(() => {{}});
  }} else {{
    await nextStart({{ hostname: "127.0.0.1" }}, ".");
  }}
}}
"#,
  );
  Ok(FrameworkDetection {
    name: "Next.js",
    entrypoint_code: entrypoint,
    include_paths: vec![".next".into()],
    dev_entrypoint_code: Some("DENO_DESKTOP_DEV=1".into()),
  })
}

fn detect_astro(dir: &Path) -> FrameworkDetection {
  let dev_entrypoint = Some(
    r#"// @ts-nocheck
import { cli } from "npm:astro";
cli(["dev", "--port", "4321", "--host", "127.0.0.1"]);
"#
    .into(),
  );
  // Check if it's SSR (has dist/server/entry.mjs) or static
  let is_ssr = dir.join("dist/server/entry.mjs").exists();
  if is_ssr {
    FrameworkDetection {
      name: "Astro",
      entrypoint_code: "// @ts-nocheck\nimport \"./dist/server/entry.mjs\";\n"
        .into(),
      include_paths: vec!["dist".into()],
      dev_entrypoint_code: dev_entrypoint,
    }
  } else {
    let mut d = static_file_server_detection("Astro", "dist");
    d.dev_entrypoint_code = dev_entrypoint;
    d
  }
}

fn detect_fresh(dir: &Path) -> FrameworkDetection {
  // Fresh 2.x uses _fresh/server.js, older uses main.ts
  let is_fresh2 = dir.join("_fresh/server.js").exists();
  if is_fresh2 {
    let vite_config_loader =
      if let Some(vite_config) = find_config_file(dir, "vite.config") {
        format!(
          r#"
const userConfigMod = await import("./{vite_config}");
const exportedConfig = userConfigMod.default ?? userConfigMod;
const userConfig = (typeof exportedConfig === "function"
  ? await exportedConfig({{
      command: "serve",
      mode: Deno.env.get("NODE_ENV") ?? "development",
      isSsrBuild: false,
      isPreview: false,
    }})
  : await exportedConfig) ?? {{}};
"#
        )
      } else {
        "const userConfig = { plugins: [fresh()] };".to_string()
      };
    // Fresh 2.x: production imports _fresh/server.js, dev starts Vite
    FrameworkDetection {
      name: "Fresh",
      entrypoint_code: r#"// @ts-nocheck
import { createServer } from "npm:vite";
import { fresh } from "@fresh/plugin-vite";
if (Deno.env.get("DENO_DESKTOP_DEV")) {
  __DENO_DESKTOP_VITE_CONFIG__
  const freshDesktopFixup = {
    name: "fresh-desktop:fixup",
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const origWrite = res.write.bind(res);
        const origEnd = res.end.bind(res);
        const chunks = [];
        res.write = (chunk, ...args) => { chunks.push(Buffer.from(chunk)); return true; };
        res.end = (chunk, ...args) => {
          if (chunk) chunks.push(Buffer.from(chunk));
          let body = Buffer.concat(chunks).toString();
          const ct = res.getHeader("content-type") || "";
          if (ct.includes("text/html")) {
            body = body.replace(/from "(fresh-(?:island|route)::)/g, 'from "/@id/$1');
          }
          res.setHeader("content-length", Buffer.byteLength(body));
          origEnd(body);
        };
        next();
      });
    },
  };
  const server = await createServer({
    ...userConfig,
    configFile: false,
    plugins: [...(userConfig.plugins ?? [fresh()]), freshDesktopFixup],
    server: {
      ...(userConfig.server ?? {}),
      port: 41520,
      host: "127.0.0.1",
    },
  });
  await server.listen();
  await new Promise(() => {});
} else {
  const mod = await import("./_fresh/server.js");
  Deno.serve({ port: 41520, hostname: "127.0.0.1" }, mod.default.fetch);
}
"#
      .replace("__DENO_DESKTOP_VITE_CONFIG__", &vite_config_loader)
      .into(),
      include_paths: vec!["_fresh".into()],
      dev_entrypoint_code: Some("DENO_DESKTOP_DEV=1".into()),
    }
  } else {
    // Fresh 1.x
    FrameworkDetection {
      name: "Fresh",
      entrypoint_code: "// @ts-nocheck\nimport \"./main.ts\";\n".into(),
      include_paths: vec!["_fresh".into()],
      dev_entrypoint_code: Some(
        "// @ts-nocheck\nimport \"./dev.ts\";\n".into(),
      ),
    }
  }
}

fn detect_remix(_dir: &Path) -> FrameworkDetection {
  FrameworkDetection {
    name: "Remix",
    entrypoint_code:
      "// @ts-nocheck\nimport \"./node_modules/.bin/remix-serve\";\n".into(),
    include_paths: vec!["build".into()],
    dev_entrypoint_code: Some(
      r#"// @ts-nocheck
import { run } from "npm:@remix-run/dev/dist/cli/run.js";
run(["dev"]);
"#
      .into(),
    ),
  }
}

fn detect_nuxt(dir: &Path) -> FrameworkDetection {
  detect_nitro_framework(dir, "Nuxt")
}

fn detect_sveltekit(dir: &Path) -> FrameworkDetection {
  // SvelteKit with @deno/svelte-adapter outputs to .deno-deploy/
  let prod_entrypoint = if dir.join(".deno-deploy/server.ts").exists() {
    "// @ts-nocheck\nimport \"./.deno-deploy/server.ts\";\n".to_string()
  } else {
    // Nitro-based output
    let ext = if dir.join(".output/server/index.ts").exists() {
      "ts"
    } else {
      "mjs"
    };
    format!("// @ts-nocheck\nimport \"./.output/server/index.{ext}\";\n")
  };
  let include_paths = if dir.join(".deno-deploy/server.ts").exists() {
    vec![".deno-deploy".into()]
  } else {
    vec![".output".into()]
  };
  FrameworkDetection {
    name: "SvelteKit",
    entrypoint_code: format!(
      r#"// @ts-nocheck
import {{ createServer }} from "npm:vite";
if (Deno.env.get("DENO_DESKTOP_DEV")) {{
  const server = await createServer({{
    server: {{ port: 41520, host: "127.0.0.1" }},
  }});
  await server.listen();
  await new Promise(() => {{}});
}} else {{
  {prod_entrypoint}
}}
"#
    ),
    include_paths,
    dev_entrypoint_code: Some("DENO_DESKTOP_DEV=1".into()),
  }
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
  // Nuxt uses nuxi, others use nitro dev directly
  let dev_entrypoint = if name == "Nuxt" {
    Some(
      r#"// @ts-nocheck
import { runCommand } from "npm:nuxi/cli";
runCommand("dev", ["--port", "3000", "--host", "127.0.0.1"]);
"#
      .into(),
    )
  } else {
    None
  };
  FrameworkDetection {
    name,
    entrypoint_code: format!(
      "// @ts-nocheck\nimport \"./.output/server/index.{ext}\";\n"
    ),
    include_paths: vec![".output".into()],
    dev_entrypoint_code: dev_entrypoint,
  }
}

fn detect_vite_ssr(dir: &Path) -> Option<FrameworkDetection> {
  let server_file = ["server.js", "server.ts", "server.mjs"]
    .iter()
    .find(|f| dir.join(f).exists())?;
  Some(FrameworkDetection {
    name: "Vite",
    entrypoint_code: format!(
      "// @ts-nocheck\nimport \"./{server_file}\";\n"
    ),
    include_paths: vec!["dist".into()],
    dev_entrypoint_code: Some(
      r#"// @ts-nocheck
import { createServer } from "npm:vite";
const server = await createServer({ server: { port: 5173, host: "127.0.0.1" } });
await server.listen();
"#
      .into(),
    ),
  })
}

/// Generate a detection result that serves a static directory using Deno.serve.
fn static_file_server_detection(
  name: &'static str,
  static_dir: &str,
) -> FrameworkDetection {
  FrameworkDetection {
    name,
    entrypoint_code: format!(
      r#"// @ts-nocheck
import {{ serveDir }} from "jsr:@std/http@^1/file-server";
Deno.serve((req) => serveDir(req, {{ fsRoot: "./{static_dir}" }}));
"#
    ),
    include_paths: vec![static_dir.into()],
    dev_entrypoint_code: None,
  }
}

// --- Helpers ---

/// Check if a config file exists with any common extension.
fn has_config_file(dir: &Path, base_name: &str) -> bool {
  ["js", "mjs", "ts", "mts", "cjs"]
    .iter()
    .any(|ext| dir.join(format!("{base_name}.{ext}")).exists())
}

fn find_config_file(dir: &Path, base_name: &str) -> Option<String> {
  ["js", "mjs", "ts", "mts", "cjs"]
    .iter()
    .find_map(|ext| {
      let file_name = format!("{base_name}.{ext}");
      dir.join(&file_name).exists().then_some(file_name)
    })
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

/// Read the `imports` keys from deno.json.
fn read_deno_json_imports(dir: &Path) -> Option<Vec<String>> {
  let content = std::fs::read_to_string(dir.join("deno.json"))
    .or_else(|_| std::fs::read_to_string(dir.join("deno.jsonc")))
    .ok()?;
  let config: serde_json::Value = serde_json::from_str(&content).ok()?;
  let imports = config.get("imports")?.as_object()?;
  Some(imports.keys().cloned().collect())
}
