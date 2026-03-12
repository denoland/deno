// Copyright 2018-2026 the Deno authors. MIT license.

use deno_ast::swc::common::Span;

/// Unique identifier for a scope within a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(pub u32);

/// Unique identifier for a declaration within a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclId(pub u32);

/// Unique identifier for a reference within a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RefId(pub u32);

/// The kind of scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
  /// Module-level scope (top-level).
  Module,
  /// Function scope (function, arrow function, method).
  Function,
  /// Block scope (if, for, while, bare block, etc.).
  Block,
}

/// The kind of declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
  Var,
  Let,
  Const,
  Function,
  Class,
  Import,
  Param,
  CatchParam,
}

/// A lexical scope in the module.
#[derive(Debug, Clone)]
pub struct Scope {
  pub id: ScopeId,
  pub parent: Option<ScopeId>,
  pub kind: ScopeKind,
  pub decls: Vec<DeclId>,
}

/// A declaration (variable, function, class, import, etc.).
#[derive(Debug, Clone)]
pub struct Declaration {
  pub id: DeclId,
  pub name: String,
  pub kind: DeclKind,
  pub scope: ScopeId,
  pub name_span: Span,
  pub refs: Vec<RefId>,
}

/// A reference to a name (identifier usage).
#[derive(Debug, Clone)]
pub struct Reference {
  pub id: RefId,
  pub name: String,
  pub span: Span,
  pub scope: ScopeId,
  /// The declaration this reference resolves to, if any.
  pub resolved: Option<DeclId>,
}

/// Complete scope analysis for a module.
#[derive(Debug, Clone)]
pub struct ScopeAnalysis {
  pub scopes: Vec<Scope>,
  pub declarations: Vec<Declaration>,
  pub references: Vec<Reference>,
}

impl ScopeAnalysis {
  pub fn new() -> Self {
    Self {
      scopes: Vec::new(),
      declarations: Vec::new(),
      references: Vec::new(),
    }
  }

  pub fn get_decl(&self, id: DeclId) -> &Declaration {
    &self.declarations[id.0 as usize]
  }

  pub fn get_scope(&self, id: ScopeId) -> &Scope {
    &self.scopes[id.0 as usize]
  }

  pub fn get_ref(&self, id: RefId) -> &Reference {
    &self.references[id.0 as usize]
  }
}

impl Default for ScopeAnalysis {
  fn default() -> Self {
    Self::new()
  }
}
