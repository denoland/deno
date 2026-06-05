// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;

use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use capacity_builder::StringBuilder;
use indexmap::IndexMap;

/// Render a Wasm module as a self-contained JavaScript module that, when run,
/// instantiates the Wasm module and re-exports its exports.
///
/// Deno natively treats a `.wasm` import as an ES module whose exports are the
/// Wasm instance's exports (see `render_js_wasm_module` in `deno_core`). esbuild
/// has no such loader, so for `deno bundle` we generate equivalent JavaScript
/// that:
///
/// 1. imports the Wasm module's imports from their respective specifiers (these
///    get resolved/bundled by esbuild like any other import),
/// 2. inlines the Wasm bytes as base64 and compiles + instantiates them
///    synchronously, and
/// 3. re-exports the instance's exports as named (and possibly default) exports.
///
/// Tradeoffs of keeping the bundle self-contained: compilation happens
/// synchronously at module-eval time (which blocks the thread for that module)
/// and the base64 inline grows the output by ~1.33x. Async compilation isn't an
/// option here because esbuild can't consume the source-phase
/// `import source ... from` / `import.meta.WasmInstance` form Deno uses natively.
pub fn render_js_wasm_module(
  bytes: &[u8],
) -> Result<String, wasm_dep_analyzer::ParseError> {
  let wasm_deps = wasm_dep_analyzer::WasmDeps::parse(
    bytes,
    wasm_dep_analyzer::ParseOptions { skip_types: true },
  )?;

  struct ImportInfo {
    key_escaped: String,
    escaped_named_imports: Vec<String>,
  }

  let mut aggregated_imports: IndexMap<&str, ImportInfo> =
    IndexMap::with_capacity(wasm_deps.imports.len());
  for import in &wasm_deps.imports {
    let entry =
      aggregated_imports
        .entry(import.module)
        .or_insert_with(|| ImportInfo {
          key_escaped: import.module.escape_default().to_string(),
          escaped_named_imports: Vec::new(),
        });
    entry
      .escaped_named_imports
      .push(import.name.escape_default().to_string());
  }

  let escaped_export_names = wasm_deps
    .exports
    .iter()
    .map(|e| {
      if e.name == "default" {
        Cow::Borrowed(e.name)
      } else {
        Cow::Owned(e.name.escape_default().to_string())
      }
    })
    .collect::<Vec<_>>();

  let base64_bytes = BASE64_STANDARD.encode(bytes);

  Ok(
    StringBuilder::build(|builder| {
      for (i, (_, import_info)) in aggregated_imports.iter().enumerate() {
        builder.append("import { ");
        for (name_index, named_import) in
          import_info.escaped_named_imports.iter().enumerate()
        {
          if name_index > 0 {
            builder.append(", ");
          }
          builder.append('"');
          builder.append(named_import);
          builder.append("\" as __deno_wasm_import_");
          builder.append(i);
          builder.append('_');
          builder.append(name_index);
          builder.append("__");
        }
        builder.append(" } from \"");
        builder.append(&import_info.key_escaped);
        builder.append("\";\n");
      }

      builder.append(
        "const __deno_wasm_bytes__ = Uint8Array.from(atob(\"",
      );
      builder.append(&base64_bytes);
      builder.append("\"), (c) => c.charCodeAt(0));\n");

      if aggregated_imports.is_empty() {
        builder.append(
          "const __deno_wasm_instance__ = new WebAssembly.Instance(new WebAssembly.Module(__deno_wasm_bytes__)).exports;\n",
        );
      } else {
        builder.append("const __deno_wasm_imports__ = {\n");
        for (i, (_, import_info)) in aggregated_imports.iter().enumerate() {
          builder.append("  \"");
          builder.append(&import_info.key_escaped);
          builder.append("\": {\n");
          for (name_index, named_import) in
            import_info.escaped_named_imports.iter().enumerate()
          {
            builder.append("    \"");
            builder.append(named_import);
            builder.append("\": __deno_wasm_import_");
            builder.append(i);
            builder.append('_');
            builder.append(name_index);
            builder.append("__,\n");
          }
          builder.append("  },\n");
        }
        builder.append("};\n");
        builder.append(
          "const __deno_wasm_instance__ = new WebAssembly.Instance(new WebAssembly.Module(__deno_wasm_bytes__), __deno_wasm_imports__).exports;\n",
        );
      }

      for (idx, escaped_name) in escaped_export_names.iter().enumerate() {
        if escaped_name == "default" {
          builder.append(
            "export default __deno_wasm_instance__.default;\n",
          );
        } else {
          builder.append("const __deno_wasm_export_");
          builder.append(idx);
          builder.append("__ = __deno_wasm_instance__[\"");
          builder.append(escaped_name.as_ref());
          builder.append("\"];\nexport { __deno_wasm_export_");
          builder.append(idx);
          builder.append("__ as \"");
          builder.append(escaped_name.as_ref());
          builder.append("\" };\n");
        }
      }
    })
    .unwrap(),
  )
}

#[cfg(test)]
mod test {
  use super::*;

  // A minimal valid Wasm module: `(module (func (export "add") (param i32 i32)
  // (result i32) local.get 0 local.get 1 i32.add))`.
  const ADD_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60,
    0x02, 0x7f, 0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01,
    0x03, 0x61, 0x64, 0x64, 0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x20,
    0x00, 0x20, 0x01, 0x6a, 0x0b,
  ];

  // A minimal Wasm module that imports `addOne` from `./dep.js` and exports
  // `callImport`: `(module (import "./dep.js" "addOne" (func (param i32)
  // (result i32))) (func (export "callImport") (param i32) (result i32)
  // local.get 0 call 0))`.
  const CALC_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60,
    0x01, 0x7f, 0x01, 0x7f, 0x02, 0x13, 0x01, 0x08, 0x2e, 0x2f, 0x64, 0x65,
    0x70, 0x2e, 0x6a, 0x73, 0x06, 0x61, 0x64, 0x64, 0x4f, 0x6e, 0x65, 0x00,
    0x00, 0x03, 0x02, 0x01, 0x00, 0x07, 0x0e, 0x01, 0x0a, 0x63, 0x61, 0x6c,
    0x6c, 0x49, 0x6d, 0x70, 0x6f, 0x72, 0x74, 0x00, 0x01, 0x0a, 0x08, 0x01,
    0x06, 0x00, 0x20, 0x00, 0x10, 0x00, 0x0b,
  ];

  #[test]
  fn renders_exports_without_imports() {
    let rendered = render_js_wasm_module(ADD_WASM).unwrap();
    assert!(
      rendered.contains("const __deno_wasm_bytes__ = Uint8Array.from(atob(")
    );
    assert!(rendered.contains(
      "new WebAssembly.Instance(new WebAssembly.Module(__deno_wasm_bytes__)).exports"
    ));
    assert!(rendered.contains("__deno_wasm_instance__[\"add\"]"));
    assert!(rendered.contains("as \"add\""));
    assert!(!rendered.contains("__deno_wasm_imports__"));
  }

  #[test]
  fn renders_imports_object_and_named_imports() {
    let rendered = render_js_wasm_module(CALC_WASM).unwrap();
    // The wasm import is emitted as an ES import esbuild can resolve.
    assert!(rendered.contains(
      "import { \"addOne\" as __deno_wasm_import_0_0__ } from \"./dep.js\";"
    ));
    // ...and forwarded into the imports object passed to `WebAssembly.Instance`.
    assert!(rendered.contains("const __deno_wasm_imports__ = {"));
    assert!(rendered.contains("\"./dep.js\": {"));
    assert!(rendered.contains("\"addOne\": __deno_wasm_import_0_0__,"));
    assert!(rendered.contains(
      "new WebAssembly.Instance(new WebAssembly.Module(__deno_wasm_bytes__), __deno_wasm_imports__).exports"
    ));
    // The export is re-exported.
    assert!(rendered.contains("__deno_wasm_instance__[\"callImport\"]"));
    assert!(rendered.contains("as \"callImport\""));
  }
}
