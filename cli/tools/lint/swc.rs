use deno_ast::{
  swc::{
    ast::{
      AssignTarget, AssignTargetPat, BlockStmtOrExpr, Callee, ClassMember,
      Decl, ExportSpecifier, Expr, ExprOrSpread, FnExpr, ForHead, Function,
      Ident, IdentName, JSXAttrName, JSXAttrOrSpread, JSXAttrValue, JSXElement,
      JSXElementChild, JSXElementName, JSXEmptyExpr, JSXExpr, JSXExprContainer,
      JSXFragment, JSXMemberExpr, JSXNamespacedName, JSXObject,
      JSXOpeningElement, Lit, MemberExpr, MemberProp, ModuleDecl,
      ModuleExportName, ModuleItem, ObjectPatProp, OptChainBase, Param,
      ParamOrTsParamProp, Pat, PrivateName, Program, Prop, PropName,
      PropOrSpread, SimpleAssignTarget, Stmt, SuperProp, Tpl, TsEntityName,
      TsEnumMemberId, TsFnOrConstructorType, TsFnParam, TsLit, TsType,
      TsTypeAnn, TsTypeElement, TsTypeParam, TsTypeParamDecl, TsTypeQueryExpr,
      TsUnionOrIntersectionType, VarDeclOrExpr,
    },
    common::{Span, Spanned, SyntaxContext, DUMMY_SP},
  },
  view::{
    Accessibility, AssignOp, BinaryOp, TruePlusMinus, TsKeywordTypeKind,
    UnaryOp, UpdateOp, VarDeclKind,
  },
  ParsedSource,
};
use indexmap::IndexMap;

use super::ast_buf::{
  append_usize, assign_op_to_flag, AstNode, AstProp, Flag, FlagValue,
  SerializeCtx, StringTable,
};

pub fn serialize_ast_bin(parsed_source: &ParsedSource) -> Vec<u8> {
  let mut ctx = SerializeCtx::new();

  let parent_id = 0;

  let program = &parsed_source.program();
  let mut flags = FlagValue::new();

  // eprintln!("SWC {:#?}", program);

  let root_id = ctx.next_id();
  match program.as_ref() {
    Program::Module(module) => {
      flags.set(Flag::ProgramModule);

      let child_ids = module
        .body
        .iter()
        .map(|item| match item {
          ModuleItem::ModuleDecl(module_decl) => {
            serialize_module_decl(&mut ctx, module_decl, parent_id)
          }
          ModuleItem::Stmt(stmt) => serialize_stmt(&mut ctx, stmt, root_id),
        })
        .collect::<Vec<_>>();

      ctx.write_node(root_id, AstNode::Program, parent_id, &module.span, 2);
      ctx.write_flags(&flags);
      ctx.write_ids(AstProp::Body, child_ids);
    }
    Program::Script(script) => {
      let child_ids = script
        .body
        .iter()
        .map(|stmt| serialize_stmt(&mut ctx, stmt, root_id))
        .collect::<Vec<_>>();

      ctx.write_node(root_id, AstNode::Program, parent_id, &script.span, 2);
      ctx.write_flags(&flags);
      ctx.write_ids(AstProp::Body, child_ids);
    }
  }

  let mut buf: Vec<u8> = vec![];

  // Append serialized AST
  buf.append(&mut ctx.buf);

  let offset_str_table = buf.len();

  // Serialize string table
  // eprintln!("STRING {:#?}", ctx.str_table);
  buf.append(&mut ctx.str_table.serialize());

  let offset_id_table = buf.len();

  // Serialize ids
  append_usize(&mut buf, ctx.id_to_offset.len());

  let mut ids = ctx.id_to_offset.keys().collect::<Vec<_>>();
  ids.sort();
  for id in ids {
    let offset = ctx.id_to_offset.get(id).unwrap();
    append_usize(&mut buf, *offset);
  }

  append_usize(&mut buf, offset_str_table);
  append_usize(&mut buf, offset_id_table);
  append_usize(&mut buf, root_id);

  buf
}

fn serialize_module_decl(
  ctx: &mut SerializeCtx,
  module_decl: &ModuleDecl,
  parent_id: usize,
) -> usize {
  match module_decl {
    ModuleDecl::Import(node) => {
      ctx.push_node(AstNode::Import, parent_id, &node.span)
    }
    ModuleDecl::ExportDecl(node) => {
      ctx.push_node(AstNode::ExportDecl, parent_id, &node.span)
    }
    ModuleDecl::ExportNamed(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      flags.set(Flag::ExportType);

      let src_id = node
        .src
        .as_ref()
        .map_or(0, |src| serialize_lit(ctx, &Lit::Str(*src.clone()), id));

      let spec_ids = node
        .specifiers
        .iter()
        .map(|spec| {
          match spec {
            ExportSpecifier::Named(child) => {
              let spec_id = ctx.next_id();

              let mut flags = FlagValue::new();
              flags.set(Flag::ExportType);

              let org_id =
                serialize_module_exported_name(ctx, &child.orig, spec_id);

              let exported_id = child.exported.as_ref().map_or(0, |exported| {
                serialize_module_exported_name(ctx, exported, spec_id)
              });

              ctx.write_node(
                spec_id,
                AstNode::ExportSpecifier,
                id,
                &child.span,
                3,
              );
              ctx.write_flags(&flags);
              ctx.write_prop(AstProp::Local, org_id);
              ctx.write_prop(AstProp::Exported, exported_id);

              spec_id
            }

            // These two aren't syntactically valid
            ExportSpecifier::Namespace(_) => todo!(),
            ExportSpecifier::Default(_) => todo!(),
          }
        })
        .collect::<Vec<_>>();

      ctx.write_node(
        id,
        AstNode::ExportNamedDeclaration,
        parent_id,
        &node.span,
        3,
      );
      ctx.write_flags(&flags);
      ctx.write_prop(AstProp::Source, src_id);
      ctx.write_ids(AstProp::Specifiers, spec_ids);

      id
    }
    ModuleDecl::ExportDefaultDecl(node) => {
      ctx.push_node(AstNode::ExportDefaultDecl, parent_id, &node.span)
    }
    ModuleDecl::ExportDefaultExpr(node) => {
      ctx.push_node(AstNode::ExportDefaultExpr, parent_id, &node.span)
    }
    ModuleDecl::ExportAll(node) => {
      ctx.push_node(AstNode::ExportAll, parent_id, &node.span)
    }
    ModuleDecl::TsImportEquals(node) => {
      ctx.push_node(AstNode::TsImportEquals, parent_id, &node.span)
    }
    ModuleDecl::TsExportAssignment(node) => {
      ctx.push_node(AstNode::TsExportAssignment, parent_id, &node.span)
    }
    ModuleDecl::TsNamespaceExport(node) => {
      ctx.push_node(AstNode::TsNamespaceExport, parent_id, &node.span)
    }
  }
}

fn serialize_stmt(
  ctx: &mut SerializeCtx,
  stmt: &Stmt,
  parent_id: usize,
) -> usize {
  match stmt {
    Stmt::Block(node) => {
      let id = ctx.next_id();

      let children = node
        .stmts
        .iter()
        .map(|stmt| serialize_stmt(ctx, stmt, parent_id))
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::Block, parent_id, &node.span, 1);
      ctx.write_ids(AstProp::Body, children);

      id
    }
    Stmt::Empty(_) => 0,
    Stmt::Debugger(node) => {
      ctx.push_node(AstNode::Debugger, parent_id, &node.span)
    }
    Stmt::With(_) => todo!(),
    Stmt::Return(node) => {
      let id = ctx.next_id();

      let arg_id = node
        .arg
        .as_ref()
        .map_or(0, |arg| serialize_expr(ctx, arg, id));

      ctx.write_node(id, AstNode::Return, parent_id, &node.span, 1);
      ctx.write_prop(AstProp::Argument, arg_id);

      id
    }
    Stmt::Labeled(node) => {
      let id = ctx.next_id();

      let ident_id = serialize_ident(ctx, &node.label, id);
      let stmt_id = serialize_stmt(ctx, &node.body, id);

      ctx.write_node(id, AstNode::Labeled, parent_id, &node.span, 2);
      ctx.write_prop(AstProp::Label, ident_id);
      ctx.write_prop(AstProp::Body, stmt_id);

      id
    }
    Stmt::Break(node) => {
      let id = ctx.next_id();

      let arg_id = node
        .label
        .as_ref()
        .map_or(0, |label| serialize_ident(ctx, label, id));

      ctx.write_node(id, AstNode::Break, parent_id, &node.span, 1);
      ctx.write_prop(AstProp::Label, arg_id);

      id
    }
    Stmt::Continue(node) => {
      let id = ctx.next_id();

      let arg_id = node
        .label
        .as_ref()
        .map_or(0, |label| serialize_ident(ctx, label, id));

      ctx.write_node(id, AstNode::Continue, parent_id, &node.span, 1);
      ctx.write_prop(AstProp::Label, arg_id);

      id
    }
    Stmt::If(node) => {
      let id = ctx.next_id();

      let test_id = serialize_expr(ctx, node.test.as_ref(), id);
      let cons_id = serialize_stmt(ctx, node.cons.as_ref(), id);

      let alt_id = node
        .alt
        .as_ref()
        .map_or(0, |alt| serialize_stmt(ctx, alt, id));

      ctx.write_node(id, AstNode::IfStatement, parent_id, &node.span, 3);
      ctx.write_prop(AstProp::Test, test_id);
      ctx.write_prop(AstProp::Consequent, cons_id);
      ctx.write_prop(AstProp::Alternate, alt_id);

      id
    }
    Stmt::Switch(node) => {
      let id = ctx.next_id();

      let expr_id = serialize_expr(ctx, &node.discriminant, id);

      let case_ids = node
        .cases
        .iter()
        .map(|case| {
          let child_id = ctx.next_id();

          let test_id = case
            .test
            .as_ref()
            .map_or(0, |test| serialize_expr(ctx, test, child_id));

          let cons = case
            .cons
            .iter()
            .map(|cons| serialize_stmt(ctx, cons, child_id))
            .collect::<Vec<_>>();

          ctx.write_node(child_id, AstNode::SwitchCase, id, &case.span, 2);
          ctx.write_prop(AstProp::Test, test_id);
          ctx.write_ids(AstProp::Consequent, cons);

          child_id
        })
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::Switch, parent_id, &node.span, 2);
      ctx.write_prop(AstProp::Discriminant, expr_id);
      ctx.write_ids(AstProp::Cases, case_ids);

      id
    }
    Stmt::Throw(node) => {
      let id = ctx.next_id();

      let expr_id = serialize_expr(ctx, &node.arg, id);

      ctx.write_node(id, AstNode::Throw, parent_id, &node.span, 1);
      ctx.write_prop(AstProp::Argument, expr_id);

      id
    }
    Stmt::Try(node) => {
      let id = ctx.next_id();

      let block_id = serialize_stmt(ctx, &Stmt::Block(node.block.clone()), id);

      let catch_id = node.handler.as_ref().map_or(0, |catch| {
        let clause_id = ctx.next_id();

        let param_id = catch
          .param
          .as_ref()
          .map_or(0, |param| serialize_pat(ctx, param, clause_id));

        let body_id = serialize_stmt(ctx, &Stmt::Block(catch.body.clone()), id);

        ctx.write_node(clause_id, AstNode::CatchClause, id, &catch.span, 2);
        ctx.write_prop(AstProp::Param, param_id);
        ctx.write_prop(AstProp::Body, body_id);

        clause_id
      });

      let final_id = node.finalizer.as_ref().map_or(0, |finalizer| {
        serialize_stmt(ctx, &Stmt::Block(finalizer.clone()), id)
      });

      ctx.write_node(id, AstNode::TryStatement, parent_id, &node.span, 3);
      ctx.write_prop(AstProp::Block, block_id);
      ctx.write_prop(AstProp::Handler, catch_id);
      ctx.write_prop(AstProp::Finalizer, final_id);

      id
    }
    Stmt::While(node) => {
      let id = ctx.next_id();

      let test_id = serialize_expr(ctx, node.test.as_ref(), id);
      let stmt_id = serialize_stmt(ctx, node.body.as_ref(), id);

      ctx.write_node(id, AstNode::While, parent_id, &node.span, 2);
      ctx.write_prop(AstProp::Test, test_id);
      ctx.write_prop(AstProp::Body, stmt_id);

      id
    }
    Stmt::DoWhile(node) => {
      let id = ctx.next_id();

      let expr_id = serialize_expr(ctx, node.test.as_ref(), id);
      let stmt_id = serialize_stmt(ctx, node.body.as_ref(), id);

      ctx.write_node(id, AstNode::DoWhileStatement, parent_id, &node.span, 2);
      ctx.write_prop(AstProp::Test, expr_id);
      ctx.write_prop(AstProp::Body, stmt_id);

      id
    }
    Stmt::For(node) => {
      let id = ctx.next_id();

      let init_id = node.init.as_ref().map_or(0, |init| match init {
        VarDeclOrExpr::VarDecl(var_decl) => {
          serialize_stmt(ctx, &Stmt::Decl(Decl::Var(var_decl.clone())), id)
        }
        VarDeclOrExpr::Expr(expr) => serialize_expr(ctx, expr, id),
      });

      let test_id = node
        .test
        .as_ref()
        .map_or(0, |expr| serialize_expr(ctx, expr, id));
      let update_id = node
        .update
        .as_ref()
        .map_or(0, |expr| serialize_expr(ctx, expr, id));
      let body_id = serialize_stmt(ctx, node.body.as_ref(), id);

      ctx.write_node(id, AstNode::ForStatement, parent_id, &node.span, 4);
      ctx.write_prop(AstProp::Init, init_id);
      ctx.write_prop(AstProp::Test, test_id);
      ctx.write_prop(AstProp::Update, update_id);
      ctx.write_prop(AstProp::Body, body_id);

      id
    }
    Stmt::ForIn(node) => {
      let id = ctx.next_id();

      let left_id = serialize_for_head(ctx, &node.left, id);
      let right_id = serialize_expr(ctx, node.right.as_ref(), id);
      let body_id = serialize_stmt(ctx, node.body.as_ref(), id);

      ctx.write_node(id, AstNode::ForInStatement, parent_id, &node.span, 3);
      ctx.write_prop(AstProp::Left, left_id);
      ctx.write_prop(AstProp::Right, right_id);
      ctx.write_prop(AstProp::Block, body_id);

      id
    }
    Stmt::ForOf(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      flags.set(Flag::ForAwait);

      let left_id = serialize_for_head(ctx, &node.left, id);
      let right_id = serialize_expr(ctx, node.right.as_ref(), id);
      let body_id = serialize_stmt(ctx, node.body.as_ref(), id);

      ctx.write_node(id, AstNode::ForOfStatement, parent_id, &node.span, 4);
      ctx.write_flags(&flags);
      ctx.write_prop(AstProp::Left, left_id);
      ctx.write_prop(AstProp::Right, right_id);
      ctx.write_prop(AstProp::Body, body_id);

      id
    }
    Stmt::Decl(node) => serialize_decl(ctx, node, parent_id),
    Stmt::Expr(node) => {
      let id = ctx.next_id();

      let child_id = serialize_expr(ctx, node.expr.as_ref(), id);
      ctx.write_node(
        id,
        AstNode::ExpressionStatement,
        parent_id,
        &node.span,
        1,
      );
      ctx.write_prop(AstProp::Expression, child_id);

      id
    }
  }
}

fn serialize_expr(
  ctx: &mut SerializeCtx,
  expr: &Expr,
  parent_id: usize,
) -> usize {
  match expr {
    Expr::This(node) => ctx.push_node(AstNode::This, parent_id, &node.span),
    Expr::Array(node) => {
      let id = ctx.next_id();

      let elem_ids = node
        .elems
        .iter()
        .map(|item| {
          item
            .as_ref()
            .map_or(0, |item| serialize_expr_or_spread(ctx, item, id))
        })
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::Array, parent_id, &node.span, 1);
      ctx.write_ids(AstProp::Elements, elem_ids);

      id
    }
    Expr::Object(node) => {
      let id = ctx.next_id();

      let prop_ids = node
        .props
        .iter()
        .map(|prop| {
          match prop {
            PropOrSpread::Spread(spread_element) => serialize_spread(
              ctx,
              spread_element.expr.as_ref(),
              &spread_element.dot3_token,
              parent_id,
            ),
            PropOrSpread::Prop(prop) => {
              let mut flags = FlagValue::new();
              let prop_id = ctx.next_id();

              // FIXME: optional
              let (key_id, value_id) = match prop.as_ref() {
                Prop::Shorthand(ident) => {
                  flags.set(Flag::PropShorthand);

                  let child_id = serialize_ident(ctx, ident, prop_id);
                  (child_id, child_id)
                }
                Prop::KeyValue(key_value_prop) => {
                  if let PropName::Computed(_) = key_value_prop.key {
                    flags.set(Flag::PropComputed)
                  }

                  let key_id =
                    serialize_prop_name(ctx, &key_value_prop.key, prop_id);
                  let value_id =
                    serialize_expr(ctx, key_value_prop.value.as_ref(), prop_id);

                  (key_id, value_id)
                }
                Prop::Assign(assign_prop) => {
                  let child_id = ctx.next_id();

                  let key_id = serialize_ident(ctx, &assign_prop.key, prop_id);
                  let value_id =
                    serialize_expr(ctx, assign_prop.value.as_ref(), prop_id);

                  ctx.write_node(
                    child_id,
                    AstNode::AssignmentPattern,
                    prop_id,
                    &assign_prop.span,
                    2,
                  );
                  ctx.write_prop(AstProp::Left, key_id);
                  ctx.write_prop(AstProp::Right, value_id);

                  (key_id, value_id)
                }
                Prop::Getter(getter_prop) => {
                  flags.set(Flag::PropGetter);

                  let key_id =
                    serialize_prop_name(ctx, &getter_prop.key, prop_id);

                  let value_id = serialize_expr(
                    ctx,
                    &Expr::Fn(FnExpr {
                      ident: None,
                      function: Box::new(Function {
                        params: vec![],
                        decorators: vec![],
                        span: getter_prop.span,
                        ctxt: SyntaxContext::empty(),
                        body: getter_prop.body.clone(),
                        is_generator: false,
                        is_async: false,
                        type_params: None, // FIXME
                        return_type: None,
                      }),
                    }),
                    prop_id,
                  );

                  (key_id, value_id)
                }
                Prop::Setter(setter_prop) => {
                  flags.set(Flag::PropSetter);

                  let key_id =
                    serialize_prop_name(ctx, &setter_prop.key, prop_id);

                  let param = Param::from(*setter_prop.param.clone());

                  let value_id = serialize_expr(
                    ctx,
                    &Expr::Fn(FnExpr {
                      ident: None,
                      function: Box::new(Function {
                        params: vec![param],
                        decorators: vec![],
                        span: setter_prop.span,
                        ctxt: SyntaxContext::empty(),
                        body: setter_prop.body.clone(),
                        is_generator: false,
                        is_async: false,
                        type_params: None,
                        return_type: None,
                      }),
                    }),
                    prop_id,
                  );

                  (key_id, value_id)
                }
                Prop::Method(method_prop) => {
                  flags.set(Flag::PropMethod);

                  let key_id =
                    serialize_prop_name(ctx, &method_prop.key, prop_id);

                  let value_id = serialize_expr(
                    ctx,
                    &Expr::Fn(FnExpr {
                      ident: None,
                      function: method_prop.function.clone(),
                    }),
                    prop_id,
                  );

                  (key_id, value_id)
                }
              };

              ctx.write_node(prop_id, AstNode::Property, id, &prop.span(), 3);
              ctx.write_flags(&flags);
              ctx.write_prop(AstProp::Key, key_id);
              ctx.write_prop(AstProp::Value, value_id);

              prop_id
            }
          }
        })
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::Object, parent_id, &node.span, 1);
      ctx.write_ids(AstProp::Properties, prop_ids);

      id
    }
    Expr::Fn(node) => {
      let id = ctx.next_id();
      let fn_obj = node.function.as_ref();

      let mut flags = FlagValue::new();
      if fn_obj.is_async {
        flags.set(Flag::FnAsync)
      }
      if fn_obj.is_generator {
        flags.set(Flag::FnGenerator)
      }

      let ident_id = node
        .ident
        .as_ref()
        .map_or(0, |ident| serialize_ident(ctx, ident, id));

      let type_param_id =
        maybe_serialize_ts_type_param(ctx, &fn_obj.type_params, id);

      let param_ids = fn_obj
        .params
        .iter()
        .map(|param| serialize_pat(ctx, &param.pat, id))
        .collect::<Vec<_>>();

      let return_id = maybe_serialize_ts_type_ann(ctx, &fn_obj.return_type, id);
      let block_id = fn_obj.body.as_ref().map_or(0, |block| {
        serialize_stmt(ctx, &Stmt::Block(block.clone()), id)
      });

      ctx.write_node(
        id,
        AstNode::FunctionExpression,
        parent_id,
        &fn_obj.span,
        6,
      );
      ctx.write_flags(&flags);
      ctx.write_prop(AstProp::Id, ident_id);
      ctx.write_prop(AstProp::TypeParameters, type_param_id);
      ctx.write_ids(AstProp::Params, param_ids);
      ctx.write_prop(AstProp::ReturnType, return_id);
      ctx.write_prop(AstProp::Body, block_id);

      id
    }
    Expr::Unary(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      flags.set(match node.op {
        UnaryOp::Minus => Flag::UnaryMinus,
        UnaryOp::Plus => Flag::UnaryPlus,
        UnaryOp::Bang => Flag::UnaryBang,
        UnaryOp::Tilde => Flag::UnaryTilde,
        UnaryOp::TypeOf => Flag::UnaryTypeOf,
        UnaryOp::Void => Flag::UnaryVoid,
        UnaryOp::Delete => Flag::UnaryDelete,
      });

      let child_id = serialize_expr(ctx, &node.arg, id);

      ctx.write_node(id, AstNode::Unary, parent_id, &node.span, 2);
      ctx.write_flags(&flags);
      ctx.write_prop(AstProp::Argument, child_id);

      id
    }
    Expr::Update(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      if node.prefix {
        flags.set(Flag::UpdatePrefix);
      }
      flags.set(match node.op {
        UpdateOp::PlusPlus => Flag::UpdatePlusPlus,
        UpdateOp::MinusMinus => Flag::UpdateMinusMinus,
      });

      let child_id = serialize_expr(ctx, node.arg.as_ref(), id);

      ctx.write_node(id, AstNode::UpdateExpression, parent_id, &node.span, 2);
      ctx.write_flags(&flags);
      ctx.write_prop(AstProp::Argument, child_id);

      id
    }
    Expr::Bin(node) => {
      let (node_type, flag) = match node.op {
        BinaryOp::LogicalOr => (AstNode::LogicalExpression, Flag::LogicalOr),
        BinaryOp::LogicalAnd => (AstNode::LogicalExpression, Flag::LogicalAnd),
        BinaryOp::NullishCoalescing => {
          (AstNode::LogicalExpression, Flag::LogicalNullishCoalescin)
        }
        BinaryOp::EqEq => (AstNode::BinaryExpression, Flag::BinEqEq),
        BinaryOp::NotEq => (AstNode::BinaryExpression, Flag::BinNotEq),
        BinaryOp::EqEqEq => (AstNode::BinaryExpression, Flag::BinEqEqEq),
        BinaryOp::NotEqEq => (AstNode::BinaryExpression, Flag::BinNotEqEq),
        BinaryOp::Lt => (AstNode::BinaryExpression, Flag::BinLt),
        BinaryOp::LtEq => (AstNode::BinaryExpression, Flag::BinLtEq),
        BinaryOp::Gt => (AstNode::BinaryExpression, Flag::BinGt),
        BinaryOp::GtEq => (AstNode::BinaryExpression, Flag::BinGtEq),
        BinaryOp::LShift => (AstNode::BinaryExpression, Flag::BinLShift),
        BinaryOp::RShift => (AstNode::BinaryExpression, Flag::BinRShift),
        BinaryOp::ZeroFillRShift => {
          (AstNode::BinaryExpression, Flag::BinZeroFillRShift)
        }
        BinaryOp::Add => (AstNode::BinaryExpression, Flag::BinAdd),
        BinaryOp::Sub => (AstNode::BinaryExpression, Flag::BinSub),
        BinaryOp::Mul => (AstNode::BinaryExpression, Flag::BinMul),
        BinaryOp::Div => (AstNode::BinaryExpression, Flag::BinDiv),
        BinaryOp::Mod => (AstNode::BinaryExpression, Flag::BinMod),
        BinaryOp::BitOr => (AstNode::BinaryExpression, Flag::BinBitOr),
        BinaryOp::BitXor => (AstNode::BinaryExpression, Flag::BinBitXor),
        BinaryOp::BitAnd => (AstNode::BinaryExpression, Flag::BinBitAnd),
        BinaryOp::In => (AstNode::BinaryExpression, Flag::BinIn),
        BinaryOp::InstanceOf => {
          (AstNode::BinaryExpression, Flag::BinInstanceOf)
        }
        BinaryOp::Exp => (AstNode::BinaryExpression, Flag::BinExp),
      };

      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      flags.set(flag);

      let left_id = serialize_expr(ctx, node.left.as_ref(), id);
      let right_id = serialize_expr(ctx, node.right.as_ref(), id);

      ctx.write_node(id, node_type, parent_id, &node.span, 3);
      ctx.write_flags(&flags);
      ctx.write_prop(AstProp::Left, left_id);
      ctx.write_prop(AstProp::Right, right_id);

      id
    }
    Expr::Assign(node) => {
      let id = ctx.next_id();

      let left_id = match &node.left {
        AssignTarget::Simple(simple_assign_target) => {
          match simple_assign_target {
            SimpleAssignTarget::Ident(target) => {
              serialize_ident(ctx, &target.id, id)
            }
            SimpleAssignTarget::Member(target) => {
              serialize_expr(ctx, &Expr::Member(target.clone()), id)
            }
            SimpleAssignTarget::SuperProp(target) => todo!(),
            SimpleAssignTarget::Paren(paren_expr) => todo!(),
            SimpleAssignTarget::OptChain(target) => {
              serialize_expr(ctx, &Expr::OptChain(target.clone()), id)
            }
            SimpleAssignTarget::TsAs(ts_as_expr) => todo!(),
            SimpleAssignTarget::TsSatisfies(ts_satisfies_expr) => todo!(),
            SimpleAssignTarget::TsNonNull(ts_non_null_expr) => todo!(),
            SimpleAssignTarget::TsTypeAssertion(ts_type_assertion) => todo!(),
            SimpleAssignTarget::TsInstantiation(ts_instantiation) => todo!(),
            SimpleAssignTarget::Invalid(_) => unreachable!(),
          }
        }
        AssignTarget::Pat(target) => match target {
          AssignTargetPat::Array(array_pat) => {
            serialize_pat(ctx, &Pat::Array(array_pat.clone()), id)
          }
          AssignTargetPat::Object(object_pat) => {
            serialize_pat(ctx, &Pat::Object(object_pat.clone()), id)
          }
          AssignTargetPat::Invalid(_) => unreachable!(),
        },
      };

      let right_id = serialize_expr(ctx, node.right.as_ref(), id);

      ctx.write_node(id, AstNode::Assign, parent_id, &node.span, 3);
      ctx.write_u8(assign_op_to_flag(node.op));
      ctx.write_prop(AstProp::Left, left_id);
      ctx.write_prop(AstProp::Right, right_id);

      id
    }
    Expr::Member(node) => serialize_member_expr(ctx, node, parent_id, false),
    Expr::SuperProp(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      if let SuperProp::Computed(_) = node.prop {
        flags.set(Flag::MemberComputed)
      }

      let super_id = ctx.push_node(AstNode::Super, id, &node.obj.span);

      let child_id = match &node.prop {
        SuperProp::Ident(ident_name) => {
          serialize_ident_name(ctx, ident_name, id)
        }
        SuperProp::Computed(prop) => serialize_expr(ctx, &prop.expr, id),
      };

      ctx.write_node(id, AstNode::MemberExpression, parent_id, &node.span, 3);
      ctx.write_flags(&flags);
      ctx.write_prop(AstProp::Object, super_id);
      ctx.write_prop(AstProp::Property, child_id);

      id
    }
    Expr::Cond(node) => {
      let id = ctx.next_id();

      let test_id = serialize_expr(ctx, node.test.as_ref(), id);
      let cons_id = serialize_expr(ctx, node.cons.as_ref(), id);
      let alt_id = serialize_expr(ctx, node.alt.as_ref(), id);

      ctx.write_node(id, AstNode::Cond, parent_id, &node.span, 3);
      ctx.write_prop(AstProp::Test, test_id);
      ctx.write_prop(AstProp::Consequent, cons_id);
      ctx.write_prop(AstProp::Alternate, alt_id);

      id
    }
    Expr::Call(node) => {
      let id = ctx.next_id();

      let callee_id = match &node.callee {
        Callee::Super(super_node) => {
          ctx.push_node(AstNode::Super, id, &super_node.span)
        }
        Callee::Import(import) => todo!(),
        Callee::Expr(expr) => serialize_expr(ctx, expr, id),
      };

      let type_id = node.type_args.as_ref().map_or(0, |type_arg| {
        todo!() // FIXME
      });

      let arg_ids = node
        .args
        .iter()
        .map(|arg| serialize_expr_or_spread(ctx, arg, id))
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::CallExpression, parent_id, &node.span, 4);
      ctx.write_flags(&FlagValue::new());
      ctx.write_prop(AstProp::Callee, callee_id);
      ctx.write_prop(AstProp::TypeArguments, type_id);
      ctx.write_ids(AstProp::Arguments, arg_ids);

      id
    }
    Expr::New(node) => {
      let id = ctx.next_id();

      let callee_id = serialize_expr(ctx, node.callee.as_ref(), id);

      let arg_ids: Vec<usize> = node.args.as_ref().map_or(vec![], |args| {
        args
          .iter()
          .map(|arg| serialize_expr_or_spread(ctx, arg, id))
          .collect::<Vec<_>>()
      });

      // let type_arg_id = maybe_serialize_ts_type_param(ctx, &node.type_args, id);
      // FIXME
      let type_arg_id = 0;

      ctx.write_node(id, AstNode::New, parent_id, &node.span, 3);
      ctx.write_prop(AstProp::Callee, callee_id);
      ctx.write_prop(AstProp::TypeArguments, type_arg_id);
      ctx.write_ids(AstProp::Arguments, arg_ids);

      id
    }
    Expr::Seq(node) => {
      let id = ctx.next_id();

      let children = node
        .exprs
        .iter()
        .map(|expr| serialize_expr(ctx, expr, id))
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::SequenceExpression, parent_id, &node.span, 1);
      ctx.write_ids(AstProp::Expressions, children);

      id
    }
    Expr::Ident(node) => serialize_ident(ctx, node, parent_id),
    Expr::Lit(node) => serialize_lit(ctx, node, parent_id),
    Expr::Tpl(node) => {
      let id = ctx.next_id();

      let quasi_ids = node
        .quasis
        .iter()
        .map(|quasi| {
          let tpl_id = ctx.next_id();

          let mut flags = FlagValue::new();
          flags.set(Flag::TplTail);

          let raw_str_id = ctx.str_table.insert(quasi.raw.as_str());

          let cooked_str_id = quasi
            .cooked
            .as_ref()
            .map_or(0, |cooked| ctx.str_table.insert(cooked.as_str()));

          ctx.write_node(tpl_id, AstNode::TemplateElement, id, &quasi.span, 3);
          ctx.write_flags(&flags);
          ctx.write_prop(AstProp::Raw, raw_str_id);
          ctx.write_prop(AstProp::Cooked, cooked_str_id);

          tpl_id
        })
        .collect::<Vec<_>>();

      let expr_ids = node
        .exprs
        .iter()
        .map(|expr| serialize_expr(ctx, expr, id))
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::TemplateLiteral, parent_id, &node.span, 2);
      ctx.write_ids(AstProp::Quasis, quasi_ids);
      ctx.write_ids(AstProp::Expressions, expr_ids);

      id
    }
    Expr::TaggedTpl(node) => {
      let id = ctx.next_id();

      let tag_id = serialize_expr(ctx, &node.tag, id);

      // FIXME
      let type_param_id = 0;
      let quasi_id = serialize_expr(ctx, &Expr::Tpl(*node.tpl.clone()), id);

      ctx.write_node(
        id,
        AstNode::TaggedTemplateExpression,
        parent_id,
        &node.span,
        3,
      );
      ctx.write_prop(AstProp::Tag, tag_id);
      ctx.write_prop(AstProp::TypeArguments, type_param_id);
      ctx.write_prop(AstProp::Quasi, quasi_id);

      id
    }
    Expr::Arrow(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      if node.is_async {
        flags.set(Flag::FnAsync);
      }
      if node.is_generator {
        flags.set(Flag::FnGenerator);
      }

      let type_param_id =
        maybe_serialize_ts_type_param(ctx, &node.type_params, id);

      let param_ids = node
        .params
        .iter()
        .map(|param| serialize_pat(ctx, param, id))
        .collect::<Vec<_>>();

      let body_id = match node.body.as_ref() {
        BlockStmtOrExpr::BlockStmt(block_stmt) => {
          serialize_stmt(ctx, &Stmt::Block(block_stmt.clone()), id)
        }
        BlockStmtOrExpr::Expr(expr) => serialize_expr(ctx, expr.as_ref(), id),
      };

      let return_type_id =
        maybe_serialize_ts_type_ann(ctx, &node.return_type, id);

      ctx.write_node(
        id,
        AstNode::ArrowFunctionExpression,
        parent_id,
        &node.span,
        4,
      );
      ctx.write_flags(&flags);
      ctx.write_prop(AstProp::TypeParameters, type_param_id);
      ctx.write_ids(AstProp::Params, param_ids);
      ctx.write_prop(AstProp::Body, body_id);
      ctx.write_prop(AstProp::ReturnType, return_type_id);

      id
    }
    Expr::Class(node) => {
      let id = ctx.push_node(AstNode::ClassExpr, parent_id, &node.class.span);

      // FIXME

      id
    }
    Expr::Yield(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      if node.delegate {
        flags.set(Flag::YieldDelegate)
      }

      let arg_id = node
        .arg
        .as_ref()
        .map_or(0, |arg| serialize_expr(ctx, arg.as_ref(), id));

      ctx.write_node(id, AstNode::YieldExpression, parent_id, &node.span, 2);
      ctx.write_flags(&flags);
      ctx.write_prop(AstProp::Argument, arg_id);

      id
    }
    Expr::MetaProp(node) => {
      ctx.push_node(AstNode::MetaProp, parent_id, &node.span)
    }
    Expr::Await(node) => {
      let id = ctx.next_id();
      let arg_id = serialize_expr(ctx, node.arg.as_ref(), id);

      ctx.write_node(id, AstNode::AwaitExpression, parent_id, &node.span, 1);
      ctx.write_prop(AstProp::Argument, arg_id);

      id
    }
    Expr::Paren(node) => {
      // Paren nodes are treated as a syntax only thing in TSEStree
      // and are never materialized to actual AST nodes.
      serialize_expr(ctx, &node.expr, parent_id)
    }
    Expr::JSXMember(node) => serialize_jsx_member_expr(ctx, node, parent_id),
    Expr::JSXNamespacedName(node) => {
      serialize_jsx_namespaced_name(ctx, node, parent_id)
    }
    Expr::JSXEmpty(node) => serialize_jsx_empty_expr(ctx, node, parent_id),
    Expr::JSXElement(node) => serialize_jsx_element(ctx, node, parent_id),
    Expr::JSXFragment(node) => serialize_jsx_fragment(ctx, node, parent_id),
    Expr::TsTypeAssertion(node) => {
      let id = ctx.next_id();

      let expr_id = serialize_expr(ctx, &node.expr, parent_id);
      let type_ann_id = serialize_ts_type(ctx, &node.type_ann, id);

      ctx.write_node(id, AstNode::TSTypeAssertion, parent_id, &node.span, 2);
      ctx.write_prop(AstProp::Expression, expr_id);
      ctx.write_prop(AstProp::TypeAnnotation, type_ann_id);

      id
    }
    Expr::TsConstAssertion(node) => {
      let id = ctx.next_id();
      let expr_id = serialize_expr(ctx, node.expr.as_ref(), id);

      ctx.write_node(id, AstNode::TsConstAssertion, parent_id, &node.span, 1);
      // FIXME
      ctx.write_prop(AstProp::Argument, expr_id);

      id
    }
    Expr::TsNonNull(node) => {
      let id = ctx.next_id();
      let expr_id = serialize_expr(ctx, node.expr.as_ref(), id);

      ctx.write_node(
        id,
        AstNode::TSNonNullExpression,
        parent_id,
        &node.span,
        1,
      );
      ctx.write_prop(AstProp::Expression, expr_id);

      id
    }
    Expr::TsAs(node) => {
      let id = ctx.next_id();

      let expr_id = serialize_expr(ctx, node.expr.as_ref(), id);
      let type_ann_id = serialize_ts_type(ctx, node.type_ann.as_ref(), id);

      ctx.write_node(id, AstNode::TSAsExpression, parent_id, &node.span, 2);
      ctx.write_prop(AstProp::Expression, expr_id);
      ctx.write_prop(AstProp::TypeAnnotation, type_ann_id);

      id
    }
    Expr::TsInstantiation(node) => {
      let id = ctx.next_id();

      let expr_id = serialize_expr(ctx, node.expr.as_ref(), id);
      // FIXME
      // let expr_id = serialize_expr(ctx, ts_instantiation.type_args.as_ref(), id);
      ctx.write_node(id, AstNode::TsInstantiation, parent_id, &node.span, 1);
      ctx.write_id(expr_id);

      id
    }
    Expr::TsSatisfies(node) => {
      let id = ctx.next_id();

      let expr_id = serialize_expr(ctx, node.expr.as_ref(), id);
      let type_id = serialize_ts_type(ctx, node.type_ann.as_ref(), id);

      ctx.write_node(
        id,
        AstNode::TSSatisfiesExpression,
        parent_id,
        &node.span,
        2,
      );
      ctx.write_prop(AstProp::Expression, expr_id);
      ctx.write_prop(AstProp::TypeAnnotation, type_id);

      id
    }
    Expr::PrivateName(node) => serialize_private_name(ctx, node, parent_id),
    Expr::OptChain(node) => {
      let id = ctx.next_id();

      let arg_id = match node.base.as_ref() {
        OptChainBase::Member(member_expr) => {
          serialize_member_expr(ctx, member_expr, id, true)
        }
        OptChainBase::Call(opt_call) => {
          let call_id = ctx.next_id();

          let mut flags = FlagValue::new();
          flags.set(Flag::FnOptional);

          let callee_id = serialize_expr(ctx, &opt_call.callee, id);
          let type_id = opt_call.type_args.as_ref().map_or(0, |type_arg| {
            todo!() // FIXME
          });

          let arg_ids = opt_call
            .args
            .iter()
            .map(|arg| serialize_expr_or_spread(ctx, arg, id))
            .collect::<Vec<_>>();

          ctx.write_node(
            call_id,
            AstNode::CallExpression,
            id,
            &opt_call.span,
            4,
          );
          ctx.write_flags(&flags);
          ctx.write_prop(AstProp::Callee, callee_id);
          ctx.write_prop(AstProp::TypeArguments, type_id);
          ctx.write_ids(AstProp::Arguments, arg_ids);

          call_id
        }
      };

      ctx.write_node(id, AstNode::ChainExpression, parent_id, &node.span, 1);
      ctx.write_prop(AstProp::Expression, arg_id);

      id
    }
    Expr::Invalid(_) => {
      unreachable!()
    }
  }
}

fn serialize_member_expr(
  ctx: &mut SerializeCtx,
  node: &MemberExpr,
  parent_id: usize,
  optional: bool,
) -> usize {
  let id = ctx.next_id();

  let mut flags = FlagValue::new();
  if optional {
    flags.set(Flag::MemberOptional)
  }

  let obj_id = serialize_expr(ctx, node.obj.as_ref(), id);

  let prop_id = match &node.prop {
    MemberProp::Ident(ident_name) => serialize_ident_name(ctx, ident_name, id),
    MemberProp::PrivateName(private_name) => {
      serialize_private_name(ctx, private_name, id)
    }
    MemberProp::Computed(computed_prop_name) => {
      flags.set(Flag::MemberComputed);
      serialize_expr(ctx, computed_prop_name.expr.as_ref(), id)
    }
  };

  ctx.write_node(id, AstNode::MemberExpression, parent_id, &node.span, 3);
  ctx.write_flags(&flags);
  ctx.write_prop(AstProp::Object, obj_id);
  ctx.write_prop(AstProp::Property, prop_id);

  id
}

fn serialize_expr_or_spread(
  ctx: &mut SerializeCtx,
  arg: &ExprOrSpread,
  parent_id: usize,
) -> usize {
  if let Some(spread) = &arg.spread {
    serialize_spread(ctx, &arg.expr, spread, parent_id)
  } else {
    serialize_expr(ctx, arg.expr.as_ref(), parent_id)
  }
}

fn serialize_ident(
  ctx: &mut SerializeCtx,
  ident: &Ident,
  parent_id: usize,
) -> usize {
  let str_id = ctx.str_table.insert(ident.sym.as_str());

  let id = ctx.next_id();
  ctx.write_node(id, AstNode::Identifier, parent_id, &ident.span, 1);
  ctx.write_prop(AstProp::Name, str_id);

  id
}

fn serialize_module_exported_name(
  ctx: &mut SerializeCtx,
  name: &ModuleExportName,
  parent_id: usize,
) -> usize {
  match &name {
    ModuleExportName::Ident(ident) => serialize_ident(ctx, ident, parent_id),
    ModuleExportName::Str(lit) => {
      serialize_lit(ctx, &Lit::Str(lit.clone()), parent_id)
    }
  }
}

fn serialize_decl(
  ctx: &mut SerializeCtx,
  decl: &Decl,
  parent_id: usize,
) -> usize {
  match decl {
    Decl::Class(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      if node.declare {
        flags.set(Flag::ClassDeclare)
      }
      if node.class.is_abstract {
        flags.set(Flag::ClassAbstract)
      }

      let ident_id = serialize_ident(ctx, &node.ident, id);
      let type_param_id =
        maybe_serialize_ts_type_param(ctx, &node.class.type_params, id);

      let super_class_id = node
        .class
        .super_class
        .as_ref()
        .map_or(0, |super_class| serialize_expr(ctx, super_class, id));

      let super_type_params =
        node
          .class
          .super_type_params
          .as_ref()
          .map_or(0, |super_params| {
            // FIXME
            todo!()
          });

      let implement_ids = node
        .class
        .implements
        .iter()
        .map(|implements| {
          // FIXME
          todo!()
        })
        .collect::<Vec<_>>();

      let member_ids = node
        .class
        .body
        .iter()
        .map(|member| {
          match member {
            ClassMember::Constructor(constructor) => {
              let member_id = ctx.next_id();

              let mut flags = FlagValue::new();
              flags.set(Flag::ClassConstructor);
              accessibility_to_flag(&mut flags, constructor.accessibility);

              let key_id =
                serialize_prop_name(ctx, &constructor.key, member_id);
              let body_id = constructor.body.as_ref().map_or(0, |body| {
                serialize_stmt(ctx, &Stmt::Block(body.clone()), member_id)
              });

              let params = constructor
                .params
                .iter()
                .map(|param| match param {
                  ParamOrTsParamProp::TsParamProp(ts_param_prop) => todo!(),
                  ParamOrTsParamProp::Param(param) => {
                    serialize_pat(ctx, &param.pat, member_id)
                  }
                })
                .collect::<Vec<_>>();

              ctx.write_node(
                member_id,
                AstNode::MethodDefinition,
                id,
                &constructor.span,
                4,
              );
              ctx.write_flags(&flags);
              ctx.write_id(key_id);
              ctx.write_id(body_id);
              // FIXME
              ctx.write_ids(AstProp::Arguments, params);

              member_id
            }
            ClassMember::Method(method) => {
              let member_id = ctx.next_id();

              let mut flags = FlagValue::new();
              flags.set(Flag::ClassMethod);
              if method.function.is_async {
                // FIXME
              }

              accessibility_to_flag(&mut flags, method.accessibility);

              let key_id = serialize_prop_name(ctx, &method.key, member_id);

              let body_id = method.function.body.as_ref().map_or(0, |body| {
                serialize_stmt(ctx, &Stmt::Block(body.clone()), member_id)
              });

              let params = method
                .function
                .params
                .iter()
                .map(|param| serialize_pat(ctx, &param.pat, member_id))
                .collect::<Vec<_>>();

              ctx.write_node(
                member_id,
                AstNode::MethodDefinition,
                id,
                &method.span,
                4,
              );
              ctx.write_flags(&flags);
              ctx.write_id(key_id);
              ctx.write_id(body_id);
              ctx.write_ids(AstProp::Params, params);

              member_id
            }
            ClassMember::PrivateMethod(private_method) => todo!(),
            ClassMember::ClassProp(class_prop) => todo!(),
            ClassMember::PrivateProp(private_prop) => todo!(),
            ClassMember::TsIndexSignature(ts_index_signature) => todo!(),
            ClassMember::Empty(_) => unreachable!(),
            ClassMember::StaticBlock(static_block) => todo!(),
            ClassMember::AutoAccessor(auto_accessor) => todo!(),
          }
        })
        .collect::<Vec<_>>();

      ctx.write_node(
        id,
        AstNode::ClassDeclaration,
        parent_id,
        &node.class.span,
        7,
      );
      ctx.write_flags(&flags);
      // FIXME
      ctx.write_id(ident_id);
      // FIXME
      ctx.write_id(type_param_id);
      // FIXME
      ctx.write_id(super_class_id);
      // FIXME
      ctx.write_id(super_type_params);

      // FIXME
      ctx.write_ids(AstProp::Params, implement_ids);
      // FIXME
      ctx.write_ids(AstProp::Params, member_ids);

      id
    }
    Decl::Fn(node) => {
      let id = ctx.next_id();
      let mut flags = FlagValue::new();
      if node.declare {
        flags.set(Flag::FnDeclare)
      }
      if node.function.is_async {
        flags.set(Flag::FnAsync)
      }
      if node.function.is_generator {
        flags.set(Flag::FnGenerator)
      }

      let ident_id = serialize_ident(ctx, &node.ident, parent_id);
      let type_param_id =
        maybe_serialize_ts_type_param(ctx, &node.function.type_params, id);
      let return_type =
        maybe_serialize_ts_type_ann(ctx, &node.function.return_type, id);

      let body_id = node.function.body.as_ref().map_or(0, |body| {
        serialize_stmt(ctx, &Stmt::Block(body.clone()), id)
      });

      let params = node
        .function
        .params
        .iter()
        .map(|param| serialize_pat(ctx, &param.pat, id))
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::Fn, parent_id, &node.function.span, 6);
      ctx.write_flags(&flags);
      ctx.write_prop(AstProp::Id, ident_id);
      ctx.write_prop(AstProp::TypeParameters, type_param_id);
      ctx.write_prop(AstProp::ReturnType, return_type);
      ctx.write_prop(AstProp::Body, body_id);
      ctx.write_ids(AstProp::Params, params);

      id
    }
    Decl::Var(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      if node.declare {
        flags.set(Flag::VarDeclare)
      }
      flags.set(match node.kind {
        VarDeclKind::Var => Flag::VarVar,
        VarDeclKind::Let => Flag::VarLet,
        VarDeclKind::Const => Flag::VarConst,
      });

      let children = node
        .decls
        .iter()
        .map(|decl| {
          let child_id = ctx.next_id();

          // FIXME: Definite?

          let decl_id = serialize_pat(ctx, &decl.name, child_id);

          let init_id = decl
            .init
            .as_ref()
            .map_or(0, |init| serialize_expr(ctx, init.as_ref(), child_id));

          ctx.write_node(
            child_id,
            AstNode::VariableDeclarator,
            id,
            &decl.span,
            2,
          );
          ctx.write_prop(AstProp::Id, decl_id);
          ctx.write_prop(AstProp::Init, init_id);

          child_id
        })
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::Var, parent_id, &node.span, 2);
      ctx.write_flags(&flags);
      ctx.write_ids(AstProp::Declarations, children);

      id
    }
    Decl::Using(node) => {
      let id = ctx.push_node(AstNode::Using, parent_id, &node.span);

      for (i, decl) in node.decls.iter().enumerate() {
        // FIXME
      }

      id
    }
    Decl::TsInterface(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      if node.declare {
        flags.set(Flag::TsDeclare);
      }

      let ident_id = serialize_ident(ctx, &node.id, id);
      let type_param =
        maybe_serialize_ts_type_param(ctx, &node.type_params, id);

      let extend_ids = node
        .extends
        .iter()
        .map(|item| {
          let child_id = ctx.next_id();
          let expr_id = serialize_expr(ctx, &item.expr, child_id);

          ctx.write_node(
            child_id,
            AstNode::TSInterfaceHeritage,
            id,
            &item.span,
            1,
          );
          // FIXME
          ctx.write_id(expr_id);

          child_id
        })
        .collect::<Vec<_>>();

      let body_elem_ids = node
        .body
        .body
        .iter()
        .map(|item| match item {
          TsTypeElement::TsCallSignatureDecl(ts_call) => {
            let item_id = ctx.next_id();

            let type_param_id =
              maybe_serialize_ts_type_param(ctx, &ts_call.type_params, id);
            let return_type_id =
              maybe_serialize_ts_type_ann(ctx, &ts_call.type_ann, id);
            let param_ids = ts_call
              .params
              .iter()
              .map(|param| serialize_ts_fn_param(ctx, param, id))
              .collect::<Vec<_>>();

            ctx.write_node(
              item_id,
              AstNode::TsCallSignatureDeclaration,
              id,
              &ts_call.span,
              3,
            );
            // FIXME
            ctx.write_prop(AstProp::TypeAnnotation, type_param_id);
            // FIXME
            ctx.write_ids(AstProp::Params, param_ids);
            ctx.write_prop(AstProp::ReturnType, return_type_id);

            item_id
          }
          TsTypeElement::TsConstructSignatureDecl(
            ts_construct_signature_decl,
          ) => todo!(),
          TsTypeElement::TsPropertySignature(ts_property_signature) => todo!(),
          TsTypeElement::TsGetterSignature(ts_getter_signature) => todo!(),
          TsTypeElement::TsSetterSignature(ts_setter_signature) => todo!(),
          TsTypeElement::TsMethodSignature(ts_method_signature) => todo!(),
          TsTypeElement::TsIndexSignature(ts_index_signature) => todo!(),
        })
        .collect::<Vec<_>>();

      let body_id = ctx.next_id();
      ctx.write_node(body_id, AstNode::TSInterfaceBody, id, &node.body.span, 4);

      // FIXME
      // ctx.write_ids( body_elem_ids);

      ctx.write_node(id, AstNode::TSInterface, parent_id, &node.span, 3);
      ctx.write_flags(&flags);
      ctx.write_id(ident_id);
      ctx.write_id(type_param);

      // FIXME
      // ctx.write_ids(extend_ids);

      id
    }
    Decl::TsTypeAlias(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      if node.declare {
        flags.set(Flag::TsDeclare);
      }

      let ident_id = serialize_ident(ctx, &node.id, id);
      let type_ann_id = serialize_ts_type(ctx, &node.type_ann, id);
      let type_param_id =
        maybe_serialize_ts_type_param(ctx, &node.type_params, id);

      ctx.write_node(id, AstNode::TsTypeAlias, parent_id, &node.span, 4);
      ctx.write_flags(&flags);
      ctx.write_prop(AstProp::Id, ident_id);
      ctx.write_prop(AstProp::TypeParameters, type_param_id);
      ctx.write_prop(AstProp::TypeAnnotation, type_ann_id);

      id
    }
    Decl::TsEnum(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      if node.declare {
        flags.set(Flag::TsDeclare);
      }
      if node.is_const {
        flags.set(Flag::TsConst);
      }

      let ident_id = serialize_ident(ctx, &node.id, parent_id);

      let member_ids = node
        .members
        .iter()
        .map(|member| {
          let member_id = ctx.next_id();

          let ident_id = match &member.id {
            TsEnumMemberId::Ident(ident) => {
              serialize_ident(ctx, &ident, member_id)
            }
            TsEnumMemberId::Str(lit_str) => {
              serialize_lit(ctx, &Lit::Str(lit_str.clone()), member_id)
            }
          };

          let init_id = member
            .init
            .as_ref()
            .map_or(0, |init| serialize_expr(ctx, init, member_id));

          ctx.write_node(member_id, AstNode::TSEnumMember, id, &member.span, 2);
          ctx.write_id(ident_id);
          ctx.write_id(init_id);

          member_id
        })
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::TSEnumDeclaration, parent_id, &node.span, 3);
      ctx.write_flags(&flags);
      ctx.write_id(ident_id);
      ctx.write_ids(AstProp::Members, member_ids);

      id
    }
    Decl::TsModule(ts_module_decl) => {
      ctx.push_node(AstNode::TsModule, parent_id, &ts_module_decl.span)
    }
  }
}

fn accessibility_to_flag(
  flags: &mut FlagValue,
  accessibility: Option<Accessibility>,
) {
  if let Some(accessibility) = &accessibility {
    let value = match accessibility {
      Accessibility::Public => Flag::ClassPublic,
      Accessibility::Protected => Flag::ClassProtected,
      Accessibility::Private => Flag::ClassPrivate,
    };

    flags.set(value);
  }
}

fn serialize_private_name(
  ctx: &mut SerializeCtx,
  node: &PrivateName,
  parent_id: usize,
) -> usize {
  let id = ctx.next_id();
  let str_id = ctx.str_table.insert(node.name.as_str());

  ctx.write_node(id, AstNode::PrivateIdentifier, parent_id, &node.span, 1);
  ctx.write_prop(AstProp::Name, str_id);

  id
}

fn serialize_jsx_element(
  ctx: &mut SerializeCtx,
  node: &JSXElement,
  parent_id: usize,
) -> usize {
  let id = ctx.next_id();

  let opening_id = serialize_jsx_opening_element(ctx, &node.opening, id);

  let closing_id = node.closing.as_ref().map_or(0, |closing| {
    let closing_id = ctx.next_id();

    let child_id = serialize_jsx_element_name(ctx, &closing.name, id);
    ctx.write_node(
      closing_id,
      AstNode::JSXClosingElement,
      id,
      &closing.span,
      1,
    );
    ctx.write_prop(AstProp::Name, child_id);

    closing_id
  });

  let children = serialize_jsx_children(ctx, &node.children, id);

  ctx.write_node(id, AstNode::JSXElement, parent_id, &node.span, 3);
  ctx.write_prop(AstProp::OpeningElement, opening_id);
  ctx.write_prop(AstProp::ClosingElement, closing_id);
  ctx.write_ids(AstProp::Children, children);

  id
}

fn serialize_jsx_fragment(
  ctx: &mut SerializeCtx,
  node: &JSXFragment,
  parent_id: usize,
) -> usize {
  let id = ctx.next_id();

  let opening_id =
    ctx.push_node(AstNode::JSXOpeningFragment, id, &node.opening.span);
  let closing_id =
    ctx.push_node(AstNode::JSXClosingFragment, id, &node.closing.span);

  let children = serialize_jsx_children(ctx, &node.children, id);

  ctx.write_node(id, AstNode::JSXFragment, parent_id, &node.span, 4);
  ctx.write_prop(AstProp::OpeningFragment, opening_id);
  ctx.write_prop(AstProp::ClosingFragment, closing_id);
  ctx.write_ids(AstProp::Children, children);

  id
}

fn serialize_jsx_children(
  ctx: &mut SerializeCtx,
  children: &[JSXElementChild],
  parent_id: usize,
) -> Vec<usize> {
  children
    .iter()
    .map(|child| {
      match child {
        JSXElementChild::JSXText(text) => {
          let id = ctx.next_id();

          let raw_id = ctx.str_table.insert(text.raw.as_str());
          let value_id = ctx.str_table.insert(text.value.as_str());

          ctx.write_node(id, AstNode::JSXText, parent_id, &text.span, 2);
          ctx.write_prop(AstProp::Raw, raw_id);
          ctx.write_prop(AstProp::Value, value_id);

          id
        }
        JSXElementChild::JSXExprContainer(container) => {
          serialize_jsx_container_expr(ctx, container, parent_id)
        }
        JSXElementChild::JSXElement(el) => {
          serialize_jsx_element(ctx, el, parent_id)
        }
        JSXElementChild::JSXFragment(frag) => {
          serialize_jsx_fragment(ctx, frag, parent_id)
        }
        // No parser supports this
        JSXElementChild::JSXSpreadChild(_) => unreachable!(),
      }
    })
    .collect::<Vec<_>>()
}

fn serialize_jsx_member_expr(
  ctx: &mut SerializeCtx,
  node: &JSXMemberExpr,
  parent_id: usize,
) -> usize {
  let id = ctx.next_id();

  let obj_id = match &node.obj {
    JSXObject::JSXMemberExpr(member) => {
      serialize_jsx_member_expr(ctx, member, id)
    }
    JSXObject::Ident(ident) => serialize_jsx_identifier(ctx, ident, parent_id),
  };

  let prop_id = serialize_ident_name_as_jsx_identifier(ctx, &node.prop, id);

  ctx.write_node(id, AstNode::JSXMemberExpression, parent_id, &node.span, 2);
  ctx.write_prop(AstProp::Object, obj_id);
  ctx.write_prop(AstProp::Property, prop_id);

  id
}

fn serialize_jsx_element_name(
  ctx: &mut SerializeCtx,
  node: &JSXElementName,
  parent_id: usize,
) -> usize {
  match &node {
    JSXElementName::Ident(ident) => {
      serialize_jsx_identifier(ctx, ident, parent_id)
    }
    JSXElementName::JSXMemberExpr(member) => {
      serialize_jsx_member_expr(ctx, member, parent_id)
    }
    JSXElementName::JSXNamespacedName(ns) => {
      serialize_jsx_namespaced_name(ctx, ns, parent_id)
    }
  }
}

fn serialize_jsx_opening_element(
  ctx: &mut SerializeCtx,
  node: &JSXOpeningElement,
  parent_id: usize,
) -> usize {
  let id = ctx.next_id();

  let mut flags = FlagValue::new();
  if node.self_closing {
    flags.set(Flag::JSXSelfClosing);
  }

  let name_id = serialize_jsx_element_name(ctx, &node.name, id);

  // FIXME: type args

  let attr_ids = node
    .attrs
    .iter()
    .map(|attr| match attr {
      JSXAttrOrSpread::JSXAttr(jsxattr) => {
        let attr_id = ctx.next_id();

        let name_id = match &jsxattr.name {
          JSXAttrName::Ident(name) => {
            serialize_ident_name_as_jsx_identifier(ctx, name, attr_id)
          }
          JSXAttrName::JSXNamespacedName(node) => {
            serialize_jsx_namespaced_name(ctx, node, attr_id)
          }
        };

        let value_id = jsxattr.value.as_ref().map_or(0, |value| match value {
          JSXAttrValue::Lit(lit) => serialize_lit(ctx, lit, attr_id),
          JSXAttrValue::JSXExprContainer(container) => {
            serialize_jsx_container_expr(ctx, container, attr_id)
          }
          JSXAttrValue::JSXElement(el) => {
            serialize_jsx_element(ctx, el, attr_id)
          }
          JSXAttrValue::JSXFragment(frag) => {
            serialize_jsx_fragment(ctx, frag, attr_id)
          }
        });

        ctx.write_node(attr_id, AstNode::JSXAttribute, id, &jsxattr.span, 2);
        ctx.write_prop(AstProp::Name, name_id);
        ctx.write_prop(AstProp::Value, value_id);

        attr_id
      }
      JSXAttrOrSpread::SpreadElement(spread) => {
        let attr_id = ctx.next_id();
        let child_id = serialize_expr(ctx, &spread.expr, attr_id);

        ctx.write_node(attr_id, AstNode::JSXAttribute, id, &spread.span(), 1);
        ctx.write_prop(AstProp::Argument, child_id);

        attr_id
      }
    })
    .collect::<Vec<_>>();

  ctx.write_node(id, AstNode::JSXOpeningElement, parent_id, &node.span, 3);
  ctx.write_flags(&flags);
  ctx.write_prop(AstProp::Name, name_id);
  ctx.write_ids(AstProp::Attributes, attr_ids);

  id
}

fn serialize_jsx_container_expr(
  ctx: &mut SerializeCtx,
  node: &JSXExprContainer,
  parent_id: usize,
) -> usize {
  let id = ctx.next_id();

  let child_id = match &node.expr {
    JSXExpr::JSXEmptyExpr(expr) => serialize_jsx_empty_expr(ctx, expr, id),
    JSXExpr::Expr(expr) => serialize_expr(ctx, expr, id),
  };

  ctx.write_node(
    id,
    AstNode::JSXExpressionContainer,
    parent_id,
    &node.span,
    1,
  );
  ctx.write_prop(AstProp::Expression, child_id);

  id
}

fn serialize_jsx_empty_expr(
  ctx: &mut SerializeCtx,
  node: &JSXEmptyExpr,
  parent_id: usize,
) -> usize {
  ctx.push_node(AstNode::JSXEmptyExpression, parent_id, &node.span)
}

fn serialize_jsx_namespaced_name(
  ctx: &mut SerializeCtx,
  node: &JSXNamespacedName,
  parent_id: usize,
) -> usize {
  let id = ctx.next_id();

  let ns_id = serialize_ident_name_as_jsx_identifier(ctx, &node.ns, id);
  let name_id = serialize_ident_name_as_jsx_identifier(ctx, &node.name, id);

  ctx.write_node(id, AstNode::JSXNamespacedName, parent_id, &node.span, 2);
  ctx.write_prop(AstProp::Namespace, ns_id);
  ctx.write_prop(AstProp::Name, name_id);

  id
}

fn serialize_ident_name_as_jsx_identifier(
  ctx: &mut SerializeCtx,
  node: &IdentName,
  parent_id: usize,
) -> usize {
  let id = ctx.next_id();

  let str_id = ctx.str_table.insert(node.sym.as_str());
  ctx.write_node(id, AstNode::JSXIdentifier, parent_id, &node.span, 1);
  ctx.write_prop(AstProp::Name, str_id);

  id
}

fn serialize_jsx_identifier(
  ctx: &mut SerializeCtx,
  node: &Ident,
  parent_id: usize,
) -> usize {
  let id = ctx.next_id();

  let str_id = ctx.str_table.insert(node.sym.as_str());
  ctx.write_node(id, AstNode::JSXIdentifier, parent_id, &node.span, 1);
  ctx.write_prop(AstProp::Name, str_id);

  id
}

fn serialize_pat(ctx: &mut SerializeCtx, pat: &Pat, parent_id: usize) -> usize {
  match pat {
    Pat::Ident(node) => serialize_ident(ctx, &node.id, parent_id),
    Pat::Array(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      if node.optional {
        flags.set(Flag::ParamOptional);
      }

      let type_ann_id = maybe_serialize_ts_type_ann(ctx, &node.type_ann, id);

      let children = node
        .elems
        .iter()
        .map(|pat| pat.as_ref().map_or(0, |v| serialize_pat(ctx, &v, id)))
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::ArrayPattern, parent_id, &node.span, 3);
      ctx.write_flags(&flags);
      ctx.write_prop(AstProp::TypeAnnotation, type_ann_id);
      ctx.write_ids(AstProp::Elements, children);

      id
    }
    Pat::Rest(node) => {
      let id = ctx.next_id();

      let type_ann_id = maybe_serialize_ts_type_ann(ctx, &node.type_ann, id);
      let arg_id = serialize_pat(ctx, &node.arg, parent_id);

      ctx.write_node(id, AstNode::RestElement, parent_id, &node.span, 2);
      ctx.write_prop(AstProp::TypeAnnotation, type_ann_id);
      ctx.write_prop(AstProp::Argument, arg_id);

      id
    }
    Pat::Object(node) => {
      let id = ctx.next_id();

      let mut flags = FlagValue::new();
      if node.optional {
        flags.set(Flag::ParamOptional);
      }

      // FIXME: Type Ann
      if let Some(type_ann) = &node.type_ann {}

      let children = node
        .props
        .iter()
        .map(|prop| match prop {
          ObjectPatProp::KeyValue(key_value_prop) => {
            let child_id = ctx.next_id();
            let mut flags = FlagValue::new();
            if let PropName::Computed(_) = key_value_prop.key {
              flags.set(Flag::PropComputed)
            }

            let key_id =
              serialize_prop_name(ctx, &key_value_prop.key, child_id);
            let value_id =
              serialize_pat(ctx, key_value_prop.value.as_ref(), child_id);

            ctx.write_node(
              child_id,
              AstNode::Property,
              id,
              &key_value_prop.span(),
              3,
            );
            ctx.write_flags(&flags);
            ctx.write_prop(AstProp::Key, key_id);
            ctx.write_prop(AstProp::Value, value_id);

            child_id
          }
          ObjectPatProp::Assign(assign_pat_prop) => {
            let child_id = ctx.next_id();

            let ident_id =
              serialize_ident(ctx, &assign_pat_prop.key.id, parent_id);

            let value_id = assign_pat_prop
              .value
              .as_ref()
              .map_or(0, |value| serialize_expr(ctx, value, child_id));

            ctx.write_node(
              child_id,
              AstNode::Property,
              id,
              &assign_pat_prop.span,
              4,
            );
            ctx.write_flags(&FlagValue::new());
            ctx.write_prop(AstProp::Key, ident_id);
            ctx.write_prop(AstProp::Value, value_id);

            child_id
          }
          ObjectPatProp::Rest(rest_pat) => {
            serialize_pat(ctx, &Pat::Rest(rest_pat.clone()), parent_id)
          }
        })
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::ObjectPattern, parent_id, &node.span, 2);
      ctx.write_flags(&flags);
      ctx.write_ids(AstProp::Properties, children);

      id
    }
    Pat::Assign(node) => {
      let id = ctx.next_id();

      let left_id = serialize_pat(ctx, &node.left, id);
      let right_id = serialize_expr(ctx, &node.right, id);

      ctx.write_node(id, AstNode::AssignmentPattern, parent_id, &node.span, 2);
      ctx.write_prop(AstProp::Left, left_id);
      ctx.write_prop(AstProp::Right, right_id);

      id
    }
    Pat::Invalid(_) => unreachable!(),
    Pat::Expr(node) => serialize_expr(ctx, node, parent_id),
  }
}

fn serialize_for_head(
  ctx: &mut SerializeCtx,
  for_head: &ForHead,
  parent_id: usize,
) -> usize {
  match for_head {
    ForHead::VarDecl(var_decl) => {
      serialize_decl(ctx, &Decl::Var(var_decl.clone()), parent_id)
    }
    ForHead::UsingDecl(using_decl) => {
      serialize_decl(ctx, &Decl::Using(using_decl.clone()), parent_id)
    }
    ForHead::Pat(pat) => serialize_pat(ctx, pat, parent_id),
  }
}

fn serialize_spread(
  ctx: &mut SerializeCtx,
  expr: &Expr,
  span: &Span,
  parent_id: usize,
) -> usize {
  let id = ctx.next_id();
  let expr_id = serialize_expr(ctx, expr, id);

  ctx.write_node(id, AstNode::Spread, parent_id, span, 1);
  ctx.write_prop(AstProp::Argument, expr_id);

  id
}

fn serialize_ident_name(
  ctx: &mut SerializeCtx,
  ident_name: &IdentName,
  parent_id: usize,
) -> usize {
  let id = ctx.push_node(AstNode::Identifier, parent_id, &ident_name.span);

  let str_id = ctx.str_table.insert(ident_name.sym.as_str());
  append_usize(&mut ctx.buf, str_id);

  id
}

fn serialize_prop_name(
  ctx: &mut SerializeCtx,
  prop_name: &PropName,
  parent_id: usize,
) -> usize {
  match prop_name {
    PropName::Ident(ident_name) => {
      serialize_ident_name(ctx, ident_name, parent_id)
    }
    PropName::Str(str_prop) => {
      let child_id =
        ctx.push_node(AstNode::StringLiteral, parent_id, &str_prop.span);

      let str_id = ctx.str_table.insert(str_prop.value.as_str());
      append_usize(&mut ctx.buf, str_id);

      child_id
    }
    PropName::Num(number) => {
      serialize_lit(ctx, &Lit::Num(number.clone()), parent_id)
    }
    PropName::Computed(node) => serialize_expr(ctx, &node.expr, parent_id),
    PropName::BigInt(big_int) => {
      serialize_lit(ctx, &Lit::BigInt(big_int.clone()), parent_id)
    }
  }
}

fn serialize_lit(ctx: &mut SerializeCtx, lit: &Lit, parent_id: usize) -> usize {
  match lit {
    Lit::Str(node) => {
      let id = ctx.next_id();

      let str_id = ctx.str_table.insert(node.value.as_str());

      ctx.write_node(id, AstNode::StringLiteral, parent_id, &node.span, 1);
      ctx.write_prop(AstProp::Value, str_id);

      id
    }
    Lit::Bool(lit_bool) => {
      let id = ctx.push_node(AstNode::Bool, parent_id, &lit_bool.span);

      let value: u8 = if lit_bool.value { 1 } else { 0 };
      ctx.buf.push(value);

      id
    }
    Lit::Null(node) => ctx.push_node(AstNode::Null, parent_id, &node.span),
    Lit::Num(node) => {
      let id = ctx.next_id();

      let value = node.raw.as_ref().unwrap();
      let str_id = ctx.str_table.insert(value.as_str());

      ctx.write_node(id, AstNode::NumericLiteral, parent_id, &node.span, 1);
      ctx.write_prop(AstProp::Value, str_id);

      id
    }
    Lit::BigInt(node) => {
      let id = ctx.push_node(AstNode::BigIntLiteral, parent_id, &node.span);

      let str_id = ctx.str_table.insert(&node.value.to_string());
      append_usize(&mut ctx.buf, str_id);

      id
    }
    Lit::Regex(node) => {
      let id = ctx.push_node(AstNode::RegExpLiteral, parent_id, &node.span);

      let pattern_id = ctx.str_table.insert(node.exp.as_str());
      let flag_id = ctx.str_table.insert(node.flags.as_str());

      append_usize(&mut ctx.buf, pattern_id);
      append_usize(&mut ctx.buf, flag_id);

      id
    }
    Lit::JSXText(jsxtext) => {
      ctx.push_node(AstNode::JSXText, parent_id, &jsxtext.span)
    }
  }
}

fn serialize_ts_type(
  ctx: &mut SerializeCtx,
  node: &TsType,
  parent_id: usize,
) -> usize {
  match node {
    TsType::TsKeywordType(node) => {
      let kind = match node.kind {
        TsKeywordTypeKind::TsAnyKeyword => AstNode::TSAnyKeyword,
        TsKeywordTypeKind::TsUnknownKeyword => AstNode::TSUnknownKeyword,
        TsKeywordTypeKind::TsNumberKeyword => AstNode::TSNumberKeyword,
        TsKeywordTypeKind::TsObjectKeyword => AstNode::TSObjectKeyword,
        TsKeywordTypeKind::TsBooleanKeyword => AstNode::TSBooleanKeyword,
        TsKeywordTypeKind::TsBigIntKeyword => AstNode::TSBigIntKeyword,
        TsKeywordTypeKind::TsStringKeyword => AstNode::TSStringKeyword,
        TsKeywordTypeKind::TsSymbolKeyword => AstNode::TSSymbolKeyword,
        TsKeywordTypeKind::TsVoidKeyword => AstNode::TSVoidKeyword,
        TsKeywordTypeKind::TsUndefinedKeyword => AstNode::TSUndefinedKeyword,
        TsKeywordTypeKind::TsNullKeyword => AstNode::TSNullKeyword,
        TsKeywordTypeKind::TsNeverKeyword => AstNode::TSNeverKeyword,
        TsKeywordTypeKind::TsIntrinsicKeyword => AstNode::TSIntrinsicKeyword,
      };

      ctx.push_node(kind, parent_id, &node.span)
    }
    TsType::TsThisType(node) => {
      ctx.push_node(AstNode::TSThisType, parent_id, &node.span)
    }
    TsType::TsFnOrConstructorType(node) => {
      match node {
        TsFnOrConstructorType::TsFnType(node) => {
          let id = ctx.next_id();

          let param_ids = node
            .params
            .iter()
            .map(|param| serialize_ts_fn_param(ctx, param, id))
            .collect::<Vec<_>>();

          ctx.write_node(id, AstNode::TSFunctionType, parent_id, &node.span, 1);
          // FIXME
          ctx.write_ids(AstProp::Params, param_ids);
          //
          id
        }
        TsFnOrConstructorType::TsConstructorType(ts_constructor_type) => {
          todo!()
        }
      }
    }
    TsType::TsTypeRef(node) => {
      let id = ctx.next_id();
      let name_id = serialize_ts_entity_name(ctx, &node.type_name, id);

      // FIXME params

      ctx.write_node(id, AstNode::TSTypeReference, parent_id, &node.span, 1);
      ctx.write_id(name_id);

      id
    }
    TsType::TsTypeQuery(node) => {
      let id = ctx.next_id();

      let name_id = match &node.expr_name {
        TsTypeQueryExpr::TsEntityName(entity) => {
          serialize_ts_entity_name(ctx, entity, id)
        }
        TsTypeQueryExpr::Import(ts_import_type) => todo!(),
      };

      // FIXME: params

      ctx.write_node(id, AstNode::TSTypeQuery, parent_id, &node.span, 1);
      ctx.write_id(name_id);

      id
    }
    TsType::TsTypeLit(ts_type_lit) => todo!(),
    TsType::TsArrayType(ts_array_type) => todo!(),
    TsType::TsTupleType(node) => {
      let id = ctx.next_id();
      let children = node
        .elem_types
        .iter()
        .map(|elem| todo!())
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::TSTupleType, parent_id, &node.span, 1);
      ctx.write_ids(AstProp::ElementTypes, children);

      id
    }
    TsType::TsOptionalType(ts_optional_type) => todo!(),
    TsType::TsRestType(ts_rest_type) => todo!(),
    TsType::TsUnionOrIntersectionType(node) => match node {
      TsUnionOrIntersectionType::TsUnionType(node) => {
        let id = ctx.next_id();

        let children = node
          .types
          .iter()
          .map(|item| serialize_ts_type(ctx, item, id))
          .collect::<Vec<_>>();

        ctx.write_node(id, AstNode::TSUnionType, parent_id, &node.span, 1);
        ctx.write_ids(AstProp::Types, children);

        id
      }
      TsUnionOrIntersectionType::TsIntersectionType(node) => {
        let id = ctx.next_id();

        let children = node
          .types
          .iter()
          .map(|item| serialize_ts_type(ctx, item, id))
          .collect::<Vec<_>>();

        ctx.write_node(
          id,
          AstNode::TSIntersectionType,
          parent_id,
          &node.span,
          1,
        );
        ctx.write_ids(AstProp::Types, children);

        id
      }
    },
    TsType::TsConditionalType(node) => {
      let id = ctx.next_id();
      let check_id = serialize_ts_type(ctx, &node.check_type, id);
      let extends_id = serialize_ts_type(ctx, &node.extends_type, id);
      let true_id = serialize_ts_type(ctx, &node.true_type, id);
      let false_id = serialize_ts_type(ctx, &node.false_type, id);

      ctx.write_node(id, AstNode::TSConditionalType, parent_id, &node.span, 4);
      ctx.write_id(check_id);
      ctx.write_id(extends_id);
      ctx.write_id(true_id);
      ctx.write_id(false_id);

      id
    }
    TsType::TsInferType(node) => {
      let id = ctx.next_id();
      let param_id = serialize_ts_type_param(ctx, &node.type_param, parent_id);

      ctx.write_node(id, AstNode::TSInferType, parent_id, &node.span, 1);
      ctx.write_id(param_id);

      id
    }
    TsType::TsParenthesizedType(ts_parenthesized_type) => todo!(),
    TsType::TsTypeOperator(ts_type_operator) => todo!(),
    TsType::TsIndexedAccessType(ts_indexed_access_type) => todo!(),
    TsType::TsMappedType(node) => {
      let id = ctx.next_id();

      let mut optional_flags = FlagValue::new();
      let mut readonly_flags = FlagValue::new();

      if let Some(optional) = node.optional {
        optional_flags.set(match optional {
          TruePlusMinus::True => Flag::TsTrue,
          TruePlusMinus::Plus => Flag::TsPlus,
          TruePlusMinus::Minus => Flag::TsMinus,
        });
      }
      if let Some(readonly) = node.readonly {
        readonly_flags.set(match readonly {
          TruePlusMinus::True => Flag::TsTrue,
          TruePlusMinus::Plus => Flag::TsPlus,
          TruePlusMinus::Minus => Flag::TsMinus,
        });
      }

      let name_id = maybe_serialize_ts_type(ctx, &node.name_type, id);
      let type_ann_id = maybe_serialize_ts_type(ctx, &node.type_ann, id);

      // FIXME

      ctx.write_node(id, AstNode::TSMappedType, parent_id, &node.span, 4);
      ctx.write_flags(&optional_flags);
      ctx.write_flags(&readonly_flags);
      ctx.write_id(name_id);
      ctx.write_id(type_ann_id);

      id
    }
    TsType::TsLitType(node) => {
      let id = ctx.next_id();

      let child_id = match &node.lit {
        TsLit::Number(lit) => serialize_lit(ctx, &Lit::Num(lit.clone()), id),
        TsLit::Str(lit) => serialize_lit(ctx, &Lit::Str(lit.clone()), id),
        TsLit::Bool(lit) => serialize_lit(ctx, &Lit::Bool(lit.clone()), id),
        TsLit::BigInt(lit) => serialize_lit(ctx, &Lit::BigInt(lit.clone()), id),
        TsLit::Tpl(lit) => serialize_expr(
          ctx,
          &Expr::Tpl(Tpl {
            span: lit.span,
            exprs: vec![],
            quasis: lit.quasis.clone(),
          }),
          id,
        ),
      };
      ctx.write_node(id, AstNode::TSLiteralType, parent_id, &node.span, 1);
      ctx.write_id(child_id);

      id
    }
    TsType::TsTypePredicate(ts_type_predicate) => todo!(),
    TsType::TsImportType(ts_import_type) => todo!(),
  }
}

fn serialize_ts_entity_name(
  ctx: &mut SerializeCtx,
  node: &TsEntityName,
  parent_id: usize,
) -> usize {
  match &node {
    TsEntityName::TsQualifiedName(ts_qualified_name) => todo!(),
    TsEntityName::Ident(ident) => serialize_ident(ctx, ident, parent_id),
  }
}

fn maybe_serialize_ts_type_ann(
  ctx: &mut SerializeCtx,
  node: &Option<Box<TsTypeAnn>>,
  parent_id: usize,
) -> usize {
  node.as_ref().map_or(0, |type_ann| {
    serialize_ts_type_ann(ctx, type_ann, parent_id)
  })
}

fn serialize_ts_type_ann(
  ctx: &mut SerializeCtx,
  node: &TsTypeAnn,
  parent_id: usize,
) -> usize {
  let id = ctx.next_id();

  let type_ann_id = serialize_ts_type(ctx, &node.type_ann, id);

  ctx.write_node(id, AstNode::TSTypeAnnotation, parent_id, &node.span, 1);
  ctx.write_prop(AstProp::TypeAnnotation, type_ann_id);

  id
}

fn maybe_serialize_ts_type(
  ctx: &mut SerializeCtx,
  node: &Option<Box<TsType>>,
  parent_id: usize,
) -> usize {
  node
    .as_ref()
    .map_or(0, |item| serialize_ts_type(ctx, item, parent_id))
}

fn serialize_ts_type_param(
  ctx: &mut SerializeCtx,
  node: &TsTypeParam,
  parent_id: usize,
) -> usize {
  let id = ctx.next_id();

  let mut flags = FlagValue::new();

  // FIXME: flags

  let name_id = serialize_ident(ctx, &node.name, id);
  let constraint_id = maybe_serialize_ts_type(ctx, &node.constraint, id);
  let default_id = maybe_serialize_ts_type(ctx, &node.default, id);

  ctx.write_node(id, AstNode::TSTypeParameter, parent_id, &node.span, 4);
  ctx.write_flags(&flags);
  ctx.write_prop(AstProp::Name, name_id);
  ctx.write_id(constraint_id);
  ctx.write_id(default_id);

  id
}

fn maybe_serialize_ts_type_param(
  ctx: &mut SerializeCtx,
  node: &Option<Box<TsTypeParamDecl>>,
  parent_id: usize,
) -> usize {
  node.as_ref().map_or(0, |node| {
    let id = ctx.next_id();

    let children = node
      .params
      .iter()
      .map(|param| serialize_ts_type_param(ctx, param, id))
      .collect::<Vec<_>>();

    ctx.write_node(
      id,
      AstNode::TSTypeParameterDeclaration,
      parent_id,
      &node.span,
      1,
    );
    ctx.write_ids(AstProp::Params, children);

    id
  })
}

fn serialize_ts_fn_param(
  ctx: &mut SerializeCtx,
  node: &TsFnParam,
  parent_id: usize,
) -> usize {
  match node {
    TsFnParam::Ident(ident) => serialize_ident(ctx, ident, parent_id),
    TsFnParam::Array(pat) => {
      serialize_pat(ctx, &Pat::Array(pat.clone()), parent_id)
    }
    TsFnParam::Rest(pat) => {
      serialize_pat(ctx, &Pat::Rest(pat.clone()), parent_id)
    }
    TsFnParam::Object(pat) => {
      serialize_pat(ctx, &Pat::Object(pat.clone()), parent_id)
    }
  }
}
