// Copyright 2018-2026 the Deno authors. MIT license.

//! Import analyzer for the vbundle module.
//!
//! This module extracts import and export information from parsed source code
//! using deno_ast's SWC AST types and visitor pattern.

use std::sync::Arc;

use deno_ast::swc::ast;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParseParams;
use deno_core::error::AnyError;

use super::source_graph::ImportInfo;
use super::source_graph::NamedImport;
use super::source_graph::NamedReExport;
use super::source_graph::ReExportInfo;

/// Result of analyzing a module's imports.
#[derive(Debug, Default)]
pub struct AnalysisResult {
  /// Static import declarations.
  pub imports: Vec<ImportInfo>,
  /// Dynamic import() calls.
  pub dynamic_imports: Vec<ImportInfo>,
  /// Re-export declarations.
  pub re_exports: Vec<ReExportInfo>,
}

/// Parse source code and extract import information.
pub fn analyze_imports(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: MediaType,
) -> Result<AnalysisResult, AnyError> {
  // Parse the module
  let parsed = deno_ast::parse_module(ParseParams {
    specifier: specifier.clone(),
    text: Arc::from(source),
    media_type,
    capture_tokens: false,
    maybe_syntax: None,
    scope_analysis: false,
  })?;

  // Collect imports using visitor
  let mut collector = ImportCollector::new(specifier.clone());
  parsed.program().visit_with(&mut collector);

  Ok(collector.into_result())
}

/// Visitor that collects import and export information from the AST.
struct ImportCollector {
  specifier: ModuleSpecifier,
  imports: Vec<RawImport>,
  dynamic_imports: Vec<RawImport>,
  re_exports: Vec<RawReExport>,
}

/// Raw import information extracted from AST (before resolution).
struct RawImport {
  source: String,
  named: Vec<NamedImport>,
  default_import: Option<String>,
  namespace_import: Option<String>,
  is_type_only: bool,
  range: (usize, usize),
}

/// Raw re-export information extracted from AST.
struct RawReExport {
  source: String,
  named: Vec<NamedReExport>,
  is_all: bool,
}

impl ImportCollector {
  fn new(specifier: ModuleSpecifier) -> Self {
    Self {
      specifier,
      imports: Vec::new(),
      dynamic_imports: Vec::new(),
      re_exports: Vec::new(),
    }
  }

  fn into_result(self) -> AnalysisResult {
    let specifier = self.specifier;

    // Convert raw imports to ImportInfo by resolving specifiers
    let imports = self
      .imports
      .into_iter()
      .filter_map(|raw| resolve_raw_import(&specifier, raw))
      .collect();

    let dynamic_imports = self
      .dynamic_imports
      .into_iter()
      .filter_map(|raw| resolve_raw_import(&specifier, raw))
      .collect();

    let re_exports = self
      .re_exports
      .into_iter()
      .filter_map(|raw| resolve_raw_re_export(&specifier, raw))
      .collect();

    AnalysisResult {
      imports,
      dynamic_imports,
      re_exports,
    }
  }
}

fn resolve_raw_import(
  importer: &ModuleSpecifier,
  raw: RawImport,
) -> Option<ImportInfo> {
  let resolved = resolve_specifier(importer, &raw.source)?;
  Some(ImportInfo {
    specifier: resolved,
    original: raw.source,
    named: raw.named,
    default_import: raw.default_import,
    namespace_import: raw.namespace_import,
    is_type_only: raw.is_type_only,
    range: raw.range,
  })
}

fn resolve_raw_re_export(
  importer: &ModuleSpecifier,
  raw: RawReExport,
) -> Option<ReExportInfo> {
  let resolved = resolve_specifier(importer, &raw.source)?;
  Some(ReExportInfo {
    specifier: resolved,
    named: raw.named,
    is_all: raw.is_all,
  })
}

impl Visit for ImportCollector {
  fn visit_import_decl(&mut self, node: &ast::ImportDecl) {
    let source = node.src.value.to_string_lossy().into_owned();
    let is_type_only = node.type_only;

    let mut named = Vec::new();
    let mut default_import = None;
    let mut namespace_import = None;

    for spec in &node.specifiers {
      match spec {
        ast::ImportSpecifier::Named(named_spec) => {
          let name = named_spec.imported.as_ref().map_or_else(
            || named_spec.local.sym.as_str().to_string(),
            |imported| match imported {
              ast::ModuleExportName::Ident(ident) => ident.sym.as_str().to_string(),
              ast::ModuleExportName::Str(s) => s.value.to_string_lossy().into_owned(),
            },
          );
          let alias = if named_spec.imported.is_some() {
            Some(named_spec.local.sym.as_str().to_string())
          } else {
            None
          };
          named.push(NamedImport {
            name,
            alias,
            is_type_only: named_spec.is_type_only || is_type_only,
          });
        }
        ast::ImportSpecifier::Default(default_spec) => {
          default_import = Some(default_spec.local.sym.as_str().to_string());
        }
        ast::ImportSpecifier::Namespace(ns_spec) => {
          namespace_import = Some(ns_spec.local.sym.as_str().to_string());
        }
      }
    }

    let range = (node.span.lo.0 as usize, node.span.hi.0 as usize);

    self.imports.push(RawImport {
      source,
      named,
      default_import,
      namespace_import,
      is_type_only,
      range,
    });
  }

  fn visit_call_expr(&mut self, node: &ast::CallExpr) {
    // Check for dynamic import() calls
    if let ast::Callee::Import(_) = &node.callee {
      if let Some(arg) = node.args.first() {
        if let Some(source) = extract_string_literal(&arg.expr) {
          let range = (node.span.lo.0 as usize, node.span.hi.0 as usize);
          self.dynamic_imports.push(RawImport {
            source,
            named: Vec::new(),
            default_import: None,
            namespace_import: None,
            is_type_only: false,
            range,
          });
        }
      }
    }

    // Continue visiting children
    node.visit_children_with(self);
  }

  fn visit_named_export(&mut self, node: &ast::NamedExport) {
    // Handle `export { foo } from './bar'` style re-exports
    if let Some(src) = &node.src {
      let source = src.value.to_string_lossy().into_owned();
      let mut named = Vec::new();

      for spec in &node.specifiers {
        match spec {
          ast::ExportSpecifier::Named(named_spec) => {
            let name = match &named_spec.orig {
              ast::ModuleExportName::Ident(ident) => ident.sym.as_str().to_string(),
              ast::ModuleExportName::Str(s) => s.value.to_string_lossy().into_owned(),
            };
            let alias = named_spec.exported.as_ref().map(|exp| match exp {
              ast::ModuleExportName::Ident(ident) => ident.sym.as_str().to_string(),
              ast::ModuleExportName::Str(s) => s.value.to_string_lossy().into_owned(),
            });
            named.push(NamedReExport { name, alias });
          }
          ast::ExportSpecifier::Namespace(ns_spec) => {
            let alias = match &ns_spec.name {
              ast::ModuleExportName::Ident(ident) => ident.sym.as_str().to_string(),
              ast::ModuleExportName::Str(s) => s.value.to_string_lossy().into_owned(),
            };
            named.push(NamedReExport {
              name: "*".to_string(),
              alias: Some(alias),
            });
          }
          ast::ExportSpecifier::Default(_) => {
            // `export default from` - treat as named re-export of "default"
            named.push(NamedReExport {
              name: "default".to_string(),
              alias: None,
            });
          }
        }
      }

      self.re_exports.push(RawReExport {
        source,
        named,
        is_all: false,
      });
    }
  }

  fn visit_export_all(&mut self, node: &ast::ExportAll) {
    // Handle `export * from './bar'`
    let source = node.src.value.to_string_lossy().into_owned();
    self.re_exports.push(RawReExport {
      source,
      named: Vec::new(),
      is_all: true,
    });
  }
}

/// Extract a string literal value from an expression.
fn extract_string_literal(expr: &ast::Expr) -> Option<String> {
  match expr {
    ast::Expr::Lit(ast::Lit::Str(s)) => Some(s.value.to_string_lossy().into_owned()),
    ast::Expr::Tpl(tpl) if tpl.exprs.is_empty() && tpl.quasis.len() == 1 => {
      // Template literal with no expressions, e.g., `import(`./foo`)`
      tpl.quasis.first().map(|q| q.raw.to_string())
    }
    _ => None,
  }
}

/// Resolve a specifier relative to the importer.
fn resolve_specifier(
  importer: &ModuleSpecifier,
  specifier: &str,
) -> Option<ModuleSpecifier> {
  // Try to resolve as URL first
  if let Ok(url) = ModuleSpecifier::parse(specifier) {
    return Some(url);
  }

  // Resolve relative specifiers
  if specifier.starts_with("./") || specifier.starts_with("../") {
    return importer.join(specifier).ok();
  }

  // For bare specifiers (npm:, node:, jsr:), try parsing
  if specifier.starts_with("npm:")
    || specifier.starts_with("node:")
    || specifier.starts_with("jsr:")
  {
    return ModuleSpecifier::parse(specifier).ok();
  }

  // For other bare specifiers, we can't resolve them here
  // The bundler's resolver will handle these
  // Create a synthetic specifier for tracking
  ModuleSpecifier::parse(&format!("bare:{}", specifier)).ok()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_analyze_static_imports() {
    let specifier = ModuleSpecifier::parse("file:///main.ts").unwrap();
    let source = r#"
      import { foo, bar as baz } from './foo.ts';
      import Default from './default.ts';
      import * as ns from './namespace.ts';
    "#;

    let result = analyze_imports(&specifier, source, MediaType::TypeScript).unwrap();

    assert_eq!(result.imports.len(), 3);

    // Check first import
    assert!(result.imports[0].specifier.as_str().ends_with("/foo.ts"));
    assert_eq!(result.imports[0].named.len(), 2);
    assert_eq!(result.imports[0].named[0].name, "foo");
    assert!(result.imports[0].named[0].alias.is_none());
    assert_eq!(result.imports[0].named[1].name, "bar");
    assert_eq!(result.imports[0].named[1].alias.as_deref(), Some("baz"));

    // Check default import
    assert!(result.imports[1].specifier.as_str().ends_with("/default.ts"));
    assert_eq!(result.imports[1].default_import.as_deref(), Some("Default"));

    // Check namespace import
    assert!(result.imports[2].specifier.as_str().ends_with("/namespace.ts"));
    assert_eq!(result.imports[2].namespace_import.as_deref(), Some("ns"));
  }

  #[test]
  fn test_analyze_dynamic_imports() {
    let specifier = ModuleSpecifier::parse("file:///main.ts").unwrap();
    let source = r#"
      const mod = await import('./dynamic.ts');
      import('./another.ts').then(m => m.foo());
    "#;

    let result = analyze_imports(&specifier, source, MediaType::TypeScript).unwrap();

    assert_eq!(result.dynamic_imports.len(), 2);
    assert!(result.dynamic_imports[0].specifier.as_str().ends_with("/dynamic.ts"));
    assert!(result.dynamic_imports[1].specifier.as_str().ends_with("/another.ts"));
  }

  #[test]
  fn test_analyze_re_exports() {
    let specifier = ModuleSpecifier::parse("file:///main.ts").unwrap();
    let source = r#"
      export { foo, bar as baz } from './foo.ts';
      export * from './all.ts';
      export * as ns from './namespace.ts';
    "#;

    let result = analyze_imports(&specifier, source, MediaType::TypeScript).unwrap();

    assert_eq!(result.re_exports.len(), 3);

    // Check named re-export
    assert!(result.re_exports[0].specifier.as_str().ends_with("/foo.ts"));
    assert_eq!(result.re_exports[0].named.len(), 2);
    assert!(!result.re_exports[0].is_all);

    // Check export all
    assert!(result.re_exports[1].specifier.as_str().ends_with("/all.ts"));
    assert!(result.re_exports[1].is_all);

    // Check namespace re-export
    assert!(result.re_exports[2].specifier.as_str().ends_with("/namespace.ts"));
    assert_eq!(result.re_exports[2].named.len(), 1);
    assert_eq!(result.re_exports[2].named[0].name, "*");
  }

  #[test]
  fn test_analyze_type_imports() {
    let specifier = ModuleSpecifier::parse("file:///main.ts").unwrap();
    let source = r#"
      import type { Foo } from './types.ts';
      import { type Bar } from './mixed.ts';
    "#;

    let result = analyze_imports(&specifier, source, MediaType::TypeScript).unwrap();

    assert_eq!(result.imports.len(), 2);

    // type import declaration
    assert!(result.imports[0].is_type_only);
    assert!(result.imports[0].named[0].is_type_only);

    // inline type import
    assert!(!result.imports[1].is_type_only);
    assert!(result.imports[1].named[0].is_type_only);
  }
}
