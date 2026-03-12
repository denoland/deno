// Copyright 2018-2026 the Deno authors. MIT license.

//! Extract HMR metadata from a SWC AST (via `deno_ast::ParsedSource`).

use deno_ast::swc::ast::*;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_ast::ParsedSource;

use super::hmr_info::HmrInfo;
use super::module_info_swc::wtf8_to_string;

/// Extract HMR info from a parsed source.
pub fn extract_hmr_info(parsed: &ParsedSource) -> HmrInfo {
  let program = parsed.program();
  let mut extractor = HmrExtractor {
    info: HmrInfo::default(),
  };
  program.visit_with(&mut extractor);
  extractor.info
}

struct HmrExtractor {
  info: HmrInfo,
}

impl Visit for HmrExtractor {
  fn visit_call_expr(&mut self, expr: &CallExpr) {
    self.check_hot_call(expr);
    // Continue visiting children for nested calls.
    expr.visit_children_with(self);
  }
}

impl HmrExtractor {
  fn check_hot_call(&mut self, call: &CallExpr) {
    // Callee should be import.meta.hot.<method>
    let Callee::Expr(callee) = &call.callee else {
      return;
    };
    let Expr::Member(outer) = callee.as_ref() else {
      return;
    };
    let MemberProp::Ident(method_ident) = &outer.prop else {
      return;
    };
    let method = &*method_ident.sym;

    // outer.obj should be import.meta.hot
    if !is_import_meta_hot(&outer.obj) {
      return;
    }

    self.info.has_hot_api = true;

    match method {
      "accept" => self.handle_accept(call),
      "dispose" => {
        self.info.has_dispose = true;
      }
      "decline" => {
        self.info.declines = true;
      }
      "invalidate" => {
        // Just note presence of hot API.
      }
      _ => {}
    }
  }

  fn handle_accept(&mut self, call: &CallExpr) {
    if call.args.is_empty() {
      // accept() — no args, self-accept.
      self.info.self_accepts = true;
      return;
    }

    match &*call.args[0].expr {
      Expr::Lit(Lit::Str(lit)) => {
        // accept('./dep', cb) — single dep.
        self.info.accepted_deps.push(wtf8_to_string(&lit.value));
      }
      Expr::Array(array) => {
        // accept(['./a', './b'], cb) — array of deps.
        for elem in array.elems.iter().flatten() {
          if let Expr::Lit(Lit::Str(lit)) = &*elem.expr {
            self.info.accepted_deps.push(wtf8_to_string(&lit.value));
          }
        }
      }
      _ => {
        // First arg is not a string/array (e.g. a function) → self-accept.
        self.info.self_accepts = true;
      }
    }
  }
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

/// Check if an expression is `import.meta`, unwrapping TS type assertions
/// and parenthesized expressions.
fn is_import_meta(expr: &Expr) -> bool {
  match expr {
    Expr::MetaProp(meta) => meta.kind == MetaPropKind::ImportMeta,
    Expr::TsAs(ts) => is_import_meta(&ts.expr),
    Expr::TsSatisfies(ts) => is_import_meta(&ts.expr),
    Expr::TsTypeAssertion(ts) => is_import_meta(&ts.expr),
    Expr::TsNonNull(ts) => is_import_meta(&ts.expr),
    Expr::Paren(p) => is_import_meta(&p.expr),
    _ => false,
  }
}

#[cfg(test)]
mod tests {
  use deno_ast::MediaType;
  use deno_ast::ModuleSpecifier;
  use deno_ast::ParseParams;

  use super::*;

  fn parse_and_extract(source: &str) -> HmrInfo {
    let parsed = deno_ast::parse_module(ParseParams {
      specifier: ModuleSpecifier::parse("file:///test.ts").unwrap(),
      text: source.to_string().into(),
      media_type: MediaType::TypeScript,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })
    .unwrap();
    extract_hmr_info(&parsed)
  }

  #[test]
  fn test_self_accept_no_args() {
    let info = parse_and_extract("import.meta.hot.accept();");
    assert!(info.self_accepts);
    assert!(info.has_hot_api);
    assert!(info.accepted_deps.is_empty());
  }

  #[test]
  fn test_self_accept_callback() {
    let info = parse_and_extract(
      "import.meta.hot.accept((mod) => { console.log(mod); });",
    );
    assert!(info.self_accepts);
    assert!(info.has_hot_api);
  }

  #[test]
  fn test_dep_accept_string() {
    let info = parse_and_extract(
      "import.meta.hot.accept('./dep.js', (newDep) => {});",
    );
    assert!(!info.self_accepts);
    assert!(info.has_hot_api);
    assert_eq!(info.accepted_deps.len(), 1);
    assert_eq!(info.accepted_deps[0], "./dep.js");
  }

  #[test]
  fn test_dep_accept_array() {
    let info = parse_and_extract(
      "import.meta.hot.accept(['./a.js', './b.js'], ([a, b]) => {});",
    );
    assert!(!info.self_accepts);
    assert!(info.has_hot_api);
    assert_eq!(info.accepted_deps.len(), 2);
    assert_eq!(info.accepted_deps[0], "./a.js");
    assert_eq!(info.accepted_deps[1], "./b.js");
  }

  #[test]
  fn test_dispose() {
    let info = parse_and_extract(
      "import.meta.hot.dispose(() => { cleanup(); });",
    );
    assert!(info.has_dispose);
    assert!(info.has_hot_api);
    assert!(!info.self_accepts);
  }

  #[test]
  fn test_decline() {
    let info = parse_and_extract("import.meta.hot.decline();");
    assert!(info.declines);
    assert!(info.has_hot_api);
  }

  #[test]
  fn test_conditional_guard() {
    let info = parse_and_extract(
      "if (import.meta.hot) { import.meta.hot.accept(); }",
    );
    assert!(info.self_accepts);
    assert!(info.has_hot_api);
  }

  #[test]
  fn test_no_hmr_calls() {
    let info = parse_and_extract("console.log('hello');");
    assert!(!info.self_accepts);
    assert!(!info.has_hot_api);
    assert!(!info.has_dispose);
    assert!(!info.declines);
    assert!(info.accepted_deps.is_empty());
  }

  #[test]
  fn test_import_meta_url_not_hot() {
    let info = parse_and_extract("const url = import.meta.url;");
    assert!(!info.has_hot_api);
    assert!(!info.self_accepts);
  }

  #[test]
  fn test_multiple_hot_calls() {
    let info = parse_and_extract(
      "import.meta.hot.accept('./dep.js', cb);\nimport.meta.hot.dispose(() => {});",
    );
    assert!(info.has_hot_api);
    assert!(info.has_dispose);
    assert_eq!(info.accepted_deps.len(), 1);
    assert_eq!(info.accepted_deps[0], "./dep.js");
  }

  #[test]
  fn test_accept_and_self_accept() {
    let info = parse_and_extract(
      "import.meta.hot.accept();\nimport.meta.hot.accept('./dep.js', cb);",
    );
    assert!(info.self_accepts);
    assert_eq!(info.accepted_deps.len(), 1);
    assert_eq!(info.accepted_deps[0], "./dep.js");
  }

  #[test]
  fn test_nested_in_function() {
    let info = parse_and_extract(
      "function setup() { if (import.meta.hot) { import.meta.hot.accept(); } }",
    );
    assert!(info.self_accepts);
    assert!(info.has_hot_api);
  }
}
