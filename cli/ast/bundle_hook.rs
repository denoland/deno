// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use deno_ast::swc::bundler::Hook;
use deno_ast::swc::bundler::ModuleRecord;
use deno_ast::swc::common::Span;
use deno_core::error::AnyError;

/// This contains the logic for Deno to rewrite the `import.meta` when bundling.
pub struct BundleHook;

impl Hook for BundleHook {
  fn get_import_meta_props(
    &self,
    span: Span,
    module_record: &ModuleRecord,
  ) -> Result<Vec<deno_ast::swc::ast::KeyValueProp>, AnyError> {
    use deno_ast::swc::ast;

    Ok(vec![
      ast::KeyValueProp {
        key: ast::PropName::Ident(ast::Ident::new("url".into(), span)),
        value: Box::new(ast::Expr::Lit(ast::Lit::Str(ast::Str {
          span,
          value: module_record.file_name.to_string().into(),
          kind: ast::StrKind::Synthesized,
          has_escape: false,
        }))),
      },
      ast::KeyValueProp {
        key: ast::PropName::Ident(ast::Ident::new("main".into(), span)),
        value: Box::new(if module_record.is_entry {
          ast::Expr::Member(ast::MemberExpr {
            span,
            obj: ast::ExprOrSuper::Expr(Box::new(ast::Expr::MetaProp(
              ast::MetaPropExpr {
                meta: ast::Ident::new("import".into(), span),
                prop: ast::Ident::new("meta".into(), span),
              },
            ))),
            prop: Box::new(ast::Expr::Ident(ast::Ident::new(
              "main".into(),
              span,
            ))),
            computed: false,
          })
        } else {
          ast::Expr::Lit(ast::Lit::Bool(ast::Bool { span, value: false }))
        }),
      },
    ])
  }
}
