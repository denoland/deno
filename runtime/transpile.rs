// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;

use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceMapOption;
use deno_core::ModuleCodeString;
use deno_core::ModuleName;
use deno_core::SourceMapData;
use deno_error::JsErrorBox;

deno_error::js_error_wrapper!(
  deno_ast::ParseDiagnostic,
  JsParseDiagnostic,
  "Error"
);
deno_error::js_error_wrapper!(
  deno_ast::TranspileError,
  JsTranspileError,
  "Error"
);

// Transpile cache (v82jsc startup acceleration).
//
// Without a V8 startup snapshot (the quickjs backend boots via InitMode::New),
// deno transpiles every TypeScript extension source with swc on *every* boot —
// ~140 ms, the dominant startup cost. Transpilation is deterministic, so we
// persist the emitted JS keyed by a content hash and reuse it on later boots,
// turning that ~140 ms into a handful of file reads. Disable with
// `V82JSC_NO_TRANSPILE_CACHE=1`.
const TRANSPILE_CACHE_VERSION: u32 = 1;

fn transpile_cache_dir() -> Option<&'static std::path::PathBuf> {
  static DIR: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
  DIR
    .get_or_init(|| {
      if std::env::var_os("V82JSC_NO_TRANSPILE_CACHE").is_some() {
        return None;
      }
      let base = std::env::var_os("DENO_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| {
          std::env::var_os("HOME")
            .map(|h| std::path::PathBuf::from(h).join("Library/Caches"))
        })
        .unwrap_or_else(std::env::temp_dir);
      let d = base.join("v82jsc_transpile");
      std::fs::create_dir_all(&d).ok()?;
      Some(d)
    })
    .as_ref()
}

fn transpile_cache_key(name: &str, source: &str) -> u64 {
  use std::hash::{Hash, Hasher};
  let mut h = std::collections::hash_map::DefaultHasher::new();
  TRANSPILE_CACHE_VERSION.hash(&mut h);
  name.hash(&mut h);
  source.len().hash(&mut h);
  source.hash(&mut h);
  h.finish()
}

fn transpile_cache_load(key: u64) -> Option<String> {
  let p = transpile_cache_dir()?.join(format!("{key:016x}.js"));
  std::fs::read_to_string(p).ok().filter(|s| !s.is_empty())
}

fn transpile_cache_store(key: u64, code: &str) {
  if code.is_empty() {
    return;
  }
  if let Some(dir) = transpile_cache_dir() {
    let p = dir.join(format!("{key:016x}.js"));
    let tmp = dir.join(format!("{key:016x}.tmp"));
    if std::fs::write(&tmp, code).is_ok() {
      let _ = std::fs::rename(&tmp, &p);
    }
  }
}

pub fn maybe_transpile_source(
  name: ModuleName,
  source: ModuleCodeString,
) -> Result<(ModuleCodeString, Option<SourceMapData>), JsErrorBox> {
  maybe_transpile_source_inner(name, source, false)
}

pub fn maybe_transpile_and_minify_source(
  name: ModuleName,
  source: ModuleCodeString,
) -> Result<(ModuleCodeString, Option<SourceMapData>), JsErrorBox> {
  maybe_transpile_source_inner(name, source, true)
}

/// The `node:process` polyfill carries a `__NODE_VERSION__` placeholder so the
/// reported `process.version` / `process.versions.node` stays bound to the
/// single source of truth in `deno_node::NODE_VERSION` instead of duplicating
/// the literal. Substitute it here, at snapshot build time, so the value is
/// baked into the snapshot and no runtime work is needed.
fn maybe_substitute_node_version(
  name: &str,
  source: ModuleCodeString,
) -> ModuleCodeString {
  const TOKEN: &str = "__NODE_VERSION__";
  if !name.ends_with("deno_node/_process/process.ts") {
    return source;
  }
  debug_assert!(
    source.as_str().contains(TOKEN),
    "expected `{TOKEN}` placeholder in {name}"
  );
  source
    .as_str()
    .replace(TOKEN, deno_node::NODE_VERSION)
    .into()
}

fn maybe_transpile_source_inner(
  name: ModuleName,
  source: ModuleCodeString,
  minify: bool,
) -> Result<(ModuleCodeString, Option<SourceMapData>), JsErrorBox> {
  let name_string = name.to_string();
  let source = maybe_substitute_node_version(&name_string, source);
  // Always transpile `node:` built-in modules, since they might be TypeScript.
  let media_type = if name.starts_with("node:") {
    MediaType::TypeScript
  } else {
    MediaType::from_path(Path::new(&name))
  };

  match media_type {
    MediaType::TypeScript => {}
    MediaType::JavaScript | MediaType::Mjs => {
      if minify {
        let source =
          minify_source_with_rolldown(&name_string, source.as_ref())?;
        return Ok((source.into(), None));
      }
      return Ok((source, None));
    }
    _ => panic!(
      "Unsupported media type for snapshotting {media_type:?} for file {}",
      name
    ),
  }

  // Bytecode-of-transpile cache: deterministic TS->JS emit, keyed by content.
  // Only the runtime (non-minify) path is cached; the snapshot-build minify path
  // is a one-off. Cached entries carry no source map (release emits none).
  let cache_key = transpile_cache_key(&name_string, source.as_str());
  if !minify {
    if let Some(code) = transpile_cache_load(cache_key) {
      return Ok((code.into(), None));
    }
  }

  let parsed = deno_ast::parse_module(ParseParams {
    specifier: deno_core::url::Url::parse(&name).unwrap(),
    text: source.into(),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })
  .map_err(|e| JsErrorBox::from_err(JsParseDiagnostic(e)))?;
  let transpiled_source = parsed
    .transpile(
      &deno_ast::TranspileOptions {
        imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
        ..Default::default()
      },
      &deno_ast::TranspileModuleOptions::default(),
      &deno_ast::EmitOptions {
        source_map: if cfg!(debug_assertions) {
          SourceMapOption::Separate
        } else {
          SourceMapOption::None
        },
        ..Default::default()
      },
    )
    .map_err(|e| JsErrorBox::from_err(JsTranspileError(e)))?
    .into_source();

  let maybe_source_map: Option<SourceMapData> = transpiled_source
    .source_map
    .map(|sm| sm.into_bytes().into());
  let source_text = transpiled_source.text;
  if minify {
    let source_text = minify_source_with_rolldown(&name_string, &source_text)?;
    Ok((source_text.into(), None))
  } else {
    // Persist the emitted JS for the next boot when there's no separate source
    // map to carry (release builds emit none).
    if maybe_source_map.is_none() {
      transpile_cache_store(cache_key, source_text.as_str());
    }
    Ok((source_text.into(), maybe_source_map))
  }
}

#[allow(
  clippy::disallowed_methods,
  reason = "snapshot source minification runs at build time"
)]
fn minify_source_with_rolldown(
  specifier: &str,
  source: &str,
) -> Result<String, JsErrorBox> {
  static MINIFIER_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
  let minifier_path = MINIFIER_PATH.get_or_init(|| {
    let path =
      std::env::temp_dir().join("deno_snapshot_rolldown_minify_source.js");
    std::fs::write(
      &path,
      r#"import { minifySync } from "npm:rolldown/experimental";

const filename = Deno.args[0] ?? "source.js";
const source = await new Response(Deno.stdin.readable).text();
const result = minifySync(filename, source, {
  compress: false,
  mangle: false,
  codegen: {
    removeWhitespace: true,
    legalComments: "none",
  },
});
if (result.errors?.length) {
  console.error(JSON.stringify(result.errors, null, 2));
  Deno.exit(1);
}
function escapeNonAscii(code) {
  let out = "";
  for (let i = 0; i < code.length; i++) {
    const unit = code.charCodeAt(i);
    if (unit <= 0x7f) {
      out += code[i];
    } else {
      out += "\\u" + unit.toString(16).padStart(4, "0");
    }
  }
  return out;
}
const output = new TextEncoder().encode(escapeNonAscii(result.code));
let offset = 0;
while (offset < output.length) {
  offset += await Deno.stdout.write(output.subarray(offset));
}
"#,
    )
    .unwrap();
    path
  });

  let mut child = std::process::Command::new("deno")
    .arg("run")
    .arg("-A")
    .arg(minifier_path)
    .arg(specifier)
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .map_err(|e| {
      JsErrorBox::generic(format!(
        "failed to run rolldown source minifier for {specifier}: {e}"
      ))
    })?;
  child
    .stdin
    .as_mut()
    .unwrap()
    .write_all(source.as_bytes())
    .map_err(|e| {
      JsErrorBox::generic(format!(
        "failed to write source to rolldown minifier for {specifier}: {e}"
      ))
    })?;
  let output = child.wait_with_output().map_err(|e| {
    JsErrorBox::generic(format!(
      "failed to wait for rolldown source minifier for {specifier}: {e}"
    ))
  })?;
  if !output.status.success() {
    return Err(JsErrorBox::generic(format!(
      "failed to minify source {specifier}\nstdout:\n{}\nstderr:\n{}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr),
    )));
  }
  let output = String::from_utf8(output.stdout).map_err(|e| {
    JsErrorBox::generic(format!(
      "rolldown minifier produced non-utf8 output for {specifier}: {e}"
    ))
  })?;
  assert!(
    output.is_ascii(),
    "rolldown minifier produced non-ascii output for {specifier}"
  );
  Ok(output)
}
