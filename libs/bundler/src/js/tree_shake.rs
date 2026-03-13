// Copyright 2018-2026 the Deno authors. MIT license.

//! Tree shaking: remove unused top-level statements from a module.
//!
//! The algorithm has 4 phases:
//!
//! 1. **Classify** each top-level statement: determine which names it declares,
//!    which names it references, and whether it has side effects.
//! 2. **Build intra-module dependency graph**: map name → declaring statement,
//!    then for each statement find which other statements it depends on.
//! 3. **Seed and propagate**: mark statements that have side effects or declare
//!    live exports, then BFS to include transitive dependencies.
//! 4. **Remove** excluded statements.
//!
//! This operates on transpiled JS (post-TypeScript), so no TS-specific nodes.

use std::collections::HashMap;

use deno_ast::swc::ast::*;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParseParams;
use deno_ast::ParsedSource;
use rustc_hash::FxHashSet;

use super::scope::DeclId;
use super::scope::ScopeAnalysis;

/// Tree-shake a module's source code, removing unused top-level statements.
///
/// `source`: The module's source text (must match `parsed`).
/// `parsed`: The pre-parsed AST (avoids redundant re-parsing).
/// `live_export_decls`: The set of DeclIds whose exports are actually used
/// by importers. If `None`, all exports are considered live (no shaking).
/// `scope_analysis`: The module's scope analysis, used to map declared names
/// to DeclIds for liveness matching.
///
/// Returns the shaken source code, or `None` if no changes were made.
pub fn tree_shake_module(
  source: &str,
  parsed: &ParsedSource,
  live_export_decls: Option<&FxHashSet<DeclId>>,
  scope_analysis: &ScopeAnalysis,
) -> Option<String> {
  let live_decls = live_export_decls?;

  let program = parsed.program();
  let module = match program.as_ref() {
    Program::Module(m) => m,
    Program::Script(_) => return None,
  };

  let stmts_len = module.body.len();
  if stmts_len == 0 {
    return None;
  }

  // Phase 1: Classify each top-level statement.
  let mut stmt_infos: Vec<StmtInfo> = Vec::with_capacity(stmts_len);
  for item in &module.body {
    stmt_infos.push(classify_module_item(item));
  }

  // Map declared names to DeclIds using scope analysis.
  if !scope_analysis.scopes.is_empty() {
    let module_scope = &scope_analysis.scopes[0];
    for info in &mut stmt_infos {
      for name in &info.declared_names {
        for &decl_id in &module_scope.decls {
          let decl = scope_analysis.get_decl(decl_id);
          if decl.name == *name {
            info.declared_decl_ids.push(decl_id);
            break;
          }
        }
      }
    }
  }

  // Phase 2: Build intra-module dependency graph.
  let mut name_to_stmt: HashMap<&str, usize> = HashMap::new();
  for (i, info) in stmt_infos.iter().enumerate() {
    for name in &info.declared_names {
      name_to_stmt.insert(name.as_str(), i);
    }
  }

  let mut stmt_deps: Vec<Vec<usize>> = vec![Vec::new(); stmts_len];
  for (i, info) in stmt_infos.iter().enumerate() {
    for name in &info.referenced_names {
      if let Some(&dep_idx) = name_to_stmt.get(name.as_str()) {
        if dep_idx != i {
          stmt_deps[i].push(dep_idx);
        }
      }
    }
  }

  // Phase 3: Seed "must include" and propagate.
  let mut included = vec![false; stmts_len];
  let mut worklist: Vec<usize> = Vec::new();

  for (i, info) in stmt_infos.iter().enumerate() {
    let must_include =
      // Statement has side effects
      !info.can_be_removed_if_unused
      // Statement declares a live export (DeclId-based matching)
      || info.declared_decl_ids.iter().any(|id| live_decls.contains(id))
      // Statement is a re-export or export-all (handled by import removal)
      || info.is_reexport;

    if must_include {
      included[i] = true;
      worklist.push(i);
    }
  }

  // BFS: include transitive dependencies
  while let Some(idx) = worklist.pop() {
    for &dep in &stmt_deps[idx] {
      if !included[dep] {
        included[dep] = true;
        worklist.push(dep);
      }
    }
  }

  // Check if anything was actually removed.
  let removed_count = included.iter().filter(|&&x| !x).count();
  if removed_count == 0 {
    return None;
  }

  // Phase 4: Rebuild the source with only included statements.
  // We use line-based removal for simplicity — each ModuleItem's span
  // maps to a range in the source text.
  let mut result = String::with_capacity(source.len());
  let source_bytes = source.as_bytes();

  for (i, item) in module.body.iter().enumerate() {
    if included[i] {
      let span = module_item_span(item);
      // SWC spans are 1-based byte positions.
      let start = (span.lo.0 as usize).saturating_sub(1);
      let end = (span.hi.0 as usize).saturating_sub(1);
      if start < source_bytes.len() && end <= source_bytes.len() {
        result.push_str(&source[start..end]);
        result.push('\n');
      }
    }
  }

  Some(result.trim_end().to_string())
}

/// Analyze whether a module has any side-effectful top-level statements.
///
/// Bare `import './foo'` (no bindings) is counted as a side effect.
/// Named/default/namespace imports are NOT side effects.
pub fn module_has_side_effects(source: &str, specifier: &ModuleSpecifier) -> bool {
  let parsed = match deno_ast::parse_module(ParseParams {
    specifier: specifier.clone(),
    text: source.into(),
    media_type: MediaType::JavaScript,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  }) {
    Ok(p) => p,
    Err(_) => return true, // Assume side effects on parse error.
  };

  let program = parsed.program();
  let module = match program.as_ref() {
    Program::Module(m) => m,
    Program::Script(_) => return true,
  };

  for item in &module.body {
    // Bare imports are side effects.
    if let ModuleItem::ModuleDecl(ModuleDecl::Import(import)) = item {
      if import.specifiers.is_empty() {
        return true;
      }
      continue;
    }

    let info = classify_module_item(item);
    if !info.can_be_removed_if_unused {
      return true;
    }
  }

  false
}

// ---------------------------------------------------------------------------
// Statement classification
// ---------------------------------------------------------------------------

/// Info about a single top-level statement for tree-shaking decisions.
struct StmtInfo {
  can_be_removed_if_unused: bool,
  /// Names declared by this statement.
  declared_names: Vec<String>,
  /// DeclIds declared by this statement (populated from scope analysis after classification).
  declared_decl_ids: Vec<DeclId>,
  /// Names referenced by this statement (for intra-module dependency tracking).
  referenced_names: Vec<String>,
  /// Whether this is a re-export statement.
  is_reexport: bool,
}

impl StmtInfo {
  fn new(
    can_be_removed_if_unused: bool,
    declared_names: Vec<String>,
    referenced_names: Vec<String>,
    is_reexport: bool,
  ) -> Self {
    Self {
      can_be_removed_if_unused,
      declared_names,
      declared_decl_ids: Vec::new(),
      referenced_names,
      is_reexport,
    }
  }
}

fn classify_module_item(item: &ModuleItem) -> StmtInfo {
  match item {
    ModuleItem::Stmt(stmt) => classify_statement(stmt),
    ModuleItem::ModuleDecl(decl) => classify_module_decl(decl),
  }
}

fn classify_statement(stmt: &Stmt) -> StmtInfo {
  match stmt {
    Stmt::Decl(Decl::Fn(f)) => {
      let name = f.ident.sym.to_string();
      let mut refs = Vec::new();
      collect_refs_from_function(&f.function, &mut refs);
      StmtInfo::new(true, vec![name], refs, false)
    }
    Stmt::Decl(Decl::Class(c)) => {
      let name = c.ident.sym.to_string();
      let removable = class_can_be_removed_if_unused(&c.class);
      let mut refs = Vec::new();
      collect_refs_from_class(&c.class, &mut refs);
      StmtInfo::new(removable, vec![name], refs, false)
    }
    Stmt::Decl(Decl::Var(var)) => classify_var_decl(var),
    Stmt::Expr(expr_stmt) => {
      let removable = expr_can_be_removed_if_unused(&expr_stmt.expr);
      let mut refs = Vec::new();
      collect_refs_from_expr(&expr_stmt.expr, &mut refs);
      StmtInfo::new(removable, vec![], refs, false)
    }
    _ => {
      let mut refs = Vec::new();
      collect_refs_from_stmt(stmt, &mut refs);
      StmtInfo::new(false, vec![], refs, false)
    }
  }
}

fn classify_module_decl(decl: &ModuleDecl) -> StmtInfo {
  match decl {
    ModuleDecl::Import(_) => StmtInfo::new(true, vec![], vec![], false),
    ModuleDecl::ExportNamed(export) => {
      if export.src.is_some() {
        StmtInfo::new(true, vec![], vec![], true)
      } else {
        let mut refs = Vec::new();
        let mut exported_names = Vec::new();
        for s in &export.specifiers {
          if let ExportSpecifier::Named(n) = s {
            refs.push(export_name_to_string(&n.orig));
            let exported = n
              .exported
              .as_ref()
              .map(export_name_to_string)
              .unwrap_or_else(|| export_name_to_string(&n.orig));
            exported_names.push(exported);
          }
        }
        StmtInfo::new(true, exported_names, refs, false)
      }
    }
    ModuleDecl::ExportAll(_) => StmtInfo::new(true, vec![], vec![], true),
    ModuleDecl::ExportDecl(export) => classify_export_decl(&export.decl),
    ModuleDecl::ExportDefaultDecl(export) => {
      classify_export_default_decl(export)
    }
    ModuleDecl::ExportDefaultExpr(export) => {
      let removable = expr_can_be_removed_if_unused(&export.expr);
      let mut refs = Vec::new();
      collect_refs_from_expr(&export.expr, &mut refs);
      StmtInfo::new(removable, vec!["default".to_string()], refs, false)
    }
    _ => StmtInfo::new(false, vec![], vec![], false),
  }
}

fn classify_export_decl(decl: &Decl) -> StmtInfo {
  match decl {
    Decl::Fn(f) => {
      let name = f.ident.sym.to_string();
      let mut refs = Vec::new();
      collect_refs_from_function(&f.function, &mut refs);
      StmtInfo::new(true, vec![name], refs, false)
    }
    Decl::Class(c) => {
      let name = c.ident.sym.to_string();
      let removable = class_can_be_removed_if_unused(&c.class);
      let mut refs = Vec::new();
      collect_refs_from_class(&c.class, &mut refs);
      StmtInfo::new(removable, vec![name], refs, false)
    }
    Decl::Var(var) => classify_var_decl(var),
    _ => StmtInfo::new(false, vec![], vec![], false),
  }
}

fn classify_export_default_decl(export: &ExportDefaultDecl) -> StmtInfo {
  match &export.decl {
    DefaultDecl::Fn(f) => {
      let name = f
        .ident
        .as_ref()
        .map(|i| i.sym.to_string())
        .unwrap_or_else(|| "default".to_string());
      let mut names = vec![name];
      if !names.contains(&"default".to_string()) {
        names.push("default".to_string());
      }
      let mut refs = Vec::new();
      collect_refs_from_function(&f.function, &mut refs);
      StmtInfo::new(true, names, refs, false)
    }
    DefaultDecl::Class(c) => {
      let name = c
        .ident
        .as_ref()
        .map(|i| i.sym.to_string())
        .unwrap_or_else(|| "default".to_string());
      let mut names = vec![name];
      if !names.contains(&"default".to_string()) {
        names.push("default".to_string());
      }
      let removable = class_can_be_removed_if_unused(&c.class);
      let mut refs = Vec::new();
      collect_refs_from_class(&c.class, &mut refs);
      StmtInfo::new(removable, names, refs, false)
    }
    _ => StmtInfo::new(false, vec!["default".to_string()], vec![], false),
  }
}

fn classify_var_decl(var: &VarDecl) -> StmtInfo {
  let mut names = Vec::new();
  let mut refs = Vec::new();
  let mut all_removable = true;

  for decl in &var.decls {
    collect_pat_names(&decl.name, &mut names);
    if let Some(init) = &decl.init {
      if !expr_can_be_removed_if_unused(init) {
        all_removable = false;
      }
      collect_refs_from_expr(init, &mut refs);
    }
  }

  StmtInfo::new(all_removable, names, refs, false)
}

// ---------------------------------------------------------------------------
// Side effect analysis
// ---------------------------------------------------------------------------

/// Check if an expression can be removed if its result is unused.
fn expr_can_be_removed_if_unused(expr: &Expr) -> bool {
  match expr {
    // Literals are always safe
    Expr::Lit(_) => true,

    // Function/arrow expressions — body not evaluated at declaration
    Expr::Fn(_) | Expr::Arrow(_) => true,

    // import.meta
    Expr::MetaProp(_) => true,

    // Identifier — safe in module scope
    Expr::Ident(_) => true,

    // This
    Expr::This(_) => true,

    // Array literal — safe if all elements safe
    Expr::Array(arr) => arr.elems.iter().all(|elem| match elem {
      Some(ExprOrSpread { spread: Some(_), .. }) => false,
      Some(ExprOrSpread { expr, .. }) => {
        expr_can_be_removed_if_unused(expr)
      }
      None => true,
    }),

    // Object literal — safe if no spread, no side-effectful computed keys, all values safe
    Expr::Object(obj) => obj.props.iter().all(|prop| match prop {
      PropOrSpread::Spread(_) => false,
      PropOrSpread::Prop(p) => match p.as_ref() {
        Prop::KeyValue(kv) => {
          let key_safe = match &kv.key {
            PropName::Computed(c) => expr_can_be_removed_if_unused(&c.expr),
            _ => true,
          };
          key_safe && expr_can_be_removed_if_unused(&kv.value)
        }
        Prop::Shorthand(_) => true,
        Prop::Method(_) => true,
        Prop::Getter(_) | Prop::Setter(_) => true,
        Prop::Assign(_) => true,
      },
    }),

    // Unary
    Expr::Unary(u) => match u.op {
      UnaryOp::TypeOf => true,
      UnaryOp::Void | UnaryOp::Bang | UnaryOp::Tilde | UnaryOp::Minus
      | UnaryOp::Plus => expr_can_be_removed_if_unused(&u.arg),
      UnaryOp::Delete => false,
    },

    // Binary — safe if both sides safe
    Expr::Bin(b) => {
      expr_can_be_removed_if_unused(&b.left)
        && expr_can_be_removed_if_unused(&b.right)
    }

    // Ternary — safe if all three safe
    Expr::Cond(c) => {
      expr_can_be_removed_if_unused(&c.test)
        && expr_can_be_removed_if_unused(&c.cons)
        && expr_can_be_removed_if_unused(&c.alt)
    }

    // Comma — safe if all safe
    Expr::Seq(s) => {
      s.exprs.iter().all(|e| expr_can_be_removed_if_unused(e))
    }

    // Template literal (untagged) — safe if all expressions safe
    Expr::Tpl(t) => {
      t.exprs.iter().all(|e| expr_can_be_removed_if_unused(e))
    }

    // Parenthesized
    Expr::Paren(p) => expr_can_be_removed_if_unused(&p.expr),

    // Class expression
    Expr::Class(c) => class_can_be_removed_if_unused(&c.class),

    // Call/new — NOT safe by default (no @__PURE__ check yet via SWC comments)
    // TODO: Add @__PURE__ annotation support
    _ => false,
  }
}

fn class_can_be_removed_if_unused(class: &Class) -> bool {
  if !class.decorators.is_empty() {
    return false;
  }
  if let Some(s) = &class.super_class {
    if !expr_can_be_removed_if_unused(s) {
      return false;
    }
  }
  for member in &class.body {
    match member {
      ClassMember::StaticBlock(b) => {
        if !b.body.stmts.is_empty() {
          return false;
        }
      }
      ClassMember::ClassProp(p) => {
        if p.is_static {
          if let Some(key) = prop_name_expr(&p.key) {
            if !expr_can_be_removed_if_unused(key) {
              return false;
            }
          }
          if let Some(v) = &p.value {
            if !expr_can_be_removed_if_unused(v) {
              return false;
            }
          }
        }
        if !p.decorators.is_empty() {
          return false;
        }
      }
      ClassMember::Method(m) => {
        if m.is_static {
          if let Some(key) = prop_name_expr(&m.key) {
            if !expr_can_be_removed_if_unused(key) {
              return false;
            }
          }
        }
        if !m.function.decorators.is_empty() {
          return false;
        }
      }
      ClassMember::AutoAccessor(a) => {
        if a.is_static {
          if let Some(v) = &a.value {
            if !expr_can_be_removed_if_unused(v) {
              return false;
            }
          }
        }
        if !a.decorators.is_empty() {
          return false;
        }
      }
      _ => {}
    }
  }
  true
}

/// Get the expression for a computed property name.
fn prop_name_expr(key: &PropName) -> Option<&Expr> {
  match key {
    PropName::Computed(c) => Some(&c.expr),
    _ => None,
  }
}

// ---------------------------------------------------------------------------
// Reference collection (for intra-module dependency tracking)
// ---------------------------------------------------------------------------

/// Visitor that collects all referenced identifier names.
struct RefCollector {
  refs: Vec<String>,
}

impl Visit for RefCollector {
  fn visit_ident(&mut self, ident: &Ident) {
    self.refs.push(ident.sym.to_string());
  }

  // Don't descend into nested function/class declarations' binding names.
  // We only want references, not declarations.
  fn visit_fn_decl(&mut self, n: &FnDecl) {
    // Collect refs from function body, but not the function name itself.
    n.function.visit_with(self);
  }

  fn visit_class_decl(&mut self, n: &ClassDecl) {
    n.class.visit_with(self);
  }

  fn visit_var_declarator(&mut self, n: &VarDeclarator) {
    // Skip pattern names (they're declarations), visit init only.
    if let Some(init) = &n.init {
      init.visit_with(self);
    }
  }
}

fn collect_refs_from_expr(expr: &Expr, refs: &mut Vec<String>) {
  let mut collector = RefCollector {
    refs: Vec::new(),
  };
  expr.visit_with(&mut collector);
  refs.extend(collector.refs);
}

fn collect_refs_from_function(func: &Function, refs: &mut Vec<String>) {
  let mut collector = RefCollector {
    refs: Vec::new(),
  };
  func.visit_with(&mut collector);
  refs.extend(collector.refs);
}

fn collect_refs_from_class(class: &Class, refs: &mut Vec<String>) {
  let mut collector = RefCollector {
    refs: Vec::new(),
  };
  class.visit_with(&mut collector);
  refs.extend(collector.refs);
}

fn collect_refs_from_stmt(stmt: &Stmt, refs: &mut Vec<String>) {
  let mut collector = RefCollector {
    refs: Vec::new(),
  };
  stmt.visit_with(&mut collector);
  refs.extend(collector.refs);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Collect declared names from a binding pattern.
fn collect_pat_names(pat: &Pat, names: &mut Vec<String>) {
  match pat {
    Pat::Ident(id) => names.push(id.sym.to_string()),
    Pat::Array(arr) => {
      for elem in arr.elems.iter().flatten() {
        collect_pat_names(elem, names);
      }
    }
    Pat::Object(obj) => {
      for prop in &obj.props {
        match prop {
          ObjectPatProp::KeyValue(kv) => collect_pat_names(&kv.value, names),
          ObjectPatProp::Assign(a) => names.push(a.key.sym.to_string()),
          ObjectPatProp::Rest(r) => collect_pat_names(&r.arg, names),
        }
      }
    }
    Pat::Rest(r) => collect_pat_names(&r.arg, names),
    Pat::Assign(a) => collect_pat_names(&a.left, names),
    Pat::Expr(_) | Pat::Invalid(_) => {}
  }
}

fn export_name_to_string(name: &ModuleExportName) -> String {
  match name {
    ModuleExportName::Ident(i) => i.sym.to_string(),
    ModuleExportName::Str(s) => {
      super::module_info_swc::wtf8_to_string(&s.value)
    }
  }
}

fn module_item_span(item: &ModuleItem) -> deno_ast::swc::common::Span {
  match item {
    ModuleItem::Stmt(s) => stmt_span(s),
    ModuleItem::ModuleDecl(d) => module_decl_span(d),
  }
}

fn stmt_span(stmt: &Stmt) -> deno_ast::swc::common::Span {
  match stmt {
    Stmt::Block(s) => s.span,
    Stmt::Empty(s) => s.span,
    Stmt::Debugger(s) => s.span,
    Stmt::With(s) => s.span,
    Stmt::Return(s) => s.span,
    Stmt::Labeled(s) => s.span,
    Stmt::Break(s) => s.span,
    Stmt::Continue(s) => s.span,
    Stmt::If(s) => s.span,
    Stmt::Switch(s) => s.span,
    Stmt::Throw(s) => s.span,
    Stmt::Try(s) => s.span,
    Stmt::While(s) => s.span,
    Stmt::DoWhile(s) => s.span,
    Stmt::For(s) => s.span,
    Stmt::ForIn(s) => s.span,
    Stmt::ForOf(s) => s.span,
    Stmt::Decl(d) => decl_span(d),
    Stmt::Expr(s) => s.span,
  }
}

fn decl_span(decl: &Decl) -> deno_ast::swc::common::Span {
  match decl {
    Decl::Class(d) => d.class.span,
    Decl::Fn(d) => d.function.span,
    Decl::Var(d) => d.span,
    Decl::Using(d) => d.span,
    Decl::TsInterface(d) => d.span,
    Decl::TsTypeAlias(d) => d.span,
    Decl::TsEnum(d) => d.span,
    Decl::TsModule(d) => d.span,
  }
}

fn module_decl_span(decl: &ModuleDecl) -> deno_ast::swc::common::Span {
  match decl {
    ModuleDecl::Import(d) => d.span,
    ModuleDecl::ExportDecl(d) => d.span,
    ModuleDecl::ExportNamed(d) => d.span,
    ModuleDecl::ExportDefaultDecl(d) => d.span,
    ModuleDecl::ExportDefaultExpr(d) => d.span,
    ModuleDecl::ExportAll(d) => d.span,
    ModuleDecl::TsImportEquals(d) => d.span,
    ModuleDecl::TsExportAssignment(d) => d.span,
    ModuleDecl::TsNamespaceExport(d) => d.span,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use super::super::module_info_swc::extract_module_info;

  fn spec(s: &str) -> ModuleSpecifier {
    ModuleSpecifier::parse(&format!("file:///{}", s)).unwrap()
  }

  /// Parse source, extract scope analysis, convert export names to DeclIds,
  /// then run tree shaking.
  fn shake(source: &str, live_exports: &[&str]) -> String {
    let s = spec("mod.js");
    let parsed = deno_ast::parse_module(ParseParams {
      specifier: s.clone(),
      text: source.into(),
      media_type: MediaType::JavaScript,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .unwrap();

    let module_info = extract_module_info(&parsed);

    // Convert export names to live DeclIds.
    let mut live_decls = FxHashSet::default();
    for &name in live_exports {
      // Find the export binding for this name and get its decl_id.
      for export in &module_info.exports {
        if export.exported_name == name {
          if let Some(decl_id) = export.decl_id {
            live_decls.insert(decl_id);
          } else {
            // For re-exports without decl_id, find the local decl by name.
            if let Some(local) = &export.local_name {
              for &did in &module_info.scope_analysis.scopes[0].decls {
                let d = module_info.scope_analysis.get_decl(did);
                if d.name == *local {
                  live_decls.insert(did);
                  break;
                }
              }
            }
          }
        }
      }
      // Also check if name matches a top-level decl directly (for non-exported decls).
      if !module_info.scope_analysis.scopes.is_empty() {
        for &did in &module_info.scope_analysis.scopes[0].decls {
          let d = module_info.scope_analysis.get_decl(did);
          if d.name == name {
            live_decls.insert(did);
          }
        }
      }
    }

    tree_shake_module(source, &parsed, Some(&live_decls), &module_info.scope_analysis)
      .unwrap_or_else(|| source.to_string())
  }

  fn has_side_effects(source: &str) -> bool {
    let s = spec("mod.js");
    module_has_side_effects(source, &s)
  }

  // --- Side effect detection ---

  #[test]
  fn test_side_effects_bare_import() {
    assert!(has_side_effects("import './polyfill.js';"));
  }

  #[test]
  fn test_side_effects_named_import_no_side_effects() {
    assert!(!has_side_effects("import { foo } from './lib.js';"));
  }

  #[test]
  fn test_side_effects_console_call() {
    assert!(has_side_effects("console.log('hello');"));
  }

  #[test]
  fn test_side_effects_pure_declarations() {
    assert!(!has_side_effects(
      "const x = 1;\nfunction foo() {}\nclass Bar {}"
    ));
  }

  #[test]
  fn test_side_effects_global_assignment() {
    assert!(has_side_effects("globalThis.x = 1;"));
  }

  // --- Unused function removed ---

  #[test]
  fn test_tree_shake_unused_function() {
    let result = shake(
      "export function used() { return 1; }\nfunction unused() { return 2; }",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("unused"));
  }

  // --- Unused variable with literal removed ---

  #[test]
  fn test_tree_shake_unused_variable_literal() {
    let result = shake(
      "export const foo = 1;\nconst bar = 2;",
      &["foo"],
    );
    assert!(result.contains("foo"));
    assert!(!result.contains("bar"));
  }

  // --- Used function kept ---

  #[test]
  fn test_tree_shake_used_function_kept() {
    let result = shake(
      "export function foo() { return helper(); }\nfunction helper() { return 42; }",
      &["foo"],
    );
    assert!(result.contains("foo"));
    assert!(result.contains("helper"));
  }

  // --- Transitive dependency chain ---

  #[test]
  fn test_tree_shake_transitive_dependency_chain() {
    let result = shake(
      "export function a() { return b(); }\nfunction b() { return c(); }\nfunction c() { return 1; }\nfunction unused() {}",
      &["a"],
    );
    assert!(result.contains("function a"));
    assert!(result.contains("function b"));
    assert!(result.contains("function c"));
    assert!(!result.contains("unused"));
  }

  // --- Side-effectful expression kept ---

  #[test]
  fn test_tree_shake_side_effectful_expression_kept() {
    let result = shake(
      "export const foo = 1;\nconsole.log('side effect');",
      &["foo"],
    );
    assert!(result.contains("foo"));
    assert!(result.contains("console.log"));
  }

  // --- Variable with call initializer kept ---

  #[test]
  fn test_tree_shake_var_with_call_init_kept() {
    let result = shake(
      "export const foo = 1;\nconst bar = sideEffect();",
      &["foo"],
    );
    assert!(result.contains("foo"));
    assert!(result.contains("sideEffect"));
  }

  // --- Mixed used and unused ---

  #[test]
  fn test_tree_shake_mixed_used_and_unused() {
    let result = shake(
      "export const a = 1;\nexport const b = 2;\nconst c = 3;\nfunction d() {}",
      &["a"],
    );
    assert!(result.contains("a = 1"));
    assert!(!result.contains("b = 2"));
    assert!(!result.contains("c = 3"));
    assert!(!result.contains("function d"));
  }

  // --- IIFE kept ---

  #[test]
  fn test_tree_shake_iife_kept() {
    let result = shake(
      "export const foo = 1;\n(function() { globalThis.x = 1; })();",
      &["foo"],
    );
    assert!(result.contains("foo"));
    assert!(result.contains("globalThis"));
  }

  // --- Literal init removable ---

  #[test]
  fn test_tree_shake_literal_init_removable() {
    let result = shake(
      "export const used = 1;\nconst unused = 'hello';",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("unused"));
  }

  // --- Array literal removable ---

  #[test]
  fn test_tree_shake_array_literal_removable() {
    let result = shake(
      "export const used = 1;\nconst unused = [1, 2, 3];",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("unused"));
  }

  // --- Object literal removable ---

  #[test]
  fn test_tree_shake_object_literal_removable() {
    let result = shake(
      "export const used = 1;\nconst unused = { a: 1, b: 2 };",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("unused"));
  }

  // --- Object spread not removable ---

  #[test]
  fn test_tree_shake_object_spread_not_removable() {
    let result = shake(
      "export const used = 1;\nconst unused = { ...someObj };",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("someObj")); // Kept because spread has side effects
  }

  // --- Arrow function removable ---

  #[test]
  fn test_tree_shake_arrow_function_removable() {
    let result = shake(
      "export const used = 1;\nconst unused = () => 42;",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("unused"));
  }

  // --- Function expression removable ---

  #[test]
  fn test_tree_shake_function_expression_removable() {
    let result = shake(
      "export const used = 1;\nconst unused = function() { return 42; };",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("unused"));
  }

  // --- Member expression not removable ---

  #[test]
  fn test_tree_shake_member_expression_not_removable() {
    let result = shake(
      "export const used = 1;\nconst unused = obj.prop;",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("obj.prop")); // Kept because getter could have side effects
  }

  // --- Delete not removable ---

  #[test]
  fn test_tree_shake_delete_not_removable() {
    let result = shake(
      "export const used = 1;\ndelete globalThis.x;",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("delete"));
  }

  // --- Empty class removable ---

  #[test]
  fn test_tree_shake_empty_class_removable() {
    let result = shake(
      "export const used = 1;\nclass Unused {}",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("Unused"));
  }

  // --- Class with side-effectful extends ---

  #[test]
  fn test_tree_shake_class_with_extends_expression() {
    let result = shake(
      "export const used = 1;\nclass Unused extends getBase() {}",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("getBase")); // Call in extends has side effects
  }

  // --- Class with static field literal ---

  #[test]
  fn test_tree_shake_class_with_static_field_literal() {
    let result = shake(
      "export const used = 1;\nclass Unused { static x = 42; }",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("Unused"));
  }

  // --- Class with static field call ---

  #[test]
  fn test_tree_shake_class_with_static_field_call() {
    let result = shake(
      "export const used = 1;\nclass Unused { static x = sideEffect(); }",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("sideEffect")); // Static field call has side effects
  }

  // --- Class with static block ---

  #[test]
  fn test_tree_shake_class_with_static_block() {
    let result = shake(
      "export const used = 1;\nclass Unused { static { console.log('init'); } }",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("console.log")); // Static block has side effects
  }

  // --- Class with decorators not removable ---

  #[test]
  fn test_tree_shake_class_with_decorators() {
    // Note: Decorators are TS proposal, but after transpilation they become
    // function calls, so this test uses pre-transpile syntax.
    // In practice, decorated classes would have been transpiled to calls.
    let result = shake(
      "export const used = 1;\nclass Unused { static x = init(); }",
      &["used"],
    );
    assert!(result.contains("used"));
    // init() has side effects so Unused is kept
    assert!(result.contains("init"));
  }

  // --- typeof is safe ---

  #[test]
  fn test_tree_shake_typeof_safe() {
    let result = shake(
      "export const used = 1;\nconst unused = typeof window;",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("typeof"));
  }

  // --- void expression removable ---

  #[test]
  fn test_tree_shake_void_removable() {
    let result = shake(
      "export const used = 1;\nconst unused = void 0;",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("void"));
  }

  // --- Logical operators removable ---

  #[test]
  fn test_tree_shake_logical_operators_removable() {
    let result = shake(
      "export const used = 1;\nconst unused = true && false;",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("unused"));
  }

  // --- Ternary removable ---

  #[test]
  fn test_tree_shake_ternary_removable() {
    let result = shake(
      "export const used = 1;\nconst unused = true ? 1 : 2;",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("unused"));
  }

  // --- Template literal removable ---

  #[test]
  fn test_tree_shake_template_literal_removable() {
    let result = shake(
      "export const used = 1;\nconst unused = `hello ${42}`;",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("unused"));
  }

  // --- Export default expression with side effects ---

  #[test]
  fn test_tree_shake_export_default_expression_side_effect() {
    let result = shake(
      "export const used = 1;\nexport default sideEffect();",
      &["used"],
    );
    assert!(result.contains("used"));
    // Default export with side effect should be kept
    assert!(result.contains("sideEffect"));
  }

  // --- Default export function removable ---

  #[test]
  fn test_tree_shake_default_function_removable() {
    let result = shake(
      "export const used = 1;\nexport default function unused() { return 2; }",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("unused"));
  }

  // --- Try-catch not removable ---

  #[test]
  fn test_tree_shake_try_catch_not_removable() {
    let result = shake(
      "export const used = 1;\ntry { something(); } catch(e) {}",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("something"));
  }

  // --- For-of not removable ---

  #[test]
  fn test_tree_shake_for_of_not_removable() {
    let result = shake(
      "export const used = 1;\nfor (const x of items) { process(x); }",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("items"));
  }

  // --- Assignment expression not removable ---

  #[test]
  fn test_tree_shake_assignment_not_removable() {
    let result = shake(
      "export const used = 1;\nglobalThis.x = 42;",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("globalThis.x"));
  }

  // --- No shaking when live_exports is None ---

  #[test]
  fn test_tree_shake_none_live_exports() {
    let s = spec("mod.js");
    let source = "export const foo = 1;\nconst bar = 2;";
    let parsed = deno_ast::parse_module(ParseParams {
      specifier: s.clone(),
      text: source.into(),
      media_type: MediaType::JavaScript,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .unwrap();
    let mi = extract_module_info(&parsed);
    let result = tree_shake_module(source, &parsed, None, &mi.scope_analysis);
    assert!(result.is_none()); // No changes
  }

  // --- All exports live means nothing removed ---

  #[test]
  fn test_tree_shake_all_live() {
    let result = shake(
      "export const foo = 1;\nexport const bar = 2;",
      &["foo", "bar"],
    );
    assert!(result.contains("foo"));
    assert!(result.contains("bar"));
  }

  // --- Import side effects bare import ---

  #[test]
  fn test_tree_shake_bare_import_side_effects() {
    assert!(has_side_effects("import './polyfill';"));
    assert!(!has_side_effects("import { x } from './mod';"));
    assert!(!has_side_effects("import def from './mod';"));
    assert!(!has_side_effects("import * as ns from './mod';"));
  }

  // --- Destructuring removable ---

  #[test]
  fn test_tree_shake_destructuring_removable() {
    let result = shake(
      "export const used = 1;\nconst { a, b } = { a: 1, b: 2 };",
      &["used"],
    );
    assert!(result.contains("used"));
    // Object literal with no side effects = removable
    assert!(!result.contains("a, b"));
  }

  // --- Update expression not removable ---

  #[test]
  fn test_tree_shake_update_expression_not_removable() {
    let result = shake(
      "export const used = 1;\nlet x = 0;\nx++;",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("x++"));
  }

  // --- Destructuring with call not removable ---

  #[test]
  fn test_tree_shake_destructuring_not_removable() {
    let result = shake(
      "export const used = 1;\nconst { a, b } = getConfig();",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("getConfig")); // Call has side effects
  }

  // --- Class with instance fields (ignored — evaluated on construction) ---

  #[test]
  fn test_tree_shake_class_instance_fields_ignored() {
    let result = shake(
      "export const used = 1;\nclass Unused { x = sideEffect(); }",
      &["used"],
    );
    assert!(result.contains("used"));
    // Instance field sideEffect() is not called at definition time
    assert!(!result.contains("Unused"));
  }

  // --- Class with computed method key ---

  #[test]
  fn test_tree_shake_class_with_computed_method_key() {
    let result = shake(
      "export const used = 1;\nclass Unused { static [computeKey()]() {} }",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("computeKey")); // Static computed key has side effects
  }

  // --- Object with computed key not removable ---

  #[test]
  fn test_tree_shake_object_computed_key_not_removable() {
    let result = shake(
      "export const used = 1;\nconst unused = { [sideEffect()]: 1 };",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("sideEffect")); // Computed key call has side effects
  }

  // --- Unbound reference removable ---

  #[test]
  fn test_tree_shake_unbound_reference_removable() {
    let result = shake(
      "export const used = 1;\nconst unused = someGlobal;",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("unused"));
  }

  // --- Multiple declarators with partial side effects ---

  #[test]
  fn test_tree_shake_multiple_declarators_partial() {
    // When one declarator has a side effect, the whole var decl is kept.
    let result = shake(
      "export const used = 1;\nconst a = 1, b = sideEffect();",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("sideEffect")); // Whole var decl kept
  }

  // --- Entry exports preserved (all exports live) ---

  #[test]
  fn test_tree_shake_entry_exports_preserved() {
    let result = shake(
      "export const foo = 1;\nexport const bar = 2;\nexport function baz() {}",
      &["foo", "bar", "baz"],
    );
    assert!(result.contains("foo"));
    assert!(result.contains("bar"));
    assert!(result.contains("baz"));
  }

  // --- Unused default export removable ---

  #[test]
  fn test_tree_shake_unused_default_export() {
    let result = shake(
      "export const used = 1;\nexport default function unused() {}",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("unused"));
  }

  // --- Re-export kept (always included) ---

  #[test]
  fn test_tree_shake_reexport_kept() {
    let result = shake(
      "export { foo } from './other.js';\nexport const used = 1;",
      &["used"],
    );
    // Re-exports are always kept
    assert!(result.contains("foo"));
    assert!(result.contains("used"));
  }

  // --- Export all kept ---

  #[test]
  fn test_tree_shake_export_all_kept() {
    let result = shake(
      "export * from './other.js';\nexport const used = 1;",
      &["used"],
    );
    assert!(result.contains("export *"));
    assert!(result.contains("used"));
  }

  // --- Named export referencing local ---

  #[test]
  fn test_tree_shake_named_export_refs_local() {
    let result = shake(
      "function helper() { return 42; }\nexport { helper as myHelper };",
      &["myHelper"],
    );
    assert!(result.contains("helper"));
  }

  // --- Empty export (no names) ---

  #[test]
  fn test_tree_shake_unused_named_export() {
    let result = shake(
      "export function used() {}\nexport function unused() {}",
      &["used"],
    );
    assert!(result.contains("function used"));
    assert!(!result.contains("function unused"));
  }

  // --- Global assignment is side effect ---

  #[test]
  fn test_tree_shake_window_assignment_kept() {
    let result = shake(
      "export const used = 1;\nwindow.x = 42;",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("window.x"));
  }

  // --- For loop refs pull in deps ---

  #[test]
  fn test_tree_shake_for_loop_refs_pull_in_deps() {
    let result = shake(
      "export function used() { return items; }\nconst items = [1, 2, 3];\nfunction unused() {}",
      &["used"],
    );
    assert!(result.contains("function used"));
    assert!(result.contains("items"));
    assert!(!result.contains("unused"));
  }

  // --- Default export class removable ---

  #[test]
  fn test_tree_shake_default_class_removable() {
    let result = shake(
      "export const used = 1;\nexport default class Unused {}",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("Unused"));
  }

  // --- Unused variable with function expression ---

  #[test]
  fn test_tree_shake_unused_variable_function_expr() {
    let result = shake(
      "export const used = 1;\nconst unused = function named() { return 42; };",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(!result.contains("unused"));
    assert!(!result.contains("named"));
  }

  // --- Side effect: global call ---

  #[test]
  fn test_tree_shake_global_call_kept() {
    let result = shake(
      "export const used = 1;\nsetup();",
      &["used"],
    );
    assert!(result.contains("used"));
    assert!(result.contains("setup"));
  }

  // --- Export default expression unused but safe ---

  #[test]
  fn test_tree_shake_export_default_expression_unused_safe() {
    let result = shake(
      "export const used = 1;\nexport default 42;",
      &["used"],
    );
    assert!(result.contains("used"));
    // Default expr with literal is safe to remove
    assert!(!result.contains("42"));
  }
}
