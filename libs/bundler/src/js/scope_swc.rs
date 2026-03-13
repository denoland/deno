// Copyright 2018-2026 the Deno authors. MIT license.

//! Build a [`ScopeAnalysis`] from a SWC AST (via `deno_ast::ParsedSource`).
//!
//! This is the SWC equivalent of the bundler's `scope_oxc.rs` which builds
//! scope analysis from oxc's `Semantic` output.

use std::collections::HashMap;

use deno_ast::swc::ast::*;
use deno_ast::swc::common::Span;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_ast::swc::utils::find_pat_ids;

use super::scope::DeclId;
use super::scope::DeclKind;
use super::scope::Declaration;
use super::scope::RefId;
use super::scope::Reference;
use super::scope::Scope;
use super::scope::ScopeAnalysis;
use super::scope::ScopeId;
use super::scope::ScopeKind;

/// Build a `ScopeAnalysis` from an AST program.
pub fn analyze_scope(program: &Program) -> ScopeAnalysis {
  let mut analyzer = ScopeAnalyzer::new();
  program.visit_with(&mut analyzer);
  analyzer.finish()
}

/// Internal state for scope analysis.
struct ScopeAnalyzer {
  scopes: Vec<Scope>,
  declarations: Vec<Declaration>,
  references: Vec<Reference>,
  /// Stack of scope IDs for the current nesting position.
  scope_stack: Vec<ScopeId>,
  /// Map from SWC's (sym, ctxt) to DeclId for reference resolution.
  /// We use (symbol name, syntax context) as the key since SWC uses
  /// syntax context (hygiene) to distinguish between same-named bindings
  /// in different scopes.
  id_to_decl: HashMap<(String, u32), DeclId>,
}

impl ScopeAnalyzer {
  fn new() -> Self {
    let mut analyzer = Self {
      scopes: Vec::new(),
      declarations: Vec::new(),
      references: Vec::new(),
      scope_stack: Vec::new(),
      id_to_decl: HashMap::new(),
    };
    // Create the module (root) scope.
    let root_id = analyzer.push_scope(ScopeKind::Module, None);
    analyzer.scope_stack.push(root_id);
    analyzer
  }

  fn finish(self) -> ScopeAnalysis {
    ScopeAnalysis {
      scopes: self.scopes,
      declarations: self.declarations,
      references: self.references,
    }
  }

  fn current_scope(&self) -> ScopeId {
    *self.scope_stack.last().unwrap()
  }

  fn push_scope(
    &mut self,
    kind: ScopeKind,
    parent: Option<ScopeId>,
  ) -> ScopeId {
    let id = ScopeId(self.scopes.len() as u32);
    self.scopes.push(Scope {
      id,
      parent,
      kind,
      decls: Vec::new(),
    });
    id
  }

  fn enter_scope(&mut self, kind: ScopeKind) -> ScopeId {
    let parent = self.current_scope();
    let id = self.push_scope(kind, Some(parent));
    self.scope_stack.push(id);
    id
  }

  fn exit_scope(&mut self) {
    self.scope_stack.pop();
  }

  fn declare(&mut self, name: &str, kind: DeclKind, span: Span, ctxt: u32) {
    let decl_id = DeclId(self.declarations.len() as u32);
    let scope = self.current_scope();
    self.declarations.push(Declaration {
      id: decl_id,
      name: name.to_string(),
      kind,
      scope,
      name_span: span,
      refs: Vec::new(),
    });
    self.scopes[scope.0 as usize].decls.push(decl_id);
    self.id_to_decl.insert((name.to_string(), ctxt), decl_id);
  }

  fn declare_ident(&mut self, ident: &Ident, kind: DeclKind) {
    self.declare(&ident.sym, kind, ident.span, ident.ctxt.as_u32());
  }

  fn declare_pat(&mut self, pat: &Pat, kind: DeclKind) {
    let ids: Vec<Id> = find_pat_ids(pat);
    for id in ids {
      self.declare(&id.0, kind, Span::default(), id.1.as_u32());
    }
  }

  fn add_reference(&mut self, ident: &Ident) {
    let ref_id = RefId(self.references.len() as u32);
    let scope = self.current_scope();
    let resolved = self
      .id_to_decl
      .get(&(ident.sym.to_string(), ident.ctxt.as_u32()))
      .copied();

    self.references.push(Reference {
      id: ref_id,
      name: ident.sym.to_string(),
      span: ident.span,
      scope,
      resolved,
    });

    // Link back to declaration.
    if let Some(decl_id) = resolved {
      self.declarations[decl_id.0 as usize].refs.push(ref_id);
    }
  }
}

impl Visit for ScopeAnalyzer {
  fn visit_var_decl(&mut self, n: &VarDecl) {
    let kind = match n.kind {
      VarDeclKind::Var => DeclKind::Var,
      VarDeclKind::Let => DeclKind::Let,
      VarDeclKind::Const => DeclKind::Const,
    };
    for decl in &n.decls {
      // Visit init first (before declaring the binding) so references
      // inside init resolve to outer scope.
      decl.init.visit_with(self);
      self.declare_pat(&decl.name, kind);
    }
  }

  fn visit_fn_decl(&mut self, n: &FnDecl) {
    self.declare_ident(&n.ident, DeclKind::Function);
    self.enter_scope(ScopeKind::Function);
    for param in &n.function.params {
      self.declare_pat(&param.pat, DeclKind::Param);
    }
    if let Some(body) = &n.function.body {
      body.stmts.visit_with(self);
    }
    self.exit_scope();
  }

  fn visit_fn_expr(&mut self, n: &FnExpr) {
    self.enter_scope(ScopeKind::Function);
    if let Some(ident) = &n.ident {
      self.declare_ident(ident, DeclKind::Function);
    }
    for param in &n.function.params {
      self.declare_pat(&param.pat, DeclKind::Param);
    }
    if let Some(body) = &n.function.body {
      body.stmts.visit_with(self);
    }
    self.exit_scope();
  }

  fn visit_arrow_expr(&mut self, n: &ArrowExpr) {
    self.enter_scope(ScopeKind::Function);
    for pat in &n.params {
      self.declare_pat(pat, DeclKind::Param);
    }
    match &*n.body {
      BlockStmtOrExpr::BlockStmt(block) => block.stmts.visit_with(self),
      BlockStmtOrExpr::Expr(expr) => expr.visit_with(self),
    }
    self.exit_scope();
  }

  fn visit_class_decl(&mut self, n: &ClassDecl) {
    self.declare_ident(&n.ident, DeclKind::Class);
    n.class.visit_with(self);
  }

  fn visit_class_expr(&mut self, n: &ClassExpr) {
    if let Some(ident) = &n.ident {
      self.declare_ident(ident, DeclKind::Class);
    }
    n.class.visit_with(self);
  }

  fn visit_block_stmt(&mut self, n: &BlockStmt) {
    self.enter_scope(ScopeKind::Block);
    n.stmts.visit_with(self);
    self.exit_scope();
  }

  fn visit_catch_clause(&mut self, n: &CatchClause) {
    self.enter_scope(ScopeKind::Block);
    if let Some(param) = &n.param {
      self.declare_pat(param, DeclKind::CatchParam);
    }
    n.body.stmts.visit_with(self);
    self.exit_scope();
  }

  fn visit_import_named_specifier(&mut self, n: &ImportNamedSpecifier) {
    self.declare_ident(&n.local, DeclKind::Import);
  }

  fn visit_import_default_specifier(&mut self, n: &ImportDefaultSpecifier) {
    self.declare_ident(&n.local, DeclKind::Import);
  }

  fn visit_import_star_as_specifier(&mut self, n: &ImportStarAsSpecifier) {
    self.declare_ident(&n.local, DeclKind::Import);
  }

  fn visit_for_stmt(&mut self, n: &ForStmt) {
    self.enter_scope(ScopeKind::Block);
    n.init.visit_with(self);
    n.test.visit_with(self);
    n.update.visit_with(self);
    n.body.visit_with(self);
    self.exit_scope();
  }

  fn visit_for_in_stmt(&mut self, n: &ForInStmt) {
    self.enter_scope(ScopeKind::Block);
    n.left.visit_with(self);
    n.right.visit_with(self);
    n.body.visit_with(self);
    self.exit_scope();
  }

  fn visit_for_of_stmt(&mut self, n: &ForOfStmt) {
    self.enter_scope(ScopeKind::Block);
    n.left.visit_with(self);
    n.right.visit_with(self);
    n.body.visit_with(self);
    self.exit_scope();
  }

  // Handle identifier references.
  fn visit_ident(&mut self, n: &Ident) {
    // Skip if this ident is a declaration site (handled by declare_ident).
    // We only want to record usage references here.
    // SWC's hygiene (ctxt) helps us distinguish. We check if this ident's
    // (sym, ctxt) is already in our id_to_decl map but we should still
    // record the reference.
    self.add_reference(n);
  }

  // Don't traverse into type annotations — they don't produce runtime code.
  fn visit_ts_type_ann(&mut self, _n: &TsTypeAnn) {}
  fn visit_ts_type_param_decl(&mut self, _n: &TsTypeParamDecl) {}
  fn visit_ts_type_alias_decl(&mut self, _n: &TsTypeAliasDecl) {}
  fn visit_ts_interface_decl(&mut self, _n: &TsInterfaceDecl) {}
  fn visit_ts_enum_decl(&mut self, n: &TsEnumDecl) {
    // Enums produce runtime code — declare as Var.
    self.declare_ident(&n.id, DeclKind::Var);
    // Visit members for their values.
    for member in &n.members {
      if let Some(init) = &member.init {
        init.visit_with(self);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use deno_ast::MediaType;
  use deno_ast::ModuleSpecifier;
  use deno_ast::ParseParams;

  use super::*;

  fn parse_and_analyze(source: &str) -> ScopeAnalysis {
    let parsed = deno_ast::parse_module(ParseParams {
      specifier: ModuleSpecifier::parse("file:///test.ts").unwrap(),
      text: source.to_string().into(),
      media_type: MediaType::TypeScript,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })
    .unwrap();
    analyze_scope(&parsed.program())
  }

  fn find_decl<'a>(
    analysis: &'a ScopeAnalysis,
    name: &str,
  ) -> Option<&'a Declaration> {
    analysis.declarations.iter().find(|d| d.name == name)
  }

  #[test]
  fn test_const_declaration() {
    let analysis = parse_and_analyze("const x = 1;");
    let x = find_decl(&analysis, "x").expect("should have x");
    assert_eq!(x.kind, DeclKind::Const);
  }

  #[test]
  fn test_let_declaration() {
    let analysis = parse_and_analyze("let z = 3;");
    let z = find_decl(&analysis, "z").expect("should have z");
    assert_eq!(z.kind, DeclKind::Let);
  }

  #[test]
  fn test_var_declaration() {
    let analysis = parse_and_analyze("var y = 2;");
    let y = find_decl(&analysis, "y").expect("should have y");
    assert_eq!(y.kind, DeclKind::Var);
  }

  #[test]
  fn test_function_declaration() {
    let analysis = parse_and_analyze("function foo() {}");
    let foo = find_decl(&analysis, "foo").expect("should have foo");
    assert_eq!(foo.kind, DeclKind::Function);
  }

  #[test]
  fn test_class_declaration() {
    let analysis = parse_and_analyze("class Bar {}");
    let bar = find_decl(&analysis, "Bar").expect("should have Bar");
    assert_eq!(bar.kind, DeclKind::Class);
  }

  #[test]
  fn test_import_named() {
    let analysis = parse_and_analyze("import { foo } from './mod';");
    let foo = find_decl(&analysis, "foo").expect("should have foo");
    assert_eq!(foo.kind, DeclKind::Import);
  }

  #[test]
  fn test_import_default() {
    let analysis = parse_and_analyze("import foo from './mod';");
    let foo = find_decl(&analysis, "foo").expect("should have foo");
    assert_eq!(foo.kind, DeclKind::Import);
  }

  #[test]
  fn test_import_namespace() {
    let analysis = parse_and_analyze("import * as ns from './mod';");
    let ns = find_decl(&analysis, "ns").expect("should have ns");
    assert_eq!(ns.kind, DeclKind::Import);
  }

  #[test]
  fn test_function_creates_scope() {
    let analysis =
      parse_and_analyze("function foo() { const x = 1; } const y = 2;");
    let foo = find_decl(&analysis, "foo").expect("should have foo");
    let x = find_decl(&analysis, "x").expect("should have x");
    let y = find_decl(&analysis, "y").expect("should have y");

    // foo and y should be in module scope (0)
    assert_eq!(foo.scope, ScopeId(0));
    assert_eq!(y.scope, ScopeId(0));
    // x should be in a nested scope
    assert_ne!(x.scope, ScopeId(0));
  }

  #[test]
  fn test_block_scope() {
    let analysis = parse_and_analyze("{ const x = 1; } const y = 2;");
    let x = find_decl(&analysis, "x").expect("should have x");
    let y = find_decl(&analysis, "y").expect("should have y");
    assert_ne!(x.scope, y.scope);
  }

  #[test]
  fn test_reference_resolution() {
    let analysis = parse_and_analyze("const a = 1; const b = a;");
    let a_decl = find_decl(&analysis, "a").expect("should have a");

    // Should have a resolved reference to a
    let a_ref = analysis
      .references
      .iter()
      .find(|r| r.name == "a" && r.resolved == Some(a_decl.id));
    assert!(a_ref.is_some(), "should have resolved reference to a");
  }

  #[test]
  fn test_unresolved_reference() {
    let analysis = parse_and_analyze("console.log('hello');");
    let console_ref = analysis
      .references
      .iter()
      .find(|r| r.name == "console" && r.resolved.is_none());
    assert!(console_ref.is_some(), "console should be unresolved");
  }

  #[test]
  fn test_catch_param() {
    let analysis = parse_and_analyze("try {} catch (e) { e; }");
    let e = find_decl(&analysis, "e").expect("should have e");
    assert_eq!(e.kind, DeclKind::CatchParam);
  }

  #[test]
  fn test_arrow_function() {
    let analysis = parse_and_analyze("const f = (x: number) => x + 1;");
    let f = find_decl(&analysis, "f").expect("should have f");
    let x = find_decl(&analysis, "x").expect("should have x");
    assert_eq!(f.scope, ScopeId(0));
    assert_ne!(x.scope, ScopeId(0));
  }

  #[test]
  fn test_enum_declaration() {
    let analysis =
      parse_and_analyze("enum Color { Red, Green, Blue }");
    let color = find_decl(&analysis, "Color").expect("should have Color");
    assert_eq!(color.kind, DeclKind::Var); // enums treated as Var
  }

  #[test]
  fn test_destructuring() {
    let analysis = parse_and_analyze("const { a, b: c } = obj;");
    assert!(find_decl(&analysis, "a").is_some());
    assert!(find_decl(&analysis, "c").is_some());
    // 'b' is a property key, not a binding
    assert!(
      !analysis
        .declarations
        .iter()
        .any(|d| d.name == "b" && d.kind == DeclKind::Const)
    );
  }
}
