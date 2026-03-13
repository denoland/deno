// Copyright 2018-2026 the Deno authors. MIT license.

//! Converts a `deno_graph::ModuleGraph` into a `BundlerGraph`.

use deno_ast::ModuleSpecifier;
use deno_media_type::MediaType;

use crate::config::EnvironmentId;
use crate::dependency::Dependency;
use crate::dependency::ImportKind;
use crate::graph::BundlerGraph;
use crate::loader::Loader;
use crate::module::BundlerModule;
use crate::module::ModuleType;
use crate::module::SideEffectFlag;

/// Build a `BundlerGraph` from a `deno_graph::ModuleGraph`.
///
/// Walks the deno_graph module graph starting from the given entries,
/// converting each module and its dependencies into the bundler's
/// representation.
pub fn build_bundler_graph(
  deno_graph: &deno_graph::ModuleGraph,
  environment_id: EnvironmentId,
  entries: &[ModuleSpecifier],
) -> BundlerGraph {
  let mut graph = BundlerGraph::new(environment_id);

  // Walk the graph from all entry points.
  let walk_options = deno_graph::WalkOptions {
    check_js: deno_graph::CheckJsOption::False,
    follow_dynamic: true,
    kind: deno_graph::GraphKind::All,
    prefer_fast_check_graph: false,
  };

  for (specifier, entry) in deno_graph.walk(entries.iter(), walk_options) {
    match entry {
      deno_graph::ModuleEntryRef::Module(module) => {
        if graph.get_module(specifier).is_some() {
          continue; // Already added.
        }

        if let Some(bundler_module) = convert_module(module) {
          graph.add_module(bundler_module);
        }
      }
      deno_graph::ModuleEntryRef::Err(_) => {
        // Skip errored modules for now.
      }
      deno_graph::ModuleEntryRef::Redirect(_) => {
        // Redirects are handled by deno_graph automatically.
      }
    }
  }

  // Register entries.
  for entry in entries {
    // Follow redirects.
    let resolved = deno_graph.resolve(entry);
    graph.add_entry(resolved.clone());
  }

  graph
}

/// Convert a `deno_graph::Module` to a `BundlerModule`.
fn convert_module(module: &deno_graph::Module) -> Option<BundlerModule> {
  match module {
    deno_graph::Module::Js(js) => Some(convert_js_module(js)),
    deno_graph::Module::Json(json) => Some(convert_json_module(json)),
    deno_graph::Module::Npm(_)
    | deno_graph::Module::Node(_)
    | deno_graph::Module::Wasm(_)
    | deno_graph::Module::External(_) => {
      // These are handled specially (npm/node are external, wasm TBD).
      None
    }
  }
}

/// Convert a JS/TS module.
fn convert_js_module(js: &deno_graph::JsModule) -> BundlerModule {
  let dependencies = extract_dependencies(js);
  let loader = media_type_to_loader(js.media_type);
  let module_type = if js.is_script {
    ModuleType::Cjs
  } else {
    ModuleType::Esm
  };

  let source = js.source.text.to_string();

  BundlerModule {
    specifier: js.specifier.clone(),
    original_loader: loader,
    loader,
    module_type,
    dependencies,
    side_effects: SideEffectFlag::Unknown,
    source,
    parsed: None,
    module_info: None,
    hmr_info: None,
    is_async: false, // Detected later by AST analysis.
    external_imports: Vec::new(),
  }
}

/// Convert a JSON module.
fn convert_json_module(json: &deno_graph::JsonModule) -> BundlerModule {
  BundlerModule {
    specifier: json.specifier.clone(),
    original_loader: Loader::Json,
    loader: Loader::Json,
    module_type: ModuleType::Esm,
    dependencies: Vec::new(),
    side_effects: SideEffectFlag::False,
    source: json.source.text.to_string(),
    parsed: None,
    module_info: None,
    hmr_info: None,
    is_async: false,
    external_imports: Vec::new(),
  }
}

/// Extract dependencies from a JS module.
fn extract_dependencies(js: &deno_graph::JsModule) -> Vec<Dependency> {
  let mut deps = Vec::new();

  for (specifier_text, dep) in &js.dependencies {
    // Get the resolved code specifier.
    let resolved = match dep.maybe_code.maybe_specifier() {
      Some(s) => s.clone(),
      None => continue, // Type-only or unresolved.
    };

    let kind = if dep.is_dynamic {
      ImportKind::DynamicImport
    } else {
      // Check the import statements for more specific kind info.
      let first_import = dep.imports.first();
      match first_import.map(|i| &i.kind) {
        Some(deno_graph::ImportKind::Require) => ImportKind::Require,
        _ => ImportKind::Import,
      }
    };

    deps.push(Dependency {
      specifier: specifier_text.clone(),
      resolved,
      kind,
      range: None, // Could extract from dep.imports but not needed yet.
    });
  }

  deps
}

/// Map deno_media_type::MediaType to our Loader.
fn media_type_to_loader(media_type: MediaType) -> Loader {
  match media_type {
    MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => Loader::Js,
    MediaType::TypeScript | MediaType::Mts | MediaType::Cts | MediaType::Dts | MediaType::Dmts | MediaType::Dcts => Loader::Ts,
    MediaType::Jsx => Loader::Jsx,
    MediaType::Tsx => Loader::Tsx,
    MediaType::Json => Loader::Json,
    MediaType::Css => Loader::Css,
    _ => Loader::Js, // Fallback.
  }
}
