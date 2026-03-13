// Copyright 2018-2026 the Deno authors. MIT license.

//! Asset discovery: finds non-JS dependencies referenced from JS modules
//! and adds them to the BundlerGraph.
//!
//! Currently detects:
//! - `new URL('./file', import.meta.url)` patterns in JS modules
//!
//! CSS and HTML discovery will be added later when parsers are available.
//! Discovery is iterative — newly added modules are scanned until no new
//! modules are found.

use deno_ast::swc::ast::*;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_ast::ModuleSpecifier;

use crate::dependency::Dependency;
use crate::dependency::ImportKind;
use crate::graph::BundlerGraph;
use crate::loader::Loader;
use crate::module::BundlerModule;
use crate::module::ModuleType;
use crate::module::SideEffectFlag;

/// Discover non-JS assets referenced from modules in the graph.
///
/// Iteratively scans modules for asset references (e.g. `new URL(...,
/// import.meta.url)`), resolves them, loads them from disk, and adds them
/// to the graph. Repeats until no new modules are discovered.
///
/// Should be called after `transform_modules()` (all JS modules are
/// transpiled) and before `analyze_graph()`.
pub fn discover_assets(graph: &mut BundlerGraph) {
  loop {
    let new_modules = discover_pass(graph);
    if new_modules.is_empty() {
      break;
    }
    for module in new_modules {
      graph.add_module(module);
    }
  }
}

/// Run a single discovery pass over all JS modules in the graph.
///
/// Returns newly discovered modules that should be added to the graph.
/// Also updates dependency lists on existing modules.
fn discover_pass(graph: &mut BundlerGraph) -> Vec<BundlerModule> {
  // Collect all JS module specifiers.
  let js_specifiers: Vec<ModuleSpecifier> = graph
    .modules()
    .filter(|m| matches!(m.loader, Loader::Js))
    .map(|m| m.specifier.clone())
    .collect();

  // Ensure all JS modules are parsed (populates cached ParsedSource).
  // Modules with transformed_program already have an AST.
  for specifier in &js_specifiers {
    if let Some(module) = graph.get_module_mut(specifier) {
      if module.transformed_program.is_none() {
        module.ensure_parsed();
      }
    }
  }

  let mut new_modules = Vec::new();
  let mut seen_new: std::collections::HashSet<ModuleSpecifier> =
    std::collections::HashSet::new();

  for specifier in &js_specifiers {
    let module = graph.get_module(specifier).unwrap();

    // Use transformed AST if available, otherwise fall back to cached parse.
    let refs = if let Some(tp) = &module.transformed_program {
      extract_url_references(specifier, tp)
    } else if let Some(parsed) = &module.parsed {
      let program = parsed.program();
      extract_url_references(specifier, &program)
    } else {
      continue;
    };

    for url_ref in refs {
      // Skip if already in the graph or already discovered this pass.
      if graph.get_module(&url_ref.resolved).is_some()
        || seen_new.contains(&url_ref.resolved)
      {
        // Still add the dependency edge if not already present.
        if let Some(module) = graph.get_module_mut(specifier) {
          let already_has = module
            .dependencies
            .iter()
            .any(|d| d.resolved == url_ref.resolved);
          if !already_has {
            module.dependencies.push(Dependency {
              specifier: url_ref.raw_specifier.clone(),
              resolved: url_ref.resolved.clone(),
              kind: ImportKind::UrlReference,
              range: None,
            });
          }
        }
        continue;
      }

      // Determine loader from the resolved URL.
      let loader = url_ref
        .resolved
        .to_file_path()
        .ok()
        .and_then(|p| Loader::from_path(&p))
        .unwrap_or(Loader::Asset);

      // Load asset source (empty for binary assets, file content for text).
      let source = if loader.is_asset() {
        String::new()
      } else {
        url_ref
          .resolved
          .to_file_path()
          .ok()
          .and_then(|p| std::fs::read_to_string(&p).ok())
          .unwrap_or_default()
      };

      let new_module = BundlerModule {
        specifier: url_ref.resolved.clone(),
        original_loader: loader,
        loader,
        module_type: ModuleType::Esm,
        dependencies: Vec::new(),
        side_effects: SideEffectFlag::False,
        source,
        source_map: None,
      source_hash: None,
      parsed: None,
        transformed_program: None,
        module_info: None,
        hmr_info: None,
        is_async: false,
        external_imports: Vec::new(),
      };

      seen_new.insert(url_ref.resolved.clone());
      new_modules.push(new_module);

      // Add the dependency edge on the importing module.
      if let Some(module) = graph.get_module_mut(specifier) {
        module.dependencies.push(Dependency {
          specifier: url_ref.raw_specifier.clone(),
          resolved: url_ref.resolved.clone(),
          kind: ImportKind::UrlReference,
          range: None,
        });
      }
    }
  }

  new_modules
}

/// A URL reference found in a JS module.
struct UrlReference {
  /// The raw specifier string from the source (e.g., `"./image.png"`).
  raw_specifier: String,
  /// The fully resolved URL.
  resolved: ModuleSpecifier,
}

/// Extract `new URL('./file', import.meta.url)` references from a parsed module.
fn extract_url_references(
  specifier: &ModuleSpecifier,
  program: &Program,
) -> Vec<UrlReference> {
  let mut visitor = UrlReferenceVisitor {
    base_specifier: specifier.clone(),
    refs: Vec::new(),
  };

  program.visit_with(&mut visitor);

  visitor.refs
}

/// SWC visitor that finds `new URL('...', import.meta.url)` patterns.
struct UrlReferenceVisitor {
  base_specifier: ModuleSpecifier,
  refs: Vec<UrlReference>,
}

impl Visit for UrlReferenceVisitor {
  fn visit_new_expr(&mut self, node: &NewExpr) {
    // Check: `new URL(<string>, import.meta.url)`
    if let Some((raw_specifier, resolved)) =
      self.try_extract_url_constructor(node)
    {
      self.refs.push(UrlReference {
        raw_specifier,
        resolved,
      });
    }
    // Continue visiting child nodes.
    node.visit_children_with(self);
  }
}

impl UrlReferenceVisitor {
  fn try_extract_url_constructor(
    &self,
    node: &NewExpr,
  ) -> Option<(String, ModuleSpecifier)> {
    // Check callee is `URL`.
    let callee_is_url = match &*node.callee {
      Expr::Ident(ident) => &*ident.sym == "URL",
      _ => false,
    };
    if !callee_is_url {
      return None;
    }

    let args = node.args.as_ref()?;
    if args.len() < 2 {
      return None;
    }

    // First arg must be a string literal.
    let first_arg = &args[0].expr;
    let raw_specifier = match &**first_arg {
      Expr::Lit(Lit::Str(s)) => {
        String::from_utf8_lossy(s.value.as_bytes()).to_string()
      }
      Expr::Tpl(tpl) if tpl.exprs.is_empty() && tpl.quasis.len() == 1 => {
        // Template literal with no expressions: `new URL(`./file`, import.meta.url)`
        String::from_utf8_lossy(tpl.quasis[0].raw.as_bytes()).to_string()
      }
      _ => return None,
    };

    // Second arg must be `import.meta.url`.
    let second_arg = &args[1].expr;
    if !is_import_meta_url(second_arg) {
      return None;
    }

    // Resolve relative to the base specifier.
    let resolved = self.base_specifier.join(&raw_specifier).ok()?;

    Some((raw_specifier, resolved))
  }
}

/// Check if an expression is `import.meta.url`.
fn is_import_meta_url(expr: &Expr) -> bool {
  let Expr::Member(member) = expr else {
    return false;
  };
  let MemberProp::Ident(prop) = &member.prop else {
    return false;
  };
  if &*prop.sym != "url" {
    return false;
  }
  matches!(
    &*member.obj,
    Expr::MetaProp(MetaPropExpr {
      kind: MetaPropKind::ImportMeta,
      ..
    })
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::EnvironmentId;

  fn spec(s: &str) -> ModuleSpecifier {
    ModuleSpecifier::parse(&format!("file:///{}", s)).unwrap()
  }

  fn parse_js(
    specifier: &ModuleSpecifier,
    source: &str,
  ) -> deno_ast::ParsedSource {
    deno_ast::parse_module(deno_ast::ParseParams {
      specifier: specifier.clone(),
      text: source.into(),
      media_type: deno_ast::MediaType::JavaScript,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .unwrap()
  }

  fn make_js_module(
    specifier: &ModuleSpecifier,
    source: &str,
  ) -> BundlerModule {
    BundlerModule {
      specifier: specifier.clone(),
      original_loader: Loader::Js,
      loader: Loader::Js,
      module_type: ModuleType::Esm,
      dependencies: Vec::new(),
      side_effects: SideEffectFlag::Unknown,
      source: source.to_string(),
      source_map: None,
      source_hash: None,
      parsed: None,
      transformed_program: None,
      module_info: None,
      hmr_info: None,
      is_async: false,
      external_imports: Vec::new(),
    }
  }

  #[test]
  fn test_extract_new_url_import_meta() {
    let s = spec("src/main.js");
    let src = "const img = new URL('./image.png', import.meta.url);";
    let parsed = parse_js(&s, src);
    let refs = extract_url_references(&s, &parsed.program());
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].raw_specifier, "./image.png");
    assert_eq!(
      refs[0].resolved.as_str(),
      "file:///src/image.png"
    );
  }

  #[test]
  fn test_extract_new_url_template_literal() {
    let s = spec("src/main.js");
    let src = "const img = new URL(`./photo.jpg`, import.meta.url);";
    let parsed = parse_js(&s, src);
    let refs = extract_url_references(&s, &parsed.program());
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].raw_specifier, "./photo.jpg");
  }

  #[test]
  fn test_extract_ignores_dynamic_specifier() {
    let s = spec("src/main.js");
    let src = "const img = new URL(dynamicVar, import.meta.url);";
    let parsed = parse_js(&s, src);
    let refs = extract_url_references(&s, &parsed.program());
    assert_eq!(refs.len(), 0);
  }

  #[test]
  fn test_extract_ignores_non_import_meta_url() {
    let s = spec("src/main.js");
    let src = "const img = new URL('./image.png', 'http://example.com');";
    let parsed = parse_js(&s, src);
    let refs = extract_url_references(&s, &parsed.program());
    assert_eq!(refs.len(), 0);
  }

  #[test]
  fn test_extract_ignores_single_arg_url() {
    let s = spec("src/main.js");
    let src = "const u = new URL('http://example.com');";
    let parsed = parse_js(&s, src);
    let refs = extract_url_references(&s, &parsed.program());
    assert_eq!(refs.len(), 0);
  }

  #[test]
  fn test_extract_multiple_refs() {
    let s = spec("src/main.js");
    let src = r#"
        const a = new URL('./a.png', import.meta.url);
        const b = new URL('./b.woff2', import.meta.url);
        const c = new URL('./c.svg', import.meta.url);
      "#;
    let parsed = parse_js(&s, src);
    let refs = extract_url_references(&s, &parsed.program());
    assert_eq!(refs.len(), 3);
  }

  #[test]
  fn test_discover_assets_adds_to_graph() {
    let entry = spec("src/main.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_js_module(
      &entry,
      "const img = new URL('./image.png', import.meta.url);",
    ));
    graph.add_entry(entry.clone());

    discover_assets(&mut graph);

    // Asset module should be added to the graph.
    let asset_spec = spec("src/image.png");
    let asset = graph.get_module(&asset_spec);
    assert!(asset.is_some());
    let asset = asset.unwrap();
    assert_eq!(asset.loader, Loader::Asset);
    assert!(asset.source.is_empty());

    // Entry should have a dependency edge.
    let entry_mod = graph.get_module(&entry).unwrap();
    assert!(entry_mod
      .dependencies
      .iter()
      .any(|d| d.resolved == asset_spec
        && d.kind == ImportKind::UrlReference));
  }

  #[test]
  fn test_discover_assets_no_duplicates() {
    let entry = spec("src/main.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_js_module(
      &entry,
      r#"
        const a = new URL('./image.png', import.meta.url);
        const b = new URL('./image.png', import.meta.url);
      "#,
    ));
    graph.add_entry(entry.clone());

    discover_assets(&mut graph);

    // Only one asset module should exist.
    assert_eq!(graph.len(), 2); // entry + one asset

    // Entry should have one dependency edge (deduplicated).
    let entry_mod = graph.get_module(&entry).unwrap();
    let url_deps: Vec<_> = entry_mod
      .dependencies
      .iter()
      .filter(|d| d.kind == ImportKind::UrlReference)
      .collect();
    assert_eq!(url_deps.len(), 1);
  }

  #[test]
  fn test_discover_assets_skips_existing() {
    let entry = spec("src/main.js");
    let asset_spec = spec("src/image.png");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_js_module(
      &entry,
      "const img = new URL('./image.png', import.meta.url);",
    ));
    // Asset already in graph (e.g., added by a previous pass).
    graph.add_module(BundlerModule {
      specifier: asset_spec.clone(),
      original_loader: Loader::Asset,
      loader: Loader::Asset,
      module_type: ModuleType::Esm,
      dependencies: Vec::new(),
      side_effects: SideEffectFlag::False,
      source: String::new(),
      source_map: None,
      source_hash: None,
      parsed: None,
      transformed_program: None,
      module_info: None,
      hmr_info: None,
      is_async: false,
      external_imports: Vec::new(),
    });
    graph.add_entry(entry.clone());

    let initial_count = graph.len();
    discover_assets(&mut graph);

    // No new modules should be added.
    assert_eq!(graph.len(), initial_count);

    // But the dependency edge should still be added.
    let entry_mod = graph.get_module(&entry).unwrap();
    assert!(entry_mod
      .dependencies
      .iter()
      .any(|d| d.resolved == asset_spec));
  }

  #[test]
  fn test_discover_resolves_relative_paths() {
    let entry = spec("project/src/app.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_js_module(
      &entry,
      "const font = new URL('../assets/font.woff2', import.meta.url);",
    ));
    graph.add_entry(entry.clone());

    discover_assets(&mut graph);

    let font_spec = spec("project/assets/font.woff2");
    assert!(graph.get_module(&font_spec).is_some());
  }
}
