// Copyright 2018-2026 the Deno authors. MIT license.

//! Identifier deconflicting for scope hoisting.
//!
//! When multiple modules are concatenated into a single scope, their
//! top-level declarations can collide. This module detects conflicts
//! and assigns globally unique names (e.g. `foo` → `foo$1`).

use std::collections::HashMap;

use deno_ast::swc::ast::Program;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use crate::graph::BundlerGraph;
use crate::module::ModuleType;

use deno_ast::ModuleSpecifier;

/// A rename plan for a single module.
#[derive(Debug, Clone)]
pub struct ModuleRenames {
  /// Map from original name to new (deconflicted) name.
  pub renames: HashMap<String, String>,
}

/// Compute rename maps for all modules in a chunk to avoid top-level
/// name collisions when scope-hoisted.
///
/// Returns a map from module specifier to its rename plan. Modules with
/// no conflicts will have an empty renames map.
pub fn compute_deconflict_renames(
  modules: &[ModuleSpecifier],
  graph: &BundlerGraph,
) -> FxHashMap<ModuleSpecifier, ModuleRenames> {
  // Step 1: Collect all top-level declaration names per module.
  let mut module_decls: Vec<(&ModuleSpecifier, Vec<String>)> = Vec::new();

  for specifier in modules {
    let Some(module) = graph.get_module(specifier) else {
      continue;
    };

    // Skip CJS modules — they stay wrapped.
    if module.module_type == ModuleType::Cjs {
      continue;
    }

    let names = collect_top_level_names(specifier, module, graph);
    module_decls.push((specifier, names));
  }

  // Step 2: Count how many modules declare each name.
  let mut name_counts: FxHashMap<&str, usize> = FxHashMap::default();
  for (_, names) in &module_decls {
    // Use a set to avoid double-counting within a single module.
    let unique: FxHashSet<&str> = names.iter().map(|s| s.as_str()).collect();
    for name in unique {
      *name_counts.entry(name).or_insert(0) += 1;
    }
  }

  // Step 3: For conflicting names, assign unique suffixes.
  // Track the next suffix for each conflicting name.
  let mut name_suffixes: FxHashMap<&str, u32> = FxHashMap::default();
  let mut result: FxHashMap<ModuleSpecifier, ModuleRenames> =
    FxHashMap::default();

  for (specifier, names) in &module_decls {
    let unique: FxHashSet<&str> = names.iter().map(|s| s.as_str()).collect();
    let mut renames = HashMap::new();

    for name in unique {
      let count = name_counts.get(name).copied().unwrap_or(0);
      if count > 1 {
        let suffix = name_suffixes.entry(name).or_insert(0);
        if *suffix > 0 {
          // First occurrence keeps original name, subsequent get suffixed.
          renames.insert(name.to_string(), format!("{}${}", name, suffix));
        }
        *suffix += 1;
      }
    }

    result.insert(
      (*specifier).clone(),
      ModuleRenames { renames },
    );
  }

  result
}

/// Collect top-level declaration names from a module.
fn collect_top_level_names(
  specifier: &ModuleSpecifier,
  module: &crate::module::BundlerModule,
  graph: &BundlerGraph,
) -> Vec<String> {
  // Prefer module_info if available (already analyzed).
  if let Some(mi) = &module.module_info {
    return mi
      .top_level_decls
      .iter()
      .map(|d| d.name.clone())
      .collect();
  }

  // Fall back to AST scanning.
  let program = if let Some(tp) = &module.transformed_program {
    Some(tp)
  } else {
    None
  };

  if let Some(program) = program {
    collect_names_from_program(program)
  } else {
    // Try to get from parsed source.
    let module = graph.get_module(specifier).unwrap();
    if let Some(parsed) = &module.parsed {
      let program = parsed.program();
      collect_names_from_program(&program)
    } else {
      Vec::new()
    }
  }
}

/// Extract top-level declaration names from a Program AST.
fn collect_names_from_program(program: &Program) -> Vec<String> {
  use deno_ast::swc::ast::*;

  let items = match program {
    Program::Module(m) => &m.body,
    Program::Script(_) => return Vec::new(),
  };

  let mut names = Vec::new();

  for item in items {
    match item {
      ModuleItem::Stmt(stmt) => {
        collect_names_from_stmt(stmt, &mut names);
      }
      ModuleItem::ModuleDecl(decl) => match decl {
        ModuleDecl::ExportDecl(export) => {
          collect_names_from_decl(&export.decl, &mut names);
        }
        ModuleDecl::ExportDefaultDecl(export) => match &export.decl {
          DefaultDecl::Fn(f) => {
            if let Some(ident) = &f.ident {
              names.push(ident.sym.to_string());
            }
          }
          DefaultDecl::Class(c) => {
            if let Some(ident) = &c.ident {
              names.push(ident.sym.to_string());
            }
          }
          _ => {}
        },
        _ => {}
      },
    }
  }

  names
}

fn collect_names_from_stmt(stmt: &deno_ast::swc::ast::Stmt, names: &mut Vec<String>) {
  use deno_ast::swc::ast::*;

  if let Stmt::Decl(decl) = stmt {
    collect_names_from_decl(decl, names);
  }
}

fn collect_names_from_decl(decl: &deno_ast::swc::ast::Decl, names: &mut Vec<String>) {
  use deno_ast::swc::ast::*;

  match decl {
    Decl::Var(var) => {
      for declarator in &var.decls {
        collect_names_from_pat(&declarator.name, names);
      }
    }
    Decl::Fn(f) => {
      names.push(f.ident.sym.to_string());
    }
    Decl::Class(c) => {
      names.push(c.ident.sym.to_string());
    }
    _ => {}
  }
}

fn collect_names_from_pat(pat: &deno_ast::swc::ast::Pat, names: &mut Vec<String>) {
  use deno_ast::swc::ast::*;

  match pat {
    Pat::Ident(i) => {
      names.push(i.sym.to_string());
    }
    Pat::Array(arr) => {
      for elem in arr.elems.iter().flatten() {
        collect_names_from_pat(elem, names);
      }
    }
    Pat::Object(obj) => {
      for prop in &obj.props {
        match prop {
          ObjectPatProp::KeyValue(kv) => {
            collect_names_from_pat(&kv.value, names);
          }
          ObjectPatProp::Assign(a) => {
            names.push(a.key.sym.to_string());
          }
          ObjectPatProp::Rest(r) => {
            collect_names_from_pat(&r.arg, names);
          }
        }
      }
    }
    Pat::Rest(r) => {
      collect_names_from_pat(&r.arg, names);
    }
    Pat::Assign(a) => {
      collect_names_from_pat(&a.left, names);
    }
    _ => {}
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::chunk::build_chunk_graph;
  use crate::config::EnvironmentId;
  use crate::dependency::Dependency;
  use crate::dependency::ImportKind;
  use crate::loader::Loader;
  use crate::module::BundlerModule;
  use crate::module::SideEffectFlag;

  fn spec(s: &str) -> ModuleSpecifier {
    ModuleSpecifier::parse(&format!("file:///{}", s)).unwrap()
  }

  fn make_module(
    specifier: &ModuleSpecifier,
    source: &str,
    deps: Vec<Dependency>,
  ) -> BundlerModule {
    let mut m = BundlerModule {
      specifier: specifier.clone(),
      original_loader: Loader::Js,
      loader: Loader::Js,
      module_type: ModuleType::Esm,
      dependencies: deps,
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
    };
    m.ensure_parsed();
    m
  }

  fn make_dep(target: &ModuleSpecifier, kind: ImportKind) -> Dependency {
    Dependency {
      specifier: target.to_string(),
      resolved: target.clone(),
      kind,
      range: None,
    }
  }

  #[test]
  fn test_no_conflicts() {
    let a = spec("a.js");
    let b = spec("b.js");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(&a, "var x = 1;", vec![make_dep(&b, ImportKind::Import)]));
    graph.add_module(make_module(&b, "var y = 2;", vec![]));
    graph.add_entry(a.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);

    let renames = compute_deconflict_renames(&chunk.modules, &graph);

    // No conflicts, all renames should be empty.
    for (_, module_renames) in &renames {
      assert!(module_renames.renames.is_empty());
    }
  }

  #[test]
  fn test_conflicting_names() {
    let a = spec("a.js");
    let b = spec("b.js");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &a,
      "var helper = 1;",
      vec![make_dep(&b, ImportKind::Import)],
    ));
    graph.add_module(make_module(&b, "var helper = 2;", vec![]));
    graph.add_entry(a.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);

    let renames = compute_deconflict_renames(&chunk.modules, &graph);

    // One module should keep `helper`, the other should get `helper$1`.
    let total_renames: usize =
      renames.values().map(|r| r.renames.len()).sum();
    assert_eq!(total_renames, 1);

    let renamed_module = renames
      .values()
      .find(|r| !r.renames.is_empty())
      .unwrap();
    assert_eq!(
      renamed_module.renames.get("helper").unwrap(),
      "helper$1"
    );
  }
}
