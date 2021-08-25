// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use deno_core::error::AnyError;

pub struct BundleHook;

impl swc_bundler::Hook for BundleHook {
  fn get_import_meta_props(
    &self,
    span: swc_common::Span,
    module_record: &swc_bundler::ModuleRecord,
  ) -> Result<Vec<swc_ecmascript::ast::KeyValueProp>, AnyError> {
    use swc_ecmascript::ast;

    // we use custom file names, and swc "wraps" these in `<` and `>` so, we
    // want to strip those back out.
    let mut value = module_record.file_name.to_string();
    value.pop();
    value.remove(0);

    Ok(vec![
      ast::KeyValueProp {
        key: ast::PropName::Ident(ast::Ident::new("url".into(), span)),
        value: Box::new(ast::Expr::Lit(ast::Lit::Str(ast::Str {
          span,
          value: value.into(),
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
