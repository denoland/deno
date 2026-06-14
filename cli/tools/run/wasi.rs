// Copyright 2018-2026 the Deno authors. MIT license.

//! Prototype: run a WASI "command" `.wasm` file directly with `deno run`.
//!
//! When the main module is a local `.wasm` file that imports the WASI
//! `wasi_snapshot_preview1` (or `wasi_unstable`) namespace and exports a
//! `_start` function, we treat it as a WASI command instead of a
//! WebAssembly/ESM-integration module. We synthesize a small bootstrap ES
//! module that instantiates the wasm with the `node:wasi` import object and
//! calls `wasi.start(instance)`, then run that bootstrap module as the entry
//! point.

use deno_ast::ModuleSpecifier;
use deno_cache_dir::file_fetcher::File;
use deno_core::error::AnyError;

use crate::factory::CliFactory;

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

/// Returns the detected WASI version if `bytes` is a WASI command module, i.e.
/// it imports a WASI namespace and exports a `_start` function. Returns `None`
/// for plain WebAssembly/ESM-integration modules so they keep using the normal
/// module loader path.
fn detect_wasi_command(bytes: &[u8]) -> Option<WasiVersion> {
  use wasmparser::Payload;

  let mut version: Option<WasiVersion> = None;
  let mut has_start = false;

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
          if export.name == "_start"
            && export.kind == wasmparser::ExternalKind::Func
          {
            has_start = true;
          }
        }
      }
      // Bail out on malformed wasm; let the normal loader report the error.
      Err(_) => return None,
      _ => {}
    }
  }

  if has_start { version } else { None }
}

/// If `main_module` is a local `.wasm` WASI command, inserts a synthetic
/// bootstrap module into the file fetcher and returns its specifier so it can
/// be used as the entry point. Otherwise returns `Ok(None)`.
pub fn maybe_wasi_command_entry(
  factory: &CliFactory,
  main_module: &ModuleSpecifier,
) -> Result<Option<ModuleSpecifier>, AnyError> {
  if main_module.scheme() != "file" {
    return Ok(None);
  }
  let Ok(path) = main_module.to_file_path() else {
    return Ok(None);
  };
  if path.extension().and_then(|e| e.to_str()) != Some("wasm") {
    return Ok(None);
  }

  let bytes = std::fs::read(&path)?;
  let Some(version) = detect_wasi_command(&bytes) else {
    return Ok(None);
  };

  use base64::Engine;
  use base64::prelude::BASE64_STANDARD;
  let wasm_base64 = BASE64_STANDARD.encode(&bytes);

  // arg0 follows the convention used by Node's WASI: the program path.
  let arg0 = deno_core::serde_json::to_string(&path.to_string_lossy())?;
  let version_js = version.as_js_version();

  let source = format!(
    r#"import {{ WASI }} from "node:wasi";

const wasi = new WASI({{
  version: {version_js:?},
  args: [{arg0}, ...Deno.args],
  env: {{}},
}});

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
