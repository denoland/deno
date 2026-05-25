// Copyright 2018-2026 the Deno authors. MIT license.

use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_ast::ProgramRef;
use deno_ast::swc::ast::AssignOp;
use deno_ast::swc::ast::AssignTarget;
use deno_ast::swc::ast::CallExpr;
use deno_ast::swc::ast::Expr;
use deno_ast::swc::ast::Lit;
use deno_ast::swc::ast::MemberExpr;
use deno_ast::swc::ast::MemberProp;
use deno_ast::swc::ast::ModuleItem;
use deno_ast::swc::ast::SimpleAssignTarget;
use deno_ast::swc::ast::Stmt;
use deno_error::JsErrorBox;
use deno_graph::ast::ParsedSourceStore;
use url::Url;

use super::ModuleExportAnalyzer;
use crate::cache::ParsedSourceCacheRc;

pub struct DenoAstModuleExportAnalyzer {
  parsed_source_cache: ParsedSourceCacheRc,
}

impl DenoAstModuleExportAnalyzer {
  pub fn new(parsed_source_cache: ParsedSourceCacheRc) -> Self {
    Self {
      parsed_source_cache,
    }
  }
}

#[allow(
  clippy::disallowed_types,
  reason = "source text is always stored as Arc<str>"
)]
type ArcStr = std::sync::Arc<str>;

impl ModuleExportAnalyzer for DenoAstModuleExportAnalyzer {
  fn parse_module(
    &self,
    specifier: Url,
    media_type: MediaType,
    source: ArcStr,
  ) -> Result<Box<dyn super::ModuleForExportAnalysis>, JsErrorBox> {
    let maybe_parsed_source =
      self.parsed_source_cache.remove_parsed_source(&specifier);
    let parsed_source = maybe_parsed_source
      .map(Ok)
      .unwrap_or_else(|| {
        deno_ast::parse_program(deno_ast::ParseParams {
          specifier,
          text: source,
          media_type,
          capture_tokens: true,
          scope_analysis: false,
          maybe_syntax: None,
        })
      })
      .map_err(JsErrorBox::from_err)?;
    Ok(Box::new(parsed_source))
  }
}

impl super::ModuleForExportAnalysis for ParsedSource {
  fn specifier(&self) -> &Url {
    self.specifier()
  }

  fn compute_is_script(&self) -> bool {
    self.compute_is_script()
  }

  fn analyze_cjs(&self) -> super::ModuleExportsAndReExports {
    let analysis = ParsedSource::analyze_cjs(self);
    let exports = analysis.exports;
    let mut reexports = analysis.reexports;

    // Fallback for the shape `module.exports = require("./inner").IDENT;`
    // (e.g. graphql-tag@2's main entry). deno_ast's CJS analyzer
    // recognizes the bare-call form but not the member-access form, so
    // it surfaces no exports here. Detect this top-level pattern and
    // emit the inner specifier as a re-export; the existing recursive
    // re-export machinery then picks up the inner module's named
    // exports.
    if exports.is_empty()
      && reexports.is_empty()
      && let Some(spec) = find_module_exports_require_member(self)
    {
      reexports.push(spec);
    }

    super::ModuleExportsAndReExports { exports, reexports }
  }

  fn analyze_es_runtime_exports(&self) -> super::ModuleExportsAndReExports {
    let analysis = ParsedSource::analyze_es_runtime_exports(self);
    super::ModuleExportsAndReExports {
      exports: analysis.exports,
      reexports: analysis.reexports,
    }
  }
}

fn find_module_exports_require_member(ps: &ParsedSource) -> Option<String> {
  match ps.program_ref() {
    ProgramRef::Module(m) => m.body.iter().find_map(|item| match item {
      ModuleItem::Stmt(stmt) => match_module_exports_require_member(stmt),
      ModuleItem::ModuleDecl(_) => None,
    }),
    ProgramRef::Script(s) => {
      s.body.iter().find_map(match_module_exports_require_member)
    }
  }
}

fn match_module_exports_require_member(stmt: &Stmt) -> Option<String> {
  let assign = match stmt {
    Stmt::Expr(e) => e.expr.as_assign()?,
    _ => return None,
  };
  if assign.op != AssignOp::Assign {
    return None;
  }
  let target_member = match &assign.left {
    AssignTarget::Simple(SimpleAssignTarget::Member(m)) => m,
    _ => return None,
  };
  if !is_module_exports_member(target_member) {
    return None;
  }
  // RHS shape: zero or more `.IDENT` member accesses wrapping a
  // `require("…")` call. Walk down to the call.
  let mut current: &Expr = &assign.right;
  loop {
    match current {
      Expr::Call(call) => return call_expr_require_spec(call),
      Expr::Member(m) => current = &m.obj,
      _ => return None,
    }
  }
}

fn is_module_exports_member(m: &MemberExpr) -> bool {
  let Expr::Ident(obj_ident) = &*m.obj else {
    return false;
  };
  if &*obj_ident.sym != "module" {
    return false;
  }
  match &m.prop {
    MemberProp::Ident(i) => &*i.sym == "exports",
    MemberProp::Computed(c) => matches!(
      &*c.expr,
      Expr::Lit(Lit::Str(s)) if s.value.as_str() == Some("exports"),
    ),
    MemberProp::PrivateName(_) => false,
  }
}

fn call_expr_require_spec(call: &CallExpr) -> Option<String> {
  let callee_expr = call.callee.as_expr()?;
  let ident = callee_expr.as_ident()?;
  if &*ident.sym != "require" {
    return None;
  }
  let arg = call.args.first()?;
  if arg.spread.is_some() {
    return None;
  }
  match arg.expr.as_lit()? {
    Lit::Str(s) => s.value.as_str().map(|s| s.to_string()),
    _ => None,
  }
}
