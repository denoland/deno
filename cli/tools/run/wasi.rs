// Copyright 2018-2026 the Deno authors. MIT license.

//! Run a WASI "command" `.wasm` file directly with `deno run` (unstable).
//!
//! Gated behind `--unstable-wasi`. When the main module is a local `.wasm`
//! file that imports the WASI `wasi_snapshot_preview1` (or `wasi_unstable`)
//! namespace and exports a `_start` function, we treat it as a WASI command
//! instead of a WebAssembly/ESM-integration module. We synthesize a small
//! bootstrap ES module that instantiates the wasm with the `node:wasi` import
//! object and calls `wasi.start(instance)`, then run that bootstrap module as
//! the entry point.
//!
//! Only WASI preview1 "command" modules are supported. Reactor modules (those
//! exporting `_initialize` instead of `_start`) are rejected with a clear
//! error, and WASI 0.2+ Component Model binaries are left to the normal
//! loader. `deno compile` and the `--watch` path are not covered.
//!
//! Known prototype limitation: the wasm bytes are embedded into the bootstrap
//! module as base64 to avoid depending on `--allow-read` or
//! `--unstable-raw-imports`. This is wasteful for large binaries; a real
//! implementation should read the bytes through a host-level path.

use deno_ast::ModuleSpecifier;
use deno_cache_dir::file_fetcher::File;
use deno_core::error::AnyError;

use crate::args::Flags;
use crate::factory::CliFactory;

/// Name of the unstable feature flag (`--unstable-wasi`) that enables running
/// WASI command `.wasm` files directly.
const UNSTABLE_WASI_FEATURE: &str = "wasi";

/// WASI ABI version detected from a wasm module's imports.
#[derive(Clone, Copy)]
enum WasiVersion {
  Preview1,
  Unstable,
}

impl WasiVersion {
  fn as_js_version(self) -> &'static str {
    match self {
      WasiVersion::Preview1 => "preview1",
      WasiVersion::Unstable => "unstable",
    }
  }
}

/// How a WASI core module is shaped, based on its exports.
enum WasiModule {
  /// Imports a WASI namespace and exports `_start`: runnable as an entry point.
  Command(WasiVersion),
  /// Imports a WASI namespace and exports `_initialize` but not `_start`.
  /// Reactor modules have no entry point to call, so we reject them with a
  /// clear error rather than letting the normal loader fail obscurely.
  Reactor,
}

/// Classifies `bytes` as a WASI command or reactor module. Returns `None` for
/// plain WebAssembly/ESM-integration modules (and Component Model binaries) so
/// they keep using the normal module loader path.
fn detect_wasi_module(bytes: &[u8]) -> Option<WasiModule> {
  use wasmparser::Payload;

  // Cheap header short-circuit before the full parse below: only core wasm
  // modules (`\0asm` magic + version 1, layer 0) can be WASI preview1
  // modules. Component Model binaries (layer 1) and anything that isn't wasm
  // are left to the normal loader.
  if bytes.get(0..8) != Some(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00])
  {
    return None;
  }

  let mut version: Option<WasiVersion> = None;
  let mut has_start = false;
  let mut has_initialize = false;

  for payload in wasmparser::Parser::new(0).parse_all(bytes) {
    match payload {
      Ok(Payload::ImportSection(reader)) => {
        for imports in reader.into_iter().flatten() {
          let module = match imports {
            wasmparser::Imports::Single(_, import) => import.module,
            wasmparser::Imports::Compact1 { module, .. } => module,
            wasmparser::Imports::Compact2 { module, .. } => module,
          };
          match module {
            "wasi_snapshot_preview1" => {
              version.get_or_insert(WasiVersion::Preview1);
            }
            "wasi_unstable" => {
              version.get_or_insert(WasiVersion::Unstable);
            }
            _ => {}
          }
        }
      }
      Ok(Payload::ExportSection(reader)) => {
        for export in reader.into_iter().flatten() {
          if export.kind != wasmparser::ExternalKind::Func {
            continue;
          }
          match export.name {
            "_start" => has_start = true,
            "_initialize" => has_initialize = true,
            _ => {}
          }
        }
      }
      // Bail out on malformed wasm; let the normal loader report the error.
      Err(_) => return None,
      _ => {}
    }
  }

  // Not a WASI module at all: leave it to the normal loader.
  let version = version?;
  if has_start {
    Some(WasiModule::Command(version))
  } else if has_initialize {
    Some(WasiModule::Reactor)
  } else {
    None
  }
}

/// If `main_module` is a local `.wasm` WASI command, inserts a synthetic
/// bootstrap module into the file fetcher and returns its specifier so it can
/// be used as the entry point. Otherwise returns `Ok(None)`. Errors if the
/// module is a WASI reactor, which has no entry point to run.
///
/// No-op unless `--unstable-wasi` is set; gating here means the extra read and
/// parse below never run for plain WebAssembly modules in the common case.
pub fn maybe_wasi_command_entry(
  flags: &Flags,
  factory: &CliFactory,
  main_module: &ModuleSpecifier,
) -> Result<Option<ModuleSpecifier>, AnyError> {
  let unstable_enabled = flags
    .unstable_config
    .features
    .iter()
    .any(|f| f == UNSTABLE_WASI_FEATURE);
  if !unstable_enabled {
    return Ok(None);
  }
  if main_module.scheme() != "file" {
    return Ok(None);
  }
  let Ok(path) = main_module.to_file_path() else {
    return Ok(None);
  };
  if path.extension().and_then(|e| e.to_str()) != Some("wasm") {
    return Ok(None);
  }

  // If the file can't be read, fall through to the normal loader so it
  // produces Deno's canonical module-not-found error instead of a bare
  // `std::io::Error`.
  let Ok(bytes) = std::fs::read(&path) else {
    return Ok(None);
  };
  let version = match detect_wasi_module(&bytes) {
    Some(WasiModule::Command(version)) => version,
    Some(WasiModule::Reactor) => {
      return Err(deno_core::anyhow::anyhow!(
        "Cannot run WASI reactor module \"{}\" directly: it exports \
         `_initialize` but not `_start`. Only WASI command modules (those \
         exporting `_start`) can be run with `deno run`.",
        path.display(),
      ));
    }
    None => return Ok(None),
  };

  use base64::Engine;
  use base64::prelude::BASE64_STANDARD;
  let wasm_base64 = BASE64_STANDARD.encode(&bytes);

  // arg0 follows the convention used by Node's WASI: the program path.
  let arg0 = deno_core::serde_json::to_string(&path.to_string_lossy())?;
  let version_js = version.as_js_version();

  let source = format!(
    r#"import {{ WASI }} from "node:wasi";

// `node:wasi`'s native fd ops access the host filesystem directly and bypass
// Deno's permission checks, so gate the preopened cwd on `--allow-read` and
// the process environment on `--allow-env`. We query (never request) the
// permissions to avoid prompting: a WASI program only gets filesystem access
// when read of the cwd was already granted, and only sees the cwd; likewise
// for env. Partial grants (e.g. `--allow-read=/some/dir`) fall back to no
// access, which is safe if coarse.
const canRead =
  Deno.permissions.querySync({{ name: "read" }}).state === "granted";
const canEnv =
  Deno.permissions.querySync({{ name: "env" }}).state === "granted";
const cwd = canRead ? Deno.cwd() : null;

// Suppress `node:wasi`'s one-shot ExperimentalWarning on this synthesized
// entry path. It is stderr output the user did not ask for, and letting it
// interleave with the program's own stdout/stderr makes output ordering
// non-deterministic. Only suppress it while constructing the WASI instance,
// which is the only place it is emitted.
const wasi = (() => {{
  const emitWarning = process.emitWarning;
  process.emitWarning = () => {{}};
  try {{
    return new WASI({{
      version: {version_js:?},
      args: [{arg0}, ...Deno.args],
      env: canEnv ? Deno.env.toObject() : {{}},
      preopens: cwd === null ? {{}} : {{ "/": cwd, ".": cwd }},
    }});
  }} finally {{
    process.emitWarning = emitWarning;
  }}
}})();

const base64 = "{wasm_base64}";
const binary = atob(base64);
const bytes = new Uint8Array(binary.length);
for (let i = 0; i < binary.length; i++) {{
  bytes[i] = binary.charCodeAt(i);
}}

const module = await WebAssembly.compile(bytes);
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
Deno.exit(wasi.start(instance));
"#,
  );

  // Synthetic bootstrap specifier; `.mjs` so it's treated as an ES module.
  let bootstrap_specifier =
    ModuleSpecifier::parse(&format!("{main_module}.wasi-bootstrap.mjs"))?;

  let file_fetcher = factory.file_fetcher()?;
  file_fetcher.insert_memory_files(File {
    url: bootstrap_specifier.clone(),
    mtime: None,
    maybe_headers: None,
    source: source.into_bytes().into(),
    loaded_from: deno_cache_dir::file_fetcher::LoadedFrom::Local,
  });

  Ok(Some(bootstrap_specifier))
}
