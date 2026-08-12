// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;

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
    let reexports = analysis.reexports;
    let mut member_reexports = Vec::new();

    // Fallback for the shape `module.exports = require("./inner").MEMBER;`
    // (e.g. graphql-tag@2's main entry). deno_ast's CJS analyzer
    // recognizes the bare-call form but not the member-access form, so
    // it surfaces no exports here. Detect this top-level pattern and
    // record it as a member-scoped re-export. The recursive analyzer
    // then narrows the inner module's exports to those statically
    // attached to MEMBER, so unrelated names from the inner module
    // are not advertised by the wrapper.
    if exports.is_empty()
      && reexports.is_empty()
      && let Some((specifier, member)) =
        find_module_exports_require_member(self)
    {
      member_reexports.push(super::MemberReExport { specifier, member });
    }

    super::ModuleExportsAndReExports {
      exports,
      reexports,
      member_reexports,
    }
  }

  fn analyze_es_runtime_exports(&self) -> super::ModuleExportsAndReExports {
    let analysis = ParsedSource::analyze_es_runtime_exports(self);
    super::ModuleExportsAndReExports {
      exports: analysis.exports,
      reexports: analysis.reexports,
      member_reexports: Vec::new(),
    }
  }

  fn analyze_member_export_props(&self) -> BTreeMap<String, Vec<String>> {
    // Single walk of top-level statements: collect
    //   `exports.MEMBER = IDENT`  →  exports_aliases[MEMBER] = IDENT
    //   `IDENT.X = …`             →  ident_props[IDENT] += [X]
    // then compose into `MEMBER → sorted/deduped [X]`. Only members
    // whose IDENT also has at least one property assignment are kept,
    // since the caller's narrowing semantics are "advertise exactly the
    // names statically attached to MEMBER".
    let mut exports_aliases: Vec<(String, String)> = Vec::new();
    let mut ident_props: HashMap<String, Vec<String>> = HashMap::new();
    let mut walk = |stmt: &Stmt| {
      if let Some((member, ident)) = match_exports_member_to_ident(stmt) {
        exports_aliases.push((member, ident));
      } else if let Some((ident, prop)) = match_identifier_property(stmt) {
        ident_props.entry(ident).or_default().push(prop);
      }
    };
    match self.program_ref() {
      ProgramRef::Module(m) => {
        for item in &m.body {
          if let ModuleItem::Stmt(stmt) = item {
            walk(stmt);
          }
        }
      }
      ProgramRef::Script(s) => {
        for stmt in &s.body {
          walk(stmt);
        }
      }
    }

    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (member, ident) in exports_aliases {
      let Some(props) = ident_props.get(&ident) else {
        continue;
      };
      let mut props = props.clone();
      props.sort();
      props.dedup();
      out.insert(member, props);
    }
    out
  }
}

fn find_module_exports_require_member(
  ps: &ParsedSource,
) -> Option<(String, String)> {
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

fn match_module_exports_require_member(
  stmt: &Stmt,
) -> Option<(String, String)> {
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
  // RHS shape: exactly one `.MEMBER` member access wrapping a
  // `require("…")` call. Narrower than the previous walk (which
  // accepted nested member accesses) because the narrowing in
  // `analyze_member_export_props` only resolves a single hop.
  let outer_member = match &*assign.right {
    Expr::Member(m) => m,
    _ => return None,
  };
  let member_name = match &outer_member.prop {
    MemberProp::Ident(i) => i.sym.to_string(),
    MemberProp::Computed(c) => match &*c.expr {
      Expr::Lit(Lit::Str(s)) => s.value.as_str()?.to_string(),
      _ => return None,
    },
    MemberProp::PrivateName(_) => return None,
  };
  let call = match &*outer_member.obj {
    Expr::Call(c) => c,
    _ => return None,
  };
  let spec = call_expr_require_spec(call)?;
  Some((spec, member_name))
}

/// Match `exports.MEMBER = IDENT` (or `module.exports.MEMBER = IDENT`)
/// and return `(MEMBER, IDENT)`.
fn match_exports_member_to_ident(stmt: &Stmt) -> Option<(String, String)> {
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
  let member_name = exports_member_name(target_member)?;
  let ident = assign.right.as_ident()?;
  Some((member_name, ident.sym.to_string()))
}

/// Match `IDENT.X = …` and return `(IDENT, X)`.
fn match_identifier_property(stmt: &Stmt) -> Option<(String, String)> {
  let assign = match stmt {
    Stmt::Expr(e) => e.expr.as_assign()?,
    _ => return None,
  };
  if assign.op != AssignOp::Assign {
    return None;
  }
  let m = match &assign.left {
    AssignTarget::Simple(SimpleAssignTarget::Member(m)) => m,
    _ => return None,
  };
  let obj_ident = m.obj.as_ident()?;
  let prop = match &m.prop {
    MemberProp::Ident(i) => i.sym.to_string(),
    MemberProp::Computed(c) => match &*c.expr {
      Expr::Lit(Lit::Str(s)) => s.value.as_str()?.to_string(),
      _ => return None,
    },
    MemberProp::PrivateName(_) => return None,
  };
  Some((obj_ident.sym.to_string(), prop))
}

/// If `m` is `exports.NAME` or `module.exports.NAME`, return `NAME`.
fn exports_member_name(m: &MemberExpr) -> Option<String> {
  let prop = match &m.prop {
    MemberProp::Ident(i) => i.sym.to_string(),
    MemberProp::Computed(c) => match &*c.expr {
      Expr::Lit(Lit::Str(s)) => s.value.as_str()?.to_string(),
      _ => return None,
    },
    MemberProp::PrivateName(_) => return None,
  };
  let obj_is_exports = match &*m.obj {
    Expr::Ident(i) => &*i.sym == "exports",
    Expr::Member(inner) => is_module_exports_member(inner),
    _ => false,
  };
  if obj_is_exports { Some(prop) } else { None }
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
