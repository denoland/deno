// Copyright 2018-2026 the Deno authors. MIT license.

//! Transform pipeline: applies SWC VisitMut transforms to graph modules.
//!
//! This runs after transpilation (all modules are JS) and before analysis.
//! It parses each module, applies configured transforms, and emits back to
//! source strings.

use std::collections::HashMap;
use deno_ast::swc::ast::Program;
use deno_ast::swc::codegen::text_writer::JsWriter;
use deno_ast::swc::codegen::Emitter;
use deno_ast::swc::codegen::Node;
use deno_ast::swc::common::SourceMap;
use deno_ast::swc_codegen_config;

use crate::graph::BundlerGraph;
use deno_ast::bundler_transforms::ConstantFolder;
use deno_ast::bundler_transforms::DeadBranchEliminator;
use deno_ast::bundler_transforms::DefineReplacer;
use deno_ast::bundler_transforms::ImportMetaRewriter;
use deno_ast::bundler_transforms::{
  convert_top_level_to_var, eliminate_dead_branches,
};
use crate::loader::Loader;

/// Options controlling which transforms to apply.
#[derive(Debug, Clone, Default)]
pub struct TransformOptions {
  /// Global expression replacements (e.g. `"process.env.NODE_ENV"` → `"\"production\""`).
  pub define: HashMap<String, String>,
  /// Whether to run dead branch elimination using the define map.
  pub dead_code_elimination: bool,
  /// Whether to convert top-level let/const to var (for scope hoisting).
  pub convert_to_var: bool,
  /// Whether this is a production build (enables more aggressive transforms).
  pub production: bool,
}

/// Apply transforms to all JS modules in the graph.
///
/// Should be called after `transpile_graph()` (all modules are JS) and before
/// `analyze_graph()`.
pub fn transform_graph(
  graph: &mut BundlerGraph,
  options: &TransformOptions,
) {
  // Skip if there's nothing to do.
  if options.define.is_empty()
    && !options.convert_to_var
    && !options.dead_code_elimination
  {
    return;
  }

  let specifiers: Vec<_> = graph
    .modules()
    .filter(|m| matches!(m.loader, Loader::Js))
    .map(|m| m.specifier.clone())
    .collect();

  // Ensure all modules are parsed (reuses cache if available).
  for specifier in &specifiers {
    if let Some(module) = graph.get_module_mut(specifier) {
      module.ensure_parsed();
    }
  }

  for specifier in specifiers {
    let module = graph.get_module(&specifier).unwrap();
    let Some(parsed) = &module.parsed else {
      continue;
    };

    // Clone the AST from cached parse so we can mutate it.
    let mut program = (*parsed.program()).clone();
    let mut changed = false;

    // Apply define replacements.
    if !options.define.is_empty() {
      use deno_ast::swc::ecma_visit::VisitMutWith;
      let mut replacer = DefineReplacer {
        defines: options.define.clone(),
      };
      program.visit_mut_with(&mut replacer);
      changed = true;
    }

    // Constant folding (after define replacement so `"production" === "production"` → `true`).
    if options.dead_code_elimination && !options.define.is_empty() {
      use deno_ast::swc::ecma_visit::VisitMutWith;
      program.visit_mut_with(&mut ConstantFolder);
      changed = true;
    }

    // Dead branch elimination (recursive — handles if/ternary at any depth).
    if options.dead_code_elimination && !options.define.is_empty() {
      use deno_ast::swc::ecma_visit::VisitMutWith;
      program.visit_mut_with(&mut DeadBranchEliminator);
      // Also run the top-level define-aware elimination for patterns
      // that involve define values directly (not yet folded to literals).
      if let Program::Module(module) = &mut program {
        eliminate_dead_branches(&mut module.body, &options.define);
      }
      changed = true;
    }

    // Convert top-level let/const to var.
    if options.convert_to_var {
      if let Program::Module(module) = &mut program {
        convert_top_level_to_var(&mut module.body);
        changed = true;
      }
    }

    if changed {
      if let Some(emitted) = emit_program(&program) {
        if let Some(module) = graph.get_module_mut(&specifier) {
          module.source = emitted;
          module.parsed = None;
          // Store transformed AST so downstream analysis doesn't re-parse.
          module.transformed_program = Some(program);
        }
      }
    }
  }
}

/// Apply `import.meta` rewriting to a single module.
///
/// Separated because it needs per-module url/dirname/filename values.
pub fn transform_import_meta(
  graph: &mut BundlerGraph,
) {
  let specifiers: Vec<_> = graph
    .modules()
    .filter(|m| matches!(m.loader, Loader::Js))
    .map(|m| (m.specifier.clone(), m.source.clone()))
    .collect();

  // Ensure all modules are parsed (reuses cache if available).
  for (specifier, _) in &specifiers {
    if let Some(module) = graph.get_module_mut(specifier) {
      module.ensure_parsed();
    }
  }

  for (specifier, source) in specifiers {
    // Only rewrite if source contains import.meta.
    if !source.contains("import.meta.") {
      continue;
    }

    let url = specifier.to_string();
    let (dirname, filename) = if specifier.scheme() == "file" {
      if let Ok(path) = specifier.to_file_path() {
        let dirname = path
          .parent()
          .map(|p| p.to_string_lossy().to_string())
          .unwrap_or_default();
        let filename = path.to_string_lossy().to_string();
        (dirname, filename)
      } else {
        (String::new(), String::new())
      }
    } else {
      (String::new(), String::new())
    };

    let module = graph.get_module(&specifier).unwrap();
    let Some(parsed) = &module.parsed else {
      continue;
    };

    let mut program = (*parsed.program()).clone();

    use deno_ast::swc::ecma_visit::VisitMutWith;
    let mut rewriter = ImportMetaRewriter {
      url,
      dirname,
      filename,
    };
    program.visit_mut_with(&mut rewriter);

    if let Some(emitted) = emit_program(&program) {
      if let Some(module) = graph.get_module_mut(&specifier) {
        module.source = emitted;
        module.parsed = None;
        module.transformed_program = Some(program);
      }
    }
  }
}

/// Emit an SWC Program back to a JavaScript source string.
pub fn emit_program(program: &Program) -> Option<String> {
  let cm = std::rc::Rc::new(SourceMap::default());
  let mut buf = vec![];
  {
    let mut writer = Box::new(JsWriter::new(cm.clone(), "\n", &mut buf, None));
    writer.set_indent_str("  ");

    let mut emitter = Emitter {
      cfg: swc_codegen_config(),
      comments: None,
      cm,
      wr: writer,
    };

    program.emit_with(&mut emitter).ok()?;
  }

  String::from_utf8(buf).ok()
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

  fn make_module(specifier: &ModuleSpecifier, source: &str) -> BundlerModule {
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
  fn test_define_replacement() {
    let s = spec("mod.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &s,
      "console.log(process.env.NODE_ENV);",
    ));
    graph.add_entry(s.clone());

    let mut defines = HashMap::new();
    defines.insert(
      "process.env.NODE_ENV".to_string(),
      "\"production\"".to_string(),
    );

    transform_graph(
      &mut graph,
      &TransformOptions {
        define: defines,
        dead_code_elimination: false,
        convert_to_var: false,
        production: false,
      },
    );

    let module = graph.get_module(&s).unwrap();
    assert!(module.source.contains("\"production\""));
    assert!(!module.source.contains("process.env.NODE_ENV"));
  }

  #[test]
  fn test_dead_branch_elimination() {
    let s = spec("mod.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &s,
      "if (process.env.NODE_ENV === \"production\") { console.log('prod'); } else { console.log('dev'); }",
    ));
    graph.add_entry(s.clone());

    let mut defines = HashMap::new();
    defines.insert(
      "process.env.NODE_ENV".to_string(),
      "\"production\"".to_string(),
    );

    transform_graph(
      &mut graph,
      &TransformOptions {
        define: defines,
        dead_code_elimination: true,
        convert_to_var: false,
        production: false,
      },
    );

    let module = graph.get_module(&s).unwrap();
    assert!(module.source.contains("prod"));
    assert!(!module.source.contains("dev"));
  }

  #[test]
  fn test_convert_to_var() {
    let s = spec("mod.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(&s, "const x = 1;\nlet y = 2;"));
    graph.add_entry(s.clone());

    transform_graph(
      &mut graph,
      &TransformOptions {
        define: HashMap::new(),
        dead_code_elimination: false,
        convert_to_var: true,
        production: false,
      },
    );

    let module = graph.get_module(&s).unwrap();
    assert!(!module.source.contains("const "));
    assert!(!module.source.contains("let "));
    assert!(module.source.contains("var "));
  }

  #[test]
  fn test_import_meta_rewriting() {
    let s = ModuleSpecifier::parse("file:///project/src/mod.js").unwrap();
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &s,
      "console.log(import.meta.url);",
    ));
    graph.add_entry(s.clone());

    transform_import_meta(&mut graph);

    let module = graph.get_module(&s).unwrap();
    assert!(module.source.contains("file:///project/src/mod.js"));
    assert!(!module.source.contains("import.meta.url"));
  }

  #[test]
  fn test_skip_no_transforms_needed() {
    let s = spec("mod.js");
    let source = "console.log('hello');";
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(&s, source));
    graph.add_entry(s.clone());

    // Empty options should skip all transforms.
    transform_graph(
      &mut graph,
      &TransformOptions::default(),
    );

    let module = graph.get_module(&s).unwrap();
    // Source should be unchanged.
    assert_eq!(module.source, source);
  }

  #[test]
  fn test_define_fold_dce_chain() {
    // End-to-end: define replacement + constant folding + dead branch elimination.
    // `process.env.NODE_ENV !== "production"` → `"production" !== "production"` → `false`
    // → `if (false) { devSetup(); }` → removed
    let s = spec("mod.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &s,
      "if (process.env.NODE_ENV !== \"production\") { devSetup(); }\nconsole.log('app');",
    ));
    graph.add_entry(s.clone());

    let mut defines = HashMap::new();
    defines.insert(
      "process.env.NODE_ENV".to_string(),
      "\"production\"".to_string(),
    );

    transform_graph(
      &mut graph,
      &TransformOptions {
        define: defines,
        dead_code_elimination: true,
        convert_to_var: false,
        production: true,
      },
    );

    let module = graph.get_module(&s).unwrap();
    assert!(!module.source.contains("devSetup"));
    assert!(module.source.contains("app"));
  }

  #[test]
  fn test_nested_dce_in_function() {
    // Dead branch elimination inside a function body.
    let s = spec("mod.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &s,
      "function init() { if (process.env.NODE_ENV === \"production\") { prod(); } else { dev(); } }",
    ));
    graph.add_entry(s.clone());

    let mut defines = HashMap::new();
    defines.insert(
      "process.env.NODE_ENV".to_string(),
      "\"production\"".to_string(),
    );

    transform_graph(
      &mut graph,
      &TransformOptions {
        define: defines,
        dead_code_elimination: true,
        convert_to_var: false,
        production: true,
      },
    );

    let module = graph.get_module(&s).unwrap();
    assert!(module.source.contains("prod"));
    assert!(!module.source.contains("dev"));
  }
}
