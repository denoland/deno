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

/// Remove all import declarations from a module body.
pub fn remove_imports(module_body: &mut Vec<ModuleItem>) {
  module_body.retain(|item| {
    !matches!(item, ModuleItem::ModuleDecl(ModuleDecl::Import(_)))
  });
}

/// Strip export syntax from declarations, converting them to plain statements.
///
/// - `export const foo = 1` → `const foo = 1`
/// - `export default function foo() {}` → `function foo() {}`
/// - `export default <expr>` → `var <default_var> = <expr>`
/// - `export { foo }` / `export * from '...'` → removed
pub fn strip_exports(
  module_body: &mut Vec<ModuleItem>,
  default_var_name: &str,
) {
  let old_body = std::mem::take(module_body);
  for item in old_body {
    match item {
      ModuleItem::ModuleDecl(decl) => match decl {
        ModuleDecl::ExportDecl(export) => {
          // export const/function/class → plain declaration
          module_body.push(ModuleItem::Stmt(Stmt::Decl(export.decl)));
        }
        ModuleDecl::ExportNamed(_) => {
          // export { foo } / export { foo } from '...' → remove
        }
        ModuleDecl::ExportAll(_) => {
          // export * from '...' → remove
        }
        ModuleDecl::ExportDefaultExpr(export) => {
          // export default <expr> → var __default = <expr>
          let var_decl = make_var_init(default_var_name, *export.expr);
          module_body.push(ModuleItem::Stmt(Stmt::Decl(Decl::Var(
            Box::new(var_decl),
          ))));
        }
        ModuleDecl::ExportDefaultDecl(export) => {
          match export.decl {
            DefaultDecl::Fn(fn_expr) => {
              if fn_expr.ident.is_some() {
                // export default function foo() {} → function foo() {}
                module_body.push(ModuleItem::Stmt(Stmt::Decl(
                  Decl::Fn(FnDecl {
                    ident: fn_expr.ident.unwrap(),
                    declare: false,
                    function: fn_expr.function,
                  }),
                )));
              } else {
                // Anonymous → var __default = function() {}
                let var_decl = make_var_init(
                  default_var_name,
                  Expr::Fn(FnExpr {
                    ident: None,
                    function: fn_expr.function,
                  }),
                );
                module_body.push(ModuleItem::Stmt(Stmt::Decl(
                  Decl::Var(Box::new(var_decl)),
                )));
              }
            }
            DefaultDecl::Class(class_expr) => {
              if class_expr.ident.is_some() {
                // export default class Foo {} → class Foo {}
                module_body.push(ModuleItem::Stmt(Stmt::Decl(
                  Decl::Class(ClassDecl {
                    ident: class_expr.ident.unwrap(),
                    declare: false,
                    class: class_expr.class,
                  }),
                )));
              } else {
                // Anonymous → var __default = class {}
                let var_decl = make_var_init(
                  default_var_name,
                  Expr::Class(ClassExpr {
                    ident: None,
                    class: class_expr.class,
                  }),
                );
                module_body.push(ModuleItem::Stmt(Stmt::Decl(
                  Decl::Var(Box::new(var_decl)),
                )));
              }
            }
            DefaultDecl::TsInterfaceDecl(_) => {
              // Type-only, skip.
            }
          }
        }
        other @ (ModuleDecl::Import(_)
        | ModuleDecl::TsImportEquals(_)
        | ModuleDecl::TsExportAssignment(_)
        | ModuleDecl::TsNamespaceExport(_)) => {
          // Keep imports and TS-specific declarations.
          module_body.push(ModuleItem::ModuleDecl(other));
        }
      },
      other => module_body.push(other),
    }
  }
}

/// Eliminate dead branches from top-level if-statements based on
/// statically evaluable conditions using the defines map.
pub fn eliminate_dead_branches(
  module_body: &mut Vec<ModuleItem>,
  defines: &HashMap<String, String>,
) {
  if defines.is_empty() {
    return;
  }
  let old_body = std::mem::take(module_body);
  for item in old_body {
    match item {
      ModuleItem::Stmt(Stmt::If(if_stmt)) => {
        if let Some(result) = try_eval_condition(&if_stmt.test, defines) {
          if result {
            // Condition is true → keep consequent.
            module_body
              .push(ModuleItem::Stmt(*if_stmt.cons));
          } else if let Some(alt) = if_stmt.alt {
            // Condition is false → keep alternate.
            module_body.push(ModuleItem::Stmt(*alt));
          }
          // else: false with no alternate → remove entirely.
        } else {
          module_body
            .push(ModuleItem::Stmt(Stmt::If(if_stmt)));
        }
      }
      other => module_body.push(other),
    }
  }
}

/// Convert top-level `let`/`const` to `var` to avoid TDZ issues in
/// scope-hoisted bundles.
pub fn convert_top_level_to_var(module_body: &mut [ModuleItem]) {
  for item in module_body.iter_mut() {
    match item {
      ModuleItem::Stmt(Stmt::Decl(Decl::Var(var_decl))) => {
        if matches!(
          var_decl.kind,
          VarDeclKind::Let | VarDeclKind::Const
        ) {
          var_decl.kind = VarDeclKind::Var;
        }
      }
      ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
        if let Decl::Var(var_decl) = &mut export.decl {
          if matches!(
            var_decl.kind,
            VarDeclKind::Let | VarDeclKind::Const
          ) {
            var_decl.kind = VarDeclKind::Var;
          }
        }
      }
      _ => {}
    }
  }
}

// -- Helpers --

/// Create a `var <name> = <init>` declaration.
fn make_var_init(name: &str, init: Expr) -> VarDecl {
  VarDecl {
    span: DUMMY_SP,
    ctxt: Default::default(),
    kind: VarDeclKind::Var,
    declare: false,
    decls: vec![VarDeclarator {
      span: DUMMY_SP,
      name: Pat::Ident(BindingIdent {
        id: Ident::new_no_ctxt(name.into(), DUMMY_SP),
        type_ann: None,
      }),
      init: Some(Box::new(init)),
      definite: false,
    }],
  }
}

/// Try to statically evaluate a condition expression to a boolean
/// using the defines map.
fn try_eval_condition(
  expr: &Expr,
  defines: &HashMap<String, String>,
) -> Option<bool> {
  match expr {
    Expr::Paren(p) => try_eval_condition(&p.expr, defines),
    Expr::Bin(bin) => {
      let left = resolve_to_string(&bin.left, defines)?;
      let right = resolve_to_string(&bin.right, defines)?;
      match bin.op {
        BinaryOp::EqEq | BinaryOp::EqEqEq => Some(left == right),
        BinaryOp::NotEq | BinaryOp::NotEqEq => Some(left != right),
        _ => None,
      }
    }
    // Check if the expression itself resolves to a truthy/falsy define.
    _ => {
      let key = expr_to_dot_string(expr)?;
      let value = defines.get(&key)?;
      match value.trim() {
        "true" => Some(true),
        "false" => Some(false),
        s if s.starts_with('"') || s.starts_with('\'') => {
          // Non-empty string is truthy.
          Some(s.len() > 2)
        }
        "0" => Some(false),
        "null" | "undefined" => Some(false),
        _ => None,
      }
    }
  }
}

/// Resolve an expression to a string value using the defines map.
/// Returns the "canonical" value — for string literals and quoted defines,
/// returns the unquoted inner string.
fn resolve_to_string(
  expr: &Expr,
  defines: &HashMap<String, String>,
) -> Option<String> {
  match expr {
    Expr::Lit(Lit::Str(s)) => {
      Some(super::module_info_swc::wtf8_to_string(&s.value))
    }
    Expr::Lit(Lit::Num(n)) => Some(n.value.to_string()),
    Expr::Lit(Lit::Bool(b)) => Some(b.value.to_string()),
    _ => {
      let key = expr_to_dot_string(expr)?;
      let value = defines.get(&key)?;
      // Strip quotes from define values so they compare correctly
      // with unquoted string literals from the AST.
      Some(unquote_define_value(value))
    }
  }
}

/// Strip surrounding quotes from a define value if present.
fn unquote_define_value(s: &str) -> String {
  let s = s.trim();
  if (s.starts_with('"') && s.ends_with('"'))
    || (s.starts_with('\'') && s.ends_with('\''))
  {
    s[1..s.len() - 1].to_string()
  } else {
    s.to_string()
  }
}

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
  use deno_ast::MediaType;
  use deno_ast::ModuleSpecifier;
  use deno_ast::ParseParams;

  use super::*;

  /// Parse source and return the module body as a mutable Vec<ModuleItem>.
  fn parse_body(source: &str) -> Vec<ModuleItem> {
    let parsed = deno_ast::parse_module(ParseParams {
      specifier: ModuleSpecifier::parse("file:///test.ts").unwrap(),
      text: source.to_string().into(),
      media_type: MediaType::TypeScript,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })
    .unwrap();
    let program = parsed.program();
    match program.as_ref() {
      Program::Module(m) => m.body.clone(),
      Program::Script(s) => {
        s.body.iter().cloned().map(ModuleItem::Stmt).collect()
      }
    }
  }

  fn count_imports(body: &[ModuleItem]) -> usize {
    body
      .iter()
      .filter(|item| {
        matches!(item, ModuleItem::ModuleDecl(ModuleDecl::Import(_)))
      })
      .count()
  }

  fn count_exports(body: &[ModuleItem]) -> usize {
    body
      .iter()
      .filter(|item| {
        matches!(
          item,
          ModuleItem::ModuleDecl(
            ModuleDecl::ExportDecl(_)
              | ModuleDecl::ExportNamed(_)
              | ModuleDecl::ExportAll(_)
              | ModuleDecl::ExportDefaultDecl(_)
              | ModuleDecl::ExportDefaultExpr(_)
          )
        )
      })
      .count()
  }

  // -- remove_imports tests --

  #[test]
  fn test_remove_imports() {
    let mut body =
      parse_body("import { foo } from './mod';\nconst x = foo;");
    assert_eq!(count_imports(&body), 1);
    remove_imports(&mut body);
    assert_eq!(count_imports(&body), 0);
    assert_eq!(body.len(), 1); // only const x = foo remains
  }

  #[test]
  fn test_remove_imports_keeps_non_imports() {
    let mut body = parse_body("const a = 1;\nconst b = 2;");
    let len_before = body.len();
    remove_imports(&mut body);
    assert_eq!(body.len(), len_before);
  }

  // -- strip_exports tests --

  #[test]
  fn test_strip_export_decl() {
    let mut body = parse_body("export const foo = 1;");
    assert_eq!(count_exports(&body), 1);
    strip_exports(&mut body, "__default");
    assert_eq!(count_exports(&body), 0);
    assert_eq!(body.len(), 1);
    // Should be a plain const declaration now.
    assert!(matches!(&body[0], ModuleItem::Stmt(Stmt::Decl(Decl::Var(_)))));
  }

  #[test]
  fn test_strip_export_default_expr() {
    let mut body = parse_body("export default 42;");
    assert_eq!(count_exports(&body), 1);
    strip_exports(&mut body, "__default");
    assert_eq!(count_exports(&body), 0);
    // Should be: var __default = 42
    assert_eq!(body.len(), 1);
    if let ModuleItem::Stmt(Stmt::Decl(Decl::Var(var_decl))) = &body[0] {
      assert_eq!(var_decl.kind, VarDeclKind::Var);
      if let Pat::Ident(binding) = &var_decl.decls[0].name {
        assert_eq!(&*binding.id.sym, "__default");
      } else {
        panic!("expected ident pattern");
      }
    } else {
      panic!("expected var decl");
    }
  }

  #[test]
  fn test_strip_export_default_named_fn() {
    let mut body =
      parse_body("export default function foo() { return 1; }");
    strip_exports(&mut body, "__default");
    assert_eq!(count_exports(&body), 0);
    // Should be: function foo() { return 1; }
    assert!(matches!(
      &body[0],
      ModuleItem::Stmt(Stmt::Decl(Decl::Fn(_)))
    ));
  }

  #[test]
  fn test_strip_export_default_anon_fn() {
    let mut body =
      parse_body("export default function() { return 1; }");
    strip_exports(&mut body, "__default");
    assert_eq!(count_exports(&body), 0);
    // Should be: var __default = function() { return 1; }
    assert!(matches!(
      &body[0],
      ModuleItem::Stmt(Stmt::Decl(Decl::Var(_)))
    ));
  }

  #[test]
  fn test_strip_reexport() {
    let mut body = parse_body("export { foo } from './mod';");
    assert_eq!(count_exports(&body), 1);
    strip_exports(&mut body, "__default");
    assert_eq!(body.len(), 0); // removed entirely
  }

  #[test]
  fn test_strip_export_all() {
    let mut body = parse_body("export * from './mod';");
    strip_exports(&mut body, "__default");
    assert_eq!(body.len(), 0);
  }

  // -- eliminate_dead_branches tests --

  #[test]
  fn test_eliminate_true_branch() {
    let mut body = parse_body(
      "if (process.env.NODE_ENV === \"production\") { console.log('prod'); } else { console.log('dev'); }",
    );
    let mut defines = HashMap::new();
    defines.insert(
      "process.env.NODE_ENV".to_string(),
      "\"production\"".to_string(),
    );
    assert_eq!(body.len(), 1);
    eliminate_dead_branches(&mut body, &defines);
    // Should keep only the consequent.
    assert_eq!(body.len(), 1);
    assert!(matches!(&body[0], ModuleItem::Stmt(Stmt::Block(_))));
  }

  #[test]
  fn test_eliminate_false_branch() {
    let mut body = parse_body(
      "if (process.env.NODE_ENV === \"production\") { console.log('prod'); } else { console.log('dev'); }",
    );
    let mut defines = HashMap::new();
    defines.insert(
      "process.env.NODE_ENV".to_string(),
      "\"development\"".to_string(),
    );
    eliminate_dead_branches(&mut body, &defines);
    // Should keep only the alternate.
    assert_eq!(body.len(), 1);
    assert!(matches!(&body[0], ModuleItem::Stmt(Stmt::Block(_))));
  }

  #[test]
  fn test_eliminate_false_no_else() {
    let mut body = parse_body(
      "if (process.env.NODE_ENV === \"production\") { console.log('prod'); }",
    );
    let mut defines = HashMap::new();
    defines.insert(
      "process.env.NODE_ENV".to_string(),
      "\"development\"".to_string(),
    );
    eliminate_dead_branches(&mut body, &defines);
    // Should remove entirely.
    assert_eq!(body.len(), 0);
  }

  // -- convert_top_level_to_var tests --

  #[test]
  fn test_convert_const_to_var() {
    let mut body = parse_body("const x = 1;");
    if let ModuleItem::Stmt(Stmt::Decl(Decl::Var(ref v))) = body[0] {
      assert_eq!(v.kind, VarDeclKind::Const);
    }
    convert_top_level_to_var(&mut body);
    if let ModuleItem::Stmt(Stmt::Decl(Decl::Var(ref v))) = body[0] {
      assert_eq!(v.kind, VarDeclKind::Var);
    } else {
      panic!("expected var decl");
    }
  }

  #[test]
  fn test_convert_let_to_var() {
    let mut body = parse_body("let x = 1;");
    convert_top_level_to_var(&mut body);
    if let ModuleItem::Stmt(Stmt::Decl(Decl::Var(ref v))) = body[0] {
      assert_eq!(v.kind, VarDeclKind::Var);
    } else {
      panic!("expected var decl");
    }
  }

  #[test]
  fn test_convert_var_stays_var() {
    let mut body = parse_body("var x = 1;");
    convert_top_level_to_var(&mut body);
    if let ModuleItem::Stmt(Stmt::Decl(Decl::Var(ref v))) = body[0] {
      assert_eq!(v.kind, VarDeclKind::Var);
    } else {
      panic!("expected var decl");
    }
  }

  // -- try_eval_condition tests --

  #[test]
  fn test_eval_equality_true() {
    let mut defines = HashMap::new();
    defines
      .insert("process.env.NODE_ENV".into(), "\"production\"".into());
    let body =
      parse_body("process.env.NODE_ENV === \"production\"");
    // The body is an expression statement.
    if let ModuleItem::Stmt(Stmt::Expr(expr_stmt)) = &body[0] {
      assert_eq!(try_eval_condition(&expr_stmt.expr, &defines), Some(true));
    }
  }

  #[test]
  fn test_eval_equality_false() {
    let mut defines = HashMap::new();
    defines.insert(
      "process.env.NODE_ENV".into(),
      "\"development\"".into(),
    );
    let body =
      parse_body("process.env.NODE_ENV === \"production\"");
    if let ModuleItem::Stmt(Stmt::Expr(expr_stmt)) = &body[0] {
      assert_eq!(
        try_eval_condition(&expr_stmt.expr, &defines),
        Some(false)
      );
    }
  }

  #[test]
  fn test_eval_not_equal() {
    let mut defines = HashMap::new();
    defines
      .insert("process.env.NODE_ENV".into(), "\"production\"".into());
    let body =
      parse_body("process.env.NODE_ENV !== \"development\"");
    if let ModuleItem::Stmt(Stmt::Expr(expr_stmt)) = &body[0] {
      assert_eq!(try_eval_condition(&expr_stmt.expr, &defines), Some(true));
    }
  }

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
