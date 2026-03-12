// Copyright 2018-2026 the Deno authors. MIT license.

//! SWC-based transforms for the bundler.
//!
//! Each transform is implemented as a `VisitMut` pass over the SWC AST.

use std::collections::HashMap;

use deno_ast::swc::ast::*;
use deno_ast::swc::common::DUMMY_SP;
use deno_ast::swc::ecma_visit::VisitMut;
use deno_ast::swc::ecma_visit::VisitMutWith;

/// Replace defined expressions with constant values.
///
/// Handles member expression chains (e.g. `process.env.NODE_ENV`),
/// bare identifiers (e.g. `__DEV__`), and typeof expressions.
pub struct DefineReplacer {
  /// Map from dotted expression string to replacement source.
  /// E.g. `"process.env.NODE_ENV"` → `"\"production\""`.
  pub defines: HashMap<String, String>,
}

impl VisitMut for DefineReplacer {
  fn visit_mut_expr(&mut self, expr: &mut Expr) {
    // Try to match this expression against define keys.
    let key = expr_to_dot_string(expr);
    if let Some(key) = &key {
      if let Some(replacement) = self.defines.get(key.as_str()) {
        if let Some(new_expr) = parse_replacement(replacement) {
          *expr = new_expr;
          return;
        }
      }
    }

    // Check typeof replacements: typeof <expr> → "string"
    if let Expr::Unary(unary) = expr {
      if unary.op == UnaryOp::TypeOf {
        let key =
          expr_to_dot_string(&unary.arg).map(|k| format!("typeof {}", k));
        if let Some(key) = &key {
          if let Some(replacement) = self.defines.get(key.as_str()) {
            if let Some(new_expr) = parse_replacement(replacement) {
              *expr = new_expr;
              return;
            }
          }
        }
      }
    }

    expr.visit_mut_children_with(self);
  }
}

/// Rewrite `import.meta.hot` references to `__hot` identifier.
pub struct HmrHotApiRewriter;

impl VisitMut for HmrHotApiRewriter {
  fn visit_mut_expr(&mut self, expr: &mut Expr) {
    if is_import_meta_hot(expr) {
      *expr = Expr::Ident(Ident::new_no_ctxt("__hot".into(), DUMMY_SP));
      return;
    }
    expr.visit_mut_children_with(self);
  }
}

/// Rewrite `import.meta.url`, `import.meta.dirname`, `import.meta.filename`
/// to inline string literals.
pub struct ImportMetaRewriter {
  pub url: String,
  pub dirname: String,
  pub filename: String,
}

impl VisitMut for ImportMetaRewriter {
  fn visit_mut_expr(&mut self, expr: &mut Expr) {
    if let Expr::Member(member) = expr {
      if is_import_meta(&member.obj) {
        if let MemberProp::Ident(prop) = &member.prop {
          let replacement = match &*prop.sym {
            "url" => Some(&self.url),
            "dirname" => Some(&self.dirname),
            "filename" => Some(&self.filename),
            _ => None,
          };
          if let Some(value) = replacement {
            *expr = Expr::Lit(Lit::Str(Str {
              span: DUMMY_SP,
              value: value.clone().into(),
              raw: None,
            }));
            return;
          }
        }
      }
    }
    expr.visit_mut_children_with(self);
  }
}

// -- Helpers --

/// Convert a member expression chain to a dotted string.
/// E.g. `process.env.NODE_ENV` → `"process.env.NODE_ENV"`.
fn expr_to_dot_string(expr: &Expr) -> Option<String> {
  match expr {
    Expr::Ident(ident) => Some(ident.sym.to_string()),
    Expr::Member(member) => {
      let obj = expr_to_dot_string(&member.obj)?;
      if let MemberProp::Ident(prop) = &member.prop {
        Some(format!("{}.{}", obj, prop.sym))
      } else {
        None
      }
    }
    Expr::MetaProp(meta) if meta.kind == MetaPropKind::ImportMeta => {
      Some("import.meta".to_string())
    }
    _ => None,
  }
}

/// Parse a simple replacement string into an expression.
/// Supports: string literals (`"production"`), numbers (`0`, `1`),
/// booleans (`true`, `false`), `null`, `undefined`, identifiers.
fn parse_replacement(s: &str) -> Option<Expr> {
  let s = s.trim();
  if (s.starts_with('"') && s.ends_with('"'))
    || (s.starts_with('\'') && s.ends_with('\''))
  {
    let inner = &s[1..s.len() - 1];
    return Some(Expr::Lit(Lit::Str(Str {
      span: DUMMY_SP,
      value: inner.into(),
      raw: None,
    })));
  }
  if s == "true" {
    return Some(Expr::Lit(Lit::Bool(Bool {
      span: DUMMY_SP,
      value: true,
    })));
  }
  if s == "false" {
    return Some(Expr::Lit(Lit::Bool(Bool {
      span: DUMMY_SP,
      value: false,
    })));
  }
  if s == "null" {
    return Some(Expr::Lit(Lit::Null(Null { span: DUMMY_SP })));
  }
  if s == "undefined" {
    return Some(Expr::Ident(Ident::new_no_ctxt(
      "undefined".into(),
      DUMMY_SP,
    )));
  }
  if let Ok(n) = s.parse::<f64>() {
    return Some(Expr::Lit(Lit::Num(Number {
      span: DUMMY_SP,
      value: n,
      raw: None,
    })));
  }
  // Treat as identifier.
  Some(Expr::Ident(Ident::new_no_ctxt(s.into(), DUMMY_SP)))
}

/// Check if an expression is `import.meta.hot`.
fn is_import_meta_hot(expr: &Expr) -> bool {
  let Expr::Member(member) = expr else {
    return false;
  };
  let MemberProp::Ident(prop) = &member.prop else {
    return false;
  };
  if &*prop.sym != "hot" {
    return false;
  }
  is_import_meta(&member.obj)
}

/// Check if an expression is `import.meta`.
fn is_import_meta(expr: &Expr) -> bool {
  match expr {
    Expr::MetaProp(meta) => meta.kind == MetaPropKind::ImportMeta,
    Expr::Paren(p) => is_import_meta(&p.expr),
    Expr::TsAs(ts) => is_import_meta(&ts.expr),
    Expr::TsSatisfies(ts) => is_import_meta(&ts.expr),
    Expr::TsTypeAssertion(ts) => is_import_meta(&ts.expr),
    Expr::TsNonNull(ts) => is_import_meta(&ts.expr),
    _ => false,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_define_replacer_parse_string() {
    let expr = parse_replacement("\"production\"").unwrap();
    match expr {
      Expr::Lit(Lit::Str(s)) => {
        assert_eq!(&*s.value, "production");
      }
      _ => panic!("expected string literal"),
    }
  }

  #[test]
  fn test_define_replacer_parse_bool() {
    let expr = parse_replacement("true").unwrap();
    match expr {
      Expr::Lit(Lit::Bool(b)) => assert!(b.value),
      _ => panic!("expected bool"),
    }
  }

  #[test]
  fn test_define_replacer_parse_number() {
    let expr = parse_replacement("42").unwrap();
    match expr {
      Expr::Lit(Lit::Num(n)) => assert_eq!(n.value, 42.0),
      _ => panic!("expected number"),
    }
  }

  #[test]
  fn test_define_replacer_parse_null() {
    let expr = parse_replacement("null").unwrap();
    assert!(matches!(expr, Expr::Lit(Lit::Null(_))));
  }

  #[test]
  fn test_define_replacer_parse_undefined() {
    let expr = parse_replacement("undefined").unwrap();
    match expr {
      Expr::Ident(ident) => assert_eq!(&*ident.sym, "undefined"),
      _ => panic!("expected ident"),
    }
  }

  #[test]
  fn test_expr_to_dot_string_ident() {
    let expr = Expr::Ident(Ident::new_no_ctxt("foo".into(), DUMMY_SP));
    assert_eq!(expr_to_dot_string(&expr), Some("foo".to_string()));
  }

  #[test]
  fn test_expr_to_dot_string_member() {
    let expr = Expr::Member(MemberExpr {
      span: DUMMY_SP,
      obj: Box::new(Expr::Ident(Ident::new_no_ctxt(
        "process".into(),
        DUMMY_SP,
      ))),
      prop: MemberProp::Ident(IdentName::new("env".into(), DUMMY_SP)),
    });
    assert_eq!(
      expr_to_dot_string(&expr),
      Some("process.env".to_string())
    );
  }

  #[test]
  fn test_expr_to_dot_string_import_meta() {
    let expr = Expr::MetaProp(MetaPropExpr {
      span: DUMMY_SP,
      kind: MetaPropKind::ImportMeta,
    });
    assert_eq!(
      expr_to_dot_string(&expr),
      Some("import.meta".to_string())
    );
  }
}
