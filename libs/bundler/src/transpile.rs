// Copyright 2018-2026 the Deno authors. MIT license.

//! Transpilation of TypeScript/JSX modules to JavaScript.
//!
//! Uses `deno_ast` to strip types and transform JSX, producing
//! browser-ready JavaScript for the dev server.

use deno_ast::EmitOptions;
use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceMapOption;
use deno_ast::TranspileModuleOptions;
use deno_ast::TranspileOptions;
use deno_ast::TranspileResult;

use crate::graph::BundlerGraph;
use crate::loader::Loader;

/// Transpile all TypeScript/JSX modules in the graph to JavaScript.
///
/// Updates each module's `source` field in-place with the transpiled code.
/// Modules that are already JS or non-code (JSON, CSS, assets) are skipped.
pub fn transpile_graph(graph: &mut BundlerGraph) {
  let specifiers: Vec<_> = graph
    .modules()
    .filter(|m| needs_transpilation(m.loader))
    .map(|m| m.specifier.clone())
    .collect();

  for specifier in specifiers {
    let module = graph.get_module(&specifier).unwrap();
    let media_type = loader_to_media_type(module.loader);
    let source = module.source.clone();

    match transpile_source(&specifier, &source, media_type) {
      Ok(transpiled) => {
        if let Some(module) = graph.get_module_mut(&specifier) {
          module.source = transpiled;
          // Update loader to JS since it's now transpiled.
          module.loader = Loader::Js;
        }
      }
      Err(e) => {
        eprintln!("Failed to transpile {}: {}", specifier, e);
      }
    }
  }
}

/// Transpile a single module's source code.
pub fn transpile_source(
  specifier: &deno_ast::ModuleSpecifier,
  source: &str,
  media_type: MediaType,
) -> Result<String, deno_ast::ParseDiagnostic> {
  let parsed = deno_ast::parse_module(ParseParams {
    specifier: specifier.clone(),
    text: source.into(),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })?;

  let transpile_options = TranspileOptions::default();
  let module_options = TranspileModuleOptions::default();
  let emit_options = EmitOptions {
    source_map: SourceMapOption::Inline,
    inline_sources: true,
    ..Default::default()
  };

  match parsed.transpile(&transpile_options, &module_options, &emit_options)
  {
    Ok(result) => Ok(match result {
      TranspileResult::Owned(emitted) => emitted.text,
      TranspileResult::Cloned(emitted) => emitted.text,
    }),
    Err(e) => {
      // TranspileError doesn't impl ParseDiagnostic, just return the source as-is.
      eprintln!("Transpile error for {}: {}", specifier, e);
      Ok(source.to_string())
    }
  }
}

/// Whether a loader type requires transpilation.
fn needs_transpilation(loader: Loader) -> bool {
  matches!(loader, Loader::Ts | Loader::Tsx | Loader::Jsx)
}

/// Convert our Loader to deno_ast MediaType.
fn loader_to_media_type(loader: Loader) -> MediaType {
  match loader {
    Loader::Js => MediaType::JavaScript,
    Loader::Jsx => MediaType::Jsx,
    Loader::Ts => MediaType::TypeScript,
    Loader::Tsx => MediaType::Tsx,
    Loader::Json => MediaType::Json,
    _ => MediaType::JavaScript,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::EnvironmentId;
  use crate::module::BundlerModule;
  use crate::module::ModuleType;
  use crate::module::SideEffectFlag;
  use deno_ast::ModuleSpecifier;

  fn spec(s: &str) -> ModuleSpecifier {
    ModuleSpecifier::parse(&format!("file:///{}", s)).unwrap()
  }

  fn make_module(
    specifier: &ModuleSpecifier,
    source: &str,
    loader: Loader,
  ) -> BundlerModule {
    BundlerModule {
      specifier: specifier.clone(),
      original_loader: loader,
      loader,
      module_type: ModuleType::Esm,
      dependencies: Vec::new(),
      side_effects: SideEffectFlag::Unknown,
      source: source.to_string(),
      module_info: None,
      hmr_info: None,
      is_async: false,
      external_imports: Vec::new(),
    }
  }

  #[test]
  fn test_transpile_typescript() {
    let s = spec("mod.ts");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &s,
      "const x: number = 42;\nexport default x;",
      Loader::Ts,
    ));
    graph.add_entry(s.clone());

    transpile_graph(&mut graph);

    let module = graph.get_module(&s).unwrap();
    // Type annotation should be stripped.
    assert!(!module.source.contains(": number"));
    assert!(module.source.contains("42"));
    assert_eq!(module.loader, Loader::Js);
  }

  #[test]
  fn test_transpile_tsx() {
    let s = spec("app.tsx");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &s,
      "export default function App() { return <div>hello</div>; }",
      Loader::Tsx,
    ));
    graph.add_entry(s.clone());

    transpile_graph(&mut graph);

    let module = graph.get_module(&s).unwrap();
    // JSX should be transformed.
    assert!(!module.source.contains("<div>"));
    assert_eq!(module.loader, Loader::Js);
  }

  #[test]
  fn test_skip_js() {
    let s = spec("mod.js");
    let source = "const x = 42;";
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(&s, source, Loader::Js));
    graph.add_entry(s.clone());

    transpile_graph(&mut graph);

    let module = graph.get_module(&s).unwrap();
    // JS should be unchanged.
    assert_eq!(module.source, source);
    assert_eq!(module.loader, Loader::Js);
  }

  #[test]
  fn test_transpile_single_source() {
    let s = spec("mod.ts");
    let result = transpile_source(
      &s,
      "const x: string = 'hello';",
      MediaType::TypeScript,
    )
    .unwrap();
    assert!(!result.contains(": string"));
    assert!(result.contains("hello"));
  }
}
