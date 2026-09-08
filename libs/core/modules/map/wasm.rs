// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;

use indexmap::IndexMap;
use wasm_dep_analyzer::WasmDeps;

use super::ModuleMap;
use crate::ModuleSourceCode;
use crate::modules::ModuleConcreteError;
use crate::modules::ModuleError;
use crate::modules::ModuleId;
use crate::modules::ModuleName;
use crate::modules::ModuleReference;
use crate::modules::ModuleSource;
use crate::modules::ModuleType;
use crate::modules::module_map_data::ModuleSourceKey;

impl ModuleMap {
  pub(crate) fn new_wasm_module_source(
    &self,
    scope: &mut v8::PinScope,
    module_reference: &ModuleReference,
    mut loaded_source: ModuleSource,
  ) -> Result<ModuleSource, ModuleError> {
    if let Some(module_url_found) = loaded_source.cheap_copy_module_url_found()
    {
      self.data.borrow_mut().alias(
        loaded_source.cheap_copy_module_url_specified(),
        &loaded_source.module_type.clone().into(),
        module_url_found,
      );
    }
    let reference_key = ModuleSourceKey::from_reference(module_reference);
    if self.data.borrow().sources.contains_key(&reference_key) {
      return Ok(loaded_source);
    }
    let loaded_key = ModuleSourceKey::from_loaded_source(&mut loaded_source);
    if let Some(source) = self.data.borrow().sources.get(&loaded_key).cloned() {
      self.data.borrow_mut().sources.insert(reference_key, source);
      return Ok(loaded_source);
    }

    let ModuleSourceCode::Bytes(code) = &loaded_source.code else {
      return Err(ModuleError::Concrete(ModuleConcreteError::WasmNotBytes));
    };
    let Some(wasm_module) =
      v8::WasmModuleObject::compile(scope, code.as_bytes())
    else {
      return Err(
        ModuleConcreteError::WasmCompile(loaded_key.name.to_string()).into(),
      );
    };
    let wasm_module_object: v8::Local<v8::Object> = wasm_module.into();
    let source = v8::Global::new(scope, wasm_module_object);
    {
      let mut data = self.data.borrow_mut();
      data.sources.insert(reference_key, source.clone());
      data.sources.insert(loaded_key, source);
    }
    Ok(loaded_source)
  }

  pub(crate) fn new_wasm_module(
    &self,
    scope: &mut v8::PinScope,
    name: ModuleName,
    source: ModuleSourceCode,
    is_dynamic_import: bool,
  ) -> Result<ModuleId, ModuleError> {
    let bytes = source.as_bytes();
    let wasm_module_analysis = WasmDeps::parse(
      bytes,
      wasm_dep_analyzer::ParseOptions { skip_types: true },
    )
    .map_err(ModuleConcreteError::WasmParse)?;

    let js_wasm_module_source =
      render_js_wasm_module(name.as_str(), wasm_module_analysis);

    self.new_module_from_js_source(
      scope,
      false,
      ModuleType::Wasm,
      name,
      js_wasm_module_source.into(),
      is_dynamic_import,
      None,
    )
  }
}

/// Helper injected into the synthetic module for `.wasm` files that have global
/// exports. Per the Wasm ESM integration, a global export is unwrapped to its
/// underlying JS value (e.g. an `i32` global exports the number directly)
/// instead of being exposed as a `WebAssembly.Global` object, matching Node.js.
/// Reading `.value` throws for `v128` globals, so we fall back to the
/// `WebAssembly.Global` object in that case. The value is read once at
/// instantiation (a snapshot), so a later mutation of a mutable global is not
/// reflected in the export, which also matches Node. Wasm modules importing
/// the global are not affected by the snapshot: they link against the
/// original `WebAssembly.Global` via `import.meta.wasmInstances`.
const WASM_GLOBAL_UNWRAP_HELPER: &str = "const unwrapWasmGlobal = (g) => { try { return g.value; } catch { return g; } };\n";

/// Whether a Wasm export is a global. The export kind is read directly from the
/// export section, so this is reliable even though we parse the module with
/// `skip_types: true` and never resolve the global's value type.
fn is_wasm_global_export(export_type: &wasm_dep_analyzer::ExportType) -> bool {
  matches!(export_type, wasm_dep_analyzer::ExportType::Global(_))
}

/// Whether a Wasm import is a global. Like [`is_wasm_global_export`], the import
/// kind is read directly from the import section, independent of the global's
/// value type.
fn is_wasm_global_import(import_type: &wasm_dep_analyzer::ImportType) -> bool {
  matches!(import_type, wasm_dep_analyzer::ImportType::Global(_))
}

/// Values that [`StringBuilder::append`] knows how to write.
trait AppendToString {
  fn append_to(self, out: &mut String);
}

impl AppendToString for &str {
  fn append_to(self, out: &mut String) {
    out.push_str(self);
  }
}

impl AppendToString for &String {
  fn append_to(self, out: &mut String) {
    out.push_str(self);
  }
}

impl AppendToString for &Cow<'_, str> {
  fn append_to(self, out: &mut String) {
    out.push_str(self);
  }
}

impl AppendToString for char {
  fn append_to(self, out: &mut String) {
    out.push(self);
  }
}

impl AppendToString for usize {
  fn append_to(self, out: &mut String) {
    // Cold path (module rendering), so the temporary allocation is fine.
    out.push_str(&self.to_string());
  }
}

/// Append-only string builder over a preallocated [`String`].
///
/// Just enough of an API to render the synthetic Wasm wrapper module below
/// without depending on a proc-macro string-building crate.
struct StringBuilder(String);

impl StringBuilder {
  fn with_capacity(capacity: usize) -> Self {
    Self(String::with_capacity(capacity))
  }

  fn append(&mut self, value: impl AppendToString) {
    value.append_to(&mut self.0);
  }

  fn build(self) -> String {
    self.0
  }
}

fn render_js_wasm_module(specifier: &str, wasm_deps: WasmDeps) -> String {
  struct NamedImport {
    escaped_name: String,
    is_global: bool,
  }

  struct ImportInfo {
    key_escaped: String,
    named_imports: Vec<NamedImport>,
    has_global_import: bool,
  }

  fn aggregate_wasm_module_imports<'a>(
    imports: &'a [wasm_dep_analyzer::Import],
  ) -> IndexMap<&'a str, ImportInfo> {
    let mut imports_map = IndexMap::with_capacity(imports.len());

    for import in imports {
      let entry =
        imports_map
          .entry(import.module)
          .or_insert_with(|| ImportInfo {
            key_escaped: import.module.escape_default().to_string(),
            named_imports: Vec::new(),
            has_global_import: false,
          });
      let is_global = is_wasm_global_import(&import.import_type);
      entry.has_global_import |= is_global;
      entry.named_imports.push(NamedImport {
        escaped_name: import.name.escape_default().to_string(),
        is_global,
      });
    }

    imports_map
  }

  let aggregated_imports = aggregate_wasm_module_imports(&wasm_deps.imports);
  let exports = wasm_deps
    .exports
    .iter()
    .map(|e| {
      let escaped_name = if e.name == "default" {
        Cow::Borrowed(e.name)
      } else {
        Cow::Owned(e.name.escape_default().to_string())
      };
      (escaped_name, is_wasm_global_export(&e.export_type))
    })
    .collect::<Vec<_>>();
  let has_global_export = exports.iter().any(|(_, is_global)| *is_global);

  // Rough starting capacity; the builder grows as needed.
  let mut builder = StringBuilder::with_capacity(
    256 + 128 * (aggregated_imports.len() + exports.len()),
  );
  {
    let builder = &mut builder;
    builder.append("import source wasmMod from \"");
    builder.append(specifier);
    builder.append("\";\n");

    // A module with global exports registers its instance exports under its
    // own namespace in `import.meta.wasmInstances`, so that an importing Wasm
    // module can link against the original `WebAssembly.Global` objects.
    if has_global_export {
      builder.append("import * as selfNs from \"");
      builder.append(specifier);
      builder.append("\";\n");
    }

    if !aggregated_imports.is_empty() {
      for (i, (_, import_info)) in aggregated_imports.iter().enumerate() {
        if import_info.has_global_import {
          builder.append("import * as import_ns_");
          builder.append(i);
          builder.append(" from \"");
          builder.append(&import_info.key_escaped);
          builder.append("\";\n");
        }
        builder.append("import { ");
        for (name_index, named_import) in
          import_info.named_imports.iter().enumerate()
        {
          if name_index > 0 {
            builder.append(", ");
          }
          builder.append('"');
          builder.append(&named_import.escaped_name);
          builder.append("\" as import_");
          builder.append(i);
          builder.append('_');
          builder.append(name_index);
        }
        builder.append(" } from \"");
        builder.append(&import_info.key_escaped);
        builder.append("\";\n");
      }

      // For global-typed imports, prefer the original `WebAssembly.Global`
      // from the dependency's instance when the dependency is itself a Wasm
      // module, so that mutable globals stay direct references between Wasm
      // modules. The JS binding only carries the unwrapped snapshot value.
      //
      // Limitation: for a circular Wasm<->Wasm mutable-global import the
      // dependency may not be evaluated yet when this `.get()` runs, so it
      // returns `undefined` and we fall back to the snapshot number, which
      // fails instantiation with a `LinkError`. Node.js has the same gap.
      for (i, (_, import_info)) in aggregated_imports.iter().enumerate() {
        if import_info.has_global_import {
          builder.append("const wasmExports_");
          builder.append(i);
          builder.append(" = import.meta.wasmInstances.get(import_ns_");
          builder.append(i);
          builder.append(");\n");
        }
      }

      builder.append("const importsObject = {\n");

      for (i, (_, import_info)) in aggregated_imports.iter().enumerate() {
        builder.append("  \"");
        builder.append(&import_info.key_escaped);
        builder.append("\": {\n");

        for (name_index, named_import) in
          import_info.named_imports.iter().enumerate()
        {
          builder.append("    \"");
          builder.append(&named_import.escaped_name);
          builder.append("\": ");
          if named_import.is_global {
            builder.append("wasmExports_");
            builder.append(i);
            builder.append(" === undefined ? import_");
            builder.append(i);
            builder.append('_');
            builder.append(name_index);
            builder.append(" : wasmExports_");
            builder.append(i);
            builder.append("[\"");
            builder.append(&named_import.escaped_name);
            builder.append("\"]");
          } else {
            builder.append("import_");
            builder.append(i);
            builder.append('_');
            builder.append(name_index);
          }
          builder.append(",\n");
        }

        builder.append("  },\n");
      }

      builder.append("};\n");

      builder.append("const modInstance = new import.meta.WasmInstance(wasmMod, importsObject);\n");
    } else {
      builder
        .append("const modInstance = new import.meta.WasmInstance(wasmMod);\n");
    }

    if has_global_export {
      // The generated source assumes `import.meta.wasmInstances` is present
      // whenever a module uses globals. The map shares `import.meta.WasmInstance`'s
      // lifecycle: both are absent only when WebAssembly is unavailable or during
      // snapshotting, where the `new import.meta.WasmInstance(...)` call above
      // would already have thrown. So if we reach here, the map exists.
      builder.append(
        "import.meta.wasmInstances.set(selfNs, modInstance.exports);\n",
      );
      builder.append(WASM_GLOBAL_UNWRAP_HELPER);
    }

    for (idx, (escaped_name, is_global)) in exports.iter().enumerate() {
      if escaped_name == "default" {
        builder.append("export default ");
        if *is_global {
          builder.append("unwrapWasmGlobal(modInstance.exports.");
          builder.append(escaped_name);
          builder.append(")");
        } else {
          builder.append("modInstance.exports.");
          builder.append(escaped_name);
        }
        builder.append(";\n");
      } else {
        builder.append("const export");
        builder.append(idx);
        builder.append(" = ");
        if *is_global {
          builder.append("unwrapWasmGlobal(modInstance.exports[\"");
          builder.append(escaped_name);
          builder.append("\"])");
        } else {
          builder.append("modInstance.exports[\"");
          builder.append(escaped_name);
          builder.append("\"]");
        }
        builder.append(";\nexport { export");
        builder.append(idx);
        builder.append(" as \"");
        builder.append(escaped_name);
        builder.append("\" };\n");
      }
    }
  }
  builder.build()
}

#[test]
fn test_render_js_wasm_module() {
  let deps = WasmDeps {
    imports: vec![],
    exports: vec![],
  };
  let rendered = render_js_wasm_module("./foo.wasm", deps);
  pretty_assertions::assert_eq!(
    rendered,
    r#"import source wasmMod from "./foo.wasm";
const modInstance = new import.meta.WasmInstance(wasmMod);
"#,
  );

  let deps = WasmDeps {
    imports: vec![
      wasm_dep_analyzer::Import {
        name: "foo",
        module: "./import.js",
        import_type: wasm_dep_analyzer::ImportType::Tag(
          wasm_dep_analyzer::TagType {
            kind: 1,
            type_index: 1,
          },
        ),
      },
      wasm_dep_analyzer::Import {
        name: "bar",
        module: "./import.js",
        import_type: wasm_dep_analyzer::ImportType::Function(1),
      },
      wasm_dep_analyzer::Import {
        name: "fizz",
        module: "./import.js",
        import_type: wasm_dep_analyzer::ImportType::Function(2),
      },
      wasm_dep_analyzer::Import {
        name: "buzz",
        module: "./buzz.js",
        import_type: wasm_dep_analyzer::ImportType::Function(3),
      },
    ],
    exports: vec![
      wasm_dep_analyzer::Export {
        name: "export1",
        index: 0,
        export_type: wasm_dep_analyzer::ExportType::Function(Ok(
          wasm_dep_analyzer::FunctionSignature {
            params: vec![],
            returns: vec![],
          },
        )),
      },
      wasm_dep_analyzer::Export {
        name: "export2",
        index: 1,
        export_type: wasm_dep_analyzer::ExportType::Table,
      },
      wasm_dep_analyzer::Export {
        name: "export3",
        index: 2,
        export_type: wasm_dep_analyzer::ExportType::Memory,
      },
      wasm_dep_analyzer::Export {
        name: "export4",
        index: 3,
        export_type: wasm_dep_analyzer::ExportType::Global(Ok(
          wasm_dep_analyzer::GlobalType {
            value_type: wasm_dep_analyzer::ValueType::F32,
            mutability: false,
          },
        )),
      },
      wasm_dep_analyzer::Export {
        name: "export5",
        index: 4,
        export_type: wasm_dep_analyzer::ExportType::Tag,
      },
      wasm_dep_analyzer::Export {
        name: "export6",
        index: 5,
        export_type: wasm_dep_analyzer::ExportType::Unknown,
      },
      wasm_dep_analyzer::Export {
        name: "default",
        index: 6,
        export_type: wasm_dep_analyzer::ExportType::Function(Ok(
          wasm_dep_analyzer::FunctionSignature {
            params: vec![],
            returns: vec![],
          },
        )),
      },
    ],
  };
  let rendered = render_js_wasm_module("./foo.wasm", deps);
  pretty_assertions::assert_eq!(
    rendered,
    r#"import source wasmMod from "./foo.wasm";
import * as selfNs from "./foo.wasm";
import { "foo" as import_0_0, "bar" as import_0_1, "fizz" as import_0_2 } from "./import.js";
import { "buzz" as import_1_0 } from "./buzz.js";
const importsObject = {
  "./import.js": {
    "foo": import_0_0,
    "bar": import_0_1,
    "fizz": import_0_2,
  },
  "./buzz.js": {
    "buzz": import_1_0,
  },
};
const modInstance = new import.meta.WasmInstance(wasmMod, importsObject);
import.meta.wasmInstances.set(selfNs, modInstance.exports);
const unwrapWasmGlobal = (g) => { try { return g.value; } catch { return g; } };
const export0 = modInstance.exports["export1"];
export { export0 as "export1" };
const export1 = modInstance.exports["export2"];
export { export1 as "export2" };
const export2 = modInstance.exports["export3"];
export { export2 as "export3" };
const export3 = unwrapWasmGlobal(modInstance.exports["export4"]);
export { export3 as "export4" };
const export4 = modInstance.exports["export5"];
export { export4 as "export5" };
const export5 = modInstance.exports["export6"];
export { export5 as "export6" };
export default modInstance.exports.default;
"#,
  );

  let deps = WasmDeps {
    imports: vec![wasm_dep_analyzer::Import {
      name: "\n",
      module: "\n",
      import_type: wasm_dep_analyzer::ImportType::Function(1),
    }],
    exports: vec![wasm_dep_analyzer::Export {
      name: "\n",
      index: 0,
      export_type: wasm_dep_analyzer::ExportType::Function(Ok(
        wasm_dep_analyzer::FunctionSignature {
          params: vec![],
          returns: vec![],
        },
      )),
    }],
  };
  let rendered = render_js_wasm_module("./bar.wasm", deps);
  pretty_assertions::assert_eq!(
    rendered,
    r#"import source wasmMod from "./bar.wasm";
import { "\n" as import_0_0 } from "\n";
const importsObject = {
  "\n": {
    "\n": import_0_0,
  },
};
const modInstance = new import.meta.WasmInstance(wasmMod, importsObject);
const export0 = modInstance.exports["\n"];
export { export0 as "\n" };
"#,
  );
}

#[test]
fn test_render_js_wasm_module_global_unwrap() {
  fn global(
    value_type: wasm_dep_analyzer::ValueType,
    mutability: bool,
  ) -> wasm_dep_analyzer::ExportType {
    wasm_dep_analyzer::ExportType::Global(Ok(wasm_dep_analyzer::GlobalType {
      value_type,
      mutability,
    }))
  }

  let deps = WasmDeps {
    imports: vec![],
    exports: vec![
      // immutable numeric global -> unwrapped to its value at runtime
      wasm_dep_analyzer::Export {
        name: "answer",
        index: 0,
        export_type: global(wasm_dep_analyzer::ValueType::I32, false),
      },
      // mutable numeric global -> still unwrapped (snapshot at instantiation)
      wasm_dep_analyzer::Export {
        name: "counter",
        index: 1,
        export_type: global(wasm_dep_analyzer::ValueType::I64, true),
      },
      // unresolved value type (e.g. v128 / reference type) is still a global
      // by kind, so it is wrapped; the helper falls back to the
      // WebAssembly.Global object if reading `.value` throws (v128).
      wasm_dep_analyzer::Export {
        name: "vec",
        index: 2,
        export_type: global(wasm_dep_analyzer::ValueType::Unknown, false),
      },
      // global whose value type failed to parse -> still wrapped by kind
      wasm_dep_analyzer::Export {
        name: "broken",
        index: 3,
        export_type: wasm_dep_analyzer::ExportType::Global(Err(
          wasm_dep_analyzer::ParseError::UnresolvedExportType,
        )),
      },
      // non-global export is left untouched
      wasm_dep_analyzer::Export {
        name: "fn_export",
        index: 4,
        export_type: wasm_dep_analyzer::ExportType::Function(Ok(
          wasm_dep_analyzer::FunctionSignature {
            params: vec![],
            returns: vec![],
          },
        )),
      },
      // default export that is a global -> unwrapped
      wasm_dep_analyzer::Export {
        name: "default",
        index: 5,
        export_type: global(wasm_dep_analyzer::ValueType::F64, false),
      },
    ],
  };
  let rendered = render_js_wasm_module("./globals.wasm", deps);
  pretty_assertions::assert_eq!(
    rendered,
    r#"import source wasmMod from "./globals.wasm";
import * as selfNs from "./globals.wasm";
const modInstance = new import.meta.WasmInstance(wasmMod);
import.meta.wasmInstances.set(selfNs, modInstance.exports);
const unwrapWasmGlobal = (g) => { try { return g.value; } catch { return g; } };
const export0 = unwrapWasmGlobal(modInstance.exports["answer"]);
export { export0 as "answer" };
const export1 = unwrapWasmGlobal(modInstance.exports["counter"]);
export { export1 as "counter" };
const export2 = unwrapWasmGlobal(modInstance.exports["vec"]);
export { export2 as "vec" };
const export3 = unwrapWasmGlobal(modInstance.exports["broken"]);
export { export3 as "broken" };
const export4 = modInstance.exports["fn_export"];
export { export4 as "fn_export" };
export default unwrapWasmGlobal(modInstance.exports.default);
"#,
  );
}

#[test]
fn test_render_js_wasm_module_global_import() {
  let deps = WasmDeps {
    imports: vec![
      // global-typed import -> linked against the original
      // `WebAssembly.Global` when the dependency is itself a Wasm module
      // (found in `import.meta.wasmInstances`), falling back to the JS
      // binding otherwise
      wasm_dep_analyzer::Import {
        name: "counter",
        module: "./dep.wasm",
        import_type: wasm_dep_analyzer::ImportType::Global(
          wasm_dep_analyzer::GlobalType {
            value_type: wasm_dep_analyzer::ValueType::I32,
            mutability: true,
          },
        ),
      },
      // non-global import from the same module is untouched
      wasm_dep_analyzer::Import {
        name: "bump",
        module: "./dep.wasm",
        import_type: wasm_dep_analyzer::ImportType::Function(0),
      },
      // module with no global imports gets no namespace import or lookup
      wasm_dep_analyzer::Import {
        name: "log",
        module: "./util.js",
        import_type: wasm_dep_analyzer::ImportType::Function(1),
      },
    ],
    exports: vec![wasm_dep_analyzer::Export {
      name: "read",
      index: 0,
      export_type: wasm_dep_analyzer::ExportType::Function(Ok(
        wasm_dep_analyzer::FunctionSignature {
          params: vec![],
          returns: vec![wasm_dep_analyzer::ValueType::I32],
        },
      )),
    }],
  };
  let rendered = render_js_wasm_module("./main.wasm", deps);
  pretty_assertions::assert_eq!(
    rendered,
    r#"import source wasmMod from "./main.wasm";
import * as import_ns_0 from "./dep.wasm";
import { "counter" as import_0_0, "bump" as import_0_1 } from "./dep.wasm";
import { "log" as import_1_0 } from "./util.js";
const wasmExports_0 = import.meta.wasmInstances.get(import_ns_0);
const importsObject = {
  "./dep.wasm": {
    "counter": wasmExports_0 === undefined ? import_0_0 : wasmExports_0["counter"],
    "bump": import_0_1,
  },
  "./util.js": {
    "log": import_1_0,
  },
};
const modInstance = new import.meta.WasmInstance(wasmMod, importsObject);
const export0 = modInstance.exports["read"];
export { export0 as "read" };
"#,
  );
}
