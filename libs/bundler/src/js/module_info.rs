// Copyright 2018-2026 the Deno authors. MIT license.

use rustc_hash::FxHashMap;

use super::scope::DeclId;
use super::scope::DeclKind;
use super::scope::ScopeAnalysis;

/// Complete import/export/binding analysis for a JS/TS module.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
  /// All import bindings.
  pub imports: Vec<ImportBinding>,
  /// All export bindings.
  pub exports: Vec<ExportBinding>,
  /// Top-level declarations.
  pub top_level_decls: Vec<TopLevelDecl>,
  /// Scope analysis with declarations and references.
  pub scope_analysis: ScopeAnalysis,
  /// Whether the module has import/export syntax (ESM).
  pub has_module_syntax: bool,
  /// Whether the module uses top-level await.
  pub has_tla: bool,
  /// Constant values exported by the module (for dead code elimination).
  pub constant_exports: FxHashMap<String, ConstantValue>,
  /// DeclId of the default export declaration, if any.
  pub default_export_decl_id: Option<DeclId>,
}

/// A constant value that can be determined at compile time.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstantValue {
  Number(f64),
  String(String),
  Bool(bool),
  Null,
  Undefined,
}

/// An import binding extracted from the module.
#[derive(Debug, Clone)]
pub struct ImportBinding {
  /// The local name this import is bound to.
  pub local_name: String,
  /// What is being imported (named, default, or namespace).
  pub imported: ImportedName,
  /// The module specifier string.
  pub source: String,
  /// The DeclId for this import's declaration in scope analysis.
  pub decl_id: DeclId,
}

/// What is being imported from a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedName {
  /// A named export: `import { foo } from './mod'`
  Named(String),
  /// The default export: `import foo from './mod'`
  Default,
  /// The namespace: `import * as mod from './mod'`
  Namespace,
}

/// An export binding extracted from the module.
#[derive(Debug, Clone)]
pub struct ExportBinding {
  /// The exported name (as seen by importers).
  pub exported_name: String,
  /// The local name, if different from exported name.
  pub local_name: Option<String>,
  /// The kind of export.
  pub kind: ExportKind,
  /// The DeclId of the local declaration this export refers to.
  /// `None` for re-exports (`ReExport`, `ReExportAll`) since the
  /// declaration lives in another module.
  pub decl_id: Option<DeclId>,
}

/// The kind of export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportKind {
  /// Export of a local binding: `export { foo }`
  Local,
  /// Re-export from another module: `export { foo } from './mod'`
  ReExport { source: String },
  /// Re-export all: `export * from './mod'`
  ReExportAll { source: String },
  /// Default export of a declaration: `export default function foo() {}`
  Default,
  /// Default export of an expression: `export default 42`
  DefaultExpression,
}

/// A top-level declaration in the module scope.
#[derive(Debug, Clone)]
pub struct TopLevelDecl {
  pub name: String,
  pub kind: DeclKind,
  pub decl_id: DeclId,
}
