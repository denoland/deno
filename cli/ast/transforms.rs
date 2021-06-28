use swc_common::DUMMY_SP;
use swc_ecmascript::ast as swc_ast;
use swc_ecmascript::visit::noop_fold_type;
use swc_ecmascript::visit::Fold;

/// Transforms import declarations to variable declarations
/// with a dynamic import. This is used to provide import
/// declaration support in the REPL.
pub struct DownlevelImportsFolder;

impl Fold for DownlevelImportsFolder {
  noop_fold_type!(); // skip typescript specific nodes

  fn fold_module_item(
    &mut self,
    module_item: swc_ast::ModuleItem,
  ) -> swc_ast::ModuleItem {
    use swc_ecmascript::ast::*;

    match &module_item {
      ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)) => {
        // Handle type only imports
        if import_decl.type_only {
          return ModuleItem::Stmt(Stmt::Empty(EmptyStmt { span: DUMMY_SP }));
        }

        // The initializer (ex. `await import('./mod.ts')`)
        let initializer = Box::new(Expr::Await(AwaitExpr {
          span: DUMMY_SP,
          arg: Box::new(Expr::Call(CallExpr {
            span: DUMMY_SP,
            callee: ExprOrSuper::Expr(Box::new(Expr::Ident(Ident {
              span: DUMMY_SP,
              sym: "import".into(),
              optional: false,
            }))),
            args: vec![ExprOrSpread {
              spread: None,
              expr: Box::new(Expr::Lit(Lit::Str(Str {
                span: DUMMY_SP,
                has_escape: false,
                kind: StrKind::Normal {
                  contains_quote: false,
                },
                value: import_decl.src.value.clone(),
              }))),
            }],
            type_args: None,
          })),
        }));

        // Handle imports for the side effects
        // ex. `import "module.ts"` -> `await import("module.ts");`
        if import_decl.specifiers.is_empty() {
          return ModuleItem::Stmt(Stmt::Expr(ExprStmt {
            span: DUMMY_SP,
            expr: initializer,
          }));
        }

        // Collect the specifiers and create the variable statement
        let named_import_props = import_decl
          .specifiers
          .iter()
          .filter_map(|specifier| match specifier {
            ImportSpecifier::Default(specifier) => Some(create_key_value(
              "default".to_string(),
              specifier.local.sym.to_string(),
            )),
            ImportSpecifier::Named(specifier) => {
              Some(match specifier.imported.as_ref() {
                Some(name) => create_key_value(
                  name.sym.to_string(),
                  specifier.local.sym.to_string(),
                ),
                None => create_assignment(specifier.local.sym.to_string()),
              })
            }
            ImportSpecifier::Namespace(_) => None,
          })
          .collect::<Vec<_>>();
        let namespace_import_name =
          import_decl
            .specifiers
            .iter()
            .find_map(|specifier| match specifier {
              ImportSpecifier::Namespace(specifier) => {
                Some(create_binding_ident(specifier.local.sym.to_string()))
              }
              _ => None,
            });

        ModuleItem::Stmt(Stmt::Decl(Decl::Var(VarDecl {
          span: DUMMY_SP,
          kind: VarDeclKind::Const,
          declare: false,
          decls: {
            let mut decls = Vec::new();

            if !named_import_props.is_empty() {
              decls.push(VarDeclarator {
                span: DUMMY_SP,
                name: Pat::Object(ObjectPat {
                  span: DUMMY_SP,
                  optional: false,
                  props: named_import_props,
                  type_ann: None,
                }),
                definite: false,
                init: Some(initializer.clone()),
              });
            }
            if let Some(namespace_import) = namespace_import_name {
              decls.push(VarDeclarator {
                span: DUMMY_SP,
                name: Pat::Ident(namespace_import),
                definite: false,
                init: Some(initializer),
              });
            }

            decls
          },
        })))
      }
      _ => module_item,
    }
  }
}

fn create_binding_ident(name: String) -> swc_ast::BindingIdent {
  swc_ast::BindingIdent {
    id: create_ident(name),
    type_ann: None,
  }
}

fn create_ident(name: String) -> swc_ast::Ident {
  swc_ast::Ident {
    span: DUMMY_SP,
    sym: name.into(),
    optional: false,
  }
}

fn create_key_value(key: String, value: String) -> swc_ast::ObjectPatProp {
  swc_ast::ObjectPatProp::KeyValue(swc_ast::KeyValuePatProp {
    key: swc_ast::PropName::Ident(swc_ast::Ident {
      span: DUMMY_SP,
      sym: key.into(),
      optional: false,
    }),
    value: Box::new(swc_ast::Pat::Ident(swc_ast::BindingIdent {
      id: swc_ast::Ident {
        span: DUMMY_SP,
        sym: value.into(),
        optional: false,
      },
      type_ann: None,
    })),
  })
}

fn create_assignment(key: String) -> swc_ast::ObjectPatProp {
  swc_ast::ObjectPatProp::Assign(swc_ast::AssignPatProp {
    span: DUMMY_SP,
    key: create_ident(key),
    value: None,
  })
}

#[cfg(test)]
mod test {
  use std::rc::Rc;
  use swc_common::FileName;
  use swc_common::SourceMap;
  use swc_ecmascript::ast::Module;
  use swc_ecmascript::codegen::text_writer::JsWriter;
  use swc_ecmascript::codegen::Node;
  use swc_ecmascript::parser::Parser;
  use swc_ecmascript::parser::StringInput;
  use swc_ecmascript::parser::Syntax;
  use swc_ecmascript::parser::TsConfig;
  use swc_ecmascript::visit::Fold;
  use swc_ecmascript::visit::FoldWith;

  use super::*;

  #[test]
  fn test_downlevel_imports_type_only() {
    test_transform(
      DownlevelImportsFolder,
      r#"import type { test } from "./mod.ts";"#,
      ";",
    );
  }

  #[test]
  fn test_downlevel_imports_specifier_only() {
    test_transform(
      DownlevelImportsFolder,
      r#"import "./mod.ts";"#,
      r#"await import("./mod.ts");"#,
    );

    test_transform(
      DownlevelImportsFolder,
      r#"import {} from "./mod.ts";"#,
      r#"await import("./mod.ts");"#,
    );
  }

  #[test]
  fn test_downlevel_imports_default() {
    test_transform(
      DownlevelImportsFolder,
      r#"import mod from "./mod.ts";"#,
      r#"const { default: mod  } = await import("./mod.ts");"#,
    );
  }

  #[test]
  fn test_downlevel_imports_named() {
    test_transform(
      DownlevelImportsFolder,
      r#"import { A } from "./mod.ts";"#,
      r#"const { A  } = await import("./mod.ts");"#,
    );

    test_transform(
      DownlevelImportsFolder,
      r#"import { A, B, C  } from "./mod.ts";"#,
      r#"const { A , B , C  } = await import("./mod.ts");"#,
    );

    test_transform(
      DownlevelImportsFolder,
      r#"import { A as LocalA, B, C as LocalC  } from "./mod.ts";"#,
      r#"const { A: LocalA , B , C: LocalC  } = await import("./mod.ts");"#,
    );
  }

  #[test]
  fn test_downlevel_imports_namespace() {
    test_transform(
      DownlevelImportsFolder,
      r#"import * as mod from "./mod.ts";"#,
      r#"const mod = await import("./mod.ts");"#,
    );
  }

  #[test]
  fn test_downlevel_imports_mixed() {
    test_transform(
      DownlevelImportsFolder,
      r#"import myDefault, { A, B as LocalB } from "./mod.ts";"#,
      r#"const { default: myDefault , A , B: LocalB  } = await import("./mod.ts");"#,
    );

    test_transform(
      DownlevelImportsFolder,
      r#"import myDefault, * as mod from "./mod.ts";"#,
      r#"const { default: myDefault  } = await import("./mod.ts"), mod = await import("./mod.ts");"#,
    );
  }

  fn test_transform(
    mut transform: impl Fold,
    src: &str,
    expected_output: &str,
  ) {
    let (source_map, module) = parse(src);
    let output = print(source_map, module.fold_with(&mut transform));
    assert_eq!(output, format!("{}\n", expected_output));
  }

  fn parse(src: &str) -> (Rc<SourceMap>, Module) {
    let source_map = Rc::new(SourceMap::default());
    let source_file = source_map.new_source_file(
      FileName::Custom("file.ts".to_string()),
      src.to_string(),
    );
    let input = StringInput::from(&*source_file);
    let syntax = Syntax::Typescript(TsConfig {
      ..Default::default()
    });
    let mut parser = Parser::new(syntax, input, None);
    (source_map, parser.parse_module().unwrap())
  }

  fn print(source_map: Rc<SourceMap>, module: Module) -> String {
    let mut buf = vec![];
    {
      let writer =
        Box::new(JsWriter::new(source_map.clone(), "\n", &mut buf, None));
      let config = swc_ecmascript::codegen::Config { minify: false };
      let mut emitter = swc_ecmascript::codegen::Emitter {
        cfg: config,
        comments: None,
        cm: source_map,
        wr: writer,
      };
      module.emit_with(&mut emitter).unwrap();
    }
    String::from_utf8(buf).unwrap()
  }
}
