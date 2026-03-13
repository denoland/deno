// Copyright 2018-2026 the Deno authors. MIT license.

//! Extract [`ModuleInfo`] from a SWC AST (via `deno_ast::ParsedSource`).
//!
//! This is the SWC equivalent of the bundler's `module_info_oxc.rs`.

use deno_ast::swc::ast::*;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use rustc_hash::FxHashMap;

/// Convert a WTF-8 atom (used for string literal values) to a String.
/// Module specifiers and identifiers are always valid UTF-8.
pub fn wtf8_to_string(atom: &deno_ast::swc::atoms::Wtf8Atom) -> String {
  String::from_utf8_lossy(atom.as_bytes()).into_owned()
}

use rustc_hash::FxHashSet;

use super::module_info::ConstantValue;
use super::module_info::ExportBinding;
use super::module_info::ExportKind;
use super::module_info::ImportBinding;
use super::module_info::ImportedName;
use super::module_info::ModuleInfo;
use super::module_info::NamespaceAccess;
use super::module_info::TopLevelDecl;
use super::scope::DeclId;
use super::scope::DeclKind;
use super::scope::ScopeAnalysis;
use super::scope_swc::analyze_scope;

/// Extract ModuleInfo from an AST program.
pub fn extract_module_info(program: &Program) -> ModuleInfo {
  let scope_analysis = analyze_scope(program);

  let mut imports = Vec::new();
  let mut exports = Vec::new();
  let mut has_module_syntax = false;

  match program {
    Program::Module(module) => {
      for item in &module.body {
        match item {
          ModuleItem::ModuleDecl(decl) => {
            has_module_syntax = true;
            extract_module_decl(
              decl,
              &scope_analysis,
              &mut imports,
              &mut exports,
            );
          }
          ModuleItem::Stmt(_) => {}
        }
      }
    }
    Program::Script(_) => {
      // Scripts have no module declarations.
    }
  }

  // Extract top-level declarations from module scope.
  let top_level_decls = if !scope_analysis.scopes.is_empty() {
    scope_analysis.scopes[0]
      .decls
      .iter()
      .map(|&decl_id| {
        let decl = scope_analysis.get_decl(decl_id);
        TopLevelDecl {
          name: decl.name.clone(),
          kind: decl.kind,
          decl_id,
        }
      })
      .collect()
  } else {
    Vec::new()
  };

  // Detect TLA.
  let has_tla = detect_tla(&program);

  // Detect constant exports.
  let constant_exports = extract_constant_exports(&program, &exports);

  // Find default export decl_id.
  let default_export_decl_id = exports.iter().find_map(|e| {
    if e.kind == ExportKind::DefaultExpression {
      // Find the synthetic __default decl in scope analysis.
      scope_analysis
        .declarations
        .iter()
        .find(|d| d.name == "__default")
        .map(|d| d.id)
    } else {
      None
    }
  });

  // Analyze namespace import usage patterns.
  let namespace_accesses =
    analyze_namespace_accesses(program, &imports);

  ModuleInfo {
    imports,
    exports,
    top_level_decls,
    scope_analysis,
    has_module_syntax,
    has_tla,
    constant_exports,
    default_export_decl_id,
    namespace_accesses,
  }
}

fn extract_module_decl(
  decl: &ModuleDecl,
  scope_analysis: &ScopeAnalysis,
  imports: &mut Vec<ImportBinding>,
  exports: &mut Vec<ExportBinding>,
) {
  match decl {
    ModuleDecl::Import(import) => {
      if import.type_only {
        return;
      }
      extract_import(import, scope_analysis, imports);
    }
    ModuleDecl::ExportNamed(export) => {
      if export.type_only {
        return;
      }
      extract_export_named(export, scope_analysis, exports);
    }
    ModuleDecl::ExportAll(export) => {
      if export.type_only {
        return;
      }
      let source = wtf8_to_string(&export.src.value);
      exports.push(ExportBinding {
        exported_name: "*".to_string(),
        local_name: None,
        kind: ExportKind::ReExportAll { source },
        decl_id: None,
      });
    }
    ModuleDecl::ExportDefaultDecl(export) => {
      extract_export_default_decl(export, scope_analysis, exports);
    }
    ModuleDecl::ExportDefaultExpr(_) => {
      let decl_id = find_module_scope_decl(scope_analysis, "__default");
      exports.push(ExportBinding {
        exported_name: "default".to_string(),
        local_name: Some("__default".to_string()),
        kind: ExportKind::DefaultExpression,
        decl_id,
      });
    }
    ModuleDecl::ExportDecl(export) => {
      extract_export_decl(&export.decl, scope_analysis, exports);
    }
    ModuleDecl::TsImportEquals(_)
    | ModuleDecl::TsExportAssignment(_)
    | ModuleDecl::TsNamespaceExport(_) => {
      // TypeScript-specific, skip for now.
    }
  }
}

// ============================================================================
// Import extraction
// ============================================================================

fn extract_import(
  import: &ImportDecl,
  scope_analysis: &ScopeAnalysis,
  imports: &mut Vec<ImportBinding>,
) {
  let source = wtf8_to_string(&import.src.value);

  for spec in &import.specifiers {
    match spec {
      ImportSpecifier::Default(s) => {
        let local = s.local.sym.to_string();
        let decl_id =
          find_import_decl(scope_analysis, &local).unwrap_or(DeclId(0));
        imports.push(ImportBinding {
          local_name: local,
          imported: ImportedName::Default,
          source: source.clone(),
          decl_id,
        });
      }
      ImportSpecifier::Namespace(s) => {
        let local = s.local.sym.to_string();
        let decl_id =
          find_import_decl(scope_analysis, &local).unwrap_or(DeclId(0));
        imports.push(ImportBinding {
          local_name: local,
          imported: ImportedName::Namespace,
          source: source.clone(),
          decl_id,
        });
      }
      ImportSpecifier::Named(s) => {
        if s.is_type_only {
          continue;
        }
        let local = s.local.sym.to_string();
        let imported_name = match &s.imported {
          Some(ModuleExportName::Ident(id)) => id.sym.to_string(),
          Some(ModuleExportName::Str(lit)) => wtf8_to_string(&lit.value),
          None => local.clone(),
        };
        let decl_id =
          find_import_decl(scope_analysis, &local).unwrap_or(DeclId(0));
        imports.push(ImportBinding {
          local_name: local,
          imported: ImportedName::Named(imported_name),
          source: source.clone(),
          decl_id,
        });
      }
    }
  }
}

fn find_import_decl(
  scope_analysis: &ScopeAnalysis,
  name: &str,
) -> Option<DeclId> {
  if scope_analysis.scopes.is_empty() {
    return None;
  }
  let module_scope = &scope_analysis.scopes[0];
  for &decl_id in &module_scope.decls {
    let decl = scope_analysis.get_decl(decl_id);
    if decl.name == name && decl.kind == DeclKind::Import {
      return Some(decl_id);
    }
  }
  None
}

/// Find any declaration in the module scope by name (not restricted to imports).
fn find_module_scope_decl(
  scope_analysis: &ScopeAnalysis,
  name: &str,
) -> Option<DeclId> {
  if scope_analysis.scopes.is_empty() {
    return None;
  }
  let module_scope = &scope_analysis.scopes[0];
  for &decl_id in &module_scope.decls {
    let decl = scope_analysis.get_decl(decl_id);
    if decl.name == name {
      return Some(decl_id);
    }
  }
  None
}

// ============================================================================
// Export extraction
// ============================================================================

fn extract_export_named(
  export: &NamedExport,
  scope_analysis: &ScopeAnalysis,
  exports: &mut Vec<ExportBinding>,
) {
  let source = export.src.as_ref().map(|s| wtf8_to_string(&s.value));

  for spec in &export.specifiers {
    match spec {
      ExportSpecifier::Named(s) => {
        if s.is_type_only {
          continue;
        }
        let local = module_export_name_to_string(&s.orig);
        let exported = s
          .exported
          .as_ref()
          .map(module_export_name_to_string)
          .unwrap_or_else(|| local.clone());
        let (kind, decl_id) = if let Some(ref src) = source {
          (
            ExportKind::ReExport {
              source: src.clone(),
            },
            None,
          )
        } else {
          (
            ExportKind::Local,
            find_module_scope_decl(scope_analysis, &local),
          )
        };
        exports.push(ExportBinding {
          exported_name: exported,
          local_name: Some(local),
          kind,
          decl_id,
        });
      }
      ExportSpecifier::Namespace(s) => {
        let exported = module_export_name_to_string(&s.name);
        let src = source.clone().unwrap_or_default();
        exports.push(ExportBinding {
          exported_name: exported,
          local_name: None,
          kind: ExportKind::ReExport { source: src },
          decl_id: None,
        });
      }
      ExportSpecifier::Default(_) => {
        // `export v from './mod'` — rarely used, treat as default re-export.
      }
    }
  }
}

fn extract_export_default_decl(
  export: &ExportDefaultDecl,
  scope_analysis: &ScopeAnalysis,
  exports: &mut Vec<ExportBinding>,
) {
  match &export.decl {
    DefaultDecl::Fn(f) => {
      if let Some(ident) = &f.ident {
        let name = ident.sym.to_string();
        let decl_id = find_module_scope_decl(scope_analysis, &name);
        exports.push(ExportBinding {
          exported_name: "default".to_string(),
          local_name: Some(name),
          kind: ExportKind::Default,
          decl_id,
        });
      } else {
        let decl_id = find_module_scope_decl(scope_analysis, "__default");
        exports.push(ExportBinding {
          exported_name: "default".to_string(),
          local_name: Some("__default".to_string()),
          kind: ExportKind::DefaultExpression,
          decl_id,
        });
      }
    }
    DefaultDecl::Class(c) => {
      if let Some(ident) = &c.ident {
        let name = ident.sym.to_string();
        let decl_id = find_module_scope_decl(scope_analysis, &name);
        exports.push(ExportBinding {
          exported_name: "default".to_string(),
          local_name: Some(name),
          kind: ExportKind::Default,
          decl_id,
        });
      } else {
        let decl_id = find_module_scope_decl(scope_analysis, "__default");
        exports.push(ExportBinding {
          exported_name: "default".to_string(),
          local_name: Some("__default".to_string()),
          kind: ExportKind::DefaultExpression,
          decl_id,
        });
      }
    }
    DefaultDecl::TsInterfaceDecl(_) => {
      // Type-only, skip.
    }
  }
}

fn extract_export_decl(
  decl: &Decl,
  scope_analysis: &ScopeAnalysis,
  exports: &mut Vec<ExportBinding>,
) {
  match decl {
    Decl::Var(var_decl) => {
      for declarator in &var_decl.decls {
        extract_pat_names(&declarator.name, scope_analysis, exports);
      }
    }
    Decl::Fn(fn_decl) => {
      let name = fn_decl.ident.sym.to_string();
      let decl_id = find_module_scope_decl(scope_analysis, &name);
      exports.push(ExportBinding {
        exported_name: name.clone(),
        local_name: Some(name),
        kind: ExportKind::Local,
        decl_id,
      });
    }
    Decl::Class(class_decl) => {
      let name = class_decl.ident.sym.to_string();
      let decl_id = find_module_scope_decl(scope_analysis, &name);
      exports.push(ExportBinding {
        exported_name: name.clone(),
        local_name: Some(name),
        kind: ExportKind::Local,
        decl_id,
      });
    }
    Decl::TsEnum(en) => {
      let name = en.id.sym.to_string();
      let decl_id = find_module_scope_decl(scope_analysis, &name);
      exports.push(ExportBinding {
        exported_name: name.clone(),
        local_name: Some(name),
        kind: ExportKind::Local,
        decl_id,
      });
    }
    Decl::TsInterface(_) | Decl::TsTypeAlias(_) | Decl::TsModule(_) => {
      // Type-only, skip.
    }
    Decl::Using(_) => {
      // `using` declarations — handle binding names.
      // For now, skip.
    }
  }
}

fn extract_pat_names(
  pat: &Pat,
  scope_analysis: &ScopeAnalysis,
  exports: &mut Vec<ExportBinding>,
) {
  match pat {
    Pat::Ident(id) => {
      let name = id.id.sym.to_string();
      let decl_id = find_module_scope_decl(scope_analysis, &name);
      exports.push(ExportBinding {
        exported_name: name.clone(),
        local_name: Some(name),
        kind: ExportKind::Local,
        decl_id,
      });
    }
    Pat::Object(obj) => {
      for prop in &obj.props {
        match prop {
          ObjectPatProp::KeyValue(kv) => {
            extract_pat_names(&kv.value, scope_analysis, exports);
          }
          ObjectPatProp::Assign(assign) => {
            let name = assign.key.sym.to_string();
            let decl_id = find_module_scope_decl(scope_analysis, &name);
            exports.push(ExportBinding {
              exported_name: name.clone(),
              local_name: Some(name),
              kind: ExportKind::Local,
              decl_id,
            });
          }
          ObjectPatProp::Rest(rest) => {
            extract_pat_names(&rest.arg, scope_analysis, exports);
          }
        }
      }
    }
    Pat::Array(arr) => {
      for elem in arr.elems.iter().flatten() {
        extract_pat_names(elem, scope_analysis, exports);
      }
    }
    Pat::Rest(rest) => {
      extract_pat_names(&rest.arg, scope_analysis, exports);
    }
    Pat::Assign(assign) => {
      extract_pat_names(&assign.left, scope_analysis, exports);
    }
    Pat::Expr(_) | Pat::Invalid(_) => {}
  }
}

fn module_export_name_to_string(name: &ModuleExportName) -> String {
  match name {
    ModuleExportName::Ident(id) => id.sym.to_string(),
    ModuleExportName::Str(lit) => wtf8_to_string(&lit.value),
  }
}

// ============================================================================
// Namespace import access analysis
// ============================================================================

/// Analyze how namespace imports are used in the module.
///
/// For each `import * as ns from '...'`, determines whether only specific
/// properties are accessed (`ns.foo`, `ns.bar`) or if the namespace escaped
/// (passed as argument, spread, assigned to another variable, etc.).
fn analyze_namespace_accesses(
  program: &Program,
  imports: &[ImportBinding],
) -> FxHashMap<DeclId, NamespaceAccess> {
  // Collect namespace import local names → DeclIds.
  let ns_bindings: FxHashMap<String, DeclId> = imports
    .iter()
    .filter(|i| matches!(i.imported, ImportedName::Namespace))
    .map(|i| (i.local_name.clone(), i.decl_id))
    .collect();

  if ns_bindings.is_empty() {
    return FxHashMap::default();
  }

  let mut visitor = NamespaceAccessVisitor {
    ns_bindings: &ns_bindings,
    accesses: ns_bindings
      .values()
      .map(|&id| (id, NamespaceAccess::Properties(FxHashSet::default())))
      .collect(),
  };
  program.visit_with(&mut visitor);
  visitor.accesses
}

/// Visitor that tracks how namespace identifiers are used.
struct NamespaceAccessVisitor<'a> {
  /// Maps namespace local name → DeclId.
  ns_bindings: &'a FxHashMap<String, DeclId>,
  /// Accumulated access info per DeclId.
  accesses: FxHashMap<DeclId, NamespaceAccess>,
}

impl<'a> NamespaceAccessVisitor<'a> {
  /// Mark a namespace as escaped (all exports live).
  fn mark_escaped(&mut self, name: &str) {
    if let Some(&decl_id) = self.ns_bindings.get(name) {
      self.accesses.insert(decl_id, NamespaceAccess::Escaped);
    }
  }

  /// Record a property access on a namespace.
  fn record_property(&mut self, name: &str, prop: String) {
    if let Some(&decl_id) = self.ns_bindings.get(name) {
      match self.accesses.get_mut(&decl_id) {
        Some(NamespaceAccess::Properties(set)) => {
          set.insert(prop);
        }
        Some(NamespaceAccess::Escaped) => {
          // Already escaped, nothing to do.
        }
        None => {
          let mut set = FxHashSet::default();
          set.insert(prop);
          self
            .accesses
            .insert(decl_id, NamespaceAccess::Properties(set));
        }
      }
    }
  }

  /// Check if an expression is a namespace identifier, returning the name.
  fn namespace_name(&self, expr: &Expr) -> Option<String> {
    if let Expr::Ident(id) = expr {
      let name = id.sym.to_string();
      if self.ns_bindings.contains_key(&name) {
        return Some(name);
      }
    }
    None
  }
}

impl Visit for NamespaceAccessVisitor<'_> {
  fn visit_member_expr(&mut self, node: &MemberExpr) {
    // `ns.foo` or `ns["foo"]` — record property access.
    if let Some(ns_name) = self.namespace_name(&node.obj) {
      match &node.prop {
        MemberProp::Ident(prop) => {
          self.record_property(&ns_name, prop.sym.to_string());
          return;
        }
        MemberProp::Computed(comp) => {
          if let Expr::Lit(Lit::Str(s)) = &*comp.expr {
            self.record_property(&ns_name, wtf8_to_string(&s.value));
            return;
          }
          // Computed with non-literal — escape.
          self.mark_escaped(&ns_name);
          return;
        }
        MemberProp::PrivateName(_) => {
          self.mark_escaped(&ns_name);
          return;
        }
      }
    }
    // Not a namespace member — recurse normally.
    node.visit_children_with(self);
  }

  fn visit_expr(&mut self, expr: &Expr) {
    match expr {
      Expr::Member(_) => {
        // Handled by visit_member_expr.
        expr.visit_children_with(self);
      }
      Expr::Ident(id) => {
        let name = id.sym.to_string();
        if self.ns_bindings.contains_key(&name) {
          // Bare namespace ident in a non-member context = escaped.
          self.mark_escaped(&name);
        }
      }
      _ => {
        expr.visit_children_with(self);
      }
    }
  }

  // Don't descend into import declarations (the binding ident there
  // is a declaration, not a usage).
  fn visit_import_decl(&mut self, _: &ImportDecl) {}
}

// ============================================================================
// Top-level await detection
// ============================================================================

fn detect_tla(program: &Program) -> bool {
  let mut detector = TlaDetector {
    function_depth: 0,
    has_tla: false,
  };
  program.visit_with(&mut detector);
  detector.has_tla
}

struct TlaDetector {
  function_depth: u32,
  has_tla: bool,
}

impl Visit for TlaDetector {
  fn visit_function(&mut self, n: &Function) {
    self.function_depth += 1;
    if !self.has_tla {
      n.visit_children_with(self);
    }
    self.function_depth -= 1;
  }

  fn visit_arrow_expr(&mut self, n: &ArrowExpr) {
    self.function_depth += 1;
    if !self.has_tla {
      n.visit_children_with(self);
    }
    self.function_depth -= 1;
  }

  fn visit_class_member(&mut self, n: &ClassMember) {
    // Static blocks are like functions for TLA purposes.
    if let ClassMember::StaticBlock(_) = n {
      self.function_depth += 1;
      if !self.has_tla {
        n.visit_children_with(self);
      }
      self.function_depth -= 1;
    } else {
      n.visit_children_with(self);
    }
  }

  fn visit_await_expr(&mut self, n: &AwaitExpr) {
    if self.function_depth == 0 {
      self.has_tla = true;
    }
    if !self.has_tla {
      n.visit_children_with(self);
    }
  }

  fn visit_for_of_stmt(&mut self, n: &ForOfStmt) {
    if self.function_depth == 0 && n.is_await {
      self.has_tla = true;
    }
    if !self.has_tla {
      n.visit_children_with(self);
    }
  }
}

// ============================================================================
// Constant export extraction
// ============================================================================

fn extract_constant_exports(
  program: &Program,
  exports: &[ExportBinding],
) -> FxHashMap<String, ConstantValue> {
  let mut local_constants: FxHashMap<String, ConstantValue> =
    FxHashMap::default();

  let items = match program {
    Program::Module(m) => &m.body,
    Program::Script(_) => return FxHashMap::default(),
  };

  // First pass: collect all const declarations.
  for item in items {
    match item {
      ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export_decl)) => {
        if let Decl::Var(var_decl) = &export_decl.decl {
          if var_decl.kind == VarDeclKind::Const {
            collect_const_values(&var_decl.decls, &mut local_constants);
          }
        }
      }
      ModuleItem::Stmt(Stmt::Decl(Decl::Var(var_decl))) => {
        if var_decl.kind == VarDeclKind::Const {
          collect_const_values(&var_decl.decls, &mut local_constants);
        }
      }
      _ => {}
    }
  }

  // Second pass: map exported names to constant values.
  let mut result = FxHashMap::default();
  for export in exports {
    if export.kind != ExportKind::Local {
      continue;
    }
    let local_name = export
      .local_name
      .as_deref()
      .unwrap_or(&export.exported_name);
    if let Some(val) = local_constants.get(local_name) {
      result.insert(export.exported_name.clone(), val.clone());
    }
  }

  result
}

fn collect_const_values(
  decls: &[VarDeclarator],
  constants: &mut FxHashMap<String, ConstantValue>,
) {
  for decl in decls {
    if let Pat::Ident(id) = &decl.name {
      if let Some(init) = &decl.init {
        if let Some(val) = try_eval_const_expr(init, constants) {
          constants.insert(id.id.sym.to_string(), val);
        }
      }
    }
  }
}

fn try_eval_const_expr(
  expr: &Expr,
  locals: &FxHashMap<String, ConstantValue>,
) -> Option<ConstantValue> {
  match expr {
    Expr::Lit(Lit::Num(n)) => Some(ConstantValue::Number(n.value)),
    Expr::Lit(Lit::Str(s)) => {
      Some(ConstantValue::String(wtf8_to_string(&s.value)))
    }
    Expr::Lit(Lit::Bool(b)) => Some(ConstantValue::Bool(b.value)),
    Expr::Lit(Lit::Null(_)) => Some(ConstantValue::Null),
    Expr::Ident(id) if &*id.sym == "undefined" => {
      Some(ConstantValue::Undefined)
    }
    Expr::Ident(id) => locals.get(&*id.sym).cloned(),
    Expr::Paren(paren) => try_eval_const_expr(&paren.expr, locals),
    Expr::Unary(unary) => {
      let operand = try_eval_const_expr(&unary.arg, locals)?;
      match unary.op {
        UnaryOp::Tilde => {
          if let ConstantValue::Number(n) = operand {
            Some(ConstantValue::Number(!(n as i32) as f64))
          } else {
            None
          }
        }
        UnaryOp::Minus => {
          if let ConstantValue::Number(n) = operand {
            Some(ConstantValue::Number(-n))
          } else {
            None
          }
        }
        UnaryOp::Plus => {
          if let ConstantValue::Number(n) = operand {
            Some(ConstantValue::Number(n))
          } else {
            None
          }
        }
        UnaryOp::Bang => match operand {
          ConstantValue::Bool(b) => Some(ConstantValue::Bool(!b)),
          ConstantValue::Number(n) => Some(ConstantValue::Bool(n == 0.0)),
          ConstantValue::Null | ConstantValue::Undefined => {
            Some(ConstantValue::Bool(true))
          }
          ConstantValue::String(s) => {
            Some(ConstantValue::Bool(s.is_empty()))
          }
        },
        _ => None,
      }
    }
    Expr::Bin(bin) => {
      let left = try_eval_const_expr(&bin.left, locals)?;
      let right = try_eval_const_expr(&bin.right, locals)?;

      let (ConstantValue::Number(l), ConstantValue::Number(r)) =
        (&left, &right)
      else {
        // String concatenation.
        if bin.op == BinaryOp::Add {
          if let (ConstantValue::String(l), ConstantValue::String(r)) =
            (&left, &right)
          {
            return Some(ConstantValue::String(format!("{l}{r}")));
          }
        }
        return None;
      };

      let l = *l;
      let r = *r;

      let result = match bin.op {
        BinaryOp::Add => l + r,
        BinaryOp::Sub => l - r,
        BinaryOp::Mul => l * r,
        BinaryOp::Div if r != 0.0 => l / r,
        BinaryOp::Mod if r != 0.0 => l % r,
        BinaryOp::BitOr => ((l as i32) | (r as i32)) as f64,
        BinaryOp::BitAnd => ((l as i32) & (r as i32)) as f64,
        BinaryOp::BitXor => ((l as i32) ^ (r as i32)) as f64,
        BinaryOp::LShift => ((l as i32) << ((r as u32) & 31)) as f64,
        BinaryOp::RShift => ((l as i32) >> ((r as u32) & 31)) as f64,
        BinaryOp::ZeroFillRShift => {
          ((l as u32) >> ((r as u32) & 31)) as f64
        }
        BinaryOp::Exp => l.powf(r),
        _ => return None,
      };

      Some(ConstantValue::Number(result))
    }
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use deno_ast::MediaType;
  use deno_ast::ModuleSpecifier;
  use deno_ast::ParseParams;

  use super::*;

  fn parse_and_extract(source: &str) -> ModuleInfo {
    let parsed = deno_ast::parse_module(ParseParams {
      specifier: ModuleSpecifier::parse("file:///test.mjs").unwrap(),
      text: source.to_string().into(),
      media_type: MediaType::JavaScript,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })
    .unwrap();
    extract_module_info(&parsed.program())
  }

  fn parse_ts_and_extract(source: &str) -> ModuleInfo {
    let parsed = deno_ast::parse_module(ParseParams {
      specifier: ModuleSpecifier::parse("file:///test.ts").unwrap(),
      text: source.to_string().into(),
      media_type: MediaType::TypeScript,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })
    .unwrap();
    extract_module_info(&parsed.program())
  }

  // --- Import tests ---

  #[test]
  fn test_import_default() {
    let info = parse_and_extract("import foo from './mod';");
    assert_eq!(info.imports.len(), 1);
    assert_eq!(info.imports[0].local_name, "foo");
    assert_eq!(info.imports[0].imported, ImportedName::Default);
    assert_eq!(info.imports[0].source, "./mod");
  }

  #[test]
  fn test_import_named() {
    let info =
      parse_and_extract("import { foo, bar as baz } from './mod';");
    assert_eq!(info.imports.len(), 2);
    assert_eq!(info.imports[0].local_name, "foo");
    assert_eq!(
      info.imports[0].imported,
      ImportedName::Named("foo".to_string())
    );
    assert_eq!(info.imports[1].local_name, "baz");
    assert_eq!(
      info.imports[1].imported,
      ImportedName::Named("bar".to_string())
    );
  }

  #[test]
  fn test_import_namespace() {
    let info = parse_and_extract("import * as ns from './mod';");
    assert_eq!(info.imports.len(), 1);
    assert_eq!(info.imports[0].local_name, "ns");
    assert_eq!(info.imports[0].imported, ImportedName::Namespace);
  }

  #[test]
  fn test_import_type_skipped() {
    let info =
      parse_ts_and_extract("import type { Foo } from './mod';");
    assert_eq!(info.imports.len(), 0);
  }

  #[test]
  fn test_import_type_specifier_skipped() {
    let info =
      parse_ts_and_extract("import { type Foo, bar } from './mod';");
    assert_eq!(info.imports.len(), 1);
    assert_eq!(info.imports[0].local_name, "bar");
  }

  // --- Export tests ---

  #[test]
  fn test_export_named() {
    let info =
      parse_and_extract("const foo = 1; const bar = 2; export { foo, bar as baz };");
    assert_eq!(info.exports.len(), 2);
    assert_eq!(info.exports[0].exported_name, "foo");
    assert_eq!(info.exports[0].local_name.as_deref(), Some("foo"));
    assert_eq!(info.exports[0].kind, ExportKind::Local);
    assert_eq!(info.exports[1].exported_name, "baz");
    assert_eq!(info.exports[1].local_name.as_deref(), Some("bar"));
  }

  #[test]
  fn test_export_default_literal() {
    let info = parse_and_extract("export default 42;");
    assert_eq!(info.exports.len(), 1);
    assert_eq!(info.exports[0].exported_name, "default");
    assert_eq!(
      info.exports[0].local_name.as_deref(),
      Some("__default")
    );
    assert_eq!(info.exports[0].kind, ExportKind::DefaultExpression);
  }

  #[test]
  fn test_export_default_named_function() {
    let info =
      parse_and_extract("export default function foo() {}");
    assert_eq!(info.exports.len(), 1);
    assert_eq!(info.exports[0].exported_name, "default");
    assert_eq!(info.exports[0].local_name.as_deref(), Some("foo"));
    assert_eq!(info.exports[0].kind, ExportKind::Default);
  }

  #[test]
  fn test_export_default_anonymous_function() {
    let info = parse_and_extract("export default function() {}");
    assert_eq!(info.exports.len(), 1);
    assert_eq!(info.exports[0].exported_name, "default");
    assert_eq!(
      info.exports[0].local_name.as_deref(),
      Some("__default")
    );
    assert_eq!(info.exports[0].kind, ExportKind::DefaultExpression);
  }

  #[test]
  fn test_export_default_named_class() {
    let info = parse_and_extract("export default class Foo {}");
    assert_eq!(info.exports.len(), 1);
    assert_eq!(info.exports[0].local_name.as_deref(), Some("Foo"));
    assert_eq!(info.exports[0].kind, ExportKind::Default);
  }

  #[test]
  fn test_export_const() {
    let info = parse_and_extract("export const x = 1;");
    assert_eq!(info.exports.len(), 1);
    assert_eq!(info.exports[0].exported_name, "x");
    assert_eq!(info.exports[0].kind, ExportKind::Local);
  }

  #[test]
  fn test_export_function() {
    let info = parse_and_extract("export function foo() {}");
    assert_eq!(info.exports.len(), 1);
    assert_eq!(info.exports[0].exported_name, "foo");
    assert_eq!(info.exports[0].kind, ExportKind::Local);
  }

  #[test]
  fn test_export_class() {
    let info = parse_and_extract("export class Foo {}");
    assert_eq!(info.exports.len(), 1);
    assert_eq!(info.exports[0].exported_name, "Foo");
    assert_eq!(info.exports[0].kind, ExportKind::Local);
  }

  #[test]
  fn test_export_star() {
    let info = parse_and_extract("export * from './mod';");
    assert_eq!(info.exports.len(), 1);
    assert_eq!(info.exports[0].exported_name, "*");
    assert!(matches!(
      info.exports[0].kind,
      ExportKind::ReExportAll { .. }
    ));
  }

  #[test]
  fn test_reexport() {
    let info =
      parse_and_extract("export { foo, bar as baz } from './mod';");
    assert_eq!(info.exports.len(), 2);
    assert_eq!(info.exports[0].exported_name, "foo");
    assert!(matches!(
      info.exports[0].kind,
      ExportKind::ReExport { .. }
    ));
    assert_eq!(info.exports[1].exported_name, "baz");
  }

  // --- Module syntax detection ---

  #[test]
  fn test_has_module_syntax() {
    let info = parse_and_extract("import './foo';");
    assert!(info.has_module_syntax);
  }

  #[test]
  fn test_no_module_syntax() {
    let info = parse_and_extract("const x = 1;");
    assert!(!info.has_module_syntax);
  }

  // --- TLA detection ---

  #[test]
  fn test_tla_detected() {
    let info = parse_and_extract("const x = await fetch('/api');");
    assert!(info.has_tla);
  }

  #[test]
  fn test_no_tla_in_function() {
    let info = parse_and_extract(
      "async function foo() { await fetch('/api'); }",
    );
    assert!(!info.has_tla);
  }

  #[test]
  fn test_no_tla_in_arrow() {
    let info = parse_and_extract(
      "const foo = async () => await fetch('/api');",
    );
    assert!(!info.has_tla);
  }

  #[test]
  fn test_tla_for_await() {
    let info =
      parse_and_extract("for await (const x of iter) {}");
    assert!(info.has_tla);
  }

  // --- Destructuring exports ---

  #[test]
  fn test_export_destructured_object() {
    let info = parse_and_extract("export const { a, b: c } = obj;");
    assert_eq!(info.exports.len(), 2);
    let names: Vec<&str> =
      info.exports.iter().map(|e| e.exported_name.as_str()).collect();
    assert!(names.contains(&"a"));
    assert!(names.contains(&"c"));
  }

  #[test]
  fn test_export_destructured_array() {
    let info = parse_and_extract("export const [a, b] = arr;");
    assert_eq!(info.exports.len(), 2);
    let names: Vec<&str> =
      info.exports.iter().map(|e| e.exported_name.as_str()).collect();
    assert!(names.contains(&"a"));
    assert!(names.contains(&"b"));
  }

  // --- Top level decls ---

  #[test]
  fn test_top_level_decls() {
    let info = parse_and_extract(
      "const x = 1; function foo() {} class Bar {}",
    );
    let names: Vec<&str> =
      info.top_level_decls.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"x"));
    assert!(names.contains(&"foo"));
    assert!(names.contains(&"Bar"));
  }

  // --- Constant exports ---

  #[test]
  fn test_constant_export_number() {
    let info = parse_and_extract("export const X = 42;");
    assert_eq!(info.constant_exports.len(), 1);
    assert_eq!(
      *info.constant_exports.get("X").unwrap(),
      ConstantValue::Number(42.0)
    );
  }

  #[test]
  fn test_constant_export_string() {
    let info = parse_and_extract("export const S = \"hello\";");
    assert_eq!(
      *info.constant_exports.get("S").unwrap(),
      ConstantValue::String("hello".to_string())
    );
  }

  #[test]
  fn test_constant_export_binary() {
    let info = parse_and_extract("export const X = 1 << 5;");
    assert_eq!(
      *info.constant_exports.get("X").unwrap(),
      ConstantValue::Number(32.0)
    );
  }

  #[test]
  fn test_constant_export_cross_ref() {
    let info = parse_and_extract(
      "const A = 1; const B = 2; export const C = A | B;",
    );
    assert_eq!(
      *info.constant_exports.get("C").unwrap(),
      ConstantValue::Number(3.0)
    );
  }

  #[test]
  fn test_constant_export_preact_style() {
    let info = parse_and_extract(
      "export const MODE_HYDRATE = 1 << 5;\n\
       export const MODE_SUSPENDED = 1 << 7;\n\
       export const RESET_MODE = ~(MODE_HYDRATE | MODE_SUSPENDED);",
    );
    assert_eq!(info.constant_exports.len(), 3);
    assert_eq!(
      *info.constant_exports.get("MODE_HYDRATE").unwrap(),
      ConstantValue::Number(32.0)
    );
    assert_eq!(
      *info.constant_exports.get("MODE_SUSPENDED").unwrap(),
      ConstantValue::Number(128.0)
    );
    assert_eq!(
      *info.constant_exports.get("RESET_MODE").unwrap(),
      ConstantValue::Number(-161.0)
    );
  }

  #[test]
  fn test_non_constant_export() {
    let info = parse_and_extract("export const X = {};");
    assert!(info.constant_exports.is_empty());
  }

  // --- Namespace access tracking ---

  #[test]
  fn test_namespace_property_access() {
    let info = parse_and_extract(
      "import * as ns from './lib';\nns.foo();\nns.bar;",
    );
    assert_eq!(info.imports.len(), 1);
    let decl_id = info.imports[0].decl_id;
    let access = info.namespace_accesses.get(&decl_id).unwrap();
    match access {
      NamespaceAccess::Properties(props) => {
        assert!(props.contains("foo"));
        assert!(props.contains("bar"));
        assert_eq!(props.len(), 2);
      }
      NamespaceAccess::Escaped => panic!("expected Properties, got Escaped"),
    }
  }

  #[test]
  fn test_namespace_escaped_as_argument() {
    let info = parse_and_extract(
      "import * as ns from './lib';\ndoSomething(ns);",
    );
    let decl_id = info.imports[0].decl_id;
    let access = info.namespace_accesses.get(&decl_id).unwrap();
    assert_eq!(*access, NamespaceAccess::Escaped);
  }

  #[test]
  fn test_namespace_escaped_assignment() {
    let info = parse_and_extract(
      "import * as ns from './lib';\nconst x = ns;",
    );
    let decl_id = info.imports[0].decl_id;
    let access = info.namespace_accesses.get(&decl_id).unwrap();
    assert_eq!(*access, NamespaceAccess::Escaped);
  }

  #[test]
  fn test_namespace_computed_string_property() {
    let info = parse_and_extract(
      "import * as ns from './lib';\nns[\"foo\"];",
    );
    let decl_id = info.imports[0].decl_id;
    let access = info.namespace_accesses.get(&decl_id).unwrap();
    match access {
      NamespaceAccess::Properties(props) => {
        assert!(props.contains("foo"));
      }
      NamespaceAccess::Escaped => panic!("expected Properties"),
    }
  }

  #[test]
  fn test_namespace_computed_dynamic_escapes() {
    let info = parse_and_extract(
      "import * as ns from './lib';\nconst k = 'foo';\nns[k];",
    );
    let decl_id = info.imports[0].decl_id;
    let access = info.namespace_accesses.get(&decl_id).unwrap();
    assert_eq!(*access, NamespaceAccess::Escaped);
  }

  #[test]
  fn test_no_namespace_for_named_import() {
    let info = parse_and_extract(
      "import { foo } from './lib';\nfoo();",
    );
    assert!(info.namespace_accesses.is_empty());
  }
}
