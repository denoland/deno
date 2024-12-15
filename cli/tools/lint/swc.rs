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
    common::{Span, Spanned, SyntaxContext},
  },
  view::{
    Accessibility, AssignOp, BinaryOp, TruePlusMinus, TsKeywordTypeKind,
    UnaryOp, UpdateOp, VarDeclKind,
  },
  ParsedSource,
};

use super::ast_buf::{
  append_usize, AstNode, AstProp, NodeRef, PropFlags, SerializeCtx,
};

pub fn serialize_ast_bin(parsed_source: &ParsedSource) -> Vec<u8> {
  let mut ctx = SerializeCtx::new();

  let program = &parsed_source.program();

  let pos = ctx.header(AstNode::Program, NodeRef(0), &program.span(), 2);
  let source_type_pos = ctx.str_field(AstProp::SourceType);

  // eprintln!("SWC {:#?}", program);

  match program.as_ref() {
    Program::Module(module) => {
      let body_pos = ctx.ref_vec_field(AstProp::Body, module.body.len());

      let children = module
        .body
        .iter()
        .map(|item| match item {
          ModuleItem::ModuleDecl(module_decl) => {
            serialize_module_decl(&mut ctx, module_decl, pos)
          }
          ModuleItem::Stmt(stmt) => serialize_stmt(&mut ctx, stmt, pos),
        })
        .collect::<Vec<_>>();

      ctx.write_str(source_type_pos, "module");
      ctx.write_refs(body_pos, children);
    }
    Program::Script(script) => {
      let body_pos = ctx.ref_vec_field(AstProp::Body, script.body.len());
      let children = script
        .body
        .iter()
        .map(|stmt| serialize_stmt(&mut ctx, stmt, pos))
        .collect::<Vec<_>>();

      ctx.write_str(source_type_pos, "script");
      ctx.write_refs(body_pos, children);
    }
  }

  ctx.serialize()
}

fn serialize_module_decl(
  ctx: &mut SerializeCtx,
  module_decl: &ModuleDecl,
  parent: NodeRef,
) -> NodeRef {
  match module_decl {
    ModuleDecl::Import(node) => {
      ctx.push_node(AstNode::Import, parent, &node.span)
    }
    ModuleDecl::ExportDecl(node) => {
      ctx.push_node(AstNode::ExportDecl, parent, &node.span)
    }
    ModuleDecl::ExportNamed(node) => {
      let id =
        ctx.header(AstNode::ExportNamedDeclaration, parent, &node.span, 2);
      let src_pos = ctx.ref_field(AstProp::Source);
      let spec_pos =
        ctx.ref_vec_field(AstProp::Specifiers, node.specifiers.len());

      // FIXME: Flags
      // let mut flags = FlagValue::new();
      // flags.set(Flag::ExportType);

      let src_id = node
        .src
        .as_ref()
        .map(|src| serialize_lit(ctx, &Lit::Str(*src.clone()), id));

      let spec_ids = node
        .specifiers
        .iter()
        .map(|spec| {
          match spec {
            ExportSpecifier::Named(child) => {
              let spec_pos =
                ctx.header(AstNode::ExportSpecifier, id, &child.span, 2);
              let local_pos = ctx.ref_field(AstProp::Local);
              let exp_pos = ctx.ref_field(AstProp::Exported);

              // let mut flags = FlagValue::new();
              // flags.set(Flag::ExportType);

              let local =
                serialize_module_exported_name(ctx, &child.orig, spec_pos);

              let exported = child.exported.as_ref().map(|exported| {
                serialize_module_exported_name(ctx, exported, spec_pos)
              });

              // ctx.write_flags(&flags);
              ctx.write_ref(local_pos, local);
              ctx.write_maybe_ref(exp_pos, exported);

              spec_pos
            }

            // These two aren't syntactically valid
            ExportSpecifier::Namespace(_) => todo!(),
            ExportSpecifier::Default(_) => todo!(),
          }
        })
        .collect::<Vec<_>>();

      // ctx.write_flags(&flags);
      ctx.write_maybe_ref(src_pos, src_id);
      ctx.write_refs(spec_pos, spec_ids);

      id
    }
    ModuleDecl::ExportDefaultDecl(node) => {
      ctx.push_node(AstNode::ExportDefaultDecl, parent, &node.span)
    }
    ModuleDecl::ExportDefaultExpr(node) => {
      ctx.push_node(AstNode::ExportDefaultExpr, parent, &node.span)
    }
    ModuleDecl::ExportAll(node) => {
      ctx.push_node(AstNode::ExportAll, parent, &node.span)
    }
    ModuleDecl::TsImportEquals(node) => {
      ctx.push_node(AstNode::TsImportEquals, parent, &node.span)
    }
    ModuleDecl::TsExportAssignment(node) => {
      ctx.push_node(AstNode::TsExportAssignment, parent, &node.span)
    }
    ModuleDecl::TsNamespaceExport(node) => {
      ctx.push_node(AstNode::TsNamespaceExport, parent, &node.span)
    }
  }
}

fn serialize_stmt(
  ctx: &mut SerializeCtx,
  stmt: &Stmt,
  parent: NodeRef,
) -> NodeRef {
  match stmt {
    Stmt::Block(node) => {
      let pos = ctx.header(AstNode::Block, parent, &node.span, 1);
      let body_pos = ctx.ref_vec_field(AstProp::Body, node.stmts.len());

      let children = node
        .stmts
        .iter()
        .map(|stmt| serialize_stmt(ctx, stmt, pos))
        .collect::<Vec<_>>();

      ctx.write_refs(body_pos, children);

      pos
    }
    Stmt::Empty(_) => NodeRef(0),
    Stmt::Debugger(node) => {
      ctx.push_node(AstNode::Debugger, parent, &node.span)
    }
    Stmt::With(_) => todo!(),
    Stmt::Return(node) => {
      let pos = ctx.header(AstNode::Return, parent, &node.span, 1);
      let arg_pos = ctx.ref_field(AstProp::Argument);

      let arg = node.arg.as_ref().map(|arg| serialize_expr(ctx, arg, pos));
      ctx.write_maybe_ref(arg_pos, arg);

      pos
    }
    Stmt::Labeled(node) => {
      let pos = ctx.header(AstNode::Labeled, parent, &node.span, 1);
      let label_pos = ctx.ref_field(AstProp::Label);
      let body_pos = ctx.ref_field(AstProp::Body);

      let ident = serialize_ident(ctx, &node.label, pos);
      let stmt = serialize_stmt(ctx, &node.body, pos);

      ctx.write_ref(label_pos, ident);
      ctx.write_ref(body_pos, stmt);

      pos
    }
    Stmt::Break(node) => {
      let pos = ctx.header(AstNode::Break, parent, &node.span, 1);
      let label_pos = ctx.ref_field(AstProp::Label);

      let arg = node
        .label
        .as_ref()
        .map(|label| serialize_ident(ctx, label, pos));

      ctx.write_maybe_ref(label_pos, arg);

      pos
    }
    Stmt::Continue(node) => {
      let pos = ctx.header(AstNode::Continue, parent, &node.span, 1);
      let label_pos = ctx.ref_field(AstProp::Label);

      let arg = node
        .label
        .as_ref()
        .map(|label| serialize_ident(ctx, label, pos));

      ctx.write_maybe_ref(label_pos, arg);

      pos
    }
    Stmt::If(node) => {
      let pos = ctx.header(AstNode::IfStatement, parent, &node.span, 3);
      let test_pos = ctx.ref_field(AstProp::Test);
      let cons_pos = ctx.ref_field(AstProp::Consequent);
      let alt_pos = ctx.ref_field(AstProp::Alternate);

      let test = serialize_expr(ctx, node.test.as_ref(), pos);
      let cons = serialize_stmt(ctx, node.cons.as_ref(), pos);
      let alt = node.alt.as_ref().map(|alt| serialize_stmt(ctx, alt, pos));

      ctx.write_ref(test_pos, test);
      ctx.write_ref(cons_pos, cons);
      ctx.write_maybe_ref(alt_pos, alt);

      pos
    }
    Stmt::Switch(node) => {
      let id = ctx.header(AstNode::Switch, parent, &node.span, 2);
      let disc_pos = ctx.ref_field(AstProp::Discriminant);
      let cases_pos = ctx.ref_vec_field(AstProp::Cases, node.cases.len());

      let disc = serialize_expr(ctx, &node.discriminant, id);

      let cases = node
        .cases
        .iter()
        .map(|case| {
          let case_pos = ctx.header(AstNode::SwitchCase, id, &case.span, 2);
          let test_pos = ctx.ref_field(AstProp::Test);
          let cons_pos =
            ctx.ref_vec_field(AstProp::Consequent, case.cons.len());

          let test = case
            .test
            .as_ref()
            .map(|test| serialize_expr(ctx, test, case_pos));

          let cons = case
            .cons
            .iter()
            .map(|cons| serialize_stmt(ctx, cons, case_pos))
            .collect::<Vec<_>>();

          ctx.write_maybe_ref(test_pos, test);
          ctx.write_refs(cons_pos, cons);

          case_pos
        })
        .collect::<Vec<_>>();

      ctx.write_ref(disc_pos, disc);
      ctx.write_refs(cases_pos, cases);

      id
    }
    Stmt::Throw(node) => {
      let pos = ctx.header(AstNode::Throw, parent, &node.span, 1);
      let arg_pos = ctx.ref_field(AstProp::Argument);

      let arg = serialize_expr(ctx, &node.arg, pos);
      ctx.write_ref(arg_pos, arg);

      pos
    }
    Stmt::Try(node) => {
      let pos = ctx.header(AstNode::TryStatement, parent, &node.span, 3);
      let block_pos = ctx.ref_field(AstProp::Block);
      let handler_pos = ctx.ref_field(AstProp::Handler);
      let finalizer_pos = ctx.ref_field(AstProp::Finalizer);

      let block = serialize_stmt(ctx, &Stmt::Block(node.block.clone()), pos);

      let handler = node.handler.as_ref().map(|catch| {
        let clause_pos = ctx.header(AstNode::CatchClause, pos, &catch.span, 2);
        let param_pos = ctx.ref_field(AstProp::Param);
        let body_pos = ctx.ref_field(AstProp::Body);

        let param = catch
          .param
          .as_ref()
          .map(|param| serialize_pat(ctx, param, clause_pos));

        let body =
          serialize_stmt(ctx, &Stmt::Block(catch.body.clone()), clause_pos);

        ctx.write_maybe_ref(param_pos, param);
        ctx.write_ref(body_pos, body);

        clause_pos
      });

      let finalizer = node.finalizer.as_ref().map(|finalizer| {
        serialize_stmt(ctx, &Stmt::Block(finalizer.clone()), pos)
      });

      ctx.write_ref(block_pos, block);
      ctx.write_maybe_ref(handler_pos, handler);
      ctx.write_maybe_ref(finalizer_pos, finalizer);

      pos
    }
    Stmt::While(node) => {
      let pos = ctx.header(AstNode::WhileStatement, parent, &node.span, 2);
      let test_pos = ctx.ref_field(AstProp::Test);
      let body_pos = ctx.ref_field(AstProp::Body);

      let test = serialize_expr(ctx, node.test.as_ref(), pos);
      let stmt = serialize_stmt(ctx, node.body.as_ref(), pos);

      ctx.write_ref(test_pos, test);
      ctx.write_ref(body_pos, stmt);

      pos
    }
    Stmt::DoWhile(node) => {
      let pos = ctx.header(AstNode::DoWhileStatement, parent, &node.span, 2);
      let test_pos = ctx.ref_field(AstProp::Test);
      let body_pos = ctx.ref_field(AstProp::Body);

      let expr = serialize_expr(ctx, node.test.as_ref(), pos);
      let stmt = serialize_stmt(ctx, node.body.as_ref(), pos);

      ctx.write_ref(test_pos, expr);
      ctx.write_ref(body_pos, stmt);

      pos
    }
    Stmt::For(node) => {
      let pos = ctx.header(AstNode::ForStatement, parent, &node.span, 4);
      let init_pos = ctx.ref_field(AstProp::Init);
      let test_pos = ctx.ref_field(AstProp::Test);
      let update_pos = ctx.ref_field(AstProp::Update);
      let body_pos = ctx.ref_field(AstProp::Body);

      let init = node.init.as_ref().map(|init| match init {
        VarDeclOrExpr::VarDecl(var_decl) => {
          serialize_stmt(ctx, &Stmt::Decl(Decl::Var(var_decl.clone())), pos)
        }
        VarDeclOrExpr::Expr(expr) => serialize_expr(ctx, expr, pos),
      });

      let test = node
        .test
        .as_ref()
        .map(|expr| serialize_expr(ctx, expr, pos));
      let update = node
        .update
        .as_ref()
        .map(|expr| serialize_expr(ctx, expr, pos));
      let body = serialize_stmt(ctx, node.body.as_ref(), pos);

      ctx.write_maybe_ref(init_pos, init);
      ctx.write_maybe_ref(test_pos, test);
      ctx.write_maybe_ref(update_pos, update);
      ctx.write_ref(body_pos, body);

      pos
    }
    Stmt::ForIn(node) => {
      let pos = ctx.header(AstNode::ForInStatement, parent, &node.span, 3);
      let left_pos = ctx.ref_field(AstProp::Left);
      let right_pos = ctx.ref_field(AstProp::Right);
      let body_pos = ctx.ref_field(AstProp::Body);

      let left = serialize_for_head(ctx, &node.left, pos);
      let right = serialize_expr(ctx, node.right.as_ref(), pos);
      let body = serialize_stmt(ctx, node.body.as_ref(), pos);

      ctx.write_ref(left_pos, left);
      ctx.write_ref(right_pos, right);
      ctx.write_ref(body_pos, body);

      pos
    }
    Stmt::ForOf(node) => {
      let pos = ctx.header(AstNode::ForOfStatement, parent, &node.span, 4);
      let await_pos = ctx.bool_field(AstProp::Await);
      let left_pos = ctx.ref_field(AstProp::Left);
      let right_pos = ctx.ref_field(AstProp::Right);
      let body_pos = ctx.ref_field(AstProp::Body);

      let left = serialize_for_head(ctx, &node.left, pos);
      let right = serialize_expr(ctx, node.right.as_ref(), pos);
      let body = serialize_stmt(ctx, node.body.as_ref(), pos);

      ctx.write_bool(await_pos, node.is_await);
      ctx.write_ref(left_pos, left);
      ctx.write_ref(right_pos, right);
      ctx.write_ref(body_pos, body);

      pos
    }
    Stmt::Decl(node) => serialize_decl(ctx, node, parent),
    Stmt::Expr(node) => {
      let pos = ctx.header(AstNode::ExpressionStatement, parent, &node.span, 1);
      let expr_pos = ctx.ref_field(AstProp::Expression);

      let expr = serialize_expr(ctx, node.expr.as_ref(), pos);
      ctx.write_ref(expr_pos, expr);

      pos
    }
  }
}

fn serialize_expr(
  ctx: &mut SerializeCtx,
  expr: &Expr,
  parent: NodeRef,
) -> NodeRef {
  match expr {
    Expr::This(node) => ctx.push_node(AstNode::This, parent, &node.span),
    Expr::Array(node) => {
      let pos = ctx.header(AstNode::ArrayExpression, parent, &node.span, 1);
      let elems_pos = ctx.ref_vec_field(AstProp::Elements, node.elems.len());

      let elems = node
        .elems
        .iter()
        .map(|item| {
          item
            .as_ref()
            .map_or(NodeRef(0), |item| serialize_expr_or_spread(ctx, item, pos))
        })
        .collect::<Vec<_>>();

      ctx.write_refs(elems_pos, elems);

      pos
    }
    Expr::Object(node) => {
      let id = ctx.header(AstNode::Object, parent, &node.span, 1);
      let props_pos = ctx.ref_vec_field(AstProp::Properties, node.props.len());

      let prop_ids = node
        .props
        .iter()
        .map(|prop| serialize_prop_or_spread(ctx, prop, id))
        .collect::<Vec<_>>();

      ctx.write_refs(props_pos, prop_ids);

      id
    }
    Expr::Fn(node) => {
      let fn_obj = node.function.as_ref();

      let pos =
        ctx.header(AstNode::FunctionExpression, parent, &fn_obj.span, 7);

      let async_pos = ctx.bool_field(AstProp::Async);
      let gen_pos = ctx.bool_field(AstProp::Generator);
      let id_pos = ctx.ref_field(AstProp::Id);
      let tparams_pos = ctx.ref_field(AstProp::TypeParameters);
      let params_pos = ctx.ref_vec_field(AstProp::Params, fn_obj.params.len());
      let return_pos = ctx.ref_field(AstProp::ReturnType);
      let body_pos = ctx.ref_field(AstProp::Body);

      let ident = node
        .ident
        .as_ref()
        .map(|ident| serialize_ident(ctx, ident, pos));

      let type_params =
        maybe_serialize_ts_type_param(ctx, &fn_obj.type_params, pos);

      let params = fn_obj
        .params
        .iter()
        .map(|param| serialize_pat(ctx, &param.pat, pos))
        .collect::<Vec<_>>();

      let return_id =
        maybe_serialize_ts_type_ann(ctx, &fn_obj.return_type, pos);
      let body = fn_obj
        .body
        .as_ref()
        .map(|block| serialize_stmt(ctx, &Stmt::Block(block.clone()), pos));

      ctx.write_bool(async_pos, fn_obj.is_async);
      ctx.write_bool(gen_pos, fn_obj.is_generator);
      ctx.write_maybe_ref(id_pos, ident);
      ctx.write_maybe_ref(tparams_pos, type_params);
      ctx.write_refs(params_pos, params);
      ctx.write_maybe_ref(return_pos, return_id);
      ctx.write_maybe_ref(body_pos, body);

      pos
    }
    Expr::Unary(node) => {
      let pos = ctx.header(AstNode::UnaryExpression, parent, &node.span, 2);
      let flag_pos = ctx.flag_field(AstProp::Operator, PropFlags::UnaryOp);
      let arg_pos = ctx.ref_field(AstProp::Argument);

      let flags: u8 = match node.op {
        UnaryOp::Minus => 0,
        UnaryOp::Plus => 1,
        UnaryOp::Bang => 2,
        UnaryOp::Tilde => 3,
        UnaryOp::TypeOf => 4,
        UnaryOp::Void => 5,
        UnaryOp::Delete => 6,
      };

      let arg = serialize_expr(ctx, &node.arg, pos);

      ctx.write_flags(flag_pos, flags);
      ctx.write_ref(arg_pos, arg);

      pos
    }
    Expr::Update(node) => {
      let pos = ctx.header(AstNode::UpdateExpression, parent, &node.span, 3);
      let prefix_pos = ctx.bool_field(AstProp::Prefix);
      let arg_pos = ctx.ref_field(AstProp::Argument);
      let op_ops = ctx.flag_field(AstProp::Operator, PropFlags::UpdateOp);

      let arg = serialize_expr(ctx, node.arg.as_ref(), pos);

      ctx.write_bool(prefix_pos, node.prefix);
      ctx.write_ref(arg_pos, arg);
      ctx.write_flags(
        op_ops,
        match node.op {
          UpdateOp::PlusPlus => 0,
          UpdateOp::MinusMinus => 1,
        },
      );

      pos
    }
    Expr::Bin(node) => {
      let (node_type, flag) = match node.op {
        BinaryOp::LogicalAnd => (AstNode::LogicalExpression, 0),
        BinaryOp::LogicalOr => (AstNode::LogicalExpression, 1),
        BinaryOp::NullishCoalescing => (AstNode::LogicalExpression, 2),
        BinaryOp::EqEq => (AstNode::BinaryExpression, 0),
        BinaryOp::NotEq => (AstNode::BinaryExpression, 1),
        BinaryOp::EqEqEq => (AstNode::BinaryExpression, 2),
        BinaryOp::NotEqEq => (AstNode::BinaryExpression, 3),
        BinaryOp::Lt => (AstNode::BinaryExpression, 4),
        BinaryOp::LtEq => (AstNode::BinaryExpression, 5),
        BinaryOp::Gt => (AstNode::BinaryExpression, 6),
        BinaryOp::GtEq => (AstNode::BinaryExpression, 7),
        BinaryOp::LShift => (AstNode::BinaryExpression, 8),
        BinaryOp::RShift => (AstNode::BinaryExpression, 9),
        BinaryOp::ZeroFillRShift => (AstNode::BinaryExpression, 10),
        BinaryOp::Add => (AstNode::BinaryExpression, 11),
        BinaryOp::Sub => (AstNode::BinaryExpression, 12),
        BinaryOp::Mul => (AstNode::BinaryExpression, 13),
        BinaryOp::Div => (AstNode::BinaryExpression, 14),
        BinaryOp::Mod => (AstNode::BinaryExpression, 15),
        BinaryOp::BitOr => (AstNode::BinaryExpression, 16),
        BinaryOp::BitXor => (AstNode::BinaryExpression, 17),
        BinaryOp::BitAnd => (AstNode::BinaryExpression, 18),
        BinaryOp::In => (AstNode::BinaryExpression, 19),
        BinaryOp::InstanceOf => (AstNode::BinaryExpression, 20),
        BinaryOp::Exp => (AstNode::BinaryExpression, 21),
      };

      let op_kind = if node_type == AstNode::BinaryExpression {
        PropFlags::BinOp
      } else {
        PropFlags::LogicalOp
      };

      let pos = ctx.header(node_type, parent, &node.span, 3);
      let flag_pos = ctx.flag_field(AstProp::Operator, op_kind);
      let left_pos = ctx.ref_field(AstProp::Left);
      let right_pos = ctx.ref_field(AstProp::Right);

      let left_id = serialize_expr(ctx, node.left.as_ref(), pos);
      let right_id = serialize_expr(ctx, node.right.as_ref(), pos);

      ctx.write_flags(flag_pos, flag);
      ctx.write_ref(left_pos, left_id);
      ctx.write_ref(right_pos, right_id);

      pos
    }
    Expr::Assign(node) => {
      let pos = ctx.header(AstNode::Assign, parent, &node.span, 3);
      let op_pos = ctx.flag_field(AstProp::Operator, PropFlags::AssignOp);
      let left_pos = ctx.ref_field(AstProp::Left);
      let right_pos = ctx.ref_field(AstProp::Right);

      let left = match &node.left {
        AssignTarget::Simple(simple_assign_target) => {
          match simple_assign_target {
            SimpleAssignTarget::Ident(target) => {
              serialize_ident(ctx, &target.id, pos)
            }
            SimpleAssignTarget::Member(target) => {
              serialize_expr(ctx, &Expr::Member(target.clone()), pos)
            }
            SimpleAssignTarget::SuperProp(target) => todo!(),
            SimpleAssignTarget::Paren(paren_expr) => todo!(),
            SimpleAssignTarget::OptChain(target) => {
              serialize_expr(ctx, &Expr::OptChain(target.clone()), pos)
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
            serialize_pat(ctx, &Pat::Array(array_pat.clone()), pos)
          }
          AssignTargetPat::Object(object_pat) => {
            serialize_pat(ctx, &Pat::Object(object_pat.clone()), pos)
          }
          AssignTargetPat::Invalid(_) => unreachable!(),
        },
      };

      let right = serialize_expr(ctx, node.right.as_ref(), pos);

      ctx.write_flags(
        op_pos,
        match node.op {
          AssignOp::Assign => 0,
          AssignOp::AddAssign => 1,
          AssignOp::SubAssign => 2,
          AssignOp::MulAssign => 3,
          AssignOp::DivAssign => 4,
          AssignOp::ModAssign => 5,
          AssignOp::LShiftAssign => 6,
          AssignOp::RShiftAssign => 7,
          AssignOp::ZeroFillRShiftAssign => 8,
          AssignOp::BitOrAssign => 9,
          AssignOp::BitXorAssign => 10,
          AssignOp::BitAndAssign => 11,
          AssignOp::ExpAssign => 12,
          AssignOp::AndAssign => 13,
          AssignOp::OrAssign => 14,
          AssignOp::NullishAssign => 15,
        },
      );
      ctx.write_ref(left_pos, left);
      ctx.write_ref(right_pos, right);

      pos
    }
    Expr::Member(node) => serialize_member_expr(ctx, node, parent, false),
    Expr::SuperProp(node) => {
      let pos = ctx.header(AstNode::MemberExpression, parent, &node.span, 3);
      let computed_pos = ctx.bool_field(AstProp::Computed);
      let obj_pos = ctx.ref_field(AstProp::Object);
      let prop_pos = ctx.ref_field(AstProp::Property);

      let obj = ctx.push_node(AstNode::Super, pos, &node.obj.span);

      let mut computed = false;
      let prop = match &node.prop {
        SuperProp::Ident(ident_name) => {
          serialize_ident_name(ctx, ident_name, pos)
        }
        SuperProp::Computed(prop) => {
          computed = true;
          serialize_expr(ctx, &prop.expr, pos)
        }
      };

      ctx.write_bool(computed_pos, computed);
      ctx.write_ref(obj_pos, obj);
      ctx.write_ref(prop_pos, prop);

      pos
    }
    Expr::Cond(node) => {
      let pos =
        ctx.header(AstNode::ConditionalExpression, parent, &node.span, 3);
      let test_pos = ctx.ref_field(AstProp::Test);
      let cons_pos = ctx.ref_field(AstProp::Consequent);
      let alt_pos = ctx.ref_field(AstProp::Alternate);

      let test = serialize_expr(ctx, node.test.as_ref(), pos);
      let cons = serialize_expr(ctx, node.cons.as_ref(), pos);
      let alt = serialize_expr(ctx, node.alt.as_ref(), pos);

      ctx.write_ref(test_pos, test);
      ctx.write_ref(cons_pos, cons);
      ctx.write_ref(alt_pos, alt);

      pos
    }
    Expr::Call(node) => {
      let pos = ctx.header(AstNode::CallExpression, parent, &node.span, 4);
      let opt_pos = ctx.bool_field(AstProp::Optional);
      let callee_pos = ctx.ref_field(AstProp::Callee);
      let type_args_pos = ctx.ref_field(AstProp::TypeArguments);
      let args_pos = ctx.ref_vec_field(AstProp::Arguments, node.args.len());

      let callee = match &node.callee {
        Callee::Super(super_node) => {
          ctx.push_node(AstNode::Super, pos, &super_node.span)
        }
        Callee::Import(import) => todo!(),
        Callee::Expr(expr) => serialize_expr(ctx, expr, pos),
      };

      let type_arg = node.type_args.as_ref().map(|type_arg| {
        todo!() // FIXME
      });

      let args = node
        .args
        .iter()
        .map(|arg| serialize_expr_or_spread(ctx, arg, pos))
        .collect::<Vec<_>>();

      ctx.write_bool(opt_pos, false);
      ctx.write_ref(callee_pos, callee);
      ctx.write_maybe_ref(type_args_pos, type_arg);
      ctx.write_refs(args_pos, args);

      pos
    }
    Expr::New(node) => {
      let pos = ctx.header(AstNode::New, parent, &node.span, 2);
      let callee_pos = ctx.ref_field(AstProp::Callee);
      let args_pos = ctx.ref_vec_field(
        AstProp::Arguments,
        node.args.as_ref().map_or(0, |v| v.len()),
      );

      let callee = serialize_expr(ctx, node.callee.as_ref(), pos);

      let args: Vec<NodeRef> = node.args.as_ref().map_or(vec![], |args| {
        args
          .iter()
          .map(|arg| serialize_expr_or_spread(ctx, arg, pos))
          .collect::<Vec<_>>()
      });

      // let type_arg_id = maybe_serialize_ts_type_param(ctx, &node.type_args, id);
      // FIXME
      let type_arg_id = 0;

      ctx.write_ref(callee_pos, callee);
      // ctx.write_refs(type_arg_pos, type_arg_id);
      // ctx.write_ref(type_args, args);
      ctx.write_refs(args_pos, args);

      pos
    }
    Expr::Seq(node) => {
      let pos = ctx.header(AstNode::SequenceExpression, parent, &node.span, 1);
      let exprs_pos = ctx.ref_vec_field(AstProp::Expressions, node.exprs.len());

      let children = node
        .exprs
        .iter()
        .map(|expr| serialize_expr(ctx, expr, pos))
        .collect::<Vec<_>>();

      ctx.write_refs(exprs_pos, children);

      pos
    }
    Expr::Ident(node) => serialize_ident(ctx, node, parent),
    Expr::Lit(node) => serialize_lit(ctx, node, parent),
    Expr::Tpl(node) => {
      let pos = ctx.header(AstNode::TemplateLiteral, parent, &node.span, 2);
      let quasis_pos = ctx.ref_vec_field(AstProp::Quasis, node.quasis.len());
      let exprs_pos = ctx.ref_vec_field(AstProp::Expressions, node.exprs.len());

      let quasis = node
        .quasis
        .iter()
        .map(|quasi| {
          let tpl_pos =
            ctx.header(AstNode::TemplateElement, pos, &quasi.span, 3);
          let tail_pos = ctx.bool_field(AstProp::Tail);
          let raw_pos = ctx.str_field(AstProp::Raw);
          let cooked_pos = ctx.str_field(AstProp::Cooked);

          ctx.write_bool(tail_pos, quasi.tail);
          ctx.write_str(raw_pos, &quasi.raw);
          ctx.write_str(
            cooked_pos,
            &quasi
              .cooked
              .as_ref()
              .map_or("".to_string(), |v| v.to_string()),
          );

          tpl_pos
        })
        .collect::<Vec<_>>();

      let exprs = node
        .exprs
        .iter()
        .map(|expr| serialize_expr(ctx, expr, pos))
        .collect::<Vec<_>>();

      ctx.write_refs(quasis_pos, quasis);
      ctx.write_refs(exprs_pos, exprs);

      pos
    }
    Expr::TaggedTpl(node) => {
      let id =
        ctx.header(AstNode::TaggedTemplateExpression, parent, &node.span, 3);
      let tag_pos = ctx.ref_field(AstProp::Tag);
      let type_arg_pos = ctx.ref_field(AstProp::TypeArguments);
      let quasi_pos = ctx.ref_field(AstProp::Quasi);

      let tag = serialize_expr(ctx, &node.tag, id);

      // FIXME
      let type_param_id = None;
      let quasi = serialize_expr(ctx, &Expr::Tpl(*node.tpl.clone()), id);

      ctx.write_ref(tag_pos, tag);
      ctx.write_maybe_ref(type_arg_pos, type_param_id);
      ctx.write_ref(quasi_pos, quasi);

      id
    }
    Expr::Arrow(node) => {
      let pos =
        ctx.header(AstNode::ArrowFunctionExpression, parent, &node.span, 6);
      let async_pos = ctx.bool_field(AstProp::Async);
      let gen_pos = ctx.bool_field(AstProp::Generator);
      let type_param_pos = ctx.ref_field(AstProp::TypeParameters);
      let params_pos = ctx.ref_vec_field(AstProp::Params, node.params.len());
      let body_pos = ctx.ref_field(AstProp::Body);
      let return_type_pos = ctx.ref_field(AstProp::ReturnType);

      let type_param =
        maybe_serialize_ts_type_param(ctx, &node.type_params, pos);

      let params = node
        .params
        .iter()
        .map(|param| serialize_pat(ctx, param, pos))
        .collect::<Vec<_>>();

      let body = match node.body.as_ref() {
        BlockStmtOrExpr::BlockStmt(block_stmt) => {
          serialize_stmt(ctx, &Stmt::Block(block_stmt.clone()), pos)
        }
        BlockStmtOrExpr::Expr(expr) => serialize_expr(ctx, expr.as_ref(), pos),
      };

      let return_type =
        maybe_serialize_ts_type_ann(ctx, &node.return_type, pos);

      ctx.write_bool(async_pos, node.is_async);
      ctx.write_bool(gen_pos, node.is_generator);
      ctx.write_maybe_ref(type_param_pos, type_param);
      ctx.write_refs(params_pos, params);
      ctx.write_ref(body_pos, body);
      ctx.write_maybe_ref(return_type_pos, return_type);

      pos
    }
    Expr::Class(node) => {
      let id = ctx.push_node(AstNode::ClassExpr, parent, &node.class.span);

      // FIXME

      id
    }
    Expr::Yield(node) => {
      let pos = ctx.header(AstNode::YieldExpression, parent, &node.span, 2);
      let delegate_pos = ctx.bool_field(AstProp::Delegate);
      let arg_pos = ctx.ref_field(AstProp::Argument);

      let arg = node
        .arg
        .as_ref()
        .map(|arg| serialize_expr(ctx, arg.as_ref(), pos));

      ctx.write_bool(delegate_pos, node.delegate);
      ctx.write_maybe_ref(arg_pos, arg);

      pos
    }
    Expr::MetaProp(node) => {
      ctx.push_node(AstNode::MetaProp, parent, &node.span)
    }
    Expr::Await(node) => {
      let pos = ctx.header(AstNode::AwaitExpression, parent, &node.span, 1);
      let arg_pos = ctx.ref_field(AstProp::Argument);

      let arg = serialize_expr(ctx, node.arg.as_ref(), pos);

      ctx.write_ref(arg_pos, arg);

      pos
    }
    Expr::Paren(node) => {
      // Paren nodes are treated as a syntax only thing in TSEStree
      // and are never materialized to actual AST nodes.
      serialize_expr(ctx, &node.expr, parent)
    }
    Expr::JSXMember(node) => serialize_jsx_member_expr(ctx, node, parent),
    Expr::JSXNamespacedName(node) => {
      serialize_jsx_namespaced_name(ctx, node, parent)
    }
    Expr::JSXEmpty(node) => serialize_jsx_empty_expr(ctx, node, parent),
    Expr::JSXElement(node) => serialize_jsx_element(ctx, node, parent),
    Expr::JSXFragment(node) => serialize_jsx_fragment(ctx, node, parent),
    Expr::TsTypeAssertion(node) => {
      let pos = ctx.header(AstNode::TSTypeAssertion, parent, &node.span, 2);
      let expr_pos = ctx.ref_field(AstProp::Expression);
      let type_ann_pos = ctx.ref_field(AstProp::TypeAnnotation);

      let expr = serialize_expr(ctx, &node.expr, parent);
      let type_ann = serialize_ts_type(ctx, &node.type_ann, pos);

      ctx.write_ref(expr_pos, expr);
      ctx.write_ref(type_ann_pos, type_ann);

      pos
    }
    Expr::TsConstAssertion(node) => {
      let pos = ctx.header(AstNode::TsConstAssertion, parent, &node.span, 1);
      let arg_pos = ctx.ref_field(AstProp::Argument);
      let arg = serialize_expr(ctx, node.expr.as_ref(), pos);

      // FIXME
      ctx.write_ref(arg_pos, arg);

      pos
    }
    Expr::TsNonNull(node) => {
      let pos = ctx.header(AstNode::TSNonNullExpression, parent, &node.span, 1);
      let expr_pos = ctx.ref_field(AstProp::Expression);

      let expr_id = serialize_expr(ctx, node.expr.as_ref(), pos);

      ctx.write_ref(expr_pos, expr_id);

      pos
    }
    Expr::TsAs(node) => {
      let id = ctx.header(AstNode::TSAsExpression, parent, &node.span, 2);
      let expr_pos = ctx.ref_field(AstProp::Expression);
      let type_ann_pos = ctx.ref_field(AstProp::TypeAnnotation);

      let expr = serialize_expr(ctx, node.expr.as_ref(), id);
      let type_ann = serialize_ts_type(ctx, node.type_ann.as_ref(), id);

      ctx.write_ref(expr_pos, expr);
      ctx.write_ref(type_ann_pos, type_ann);

      id
    }
    Expr::TsInstantiation(node) => {
      let pos = ctx.header(AstNode::TsInstantiation, parent, &node.span, 1);
      let expr_pos = ctx.ref_field(AstProp::Expression);
      let type_args_pos = ctx.ref_field(AstProp::TypeArguments);

      let expr = serialize_expr(ctx, node.expr.as_ref(), pos);
      // FIXME
      // let expr_id = serialize_expr(ctx, ts_instantiation.type_args.as_ref(), id);
      ctx.write_ref(expr_pos, expr);

      // FIXME
      ctx.write_maybe_ref(type_args_pos, None);

      pos
    }
    Expr::TsSatisfies(node) => {
      let pos =
        ctx.header(AstNode::TSSatisfiesExpression, parent, &node.span, 2);
      let expr_pos = ctx.ref_field(AstProp::Expression);
      let type_ann_pos = ctx.ref_field(AstProp::TypeAnnotation);

      let epxr = serialize_expr(ctx, node.expr.as_ref(), pos);
      let type_ann = serialize_ts_type(ctx, node.type_ann.as_ref(), pos);

      ctx.write_ref(expr_pos, epxr);
      ctx.write_ref(type_ann_pos, type_ann);

      pos
    }
    Expr::PrivateName(node) => serialize_private_name(ctx, node, parent),
    Expr::OptChain(node) => {
      let pos = ctx.header(AstNode::ChainExpression, parent, &node.span, 1);
      let arg_pos = ctx.ref_field(AstProp::Argument);

      let arg = match node.base.as_ref() {
        OptChainBase::Member(member_expr) => {
          serialize_member_expr(ctx, member_expr, pos, true)
        }
        OptChainBase::Call(opt_call) => {
          let call_pos =
            ctx.header(AstNode::CallExpression, pos, &opt_call.span, 4);
          let opt_pos = ctx.bool_field(AstProp::Optional);
          let callee_pos = ctx.ref_field(AstProp::Callee);
          let type_args_pos = ctx.ref_field(AstProp::TypeArguments);
          let args_pos =
            ctx.ref_vec_field(AstProp::Arguments, opt_call.args.len());

          let callee = serialize_expr(ctx, &opt_call.callee, pos);
          let type_arg = opt_call.type_args.as_ref().map(|type_arg| {
            todo!() // FIXME
          });

          let args = opt_call
            .args
            .iter()
            .map(|arg| serialize_expr_or_spread(ctx, arg, pos))
            .collect::<Vec<_>>();

          ctx.write_bool(opt_pos, true);
          ctx.write_ref(callee_pos, callee);
          ctx.write_maybe_ref(type_args_pos, type_arg);
          ctx.write_refs(args_pos, args);

          call_pos
        }
      };

      ctx.write_ref(arg_pos, arg);

      pos
    }
    Expr::Invalid(_) => {
      unreachable!()
    }
  }
}

fn serialize_prop_or_spread(
  ctx: &mut SerializeCtx,
  prop: &PropOrSpread,
  parent: NodeRef,
) -> NodeRef {
  match prop {
    PropOrSpread::Spread(spread_element) => serialize_spread(
      ctx,
      spread_element.expr.as_ref(),
      &spread_element.dot3_token,
      parent,
    ),
    PropOrSpread::Prop(prop) => {
      let pos = ctx.header(AstNode::Property, parent, &prop.span(), 6);

      let shorthand_pos = ctx.bool_field(AstProp::Shorthand);
      let computed_pos = ctx.bool_field(AstProp::Computed);
      let method_pos = ctx.bool_field(AstProp::Method);
      let kind_pos = ctx.str_field(AstProp::Kind);
      let key_pos = ctx.ref_field(AstProp::Key);
      let value_pos = ctx.ref_field(AstProp::Value);

      let mut shorthand = false;
      let mut computed = false;
      let mut method = false;
      let mut kind = "init";

      // FIXME: optional
      let (key_id, value_id) = match prop.as_ref() {
        Prop::Shorthand(ident) => {
          shorthand = true;

          let value = serialize_ident(ctx, ident, pos);
          (value, value)
        }
        Prop::KeyValue(key_value_prop) => {
          if let PropName::Computed(_) = key_value_prop.key {
            computed = true;
          }

          let key = serialize_prop_name(ctx, &key_value_prop.key, pos);
          let value = serialize_expr(ctx, key_value_prop.value.as_ref(), pos);

          (key, value)
        }
        Prop::Assign(assign_prop) => {
          let child_id =
            ctx.header(AstNode::AssignmentPattern, pos, &assign_prop.span, 2);
          let left_pos = ctx.ref_field(AstProp::Left);
          let right_pos = ctx.ref_field(AstProp::Right);

          let left = serialize_ident(ctx, &assign_prop.key, pos);
          let right = serialize_expr(ctx, assign_prop.value.as_ref(), pos);

          ctx.write_ref(left_pos, left);
          ctx.write_ref(right_pos, right);

          (left, right)
        }
        Prop::Getter(getter_prop) => {
          kind = "get";

          let key = serialize_prop_name(ctx, &getter_prop.key, pos);

          let value = serialize_expr(
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
            pos,
          );

          (key, value)
        }
        Prop::Setter(setter_prop) => {
          kind = "set";

          let key_id = serialize_prop_name(ctx, &setter_prop.key, pos);

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
            pos,
          );

          (key_id, value_id)
        }
        Prop::Method(method_prop) => {
          method = true;

          let key_id = serialize_prop_name(ctx, &method_prop.key, pos);

          let value_id = serialize_expr(
            ctx,
            &Expr::Fn(FnExpr {
              ident: None,
              function: method_prop.function.clone(),
            }),
            pos,
          );

          (key_id, value_id)
        }
      };

      ctx.write_bool(shorthand_pos, shorthand);
      ctx.write_bool(computed_pos, computed);
      ctx.write_bool(method_pos, method);
      ctx.write_str(kind_pos, kind);
      ctx.write_ref(key_pos, key_id);
      ctx.write_ref(value_pos, value_id);

      pos
    }
  }
}

fn serialize_member_expr(
  ctx: &mut SerializeCtx,
  node: &MemberExpr,
  parent: NodeRef,
  optional: bool,
) -> NodeRef {
  let pos = ctx.header(AstNode::MemberExpression, parent, &node.span, 4);
  let opt_pos = ctx.bool_field(AstProp::Optional);
  let computed_pos = ctx.bool_field(AstProp::Computed);
  let obj_pos = ctx.ref_field(AstProp::Object);
  let prop_pos = ctx.ref_field(AstProp::Property);

  let obj = serialize_expr(ctx, node.obj.as_ref(), pos);

  let mut computed = false;

  let prop = match &node.prop {
    MemberProp::Ident(ident_name) => serialize_ident_name(ctx, ident_name, pos),
    MemberProp::PrivateName(private_name) => {
      serialize_private_name(ctx, private_name, pos)
    }
    MemberProp::Computed(computed_prop_name) => {
      computed = true;
      serialize_expr(ctx, computed_prop_name.expr.as_ref(), pos)
    }
  };

  ctx.write_bool(opt_pos, optional);
  ctx.write_bool(computed_pos, computed);
  ctx.write_ref(obj_pos, obj);
  ctx.write_ref(prop_pos, prop);

  pos
}

fn serialize_class_member(
  ctx: &mut SerializeCtx,
  member: &ClassMember,
  parent: NodeRef,
) -> NodeRef {
  match member {
    ClassMember::Constructor(constructor) => {
      let member_id =
        ctx.header(AstNode::MethodDefinition, parent, &constructor.span, 3);
      let key_pos = ctx.ref_field(AstProp::Key);
      let body_pos = ctx.ref_field(AstProp::Body);
      let args_pos =
        ctx.ref_vec_field(AstProp::Arguments, constructor.params.len());
      let acc_pos =
        ctx.flag_field(AstProp::Accessibility, PropFlags::Accessibility);

      // FIXME flags
      // let mut flags = FlagValue::new();
      // flags.set(Flag::ClassConstructor);

      let key = serialize_prop_name(ctx, &constructor.key, member_id);
      let body = constructor
        .body
        .as_ref()
        .map(|body| serialize_stmt(ctx, &Stmt::Block(body.clone()), member_id));

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

      ctx
        .write_flags(acc_pos, accessibility_to_flag(constructor.accessibility));
      ctx.write_ref(key_pos, key);
      ctx.write_maybe_ref(body_pos, body);
      // FIXME
      ctx.write_refs(args_pos, params);

      member_id
    }
    ClassMember::Method(method) => {
      let member_id =
        ctx.header(AstNode::MethodDefinition, parent, &method.span, 0);

      // let mut flags = FlagValue::new();
      // flags.set(Flag::ClassMethod);
      if method.function.is_async {
        // FIXME
      }

      // accessibility_to_flag(&mut flags, method.accessibility);

      let key_id = serialize_prop_name(ctx, &method.key, member_id);

      let body_id =
        method.function.body.as_ref().map(|body| {
          serialize_stmt(ctx, &Stmt::Block(body.clone()), member_id)
        });

      let params = method
        .function
        .params
        .iter()
        .map(|param| serialize_pat(ctx, &param.pat, member_id))
        .collect::<Vec<_>>();

      // ctx.write_node(member_id, );
      // ctx.write_flags(&flags);
      // ctx.write_id(key_id);
      // ctx.write_id(body_id);
      // ctx.write_ids(AstProp::Params, params);

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
}

fn serialize_expr_or_spread(
  ctx: &mut SerializeCtx,
  arg: &ExprOrSpread,
  parent: NodeRef,
) -> NodeRef {
  if let Some(spread) = &arg.spread {
    serialize_spread(ctx, &arg.expr, spread, parent)
  } else {
    serialize_expr(ctx, arg.expr.as_ref(), parent)
  }
}

fn serialize_ident(
  ctx: &mut SerializeCtx,
  ident: &Ident,
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::Identifier, parent, &ident.span, 1);
  let name_pos = ctx.str_field(AstProp::Name);
  ctx.write_str(name_pos, ident.sym.as_str());

  pos
}

fn serialize_module_exported_name(
  ctx: &mut SerializeCtx,
  name: &ModuleExportName,
  parent: NodeRef,
) -> NodeRef {
  match &name {
    ModuleExportName::Ident(ident) => serialize_ident(ctx, ident, parent),
    ModuleExportName::Str(lit) => {
      serialize_lit(ctx, &Lit::Str(lit.clone()), parent)
    }
  }
}

fn serialize_decl(
  ctx: &mut SerializeCtx,
  decl: &Decl,
  parent: NodeRef,
) -> NodeRef {
  match decl {
    Decl::Class(node) => {
      let id =
        ctx.header(AstNode::ClassDeclaration, parent, &node.class.span, 2);
      let declare_pos = ctx.ref_field(AstProp::Declare);
      let abstract_pos = ctx.ref_field(AstProp::Abstract);
      let id_pos = ctx.ref_field(AstProp::Id);
      let type_params_pos = ctx.ref_field(AstProp::TypeParameters);
      let super_pos = ctx.ref_field(AstProp::SuperClass);
      let super_type_pos = ctx.ref_field(AstProp::SuperTypeArguments);
      let impl_pos =
        ctx.ref_vec_field(AstProp::Implements, node.class.implements.len());

      // FIXME class body

      let ident = serialize_ident(ctx, &node.ident, id);
      let type_params =
        maybe_serialize_ts_type_param(ctx, &node.class.type_params, id);

      let super_class = node
        .class
        .super_class
        .as_ref()
        .map(|super_class| serialize_expr(ctx, super_class, id));

      let super_type_params =
        node.class.super_type_params.as_ref().map(|super_params| {
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
        .map(|member| serialize_class_member(ctx, member, parent))
        .collect::<Vec<_>>();

      ctx.write_bool(declare_pos, node.declare);
      ctx.write_bool(abstract_pos, node.class.is_abstract);
      ctx.write_ref(id_pos, ident);
      ctx.write_maybe_ref(type_params_pos, type_params);
      ctx.write_maybe_ref(super_pos, super_class);
      ctx.write_maybe_ref(super_type_pos, super_type_params);
      ctx.write_refs(impl_pos, implement_ids);

      id
    }
    Decl::Fn(node) => {
      let pos = ctx.header(AstNode::Fn, parent, &node.function.span, 8);
      let declare_pos = ctx.ref_field(AstProp::Declare);
      let async_pos = ctx.ref_field(AstProp::Async);
      let gen_pos = ctx.ref_field(AstProp::Generator);
      let id_pos = ctx.ref_field(AstProp::Id);
      let type_params_pos = ctx.ref_field(AstProp::TypeParameters);
      let return_pos = ctx.ref_field(AstProp::ReturnType);
      let body_pos = ctx.ref_field(AstProp::Body);
      let params_pos =
        ctx.ref_vec_field(AstProp::Params, node.function.params.len());

      let ident_id = serialize_ident(ctx, &node.ident, parent);
      let type_param_id =
        maybe_serialize_ts_type_param(ctx, &node.function.type_params, pos);
      let return_type =
        maybe_serialize_ts_type_ann(ctx, &node.function.return_type, pos);

      let body = node
        .function
        .body
        .as_ref()
        .map(|body| serialize_stmt(ctx, &Stmt::Block(body.clone()), pos));

      let params = node
        .function
        .params
        .iter()
        .map(|param| serialize_pat(ctx, &param.pat, pos))
        .collect::<Vec<_>>();

      ctx.write_bool(declare_pos, node.declare);
      ctx.write_bool(async_pos, node.function.is_async);
      ctx.write_bool(gen_pos, node.function.is_generator);
      ctx.write_ref(id_pos, ident_id);
      ctx.write_maybe_ref(type_params_pos, type_param_id);
      ctx.write_maybe_ref(return_pos, return_type);
      ctx.write_maybe_ref(body_pos, body);
      ctx.write_refs(params_pos, params);

      pos
    }
    Decl::Var(node) => {
      let id = ctx.header(AstNode::Var, parent, &node.span, 3);
      let declare_pos = ctx.bool_field(AstProp::Declare);
      let kind_pos = ctx.flag_field(AstProp::Kind, PropFlags::VarKind);
      let decls_pos =
        ctx.ref_vec_field(AstProp::Declarations, node.decls.len());

      let children = node
        .decls
        .iter()
        .map(|decl| {
          let child_id =
            ctx.header(AstNode::VariableDeclarator, id, &decl.span, 2);
          let id_pos = ctx.ref_field(AstProp::Id);
          let init_pos = ctx.ref_field(AstProp::Init);

          // FIXME: Definite?

          let ident = serialize_pat(ctx, &decl.name, child_id);

          let init = decl
            .init
            .as_ref()
            .map(|init| serialize_expr(ctx, init.as_ref(), child_id));

          ctx.write_ref(id_pos, ident);
          ctx.write_maybe_ref(init_pos, init);

          child_id
        })
        .collect::<Vec<_>>();

      ctx.write_bool(declare_pos, node.declare);
      ctx.write_flags(
        kind_pos,
        match node.kind {
          VarDeclKind::Var => 0,
          VarDeclKind::Let => 1,
          VarDeclKind::Const => 2,
        },
      );
      ctx.write_refs(decls_pos, children);

      id
    }
    Decl::Using(node) => {
      let id = ctx.push_node(AstNode::Using, parent, &node.span);

      for (i, decl) in node.decls.iter().enumerate() {
        // FIXME
      }

      id
    }
    Decl::TsInterface(node) => {
      let id = ctx.header(AstNode::TSInterface, parent, &node.span, 0);
      let declare_pos = ctx.bool_field(AstProp::Declare);

      let body_id =
        ctx.header(AstNode::TSInterfaceBody, id, &node.body.span, 0);
      // FIXME

      let ident_id = serialize_ident(ctx, &node.id, id);
      let type_param =
        maybe_serialize_ts_type_param(ctx, &node.type_params, id);

      let extend_ids = node
        .extends
        .iter()
        .map(|item| {
          let child_id =
            ctx.header(AstNode::TSInterfaceHeritage, id, &item.span, 1);
          let expr_pos = ctx.ref_field(AstProp::Expression);

          let expr = serialize_expr(ctx, &item.expr, child_id);

          // FIXME
          ctx.write_ref(expr_pos, expr);

          child_id
        })
        .collect::<Vec<_>>();

      let body_elem_ids = node
        .body
        .body
        .iter()
        .map(|item| match item {
          TsTypeElement::TsCallSignatureDecl(ts_call) => {
            let item_id = ctx.header(
              AstNode::TsCallSignatureDeclaration,
              id,
              &ts_call.span,
              3,
            );
            let type_ann_pos = ctx.ref_field(AstProp::TypeAnnotation);
            let params_pos =
              ctx.ref_vec_field(AstProp::Params, ts_call.params.len());
            let return_pos = ctx.ref_field(AstProp::ReturnType);

            let type_param =
              maybe_serialize_ts_type_param(ctx, &ts_call.type_params, id);
            let return_type =
              maybe_serialize_ts_type_ann(ctx, &ts_call.type_ann, id);
            let params = ts_call
              .params
              .iter()
              .map(|param| serialize_ts_fn_param(ctx, param, id))
              .collect::<Vec<_>>();

            // FIXME
            ctx.write_maybe_ref(type_ann_pos, type_param);
            // FIXME
            ctx.write_refs(params_pos, params);
            ctx.write_maybe_ref(return_pos, return_type);

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

      // FIXME
      // ctx.write_ids( body_elem_ids);

      // ctx.write_bool(declare_pos, node.declare);
      // ctx.write_id(ident_id);
      // ctx.write_id(type_param);

      // FIXME
      // ctx.write_ids(extend_ids);

      id
    }
    Decl::TsTypeAlias(node) => {
      let pos = ctx.header(AstNode::TsTypeAlias, parent, &node.span, 4);
      let declare_pos = ctx.bool_field(AstProp::Declare);
      let id_pos = ctx.ref_field(AstProp::Id);
      let type_params_pos = ctx.ref_field(AstProp::TypeParameters);
      let type_ann_pos = ctx.ref_field(AstProp::TypeAnnotation);

      let ident = serialize_ident(ctx, &node.id, pos);
      let type_ann = serialize_ts_type(ctx, &node.type_ann, pos);
      let type_param =
        maybe_serialize_ts_type_param(ctx, &node.type_params, pos);

      ctx.write_bool(declare_pos, node.declare);
      ctx.write_ref(id_pos, ident);
      ctx.write_maybe_ref(type_params_pos, type_param);
      ctx.write_ref(type_ann_pos, type_ann);

      pos
    }
    Decl::TsEnum(node) => {
      let pos = ctx.header(AstNode::TSEnumDeclaration, parent, &node.span, 3);
      let declare_pos = ctx.bool_field(AstProp::Declare);
      let const_pos = ctx.bool_field(AstProp::Const);
      let id_pos = ctx.ref_field(AstProp::Id);
      let body_pos = ctx.ref_field(AstProp::Body);

      let body = ctx.header(AstNode::TSEnumBody, pos, &node.span, 1);
      let members_pos = ctx.ref_vec_field(AstProp::Members, node.members.len());

      let ident_id = serialize_ident(ctx, &node.id, parent);

      let members = node
        .members
        .iter()
        .map(|member| {
          let member_id =
            ctx.header(AstNode::TSEnumMember, body, &member.span, 2);
          let id_pos = ctx.ref_field(AstProp::Id);
          let init_pos = ctx.ref_field(AstProp::Initializer);

          let ident = match &member.id {
            TsEnumMemberId::Ident(ident) => {
              serialize_ident(ctx, &ident, member_id)
            }
            TsEnumMemberId::Str(lit_str) => {
              serialize_lit(ctx, &Lit::Str(lit_str.clone()), member_id)
            }
          };

          let init = member
            .init
            .as_ref()
            .map(|init| serialize_expr(ctx, init, member_id));

          ctx.write_ref(id_pos, ident);
          ctx.write_maybe_ref(init_pos, init);

          member_id
        })
        .collect::<Vec<_>>();

      ctx.write_refs(members_pos, members);

      ctx.write_bool(declare_pos, node.declare);
      ctx.write_bool(const_pos, node.is_const);
      ctx.write_ref(id_pos, ident_id);
      ctx.write_ref(body_pos, body);

      pos
    }
    Decl::TsModule(ts_module_decl) => {
      ctx.push_node(AstNode::TsModule, parent, &ts_module_decl.span)
    }
  }
}

fn accessibility_to_flag(accessibility: Option<Accessibility>) -> u8 {
  accessibility.map_or(0, |v| match v {
    Accessibility::Public => 1,
    Accessibility::Protected => 2,
    Accessibility::Private => 3,
  })
}

fn serialize_private_name(
  ctx: &mut SerializeCtx,
  node: &PrivateName,
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::PrivateIdentifier, parent, &node.span, 1);
  let name_pos = ctx.str_field(AstProp::Name);

  ctx.write_str(name_pos, node.name.as_str());

  pos
}

fn serialize_jsx_element(
  ctx: &mut SerializeCtx,
  node: &JSXElement,
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::JSXElement, parent, &node.span, 3);
  let open_pos = ctx.ref_field(AstProp::OpeningElement);
  let close_pos = ctx.ref_field(AstProp::ClosingElement);
  let children_pos = ctx.ref_vec_field(AstProp::Children, node.children.len());

  let open = serialize_jsx_opening_element(ctx, &node.opening, pos);

  let close = node.closing.as_ref().map(|closing| {
    let closing_pos =
      ctx.header(AstNode::JSXClosingElement, pos, &closing.span, 1);
    let name_pos = ctx.ref_field(AstProp::Name);

    let name = serialize_jsx_element_name(ctx, &closing.name, closing_pos);
    ctx.write_ref(name_pos, name);

    closing_pos
  });

  let children = serialize_jsx_children(ctx, &node.children, pos);

  ctx.write_ref(open_pos, open);
  ctx.write_maybe_ref(close_pos, close);
  ctx.write_refs(children_pos, children);

  pos
}

fn serialize_jsx_fragment(
  ctx: &mut SerializeCtx,
  node: &JSXFragment,
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::JSXFragment, parent, &node.span, 3);

  let opening_pos = ctx.ref_field(AstProp::OpeningFragment);
  let closing_pos = ctx.ref_field(AstProp::ClosingFragment);
  let children_pos = ctx.ref_vec_field(AstProp::Children, node.children.len());

  let opening_id =
    ctx.push_node(AstNode::JSXOpeningFragment, pos, &node.opening.span);
  let closing_id =
    ctx.push_node(AstNode::JSXClosingFragment, pos, &node.closing.span);

  let children = serialize_jsx_children(ctx, &node.children, pos);

  ctx.write_ref(opening_pos, opening_id);
  ctx.write_ref(closing_pos, closing_id);
  ctx.write_refs(children_pos, children);

  pos
}

fn serialize_jsx_children(
  ctx: &mut SerializeCtx,
  children: &[JSXElementChild],
  parent: NodeRef,
) -> Vec<NodeRef> {
  children
    .iter()
    .map(|child| {
      match child {
        JSXElementChild::JSXText(text) => {
          let pos = ctx.header(AstNode::JSXText, parent, &text.span, 2);
          let raw_pos = ctx.str_field(AstProp::Raw);
          let value_pos = ctx.str_field(AstProp::Value);

          ctx.write_str(raw_pos, &text.raw);
          ctx.write_str(value_pos, &text.value);

          pos
        }
        JSXElementChild::JSXExprContainer(container) => {
          serialize_jsx_container_expr(ctx, container, parent)
        }
        JSXElementChild::JSXElement(el) => {
          serialize_jsx_element(ctx, el, parent)
        }
        JSXElementChild::JSXFragment(frag) => {
          serialize_jsx_fragment(ctx, frag, parent)
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
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::JSXMemberExpression, parent, &node.span, 2);
  let obj_ref = ctx.ref_field(AstProp::Object);
  let prop_ref = ctx.ref_field(AstProp::Property);

  let obj = match &node.obj {
    JSXObject::JSXMemberExpr(member) => {
      serialize_jsx_member_expr(ctx, member, pos)
    }
    JSXObject::Ident(ident) => serialize_jsx_identifier(ctx, ident, parent),
  };

  let prop = serialize_ident_name_as_jsx_identifier(ctx, &node.prop, pos);

  ctx.write_ref(obj_ref, obj);
  ctx.write_ref(prop_ref, prop);

  pos
}

fn serialize_jsx_element_name(
  ctx: &mut SerializeCtx,
  node: &JSXElementName,
  parent: NodeRef,
) -> NodeRef {
  match &node {
    JSXElementName::Ident(ident) => {
      serialize_jsx_identifier(ctx, ident, parent)
    }
    JSXElementName::JSXMemberExpr(member) => {
      serialize_jsx_member_expr(ctx, member, parent)
    }
    JSXElementName::JSXNamespacedName(ns) => {
      serialize_jsx_namespaced_name(ctx, ns, parent)
    }
  }
}

fn serialize_jsx_opening_element(
  ctx: &mut SerializeCtx,
  node: &JSXOpeningElement,
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::JSXOpeningElement, parent, &node.span, 3);
  let sclose_pos = ctx.bool_field(AstProp::SelfClosing);
  let name_pos = ctx.ref_field(AstProp::Name);
  let attrs_pos = ctx.ref_field(AstProp::Attributes);

  let name = serialize_jsx_element_name(ctx, &node.name, pos);

  // FIXME: type args

  let attrs = node
    .attrs
    .iter()
    .map(|attr| match attr {
      JSXAttrOrSpread::JSXAttr(attr) => {
        let attr_pos = ctx.header(AstNode::JSXAttribute, pos, &attr.span, 2);
        let name_pos = ctx.ref_field(AstProp::Name);
        let value_pos = ctx.ref_field(AstProp::Value);

        let name = match &attr.name {
          JSXAttrName::Ident(name) => {
            serialize_ident_name_as_jsx_identifier(ctx, name, attr_pos)
          }
          JSXAttrName::JSXNamespacedName(node) => {
            serialize_jsx_namespaced_name(ctx, node, attr_pos)
          }
        };

        let value = attr.value.as_ref().map(|value| match value {
          JSXAttrValue::Lit(lit) => serialize_lit(ctx, lit, attr_pos),
          JSXAttrValue::JSXExprContainer(container) => {
            serialize_jsx_container_expr(ctx, container, attr_pos)
          }
          JSXAttrValue::JSXElement(el) => {
            serialize_jsx_element(ctx, el, attr_pos)
          }
          JSXAttrValue::JSXFragment(frag) => {
            serialize_jsx_fragment(ctx, frag, attr_pos)
          }
        });

        ctx.write_ref(name_pos, name);
        ctx.write_maybe_ref(value_pos, value);

        attr_pos
      }
      JSXAttrOrSpread::SpreadElement(spread) => {
        let attr_pos =
          ctx.header(AstNode::JSXAttribute, pos, &spread.dot3_token, 1);
        let arg_pos = ctx.ref_field(AstProp::Argument);

        let arg = serialize_expr(ctx, &spread.expr, attr_pos);

        ctx.write_ref(arg_pos, arg);

        attr_pos
      }
    })
    .collect::<Vec<_>>();

  ctx.write_bool(sclose_pos, node.self_closing);
  ctx.write_ref(name_pos, name);
  ctx.write_refs(attrs_pos, attrs);

  pos
}

fn serialize_jsx_container_expr(
  ctx: &mut SerializeCtx,
  node: &JSXExprContainer,
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::JSXExpressionContainer, parent, &node.span, 1);
  let expr_pos = ctx.ref_field(AstProp::Expression);

  let expr = match &node.expr {
    JSXExpr::JSXEmptyExpr(expr) => serialize_jsx_empty_expr(ctx, expr, pos),
    JSXExpr::Expr(expr) => serialize_expr(ctx, expr, pos),
  };

  ctx.write_ref(expr_pos, expr);

  pos
}

fn serialize_jsx_empty_expr(
  ctx: &mut SerializeCtx,
  node: &JSXEmptyExpr,
  parent: NodeRef,
) -> NodeRef {
  ctx.push_node(AstNode::JSXEmptyExpression, parent, &node.span)
}

fn serialize_jsx_namespaced_name(
  ctx: &mut SerializeCtx,
  node: &JSXNamespacedName,
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::JSXNamespacedName, parent, &node.span, 2);
  let ns_pos = ctx.ref_field(AstProp::Namespace);
  let name_pos = ctx.ref_field(AstProp::Name);

  let ns_id = serialize_ident_name_as_jsx_identifier(ctx, &node.ns, pos);
  let name_id = serialize_ident_name_as_jsx_identifier(ctx, &node.name, pos);

  ctx.write_ref(ns_pos, ns_id);
  ctx.write_ref(name_pos, name_id);

  pos
}

fn serialize_ident_name_as_jsx_identifier(
  ctx: &mut SerializeCtx,
  node: &IdentName,
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::JSXIdentifier, parent, &node.span, 1);
  let name_pos = ctx.str_field(AstProp::Name);

  ctx.write_str(name_pos, &node.sym);

  pos
}

fn serialize_jsx_identifier(
  ctx: &mut SerializeCtx,
  node: &Ident,
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::JSXIdentifier, parent, &node.span, 1);
  let name_pos = ctx.str_field(AstProp::Name);

  ctx.write_str(name_pos, &node.sym);

  pos
}

fn serialize_pat(
  ctx: &mut SerializeCtx,
  pat: &Pat,
  parent: NodeRef,
) -> NodeRef {
  match pat {
    Pat::Ident(node) => serialize_ident(ctx, &node.id, parent),
    Pat::Array(node) => {
      let pos = ctx.header(AstNode::ArrayPattern, parent, &node.span, 3);
      let opt_pos = ctx.bool_field(AstProp::Optional);
      let type_pos = ctx.ref_field(AstProp::TypeAnnotation);
      let elems_pos = ctx.ref_vec_field(AstProp::Elements, node.elems.len());

      let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann, pos);

      let children = node
        .elems
        .iter()
        .map(|pat| {
          pat
            .as_ref()
            .map_or(NodeRef(0), |v| serialize_pat(ctx, &v, pos))
        })
        .collect::<Vec<_>>();

      ctx.write_bool(opt_pos, node.optional);
      ctx.write_maybe_ref(type_pos, type_ann);
      ctx.write_refs(elems_pos, children);

      pos
    }
    Pat::Rest(node) => {
      let pos = ctx.header(AstNode::RestElement, parent, &node.span, 2);
      let type_ann_pos = ctx.ref_field(AstProp::TypeAnnotation);
      let arg_pos = ctx.ref_field(AstProp::Argument);

      let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann, pos);
      let arg = serialize_pat(ctx, &node.arg, parent);

      ctx.write_maybe_ref(type_ann_pos, type_ann);
      ctx.write_ref(arg_pos, arg);

      pos
    }
    Pat::Object(node) => {
      let pos = ctx.header(AstNode::ObjectPattern, parent, &node.span, 2);
      let opt_pos = ctx.bool_field(AstProp::Optional);
      let props_pos = ctx.ref_field(AstProp::Properties);

      // FIXME: Type Ann
      if let Some(type_ann) = &node.type_ann {}

      let children = node
        .props
        .iter()
        .map(|prop| match prop {
          ObjectPatProp::KeyValue(key_value_prop) => {
            let child_pos =
              ctx.header(AstNode::Property, pos, &key_value_prop.span(), 3);
            let computed_pos = ctx.bool_field(AstProp::Computed);
            let key_pos = ctx.ref_field(AstProp::Key);
            let value_pos = ctx.ref_field(AstProp::Value);

            let computed = if let PropName::Computed(_) = key_value_prop.key {
              true
            } else {
              false
            };

            let key = serialize_prop_name(ctx, &key_value_prop.key, child_pos);
            let value =
              serialize_pat(ctx, key_value_prop.value.as_ref(), child_pos);

            ctx.write_bool(computed_pos, computed);
            ctx.write_ref(key_pos, key);
            ctx.write_ref(value_pos, value);

            child_pos
          }
          ObjectPatProp::Assign(assign_pat_prop) => {
            let child_pos =
              ctx.header(AstNode::Property, pos, &assign_pat_prop.span, 3);
            let computed_pos = ctx.bool_field(AstProp::Computed);
            let key_pos = ctx.ref_field(AstProp::Key);
            let value_pos = ctx.ref_field(AstProp::Value);

            let ident = serialize_ident(ctx, &assign_pat_prop.key.id, parent);

            let value = assign_pat_prop
              .value
              .as_ref()
              .map(|value| serialize_expr(ctx, value, child_pos));

            ctx.write_ref(key_pos, ident);
            ctx.write_maybe_ref(value_pos, value);

            child_pos
          }
          ObjectPatProp::Rest(rest_pat) => {
            serialize_pat(ctx, &Pat::Rest(rest_pat.clone()), parent)
          }
        })
        .collect::<Vec<_>>();

      ctx.write_bool(opt_pos, node.optional);
      ctx.write_refs(props_pos, children);

      pos
    }
    Pat::Assign(node) => {
      let pos = ctx.header(AstNode::AssignmentPattern, parent, &node.span, 2);
      let left_pos = ctx.ref_field(AstProp::Left);
      let right_pos = ctx.ref_field(AstProp::Right);

      let left = serialize_pat(ctx, &node.left, pos);
      let right = serialize_expr(ctx, &node.right, pos);

      ctx.write_ref(left_pos, left);
      ctx.write_ref(right_pos, right);

      pos
    }
    Pat::Invalid(_) => unreachable!(),
    Pat::Expr(node) => serialize_expr(ctx, node, parent),
  }
}

fn serialize_for_head(
  ctx: &mut SerializeCtx,
  for_head: &ForHead,
  parent: NodeRef,
) -> NodeRef {
  match for_head {
    ForHead::VarDecl(var_decl) => {
      serialize_decl(ctx, &Decl::Var(var_decl.clone()), parent)
    }
    ForHead::UsingDecl(using_decl) => {
      serialize_decl(ctx, &Decl::Using(using_decl.clone()), parent)
    }
    ForHead::Pat(pat) => serialize_pat(ctx, pat, parent),
  }
}

fn serialize_spread(
  ctx: &mut SerializeCtx,
  expr: &Expr,
  span: &Span,
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::Spread, parent, span, 1);
  let arg_pos = ctx.ref_field(AstProp::Argument);

  let expr_pos = serialize_expr(ctx, expr, parent);
  ctx.write_ref(arg_pos, expr_pos);

  pos
}

fn serialize_ident_name(
  ctx: &mut SerializeCtx,
  ident_name: &IdentName,
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::Identifier, parent, &ident_name.span, 1);
  let name_pos = ctx.str_field(AstProp::Name);
  ctx.write_str(name_pos, ident_name.sym.as_str());

  pos
}

fn serialize_prop_name(
  ctx: &mut SerializeCtx,
  prop_name: &PropName,
  parent: NodeRef,
) -> NodeRef {
  match prop_name {
    PropName::Ident(ident_name) => {
      serialize_ident_name(ctx, ident_name, parent)
    }
    PropName::Str(str_prop) => {
      let child_id =
        ctx.push_node(AstNode::StringLiteral, parent, &str_prop.span);

      let str_id = ctx.str_table.insert(str_prop.value.as_str());
      append_usize(&mut ctx.buf, str_id);

      child_id
    }
    PropName::Num(number) => {
      serialize_lit(ctx, &Lit::Num(number.clone()), parent)
    }
    PropName::Computed(node) => serialize_expr(ctx, &node.expr, parent),
    PropName::BigInt(big_int) => {
      serialize_lit(ctx, &Lit::BigInt(big_int.clone()), parent)
    }
  }
}

fn serialize_lit(
  ctx: &mut SerializeCtx,
  lit: &Lit,
  parent: NodeRef,
) -> NodeRef {
  match lit {
    Lit::Str(node) => {
      let pos = ctx.header(AstNode::StringLiteral, parent, &node.span, 1);
      let value_pos = ctx.str_field(AstProp::Value);

      ctx.write_str(value_pos, &node.value);

      pos
    }
    Lit::Bool(lit_bool) => {
      let pos = ctx.header(AstNode::Bool, parent, &lit_bool.span, 1);
      let value_pos = ctx.bool_field(AstProp::Value);

      ctx.write_bool(value_pos, lit_bool.value);

      pos
    }
    Lit::Null(node) => ctx.push_node(AstNode::Null, parent, &node.span),
    Lit::Num(node) => {
      let pos = ctx.header(AstNode::NumericLiteral, parent, &node.span, 1);
      let value_pos = ctx.str_field(AstProp::Value);

      let value = node.raw.as_ref().unwrap();
      ctx.write_str(value_pos, &value);

      pos
    }
    Lit::BigInt(node) => {
      let pos = ctx.header(AstNode::BigIntLiteral, parent, &node.span, 1);
      let value_pos = ctx.str_field(AstProp::Value);

      ctx.write_str(value_pos, &node.value.to_string());

      pos
    }
    Lit::Regex(node) => {
      let pos = ctx.header(AstNode::RegExpLiteral, parent, &node.span, 2);
      let pattern_pos = ctx.str_field(AstProp::Pattern);
      let flags_pos = ctx.str_field(AstProp::Flags);

      ctx.write_str(pattern_pos, &node.exp.as_str());
      ctx.write_str(flags_pos, &node.flags.as_str());

      pos
    }
    Lit::JSXText(jsxtext) => {
      ctx.push_node(AstNode::JSXText, parent, &jsxtext.span)
    }
  }
}

fn serialize_ts_type(
  ctx: &mut SerializeCtx,
  node: &TsType,
  parent: NodeRef,
) -> NodeRef {
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

      ctx.push_node(kind, parent, &node.span)
    }
    TsType::TsThisType(node) => {
      ctx.push_node(AstNode::TSThisType, parent, &node.span)
    }
    TsType::TsFnOrConstructorType(node) => match node {
      TsFnOrConstructorType::TsFnType(node) => {
        let pos = ctx.header(AstNode::TSFunctionType, parent, &node.span, 1);
        let params_pos = ctx.ref_field(AstProp::Params);

        let param_ids = node
          .params
          .iter()
          .map(|param| serialize_ts_fn_param(ctx, param, pos))
          .collect::<Vec<_>>();

        ctx.write_refs(params_pos, param_ids);

        pos
      }
      TsFnOrConstructorType::TsConstructorType(ts_constructor_type) => {
        todo!()
      }
    },
    TsType::TsTypeRef(node) => {
      let pos = ctx.header(AstNode::TSTypeReference, parent, &node.span, 1);
      let name_pos = ctx.ref_field(AstProp::TypeName);
      let name = serialize_ts_entity_name(ctx, &node.type_name, pos);

      // FIXME params

      ctx.write_ref(name_pos, name);

      pos
    }
    TsType::TsTypeQuery(node) => {
      let pos = ctx.header(AstNode::TSTypeQuery, parent, &node.span, 1);
      let name_pos = ctx.ref_field(AstProp::ExprName);

      let expr_name = match &node.expr_name {
        TsTypeQueryExpr::TsEntityName(entity) => {
          serialize_ts_entity_name(ctx, entity, pos)
        }
        TsTypeQueryExpr::Import(ts_import_type) => todo!(),
      };

      // FIXME: params

      ctx.write_ref(name_pos, expr_name);

      pos
    }
    TsType::TsTypeLit(ts_type_lit) => todo!(),
    TsType::TsArrayType(ts_array_type) => todo!(),
    TsType::TsTupleType(node) => {
      let pos = ctx.header(AstNode::TSTupleType, parent, &node.span, 1);
      let children_pos =
        ctx.ref_vec_field(AstProp::ElementTypes, node.elem_types.len());

      let children = node
        .elem_types
        .iter()
        .map(|elem| todo!())
        .collect::<Vec<_>>();

      ctx.write_refs(children_pos, children);

      pos
    }
    TsType::TsOptionalType(ts_optional_type) => todo!(),
    TsType::TsRestType(ts_rest_type) => todo!(),
    TsType::TsUnionOrIntersectionType(node) => match node {
      TsUnionOrIntersectionType::TsUnionType(node) => {
        let pos = ctx.header(AstNode::TSUnionType, parent, &node.span, 1);
        let types_pos = ctx.ref_vec_field(AstProp::Types, node.types.len());

        let children = node
          .types
          .iter()
          .map(|item| serialize_ts_type(ctx, item, pos))
          .collect::<Vec<_>>();

        ctx.write_refs(types_pos, children);

        pos
      }
      TsUnionOrIntersectionType::TsIntersectionType(node) => {
        let pos =
          ctx.header(AstNode::TSIntersectionType, parent, &node.span, 1);
        let types_pos = ctx.ref_vec_field(AstProp::Types, node.types.len());

        let children = node
          .types
          .iter()
          .map(|item| serialize_ts_type(ctx, item, pos))
          .collect::<Vec<_>>();

        ctx.write_refs(types_pos, children);

        pos
      }
    },
    TsType::TsConditionalType(node) => {
      let pos = ctx.header(AstNode::TSConditionalType, parent, &node.span, 4);
      let check_pos = ctx.ref_field(AstProp::CheckType);
      let extends_pos = ctx.ref_field(AstProp::ExtendsType);
      let true_pos = ctx.ref_field(AstProp::TrueType);
      let false_pos = ctx.ref_field(AstProp::FalseType);

      let check = serialize_ts_type(ctx, &node.check_type, pos);
      let extends = serialize_ts_type(ctx, &node.extends_type, pos);
      let v_true = serialize_ts_type(ctx, &node.true_type, pos);
      let v_false = serialize_ts_type(ctx, &node.false_type, pos);

      ctx.write_ref(check_pos, check);
      ctx.write_ref(extends_pos, extends);
      ctx.write_ref(true_pos, v_true);
      ctx.write_ref(false_pos, v_false);

      pos
    }
    TsType::TsInferType(node) => {
      let pos = ctx.header(AstNode::TSInferType, parent, &node.span, 1);
      let param_pos = ctx.ref_field(AstProp::TypeParameter);

      let param = serialize_ts_type_param(ctx, &node.type_param, parent);

      ctx.write_ref(param_pos, param);

      pos
    }
    TsType::TsParenthesizedType(ts_parenthesized_type) => todo!(),
    TsType::TsTypeOperator(ts_type_operator) => todo!(),
    TsType::TsIndexedAccessType(ts_indexed_access_type) => todo!(),
    TsType::TsMappedType(node) => {
      let pos = ctx.header(AstNode::TSMappedType, parent, &node.span, 5);

      let name_pos = ctx.ref_field(AstProp::NameType);
      let type_ann_pos = ctx.ref_field(AstProp::TypeAnnotation);
      let type_param_pos = ctx.ref_field(AstProp::TypeParameter);
      let opt_pos = ctx.flag_field(AstProp::Optional, PropFlags::TruePlusMinus);
      let readonly_pos =
        ctx.flag_field(AstProp::Readonly, PropFlags::TruePlusMinus);

      let name_id = maybe_serialize_ts_type(ctx, &node.name_type, pos);
      let type_ann = maybe_serialize_ts_type(ctx, &node.type_ann, pos);
      let type_param = serialize_ts_type_param(ctx, &node.type_param, pos);

      ctx.write_flags(opt_pos, serialize_maybe_true_plus(node.optional));
      ctx.write_flags(readonly_pos, serialize_maybe_true_plus(node.readonly));
      ctx.write_maybe_ref(name_pos, name_id);
      ctx.write_maybe_ref(type_ann_pos, type_ann);
      ctx.write_ref(type_param_pos, type_param);

      pos
    }
    TsType::TsLitType(node) => {
      let pos = ctx.header(AstNode::TSLiteralType, parent, &node.span, 1);
      let lit_pos = ctx.ref_field(AstProp::Literal);

      let lit = match &node.lit {
        TsLit::Number(lit) => serialize_lit(ctx, &Lit::Num(lit.clone()), pos),
        TsLit::Str(lit) => serialize_lit(ctx, &Lit::Str(lit.clone()), pos),
        TsLit::Bool(lit) => serialize_lit(ctx, &Lit::Bool(lit.clone()), pos),
        TsLit::BigInt(lit) => {
          serialize_lit(ctx, &Lit::BigInt(lit.clone()), pos)
        }
        TsLit::Tpl(lit) => serialize_expr(
          ctx,
          &Expr::Tpl(Tpl {
            span: lit.span,
            exprs: vec![],
            quasis: lit.quasis.clone(),
          }),
          pos,
        ),
      };

      ctx.write_ref(lit_pos, lit);

      pos
    }
    TsType::TsTypePredicate(ts_type_predicate) => todo!(),
    TsType::TsImportType(ts_import_type) => todo!(),
  }
}

fn serialize_maybe_true_plus(value: Option<TruePlusMinus>) -> u8 {
  value.map_or(0, |v| match v {
    TruePlusMinus::True => 1,
    TruePlusMinus::Plus => 2,
    TruePlusMinus::Minus => 3,
  })
}

fn serialize_ts_entity_name(
  ctx: &mut SerializeCtx,
  node: &TsEntityName,
  parent: NodeRef,
) -> NodeRef {
  match &node {
    TsEntityName::TsQualifiedName(ts_qualified_name) => todo!(),
    TsEntityName::Ident(ident) => serialize_ident(ctx, ident, parent),
  }
}

fn maybe_serialize_ts_type_ann(
  ctx: &mut SerializeCtx,
  node: &Option<Box<TsTypeAnn>>,
  parent: NodeRef,
) -> Option<NodeRef> {
  node
    .as_ref()
    .map(|type_ann| serialize_ts_type_ann(ctx, type_ann, parent))
}

fn serialize_ts_type_ann(
  ctx: &mut SerializeCtx,
  node: &TsTypeAnn,
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::TSTypeAnnotation, parent, &node.span, 1);
  let type_pos = ctx.ref_field(AstProp::TypeAnnotation);

  let v_type = serialize_ts_type(ctx, &node.type_ann, pos);

  ctx.write_ref(type_pos, v_type);

  pos
}

fn maybe_serialize_ts_type(
  ctx: &mut SerializeCtx,
  node: &Option<Box<TsType>>,
  parent: NodeRef,
) -> Option<NodeRef> {
  node
    .as_ref()
    .map(|item| serialize_ts_type(ctx, item, parent))
}

fn serialize_ts_type_param(
  ctx: &mut SerializeCtx,
  node: &TsTypeParam,
  parent: NodeRef,
) -> NodeRef {
  let pos = ctx.header(AstNode::TSTypeParameter, parent, &node.span, 6);
  let name_pos = ctx.ref_field(AstProp::Name);
  let constraint_pos = ctx.ref_field(AstProp::Constraint);
  let default_pos = ctx.ref_field(AstProp::Default);
  let const_pos = ctx.ref_field(AstProp::Const);
  let in_pos = ctx.ref_field(AstProp::In);
  let out_pos = ctx.ref_field(AstProp::Out);

  let name = serialize_ident(ctx, &node.name, pos);
  let constraint = maybe_serialize_ts_type(ctx, &node.constraint, pos);
  let default = maybe_serialize_ts_type(ctx, &node.default, pos);

  ctx.write_bool(const_pos, node.is_const);
  ctx.write_bool(in_pos, node.is_in);
  ctx.write_bool(out_pos, node.is_out);
  ctx.write_ref(name_pos, name);
  ctx.write_maybe_ref(constraint_pos, constraint);
  ctx.write_maybe_ref(default_pos, default);

  pos
}

fn maybe_serialize_ts_type_param(
  ctx: &mut SerializeCtx,
  node: &Option<Box<TsTypeParamDecl>>,
  parent: NodeRef,
) -> Option<NodeRef> {
  node.as_ref().map(|node| {
    let pos =
      ctx.header(AstNode::TSTypeParameterDeclaration, parent, &node.span, 1);
    let params_pos = ctx.ref_vec_field(AstProp::Params, node.params.len());

    let params = node
      .params
      .iter()
      .map(|param| serialize_ts_type_param(ctx, param, pos))
      .collect::<Vec<_>>();

    ctx.write_refs(params_pos, params);

    pos
  })
}

fn serialize_ts_fn_param(
  ctx: &mut SerializeCtx,
  node: &TsFnParam,
  parent: NodeRef,
) -> NodeRef {
  match node {
    TsFnParam::Ident(ident) => serialize_ident(ctx, ident, parent),
    TsFnParam::Array(pat) => {
      serialize_pat(ctx, &Pat::Array(pat.clone()), parent)
    }
    TsFnParam::Rest(pat) => serialize_pat(ctx, &Pat::Rest(pat.clone()), parent),
    TsFnParam::Object(pat) => {
      serialize_pat(ctx, &Pat::Object(pat.clone()), parent)
    }
  }
}
