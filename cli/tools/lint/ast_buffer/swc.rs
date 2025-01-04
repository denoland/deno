// Copyright 2018-2025 the Deno authors. MIT license.

use deno_ast::swc::ast::AssignTarget;
use deno_ast::swc::ast::AssignTargetPat;
use deno_ast::swc::ast::BlockStmtOrExpr;
use deno_ast::swc::ast::Callee;
use deno_ast::swc::ast::ClassMember;
use deno_ast::swc::ast::Decl;
use deno_ast::swc::ast::ExportSpecifier;
use deno_ast::swc::ast::Expr;
use deno_ast::swc::ast::ExprOrSpread;
use deno_ast::swc::ast::FnExpr;
use deno_ast::swc::ast::ForHead;
use deno_ast::swc::ast::Function;
use deno_ast::swc::ast::Ident;
use deno_ast::swc::ast::IdentName;
use deno_ast::swc::ast::JSXAttrName;
use deno_ast::swc::ast::JSXAttrOrSpread;
use deno_ast::swc::ast::JSXAttrValue;
use deno_ast::swc::ast::JSXElement;
use deno_ast::swc::ast::JSXElementChild;
use deno_ast::swc::ast::JSXElementName;
use deno_ast::swc::ast::JSXEmptyExpr;
use deno_ast::swc::ast::JSXExpr;
use deno_ast::swc::ast::JSXExprContainer;
use deno_ast::swc::ast::JSXFragment;
use deno_ast::swc::ast::JSXMemberExpr;
use deno_ast::swc::ast::JSXNamespacedName;
use deno_ast::swc::ast::JSXObject;
use deno_ast::swc::ast::JSXOpeningElement;
use deno_ast::swc::ast::Lit;
use deno_ast::swc::ast::MemberExpr;
use deno_ast::swc::ast::MemberProp;
use deno_ast::swc::ast::ModuleDecl;
use deno_ast::swc::ast::ModuleExportName;
use deno_ast::swc::ast::ModuleItem;
use deno_ast::swc::ast::ObjectPatProp;
use deno_ast::swc::ast::OptChainBase;
use deno_ast::swc::ast::Param;
use deno_ast::swc::ast::ParamOrTsParamProp;
use deno_ast::swc::ast::Pat;
use deno_ast::swc::ast::PrivateName;
use deno_ast::swc::ast::Program;
use deno_ast::swc::ast::Prop;
use deno_ast::swc::ast::PropName;
use deno_ast::swc::ast::PropOrSpread;
use deno_ast::swc::ast::SimpleAssignTarget;
use deno_ast::swc::ast::Stmt;
use deno_ast::swc::ast::SuperProp;
use deno_ast::swc::ast::Tpl;
use deno_ast::swc::ast::TsEntityName;
use deno_ast::swc::ast::TsEnumMemberId;
use deno_ast::swc::ast::TsFnOrConstructorType;
use deno_ast::swc::ast::TsFnParam;
use deno_ast::swc::ast::TsIndexSignature;
use deno_ast::swc::ast::TsLit;
use deno_ast::swc::ast::TsLitType;
use deno_ast::swc::ast::TsThisTypeOrIdent;
use deno_ast::swc::ast::TsType;
use deno_ast::swc::ast::TsTypeAnn;
use deno_ast::swc::ast::TsTypeElement;
use deno_ast::swc::ast::TsTypeParam;
use deno_ast::swc::ast::TsTypeParamDecl;
use deno_ast::swc::ast::TsTypeParamInstantiation;
use deno_ast::swc::ast::TsTypeQueryExpr;
use deno_ast::swc::ast::TsUnionOrIntersectionType;
use deno_ast::swc::ast::VarDeclOrExpr;
use deno_ast::swc::common::Span;
use deno_ast::swc::common::Spanned;
use deno_ast::swc::common::SyntaxContext;
use deno_ast::view::Accessibility;
use deno_ast::view::AssignOp;
use deno_ast::view::BinaryOp;
use deno_ast::view::TruePlusMinus;
use deno_ast::view::TsKeywordTypeKind;
use deno_ast::view::TsTypeOperatorOp;
use deno_ast::view::UnaryOp;
use deno_ast::view::UpdateOp;
use deno_ast::view::VarDeclKind;
use deno_ast::ParsedSource;

use super::buffer::AstBufSerializer;
use super::buffer::BoolPos;
use super::buffer::NodePos;
use super::buffer::NodeRef;
use super::buffer::StrPos;
use super::ts_estree::AstNode;
use super::ts_estree::AstProp;
use super::ts_estree::TsEsTreeBuilder;
use super::ts_estree::TsKeywordKind;

pub fn serialize_swc_to_buffer(parsed_source: &ParsedSource) -> Vec<u8> {
  let mut ctx = TsEsTreeBuilder::new();

  let program = &parsed_source.program();

  let raw = ctx.header(AstNode::Program, NodeRef(0), &program.span());
  let source_type_pos = ctx.str_field(AstProp::SourceType);

  match program.as_ref() {
    Program::Module(module) => {
      let body_pos = ctx.ref_vec_field(AstProp::Body, module.body.len());
      let pos = ctx.commit_schema(raw);

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
      let pos = ctx.commit_schema(raw);

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
  ctx: &mut TsEsTreeBuilder,
  module_decl: &ModuleDecl,
  parent: NodeRef,
) -> NodeRef {
  match module_decl {
    ModuleDecl::Import(node) => {
      let raw = ctx.header(AstNode::ImportExpression, parent, &node.span);
      ctx.commit_schema(raw)
    }
    ModuleDecl::ExportDecl(node) => {
      let raw = ctx.header(AstNode::ExportNamedDeclaration, parent, &node.span);
      let decl_pos = ctx.ref_field(AstProp::Declarations);
      let pos = ctx.commit_schema(raw);

      let decl = serialize_decl(ctx, &node.decl, pos);

      ctx.write_ref(decl_pos, decl);

      pos
    }
    ModuleDecl::ExportNamed(node) => {
      let raw = ctx.header(AstNode::ExportNamedDeclaration, parent, &node.span);
      let src_pos = ctx.ref_field(AstProp::Source);
      let spec_pos =
        ctx.ref_vec_field(AstProp::Specifiers, node.specifiers.len());
      let id = ctx.commit_schema(raw);

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
              let raw = ctx.header(AstNode::ExportSpecifier, id, &child.span);
              let local_pos = ctx.ref_field(AstProp::Local);
              let exp_pos = ctx.ref_field(AstProp::Exported);
              let spec_pos = ctx.commit_schema(raw);

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
      let raw =
        ctx.header(AstNode::ExportDefaultDeclaration, parent, &node.span);
      ctx.commit_schema(raw)
    }
    ModuleDecl::ExportDefaultExpr(node) => {
      let raw =
        ctx.header(AstNode::ExportDefaultDeclaration, parent, &node.span);
      ctx.commit_schema(raw)
    }
    ModuleDecl::ExportAll(node) => {
      let raw = ctx.header(AstNode::ExportAllDeclaration, parent, &node.span);
      ctx.commit_schema(raw)
    }
    ModuleDecl::TsImportEquals(node) => {
      let raw = ctx.header(AstNode::TsImportEquals, parent, &node.span);
      ctx.commit_schema(raw)
    }
    ModuleDecl::TsExportAssignment(node) => {
      let raw = ctx.header(AstNode::TsExportAssignment, parent, &node.span);
      ctx.commit_schema(raw)
    }
    ModuleDecl::TsNamespaceExport(node) => {
      let raw = ctx.header(AstNode::TsNamespaceExport, parent, &node.span);
      ctx.commit_schema(raw)
    }
  }
}

fn serialize_stmt(ctx: &mut TsEsTreeBuilder, stmt: &Stmt) -> NodeRef {
  match stmt {
    Stmt::Block(node) => {
      let children = node
        .stmts
        .iter()
        .map(|stmt| serialize_stmt(ctx, stmt))
        .collect::<Vec<_>>();

      ctx.write_block_stmt(&node.span, children)
    }
    Stmt::Empty(_) => NodeRef(0),
    Stmt::Debugger(node) => ctx.write_debugger_stmt(&node.span),
    Stmt::With(node) => {
      let obj = serialize_expr(ctx, &node.obj);
      let body = serialize_stmt(ctx, &node.body);

      ctx.write_with_stmt(&node.span, obj, body)
    }
    Stmt::Return(node) => {
      let arg = node.arg.as_ref().map(|arg| serialize_expr(ctx, arg));
      ctx.write_return_stmt(&node.span, arg)
    }
    Stmt::Labeled(node) => {
      let ident = serialize_ident(ctx, &node.label);
      let stmt = serialize_stmt(ctx, &node.body);

      ctx.write_labeled_stmt(&node.span, ident, stmt)
    }
    Stmt::Break(node) => {
      let arg = node.label.as_ref().map(|label| serialize_ident(ctx, label));
      ctx.write_break_stmt(&node.span, arg)
    }
    Stmt::Continue(node) => {
      let arg = node.label.as_ref().map(|label| serialize_ident(ctx, label));

      ctx.write_continue_stmt(&node.span, arg)
    }
    Stmt::If(node) => {
      let test = serialize_expr(ctx, node.test.as_ref());
      let cons = serialize_stmt(ctx, node.cons.as_ref());
      let alt = node.alt.as_ref().map(|alt| serialize_stmt(ctx, alt));

      ctx.write_if_stmt(&node.span, test, cons, alt)
    }
    Stmt::Switch(node) => {
      let disc = serialize_expr(ctx, &node.discriminant);

      let cases = node
        .cases
        .iter()
        .map(|case| {
          let test = case.test.as_ref().map(|test| serialize_expr(ctx, test));

          let cons = case
            .cons
            .iter()
            .map(|cons| serialize_stmt(ctx, cons))
            .collect::<Vec<_>>();

          ctx.write_switch_case(&case.span, test, cons)
        })
        .collect::<Vec<_>>();

      ctx.write_switch_stmt(&node.span, disc, cases)
    }
    Stmt::Throw(node) => {
      let arg = serialize_expr(ctx, &node.arg);
      ctx.write_throw_stmt(&node.span, arg)
    }
    Stmt::Try(node) => {
      let block = serialize_stmt(ctx, &Stmt::Block(node.block.clone()));

      let handler = node.handler.as_ref().map(|catch| {
        let param = catch.param.as_ref().map(|param| serialize_pat(ctx, param));

        let body = serialize_stmt(ctx, &Stmt::Block(catch.body.clone()));

        ctx.write_catch_clause(&catch.span, param, body)
      });

      let finalizer = node
        .finalizer
        .as_ref()
        .map(|finalizer| serialize_stmt(ctx, &Stmt::Block(finalizer.clone())));

      ctx.write_try_stmt(&node.span, block, handler, finalizer)
    }
    Stmt::While(node) => {
      let test = serialize_expr(ctx, node.test.as_ref());
      let stmt = serialize_stmt(ctx, node.body.as_ref());

      ctx.write_while_stmt(&node.span, test, stmt)
    }
    Stmt::DoWhile(node) => {
      let expr = serialize_expr(ctx, node.test.as_ref());
      let stmt = serialize_stmt(ctx, node.body.as_ref());

      ctx.write_do_while_stmt(&node.span, expr, stmt)
    }
    Stmt::For(node) => {
      let init = node.init.as_ref().map(|init| match init {
        VarDeclOrExpr::VarDecl(var_decl) => {
          serialize_stmt(ctx, &Stmt::Decl(Decl::Var(var_decl.clone())))
        }
        VarDeclOrExpr::Expr(expr) => serialize_expr(ctx, expr),
      });

      let test = node.test.as_ref().map(|expr| serialize_expr(ctx, expr));
      let update = node.update.as_ref().map(|expr| serialize_expr(ctx, expr));
      let body = serialize_stmt(ctx, node.body.as_ref());

      ctx.write_for_stmt(&node.span, init, test, update, body)
    }
    Stmt::ForIn(node) => {
      let left = serialize_for_head(ctx, &node.left);
      let right = serialize_expr(ctx, node.right.as_ref());
      let body = serialize_stmt(ctx, node.body.as_ref());

      ctx.write_for_in_stmt(&node.span, left, right, body)
    }
    Stmt::ForOf(node) => {
      let left = serialize_for_head(ctx, &node.left);
      let right = serialize_expr(ctx, node.right.as_ref());
      let body = serialize_stmt(ctx, node.body.as_ref());

      ctx.write_for_of_stmt(&node.span, node.is_await, left, right, body)
    }
    Stmt::Decl(node) => serialize_decl(ctx, node),
    Stmt::Expr(node) => {
      let expr = serialize_expr(ctx, node.expr.as_ref());
      ctx.write_expr_stmt(&node.span, expr)
    }
  }
}

fn serialize_expr(ctx: &mut TsEsTreeBuilder, expr: &Expr) -> NodeRef {
  match expr {
    Expr::This(node) => ctx.write_this_expr(&node.span),
    Expr::Array(node) => {
      let elems = node
        .elems
        .iter()
        .map(|item| {
          item
            .as_ref()
            .map_or(NodeRef(0), |item| serialize_expr_or_spread(ctx, item))
        })
        .collect::<Vec<_>>();

      ctx.write_arr_expr(&node.span, elems)
    }
    Expr::Object(node) => {
      let props = node
        .props
        .iter()
        .map(|prop| serialize_prop_or_spread(ctx, prop))
        .collect::<Vec<_>>();

      ctx.write_obj_expr(&node.span, props)
    }
    Expr::Fn(node) => {
      let fn_obj = node.function.as_ref();

      let ident = node.ident.as_ref().map(|ident| serialize_ident(ctx, ident));

      let type_params = maybe_serialize_ts_type_param(ctx, &fn_obj.type_params);

      let params = fn_obj
        .params
        .iter()
        .map(|param| serialize_pat(ctx, &param.pat))
        .collect::<Vec<_>>();

      let return_id = maybe_serialize_ts_type_ann(ctx, &fn_obj.return_type);
      let body = fn_obj
        .body
        .as_ref()
        .map(|block| serialize_stmt(ctx, &Stmt::Block(block.clone())));

      ctx.write_fn_expr(
        &fn_obj.span,
        fn_obj.is_async,
        fn_obj.is_generator,
        ident,
        type_params,
        params,
        return_id,
        body,
      )
    }
    Expr::Unary(node) => {
      let arg = serialize_expr(ctx, &node.arg);
      let op = match node.op {
        UnaryOp::Minus => "-",
        UnaryOp::Plus => "+",
        UnaryOp::Bang => "!",
        UnaryOp::Tilde => "~",
        UnaryOp::TypeOf => "typeof",
        UnaryOp::Void => "void",
        UnaryOp::Delete => "delete",
      };

      ctx.write_unary_expr(&node.span, op, arg)
    }
    Expr::Update(node) => {
      let arg = serialize_expr(ctx, node.arg.as_ref());
      let op = match node.op {
        UpdateOp::PlusPlus => "++",
        UpdateOp::MinusMinus => "--",
      };

      ctx.write_update_expr(&node.span, node.prefix, op, arg)
    }
    Expr::Bin(node) => {
      let (node_type, flag_str) = match node.op {
        BinaryOp::LogicalAnd => (AstNode::LogicalExpression, "&&"),
        BinaryOp::LogicalOr => (AstNode::LogicalExpression, "||"),
        BinaryOp::NullishCoalescing => (AstNode::LogicalExpression, "??"),
        BinaryOp::EqEq => (AstNode::BinaryExpression, "=="),
        BinaryOp::NotEq => (AstNode::BinaryExpression, "!="),
        BinaryOp::EqEqEq => (AstNode::BinaryExpression, "==="),
        BinaryOp::NotEqEq => (AstNode::BinaryExpression, "!="),
        BinaryOp::Lt => (AstNode::BinaryExpression, "<"),
        BinaryOp::LtEq => (AstNode::BinaryExpression, "<="),
        BinaryOp::Gt => (AstNode::BinaryExpression, ">"),
        BinaryOp::GtEq => (AstNode::BinaryExpression, ">="),
        BinaryOp::LShift => (AstNode::BinaryExpression, "<<"),
        BinaryOp::RShift => (AstNode::BinaryExpression, ">>"),
        BinaryOp::ZeroFillRShift => (AstNode::BinaryExpression, ">>>"),
        BinaryOp::Add => (AstNode::BinaryExpression, "+"),
        BinaryOp::Sub => (AstNode::BinaryExpression, "-"),
        BinaryOp::Mul => (AstNode::BinaryExpression, "*"),
        BinaryOp::Div => (AstNode::BinaryExpression, "/"),
        BinaryOp::Mod => (AstNode::BinaryExpression, "%"),
        BinaryOp::BitOr => (AstNode::BinaryExpression, "|"),
        BinaryOp::BitXor => (AstNode::BinaryExpression, "^"),
        BinaryOp::BitAnd => (AstNode::BinaryExpression, "&"),
        BinaryOp::In => (AstNode::BinaryExpression, "in"),
        BinaryOp::InstanceOf => (AstNode::BinaryExpression, "instanceof"),
        BinaryOp::Exp => (AstNode::BinaryExpression, "**"),
      };

      let left = serialize_expr(ctx, node.left.as_ref());
      let right = serialize_expr(ctx, node.right.as_ref());

      match node_type {
        AstNode::LogicalExpression => {
          ctx.write_logical_expr(&node.span, flag_str, left, right)
        }
        AstNode::BinaryExpression => {
          ctx.write_bin_expr(&node.span, flag_str, left, right)
        }
        _ => unreachable!(),
      }
    }
    Expr::Assign(node) => {
      let left = match &node.left {
        AssignTarget::Simple(simple_assign_target) => {
          match simple_assign_target {
            SimpleAssignTarget::Ident(target) => {
              serialize_ident(ctx, &target.id)
            }
            SimpleAssignTarget::Member(target) => {
              serialize_expr(ctx, &Expr::Member(target.clone()))
            }
            SimpleAssignTarget::SuperProp(target) => {
              serialize_expr(ctx, &Expr::SuperProp(target.clone()))
            }
            SimpleAssignTarget::Paren(target) => {
              serialize_expr(ctx, &target.expr)
            }
            SimpleAssignTarget::OptChain(target) => {
              serialize_expr(ctx, &Expr::OptChain(target.clone()))
            }
            SimpleAssignTarget::TsAs(target) => {
              serialize_expr(ctx, &Expr::TsAs(target.clone()))
            }
            SimpleAssignTarget::TsSatisfies(target) => {
              serialize_expr(ctx, &Expr::TsSatisfies(target.clone()))
            }
            SimpleAssignTarget::TsNonNull(target) => {
              serialize_expr(ctx, &Expr::TsNonNull(target.clone()))
            }
            SimpleAssignTarget::TsTypeAssertion(target) => {
              serialize_expr(ctx, &Expr::TsTypeAssertion(target.clone()))
            }
            SimpleAssignTarget::TsInstantiation(target) => {
              serialize_expr(ctx, &Expr::TsInstantiation(target.clone()))
            }
            SimpleAssignTarget::Invalid(_) => unreachable!(),
          }
        }
        AssignTarget::Pat(target) => match target {
          AssignTargetPat::Array(array_pat) => {
            serialize_pat(ctx, &Pat::Array(array_pat.clone()))
          }
          AssignTargetPat::Object(object_pat) => {
            serialize_pat(ctx, &Pat::Object(object_pat.clone()))
          }
          AssignTargetPat::Invalid(_) => unreachable!(),
        },
      };

      let right = serialize_expr(ctx, node.right.as_ref());

      let op = match node.op {
        AssignOp::Assign => "=",
        AssignOp::AddAssign => "+=",
        AssignOp::SubAssign => "-=",
        AssignOp::MulAssign => "*=",
        AssignOp::DivAssign => "/=",
        AssignOp::ModAssign => "%=",
        AssignOp::LShiftAssign => "<<=",
        AssignOp::RShiftAssign => ">>=",
        AssignOp::ZeroFillRShiftAssign => ">>>=",
        AssignOp::BitOrAssign => "|=",
        AssignOp::BitXorAssign => "^=",
        AssignOp::BitAndAssign => "&=",
        AssignOp::ExpAssign => "**=",
        AssignOp::AndAssign => "&&=",
        AssignOp::OrAssign => "||=",
        AssignOp::NullishAssign => "??=",
      };

      ctx.write_assignment_expr(&node.span, op, left, right)
    }
    Expr::Member(node) => serialize_member_expr(ctx, node, false),
    Expr::SuperProp(node) => {
      let obj = ctx.write_super(&node.obj.span);

      let mut computed = false;
      let prop = match &node.prop {
        SuperProp::Ident(ident_name) => serialize_ident_name(ctx, ident_name),
        SuperProp::Computed(prop) => {
          computed = true;
          serialize_expr(ctx, &prop.expr)
        }
      };

      ctx.write_member_expr(&node.span, false, computed, obj, prop)
    }
    Expr::Cond(node) => {
      let test = serialize_expr(ctx, node.test.as_ref());
      let cons = serialize_expr(ctx, node.cons.as_ref());
      let alt = serialize_expr(ctx, node.alt.as_ref());

      ctx.write_conditional_expr(&node.span, test, cons, alt)
    }
    Expr::Call(node) => {
      if let Callee::Import(_) = node.callee {
        let source = node
          .args
          .first()
          .map_or(NodeRef(0), |arg| serialize_expr_or_spread(ctx, arg));

        let options = node
          .args
          .get(1)
          .map_or(NodeRef(0), |arg| serialize_expr_or_spread(ctx, arg));

        ctx.write_import_expr(&node.span, source, options)
      } else {
        let callee = match &node.callee {
          Callee::Super(super_node) => ctx.write_super(&super_node.span),
          Callee::Import(_) => unreachable!("Already handled"),
          Callee::Expr(expr) => serialize_expr(ctx, expr),
        };

        let type_arg = node
          .type_args
          .clone()
          .map(|param_node| serialize_ts_param_inst(ctx, param_node.as_ref()));

        let args = node
          .args
          .iter()
          .map(|arg| serialize_expr_or_spread(ctx, arg))
          .collect::<Vec<_>>();

        ctx.write_call_expr(&node.span, false, callee, type_arg, args)
      }
    }
    Expr::New(node) => {
      let callee = serialize_expr(ctx, node.callee.as_ref());

      let args: Vec<NodeRef> = node.args.as_ref().map_or(vec![], |args| {
        args
          .iter()
          .map(|arg| serialize_expr_or_spread(ctx, arg))
          .collect::<Vec<_>>()
      });

      let type_args = node
        .type_args
        .clone()
        .map(|param_node| serialize_ts_param_inst(ctx, param_node.as_ref()));

      ctx.write_new_expr(&node.span, callee, type_args, args)
    }
    Expr::Seq(node) => {
      let children = node
        .exprs
        .iter()
        .map(|expr| serialize_expr(ctx, expr))
        .collect::<Vec<_>>();

      ctx.write_sequence_expr(&node.span, children)
    }
    Expr::Ident(node) => serialize_ident(ctx, node),
    Expr::Lit(node) => serialize_lit(ctx, node),
    Expr::Tpl(node) => {
      let quasis = node
        .quasis
        .iter()
        .map(|quasi| {
          ctx.write_template_elem(
            &quasi.span,
            quasi.tail,
            &quasi.raw,
            &quasi
              .cooked
              .as_ref()
              .map_or("".to_string(), |v| v.to_string()),
          )
        })
        .collect::<Vec<_>>();

      let exprs = node
        .exprs
        .iter()
        .map(|expr| serialize_expr(ctx, expr))
        .collect::<Vec<_>>();

      ctx.write_template_lit(&node.span, quasis, exprs)
    }
    Expr::TaggedTpl(node) => {
      let tag = serialize_expr(ctx, &node.tag);
      let type_param = node
        .type_params
        .clone()
        .map(|params| serialize_ts_param_inst(ctx, params.as_ref()));
      let quasi = serialize_expr(ctx, &Expr::Tpl(*node.tpl.clone()));

      ctx.write_tagged_template_expr(&node.span, tag, type_param, quasi)
    }
    Expr::Arrow(node) => {
      let raw =
        ctx.header(AstNode::ArrowFunctionExpression, parent, &node.span);
      let async_pos = ctx.bool_field(AstProp::Async);
      let gen_pos = ctx.bool_field(AstProp::Generator);
      let type_param_pos = ctx.ref_field(AstProp::TypeParameters);
      let params_pos = ctx.ref_vec_field(AstProp::Params, node.params.len());
      let body_pos = ctx.ref_field(AstProp::Body);
      let return_type_pos = ctx.ref_field(AstProp::ReturnType);
      let pos = ctx.commit_schema(raw);

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
      // FIXME
      let raw = ctx.header(AstNode::ClassExpression, parent, &node.class.span);
      ctx.commit_schema(raw)
    }
    Expr::Yield(node) => {
      let arg = node
        .arg
        .as_ref()
        .map(|arg| serialize_expr(ctx, arg.as_ref()));

      ctx.write_yield_expr(&node.span, node.delegate, arg)
    }
    Expr::MetaProp(node) => {
      let raw = ctx.header(AstNode::MetaProp, parent, &node.span);
      ctx.commit_schema(raw)
    }
    Expr::Await(node) => {
      let arg = serialize_expr(ctx, node.arg.as_ref());
      ctx.write_await_expr(&node.span, arg)
    }
    Expr::Paren(node) => {
      // Paren nodes are treated as a syntax only thing in TSEStree
      // and are never materialized to actual AST nodes.
      serialize_expr(ctx, &node.expr)
    }
    Expr::JSXMember(node) => serialize_jsx_member_expr(ctx, node),
    Expr::JSXNamespacedName(node) => serialize_jsx_namespaced_name(ctx, node),
    Expr::JSXEmpty(node) => ctx.write_jsx_empty_expr(&node.span),
    Expr::JSXElement(node) => serialize_jsx_element(ctx, node),
    Expr::JSXFragment(node) => serialize_jsx_fragment(ctx, node),
    Expr::TsTypeAssertion(node) => {
      let raw = ctx.header(AstNode::TSTypeAssertion, parent, &node.span);
      let expr_pos = ctx.ref_field(AstProp::Expression);
      let type_ann_pos = ctx.ref_field(AstProp::TypeAnnotation);
      let pos = ctx.commit_schema(raw);

      let expr = serialize_expr(ctx, &node.expr);
      let type_ann = serialize_ts_type(ctx, &node.type_ann);

      ctx.write_ref(expr_pos, expr);
      ctx.write_ref(type_ann_pos, type_ann);

      pos
    }
    Expr::TsConstAssertion(node) => {
      let expr = serialize_expr(ctx, node.expr.as_ref());

      // TODO(@marvinhagemeister): type_ann
      ctx.write_ts_as_expr(&node.span, expr, NodeRef(0))
    }
    Expr::TsNonNull(node) => {
      let expr = serialize_expr(ctx, node.expr.as_ref());
      ctx.write_ts_non_null(&node.span, expr)
    }
    Expr::TsAs(node) => {
      let expr = serialize_expr(ctx, node.expr.as_ref());
      let type_ann = serialize_ts_type(ctx, node.type_ann.as_ref());

      ctx.write_ts_as_expr(&node.span, expr, type_ann)
    }
    Expr::TsInstantiation(_) => {
      // let raw = ctx.header(AstNode::TsInstantiation, parent, &node.span);
      // let expr_pos = ctx.ref_field(AstProp::Expression);
      // let type_args_pos = ctx.ref_field(AstProp::TypeArguments);
      // let pos = ctx.commit_schema(raw);

      // let expr = serialize_expr(ctx, node.expr.as_ref(), pos);

      // let type_arg = serialize_ts_param_inst(ctx, node.type_args.as_ref(), pos);

      // ctx.write_ref(expr_pos, expr);
      // ctx.write_ref(type_args_pos, type_arg);

      // pos
      todo!()
    }
    Expr::TsSatisfies(node) => {
      let expr = serialize_expr(ctx, node.expr.as_ref());
      let type_ann = serialize_ts_type(ctx, node.type_ann.as_ref());

      ctx.write_ts_satisfies_expr(&node.span, expr, type_ann)
    }
    Expr::PrivateName(node) => serialize_private_name(ctx, node),
    Expr::OptChain(node) => {
      let expr = match node.base.as_ref() {
        OptChainBase::Member(member_expr) => {
          serialize_member_expr(ctx, member_expr, true)
        }
        OptChainBase::Call(opt_call) => {
          let callee = serialize_expr(ctx, &opt_call.callee);

          let type_param_id = opt_call
            .type_args
            .clone()
            .map(|params| serialize_ts_param_inst(ctx, params.as_ref()));

          let args = opt_call
            .args
            .iter()
            .map(|arg| serialize_expr_or_spread(ctx, arg))
            .collect::<Vec<_>>();

          ctx.write_call_expr(&opt_call.span, true, callee, type_param_id, args)
        }
      };

      ctx.write_chain_expr(&node.span, expr)
    }
    Expr::Invalid(_) => {
      unreachable!()
    }
  }
}

fn serialize_prop_or_spread(
  ctx: &mut TsEsTreeBuilder,
  prop: &PropOrSpread,
) -> NodeRef {
  match prop {
    PropOrSpread::Spread(spread_element) => serialize_spread(
      ctx,
      spread_element.expr.as_ref(),
      &spread_element.dot3_token,
    ),
    PropOrSpread::Prop(prop) => {
      let mut shorthand = false;
      let mut computed = false;
      let mut method = false;
      let mut kind = "init";

      // FIXME: optional
      let (key, value) = match prop.as_ref() {
        Prop::Shorthand(ident) => {
          shorthand = true;

          let value = serialize_ident(ctx, ident);
          (value, value)
        }
        Prop::KeyValue(key_value_prop) => {
          if let PropName::Computed(_) = key_value_prop.key {
            computed = true;
          }

          let key = serialize_prop_name(ctx, &key_value_prop.key);
          let value = serialize_expr(ctx, key_value_prop.value.as_ref());

          (key, value)
        }
        Prop::Assign(assign_prop) => {
          let left = serialize_ident(ctx, &assign_prop.key);
          let right = serialize_expr(ctx, assign_prop.value.as_ref());

          let child_pos = ctx.write_assign_pat(&assign_prop.span, left, right);

          (left, child_pos)
        }
        Prop::Getter(getter_prop) => {
          kind = "get";

          let key = serialize_prop_name(ctx, &getter_prop.key);

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
          );

          (key, value)
        }
        Prop::Setter(setter_prop) => {
          kind = "set";

          let key_id = serialize_prop_name(ctx, &setter_prop.key);

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
          );

          (key_id, value_id)
        }
        Prop::Method(method_prop) => {
          method = true;

          let key_id = serialize_prop_name(ctx, &method_prop.key);

          let value_id = serialize_expr(
            ctx,
            &Expr::Fn(FnExpr {
              ident: None,
              function: method_prop.function.clone(),
            }),
          );

          (key_id, value_id)
        }
      };

      ctx.write_property(
        &prop.span(),
        shorthand,
        computed,
        method,
        kind,
        key,
        value,
      )
    }
  }
}

fn serialize_member_expr(
  ctx: &mut TsEsTreeBuilder,
  node: &MemberExpr,
  optional: bool,
) -> NodeRef {
  let mut computed = false;
  let obj = serialize_expr(ctx, node.obj.as_ref());

  let prop = match &node.prop {
    MemberProp::Ident(ident_name) => serialize_ident_name(ctx, ident_name),
    MemberProp::PrivateName(private_name) => {
      serialize_private_name(ctx, private_name)
    }
    MemberProp::Computed(computed_prop_name) => {
      computed = true;
      serialize_expr(ctx, computed_prop_name.expr.as_ref())
    }
  };

  ctx.write_member_expr(&node.span, optional, computed, obj, prop)
}

fn serialize_class_member(
  ctx: &mut TsEsTreeBuilder,
  member: &ClassMember,
  parent: NodeRef,
) -> NodeRef {
  match member {
    ClassMember::Constructor(constructor) => {
      let raw =
        ctx.header(AstNode::MethodDefinition, parent, &constructor.span);
      let key_pos = ctx.ref_field(AstProp::Key);
      let body_pos = ctx.ref_field(AstProp::Body);
      let args_pos =
        ctx.ref_vec_field(AstProp::Arguments, constructor.params.len());
      let acc_pos = if constructor.accessibility.is_some() {
        NodePos::Str(ctx.str_field(AstProp::Accessibility))
      } else {
        NodePos::Undef(ctx.undefined_field(AstProp::Accessibility))
      };
      let member_id = ctx.commit_schema(raw);

      // FIXME flags

      let key = serialize_prop_name(ctx, &constructor.key, member_id);
      let body = constructor
        .body
        .as_ref()
        .map(|body| serialize_stmt(ctx, &Stmt::Block(body.clone())));

      let params = constructor
        .params
        .iter()
        .map(|param| match param {
          ParamOrTsParamProp::TsParamProp(_) => {
            todo!()
          }
          ParamOrTsParamProp::Param(param) => serialize_pat(ctx, &param.pat),
        })
        .collect::<Vec<_>>();

      if let Some(acc) = constructor.accessibility {
        if let NodePos::Str(str_pos) = acc_pos {
          ctx.write_str(str_pos, &accessibility_to_str(acc));
        }
      }

      ctx.write_ref(key_pos, key);
      ctx.write_maybe_ref(body_pos, body);
      // FIXME
      ctx.write_refs(args_pos, params);

      member_id
    }
    ClassMember::Method(method) => {
      let raw = ctx.header(AstNode::MethodDefinition, parent, &method.span);

      let member_id = ctx.commit_schema(raw);

      // let mut flags = FlagValue::new();
      // flags.set(Flag::ClassMethod);
      if method.function.is_async {
        // FIXME
      }

      // accessibility_to_flag(&mut flags, method.accessibility);

      let _key_id = serialize_prop_name(ctx, &method.key, member_id);

      let _body_id = method
        .function
        .body
        .as_ref()
        .map(|body| serialize_stmt(ctx, &Stmt::Block(body.clone())));

      let _params = method
        .function
        .params
        .iter()
        .map(|param| serialize_pat(ctx, &param.pat))
        .collect::<Vec<_>>();

      // ctx.write_node(member_id, );
      // ctx.write_flags(&flags);
      // ctx.write_id(key_id);
      // ctx.write_id(body_id);
      // ctx.write_ids(AstProp::Params, params);

      member_id
    }
    ClassMember::PrivateMethod(_) => todo!(),
    ClassMember::ClassProp(_) => todo!(),
    ClassMember::PrivateProp(_) => todo!(),
    ClassMember::TsIndexSignature(member) => {
      serialize_ts_index_sig(ctx, member, parent)
    }
    ClassMember::Empty(_) => unreachable!(),
    ClassMember::StaticBlock(_) => todo!(),
    ClassMember::AutoAccessor(_) => todo!(),
  }
}

fn serialize_expr_or_spread(
  ctx: &mut TsEsTreeBuilder,
  arg: &ExprOrSpread,
) -> NodeRef {
  if let Some(spread) = &arg.spread {
    serialize_spread(ctx, &arg.expr, spread)
  } else {
    serialize_expr(ctx, arg.expr.as_ref())
  }
}

fn serialize_ident(ctx: &mut TsEsTreeBuilder, ident: &Ident) -> NodeRef {
  ctx.write_identifier(&ident.span, ident.sym.as_str(), ident.optional, None)
}

fn serialize_module_exported_name(
  ctx: &mut TsEsTreeBuilder,
  name: &ModuleExportName,
) -> NodeRef {
  match &name {
    ModuleExportName::Ident(ident) => serialize_ident(ctx, ident),
    ModuleExportName::Str(lit) => serialize_lit(ctx, &Lit::Str(lit.clone())),
  }
}

fn serialize_decl(
  ctx: &mut TsEsTreeBuilder,
  decl: &Decl,
  parent: NodeRef,
) -> NodeRef {
  match decl {
    Decl::Class(node) => {
      let raw = ctx.header(AstNode::ClassDeclaration, parent, &node.class.span);
      let declare_pos = ctx.bool_field(AstProp::Declare);
      let abstract_pos = ctx.bool_field(AstProp::Abstract);
      let id_pos = ctx.ref_field(AstProp::Id);
      let body_pos = ctx.ref_field(AstProp::Body);
      let type_params_pos = ctx.ref_field(AstProp::TypeParameters);
      let super_pos = ctx.ref_field(AstProp::SuperClass);
      let super_type_pos = ctx.ref_field(AstProp::SuperTypeArguments);
      let impl_pos =
        ctx.ref_vec_field(AstProp::Implements, node.class.implements.len());
      let id = ctx.commit_schema(raw);

      let body_raw = ctx.header(AstNode::ClassBody, id, &node.class.span);
      let body_body_pos =
        ctx.ref_vec_field(AstProp::Body, node.class.body.len());
      let body_id = ctx.commit_schema(body_raw);

      let ident = serialize_ident(ctx, &node.ident);
      let type_params =
        maybe_serialize_ts_type_param(ctx, &node.class.type_params);

      let super_class = node
        .class
        .super_class
        .as_ref()
        .map(|super_class| serialize_expr(ctx, super_class));

      let super_type_params = node
        .class
        .super_type_params
        .as_ref()
        .map(|super_params| serialize_ts_param_inst(ctx, super_params, id));

      let implement_ids = node
        .class
        .implements
        .iter()
        .map(|implements| {
          let raw =
            ctx.header(AstNode::TSClassImplements, id, &implements.span);

          let expr_pos = ctx.ref_field(AstProp::Expression);
          let type_args_pos = ctx.ref_field(AstProp::TypeArguments);
          let child_pos = ctx.commit_schema(raw);

          let type_args = implements
            .type_args
            .clone()
            .map(|args| serialize_ts_param_inst(ctx, &args, child_pos));

          let expr = serialize_expr(ctx, &implements.expr);

          ctx.write_ref(expr_pos, expr);
          ctx.write_maybe_ref(type_args_pos, type_args);

          child_pos
        })
        .collect::<Vec<_>>();

      let member_ids = node
        .class
        .body
        .iter()
        .map(|member| serialize_class_member(ctx, member, parent))
        .collect::<Vec<_>>();

      ctx.write_ref(body_pos, body_id);

      ctx.write_bool(declare_pos, node.declare);
      ctx.write_bool(abstract_pos, node.class.is_abstract);
      ctx.write_ref(id_pos, ident);
      ctx.write_maybe_ref(type_params_pos, type_params);
      ctx.write_maybe_ref(super_pos, super_class);
      ctx.write_maybe_ref(super_type_pos, super_type_params);
      ctx.write_refs(impl_pos, implement_ids);

      // body
      ctx.write_refs(body_body_pos, member_ids);

      id
    }
    Decl::Fn(node) => {
      let ident_id = serialize_ident(ctx, &node.ident);
      let type_param_id =
        maybe_serialize_ts_type_param(ctx, &node.function.type_params);
      let return_type =
        maybe_serialize_ts_type_ann(ctx, &node.function.return_type);

      let body = node
        .function
        .body
        .as_ref()
        .map(|body| serialize_stmt(ctx, &Stmt::Block(body.clone())));

      let params = node
        .function
        .params
        .iter()
        .map(|param| serialize_pat(ctx, &param.pat))
        .collect::<Vec<_>>();

      ctx.write_fn_decl(
        &node.function.span,
        node.declare,
        node.function.is_async,
        node.function.is_generator,
        ident_id,
        type_param_id,
        return_type,
        body,
        params,
      )
    }
    Decl::Var(node) => {
      let children = node
        .decls
        .iter()
        .map(|decl| {
          // TODO(@marvinhagemeister): Definite?

          let ident = serialize_pat(ctx, &decl.name);
          let init = decl
            .init
            .as_ref()
            .map(|init| serialize_expr(ctx, init.as_ref()));

          ctx.write_var_declarator(&decl.span, ident, init)
        })
        .collect::<Vec<_>>();

      let kind = match node.kind {
        VarDeclKind::Var => "var",
        VarDeclKind::Let => "let",
        VarDeclKind::Const => "const",
      };

      ctx.write_var_decl(&node.span, node.declare, kind, children)
    }
    Decl::Using(_) => {
      todo!();
    }
    Decl::TsInterface(node) => {
      let ident_id = serialize_ident(ctx, &node.id);
      let type_param = maybe_serialize_ts_type_param(ctx, &node.type_params);

      let extend_ids = node
        .extends
        .iter()
        .map(|item| {
          let expr = serialize_expr(ctx, &item.expr);
          let type_args = item
            .type_args
            .clone()
            .map(|params| serialize_ts_param_inst(ctx, params.as_ref()));

          ctx.write_ts_interface_heritage(&item.span, expr, type_args)
        })
        .collect::<Vec<_>>();

      let body_elem_ids = node
        .body
        .body
        .iter()
        .map(|item| match item {
          TsTypeElement::TsCallSignatureDecl(ts_call) => {
            let raw = ctx.header(
              AstNode::TsCallSignatureDeclaration,
              pos,
              &ts_call.span,
            );
            let type_ann_pos = ctx.ref_field(AstProp::TypeAnnotation);
            let params_pos =
              ctx.ref_vec_field(AstProp::Params, ts_call.params.len());
            let return_pos = ctx.ref_field(AstProp::ReturnType);
            let item_id = ctx.commit_schema(raw);

            let type_param =
              maybe_serialize_ts_type_param(ctx, &ts_call.type_params, pos);
            let return_type =
              maybe_serialize_ts_type_ann(ctx, &ts_call.type_ann, pos);
            let params = ts_call
              .params
              .iter()
              .map(|param| serialize_ts_fn_param(ctx, param, pos))
              .collect::<Vec<_>>();

            ctx.write_maybe_ref(type_ann_pos, type_param);
            ctx.write_refs(params_pos, params);
            ctx.write_maybe_ref(return_pos, return_type);

            item_id
          }
          TsTypeElement::TsConstructSignatureDecl(_) => todo!(),
          TsTypeElement::TsPropertySignature(sig) => {
            let raw = ctx.header(AstNode::TSPropertySignature, pos, &sig.span);

            let computed_pos = ctx.bool_field(AstProp::Computed);
            let optional_pos = ctx.bool_field(AstProp::Optional);
            let readonly_pos = ctx.bool_field(AstProp::Readonly);
            // TODO: where is this coming from?
            let _static_bos = ctx.bool_field(AstProp::Static);
            let key_pos = ctx.ref_field(AstProp::Key);
            let type_ann_pos = ctx.ref_field(AstProp::TypeAnnotation);
            let item_pos = ctx.commit_schema(raw);

            let key = serialize_expr(ctx, &sig.key, item_pos);
            let type_ann =
              maybe_serialize_ts_type_ann(ctx, &sig.type_ann, item_pos);

            ctx.write_bool(computed_pos, sig.computed);
            ctx.write_bool(optional_pos, sig.optional);
            ctx.write_bool(readonly_pos, sig.readonly);
            ctx.write_ref(key_pos, key);
            ctx.write_maybe_ref(type_ann_pos, type_ann);

            item_pos
          }
          TsTypeElement::TsGetterSignature(sig) => {
            let key = serialize_expr(ctx, sig.key.as_ref());
            let return_type = maybe_serialize_ts_type_ann(ctx, &sig.type_ann);

            ctx.write_ts_getter_sig(&sig.span, key, return_type)
          }
          TsTypeElement::TsSetterSignature(sig) => {
            let key = serialize_expr(ctx, sig.key.as_ref());
            let param = serialize_ts_fn_param(ctx, &sig.param);

            ctx.write_ts_setter_sig(&sig.span, key, param)
          }
          TsTypeElement::TsMethodSignature(sig) => {
            let raw = ctx.header(AstNode::TSMethodSignature, pos, &sig.span);
            let computed_pos = ctx.bool_field(AstProp::Computed);
            let optional_pos = ctx.bool_field(AstProp::Optional);
            let readonly_pos = ctx.bool_field(AstProp::Readonly);
            // TODO: where is this coming from?
            let _static_bos = ctx.bool_field(AstProp::Static);
            let kind_pos = ctx.str_field(AstProp::Kind);
            let key_pos = ctx.ref_field(AstProp::Key);
            let params_pos =
              ctx.ref_vec_field(AstProp::Params, sig.params.len());
            let return_type_pos = ctx.ref_field(AstProp::ReturnType);
            let item_pos = ctx.commit_schema(raw);

            let key = serialize_expr(ctx, sig.key.as_ref());
            let params = sig
              .params
              .iter()
              .map(|param| serialize_ts_fn_param(ctx, param))
              .collect::<Vec<_>>();
            let return_type = maybe_serialize_ts_type_ann(ctx, &sig.type_ann);

            ctx.write_bool(computed_pos, false);
            ctx.write_bool(optional_pos, false);
            ctx.write_bool(readonly_pos, false);
            ctx.write_str(kind_pos, "method");
            ctx.write_ref(key_pos, key);
            ctx.write_refs(params_pos, params);
            ctx.write_maybe_ref(return_type_pos, return_type);

            item_pos
          }
          TsTypeElement::TsIndexSignature(sig) => {
            serialize_ts_index_sig(ctx, sig)
          }
        })
        .collect::<Vec<_>>();

      let body_pos =
        ctx.write_ts_interface_body(&node.body.span, body_elem_ids);
      ctx.write_ts_interface(
        &node.span,
        node.declare,
        ident_id,
        type_param,
        extend_ids,
        body_pos,
      )
    }
    Decl::TsTypeAlias(node) => {
      let ident = serialize_ident(ctx, &node.id);
      let type_ann = serialize_ts_type(ctx, &node.type_ann);
      let type_param = maybe_serialize_ts_type_param(ctx, &node.type_params);

      ctx.write_ts_type_alias(
        &node.span,
        node.declare,
        ident,
        type_param,
        type_ann,
      )
    }
    Decl::TsEnum(node) => {
      let raw = ctx.header(AstNode::TSEnumDeclaration, parent, &node.span);
      let declare_pos = ctx.bool_field(AstProp::Declare);
      let const_pos = ctx.bool_field(AstProp::Const);
      let id_pos = ctx.ref_field(AstProp::Id);
      let body_pos = ctx.ref_field(AstProp::Body);
      let pos = ctx.commit_schema(raw);

      let body_raw = ctx.header(AstNode::TSEnumBody, pos, &node.span);
      let members_pos = ctx.ref_vec_field(AstProp::Members, node.members.len());
      let body = ctx.commit_schema(body_raw);

      let ident_id = serialize_ident(ctx, &node.id);

      let members = node
        .members
        .iter()
        .map(|member| {
          let raw = ctx.header(AstNode::TSEnumMember, body, &member.span);
          let id_pos = ctx.ref_field(AstProp::Id);
          let init_pos = ctx.ref_field(AstProp::Initializer);
          let member_id = ctx.commit_schema(raw);

          let ident = match &member.id {
            TsEnumMemberId::Ident(ident) => serialize_ident(ctx, ident),
            TsEnumMemberId::Str(lit_str) => {
              serialize_lit(ctx, &Lit::Str(lit_str.clone()))
            }
          };

          let init = member.init.as_ref().map(|init| serialize_expr(ctx, init));

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
      let raw = ctx.header(AstNode::TsModule, parent, &ts_module_decl.span);
      ctx.commit_schema(raw)
    }
  }
}

fn serialize_ts_index_sig(
  ctx: &mut TsEsTreeBuilder,
  node: &TsIndexSignature,
  parent: NodeRef,
) -> NodeRef {
  let raw = ctx.header(AstNode::TSMethodSignature, parent, &node.span);
  let readonly_pos = ctx.bool_field(AstProp::Readonly);
  // TODO: where is this coming from?
  let static_pos = ctx.bool_field(AstProp::Static);
  let params_pos = ctx.ref_vec_field(AstProp::Params, node.params.len());
  let type_ann_pos = ctx.ref_field(AstProp::TypeAnnotation);
  let pos = ctx.commit_schema(raw);

  let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann);

  let params = node
    .params
    .iter()
    .map(|param| serialize_ts_fn_param(ctx, param))
    .collect::<Vec<_>>();

  ctx.write_bool(readonly_pos, false);
  ctx.write_bool(static_pos, node.is_static);
  ctx.write_refs(params_pos, params);
  ctx.write_maybe_ref(type_ann_pos, type_ann);

  pos
}

fn accessibility_to_str(accessibility: Accessibility) -> String {
  match accessibility {
    Accessibility::Public => "public".to_string(),
    Accessibility::Protected => "protected".to_string(),
    Accessibility::Private => "private".to_string(),
  }
}

fn serialize_private_name(
  ctx: &mut TsEsTreeBuilder,
  node: &PrivateName,
) -> NodeRef {
  ctx.write_private_identifier(&node.span, node.name.as_str())
}

fn serialize_jsx_element(
  ctx: &mut TsEsTreeBuilder,
  node: &JSXElement,
) -> NodeRef {
  let open = serialize_jsx_opening_element(ctx, &node.opening);

  let close = node.closing.as_ref().map(|closing| {
    let name = serialize_jsx_element_name(ctx, &closing.name);
    ctx.write_jsx_closing_elem(&closing.span, name)
  });

  let children = serialize_jsx_children(ctx, &node.children);

  ctx.write_jsx_elem(&node.span, open, close, children)
}

fn serialize_jsx_fragment(
  ctx: &mut TsEsTreeBuilder,
  node: &JSXFragment,
) -> NodeRef {
  let opening = ctx.write_jsx_opening_frag(&node.opening.span);
  let closing = ctx.write_jsx_closing_frag(&node.closing.span);
  let children = serialize_jsx_children(ctx, &node.children);

  ctx.write_jsx_frag(&node.span, opening, closing, children)
}

fn serialize_jsx_children(
  ctx: &mut TsEsTreeBuilder,
  children: &[JSXElementChild],
) -> Vec<NodeRef> {
  children
    .iter()
    .map(|child| {
      match child {
        JSXElementChild::JSXText(text) => {
          ctx.write_jsx_text(&text.span, &text.raw, &text.value)
        }
        JSXElementChild::JSXExprContainer(container) => {
          serialize_jsx_container_expr(ctx, container)
        }
        JSXElementChild::JSXElement(el) => serialize_jsx_element(ctx, el),
        JSXElementChild::JSXFragment(frag) => serialize_jsx_fragment(ctx, frag),
        // No parser supports this
        JSXElementChild::JSXSpreadChild(_) => unreachable!(),
      }
    })
    .collect::<Vec<_>>()
}

fn serialize_jsx_member_expr(
  ctx: &mut TsEsTreeBuilder,
  node: &JSXMemberExpr,
) -> NodeRef {
  let obj = match &node.obj {
    JSXObject::JSXMemberExpr(member) => serialize_jsx_member_expr(ctx, member),
    JSXObject::Ident(ident) => serialize_jsx_identifier(ctx, ident),
  };

  let prop = serialize_ident_name_as_jsx_identifier(ctx, &node.prop);

  ctx.write_jsx_member_expr(&node.span, obj, prop)
}

fn serialize_jsx_element_name(
  ctx: &mut TsEsTreeBuilder,
  node: &JSXElementName,
) -> NodeRef {
  match &node {
    JSXElementName::Ident(ident) => serialize_jsx_identifier(ctx, ident),
    JSXElementName::JSXMemberExpr(member) => {
      serialize_jsx_member_expr(ctx, member)
    }
    JSXElementName::JSXNamespacedName(ns) => {
      serialize_jsx_namespaced_name(ctx, ns)
    }
  }
}

fn serialize_jsx_opening_element(
  ctx: &mut TsEsTreeBuilder,
  node: &JSXOpeningElement,
) -> NodeRef {
  let name = serialize_jsx_element_name(ctx, &node.name);

  // FIXME: type args

  let attrs = node
    .attrs
    .iter()
    .map(|attr| match attr {
      JSXAttrOrSpread::JSXAttr(attr) => {
        let name = match &attr.name {
          JSXAttrName::Ident(name) => {
            serialize_ident_name_as_jsx_identifier(ctx, name)
          }
          JSXAttrName::JSXNamespacedName(node) => {
            serialize_jsx_namespaced_name(ctx, node)
          }
        };

        let value = attr.value.as_ref().map(|value| match value {
          JSXAttrValue::Lit(lit) => serialize_lit(ctx, lit),
          JSXAttrValue::JSXExprContainer(container) => {
            serialize_jsx_container_expr(ctx, container)
          }
          JSXAttrValue::JSXElement(el) => serialize_jsx_element(ctx, el),
          JSXAttrValue::JSXFragment(frag) => serialize_jsx_fragment(ctx, frag),
        });

        ctx.write_jsx_attr(&attr.span, name, value)
      }
      JSXAttrOrSpread::SpreadElement(spread) => {
        let arg = serialize_expr(ctx, &spread.expr);
        ctx.write_jsx_spread_attr(&spread.dot3_token, arg)
      }
    })
    .collect::<Vec<_>>();

  ctx.write_jsx_opening_elem(&node.span, node.self_closing, name, attrs)
}

fn serialize_jsx_container_expr(
  ctx: &mut TsEsTreeBuilder,
  node: &JSXExprContainer,
) -> NodeRef {
  let expr = match &node.expr {
    JSXExpr::JSXEmptyExpr(expr) => ctx.write_jsx_empty_expr(&expr.span),
    JSXExpr::Expr(expr) => serialize_expr(ctx, expr),
  };

  ctx.write_jsx_expr_container(&node.span, expr)
}

fn serialize_jsx_namespaced_name(
  ctx: &mut TsEsTreeBuilder,
  node: &JSXNamespacedName,
  parent: NodeRef,
) -> NodeRef {
  let ns = ctx.write_jsx_identifier(&node.ns.span, &node.ns.sym);
  let name = ctx.write_jsx_identifier(&node.name.span, &node.name.sym);

  ctx.write_jsx_namespaced_name(&node.span, ns, name)
}

fn serialize_ident_name_as_jsx_identifier(
  ctx: &mut TsEsTreeBuilder,
  node: &IdentName,
) -> NodeRef {
  ctx.write_jsx_identifier(&node.span, &node.sym)
}

fn serialize_jsx_identifier(
  ctx: &mut TsEsTreeBuilder,
  node: &Ident,
) -> NodeRef {
  ctx.write_jsx_identifier(&node.span, &node.sym)
}

fn serialize_pat(ctx: &mut TsEsTreeBuilder, pat: &Pat) -> NodeRef {
  match pat {
    Pat::Ident(node) => serialize_ident(ctx, &node.id),
    Pat::Array(node) => {
      let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann);

      let children = node
        .elems
        .iter()
        .map(|pat| pat.as_ref().map_or(NodeRef(0), |v| serialize_pat(ctx, v)))
        .collect::<Vec<_>>();

      ctx.write_arr_pat(&node.span, node.optional, type_ann, children)
    }
    Pat::Rest(node) => {
      let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann);
      let arg = serialize_pat(ctx, &node.arg);

      ctx.write_rest_elem(&node.span, type_ann, arg)
    }
    Pat::Object(node) => {
      let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann);

      let children = node
        .props
        .iter()
        .map(|prop| match prop {
          ObjectPatProp::KeyValue(key_value_prop) => {
            let computed = matches!(key_value_prop.key, PropName::Computed(_));

            let key = serialize_prop_name(ctx, &key_value_prop.key);
            let value = serialize_pat(ctx, key_value_prop.value.as_ref());

            ctx.write_property(
              &key_value_prop.span(),
              false,
              computed,
              false,
              "init",
              key,
              value,
            )
          }
          ObjectPatProp::Assign(assign_pat_prop) => {
            let ident = serialize_ident(ctx, &assign_pat_prop.key.id);

            // TODO(@marvinhagemeister): This seems wrong

            let value = assign_pat_prop
              .value
              .as_ref()
              .map_or(NodeRef(0), |value| serialize_expr(ctx, value));

            ctx.write_property(
              &assign_pat_prop.span,
              false,
              false,
              false,
              "init",
              ident,
              value,
            )
          }
          ObjectPatProp::Rest(rest_pat) => {
            serialize_pat(ctx, &Pat::Rest(rest_pat.clone()))
          }
        })
        .collect::<Vec<_>>();

      ctx.write_obj_pat(&node.span, node.optional, type_ann, children)
    }
    Pat::Assign(node) => {
      let left = serialize_pat(ctx, &node.left);
      let right = serialize_expr(ctx, &node.right);

      ctx.write_assign_pat(&node.span, left, right)
    }
    Pat::Invalid(_) => unreachable!(),
    Pat::Expr(node) => serialize_expr(ctx, node),
  }
}

fn serialize_for_head(
  ctx: &mut TsEsTreeBuilder,
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
    ForHead::Pat(pat) => serialize_pat(ctx, pat),
  }
}

fn serialize_spread(
  ctx: &mut TsEsTreeBuilder,
  expr: &Expr,
  span: &Span,
) -> NodeRef {
  let expr = serialize_expr(ctx, expr);
  ctx.write_spread(span, expr)
}

fn serialize_ident_name(
  ctx: &mut TsEsTreeBuilder,
  ident_name: &IdentName,
) -> NodeRef {
  ctx.write_identifier(&ident_name.span, ident_name.sym.as_str(), false, None)
}

fn serialize_prop_name(
  ctx: &mut TsEsTreeBuilder,
  prop_name: &PropName,
  parent: NodeRef,
) -> NodeRef {
  match prop_name {
    PropName::Ident(ident_name) => serialize_ident_name(ctx, ident_name),
    PropName::Str(str_prop) => serialize_lit(ctx, &Lit::Str(str_prop.clone())),
    PropName::Num(number) => serialize_lit(ctx, &Lit::Num(number.clone())),
    PropName::Computed(node) => serialize_expr(ctx, &node.expr, parent),
    PropName::BigInt(big_int) => {
      serialize_lit(ctx, &Lit::BigInt(big_int.clone()))
    }
  }
}

fn serialize_lit(ctx: &mut TsEsTreeBuilder, lit: &Lit) -> NodeRef {
  match lit {
    Lit::Str(node) => {
      let raw_value = if let Some(v) = &node.raw {
        v.to_string()
      } else {
        format!("{}", node.value).to_string()
      };

      ctx.write_str_lit(&node.span, &node.value, &raw_value)
    }
    Lit::Bool(node) => ctx.write_bool_lit(&node.span, node.value),
    Lit::Null(node) => ctx.write_null_lit(&node.span),
    Lit::Num(node) => {
      let raw_value = if let Some(v) = &node.raw {
        v.to_string()
      } else {
        format!("{}", node.value).to_string()
      };

      let value = node.raw.as_ref().unwrap();
      ctx.write_num_lit(&node.span, value, &raw_value)
    }
    Lit::BigInt(node) => {
      let raw_bigint_value = if let Some(v) = &node.raw {
        let mut s = v.to_string();
        s.pop();
        s.to_string()
      } else {
        format!("{}", node.value).to_string()
      };

      let raw_value = if let Some(v) = &node.raw {
        v.to_string()
      } else {
        format!("{}", node.value).to_string()
      };

      ctx.write_bigint_lit(
        &node.span,
        &node.value.to_string(),
        &raw_value,
        &raw_bigint_value,
      )
    }
    Lit::Regex(node) => {
      let raw = format!("/{}/{}", node.exp.as_str(), node.flags.as_str());

      ctx.write_regex_lit(
        &node.span,
        node.exp.as_str(),
        node.flags.as_str(),
        &raw,
        &raw,
      )
    }
    Lit::JSXText(node) => ctx.write_jsx_text(node.span, &node.raw, &node.value),
  }
}

fn serialize_ts_param_inst(
  ctx: &mut TsEsTreeBuilder,
  node: &TsTypeParamInstantiation,
  parent: NodeRef,
) -> NodeRef {
  let raw =
    ctx.header(AstNode::TSTypeParameterInstantiation, parent, &node.span);
  let params_pos = ctx.ref_vec_field(AstProp::Params, node.params.len());
  let pos = ctx.commit_schema(raw);

  let params = node
    .params
    .iter()
    .map(|param| serialize_ts_type(ctx, param, pos))
    .collect::<Vec<_>>();

  ctx.write_refs(params_pos, params);

  pos
}

fn serialize_ts_type(ctx: &mut TsEsTreeBuilder, node: &TsType) -> NodeRef {
  match node {
    TsType::TsKeywordType(node) => {
      let kind = match node.kind {
        TsKeywordTypeKind::TsAnyKeyword => TsKeywordKind::Any,
        TsKeywordTypeKind::TsUnknownKeyword => TsKeywordKind::Unknown,
        TsKeywordTypeKind::TsNumberKeyword => TsKeywordKind::Number,
        TsKeywordTypeKind::TsObjectKeyword => TsKeywordKind::Object,
        TsKeywordTypeKind::TsBooleanKeyword => TsKeywordKind::Boolean,
        TsKeywordTypeKind::TsBigIntKeyword => TsKeywordKind::BigInt,
        TsKeywordTypeKind::TsStringKeyword => TsKeywordKind::String,
        TsKeywordTypeKind::TsSymbolKeyword => TsKeywordKind::Symbol,
        TsKeywordTypeKind::TsVoidKeyword => TsKeywordKind::Void,
        TsKeywordTypeKind::TsUndefinedKeyword => TsKeywordKind::Undefined,
        TsKeywordTypeKind::TsNullKeyword => TsKeywordKind::Null,
        TsKeywordTypeKind::TsNeverKeyword => TsKeywordKind::Never,
        TsKeywordTypeKind::TsIntrinsicKeyword => TsKeywordKind::Intrinsic,
      };

      ctx.write_ts_keyword(kind, &node.span)
    }
    TsType::TsThisType(node) => ctx.write_ts_this_type(&node.span),
    TsType::TsFnOrConstructorType(node) => match node {
      TsFnOrConstructorType::TsFnType(node) => {
        let raw = ctx.header(AstNode::TSFunctionType, parent, &node.span);
        let params_pos = ctx.ref_vec_field(AstProp::Params, node.params.len());
        let pos = ctx.commit_schema(raw);

        let param_ids = node
          .params
          .iter()
          .map(|param| serialize_ts_fn_param(ctx, param))
          .collect::<Vec<_>>();

        ctx.write_refs(params_pos, param_ids);

        pos
      }
      TsFnOrConstructorType::TsConstructorType(_) => {
        todo!()
      }
    },
    TsType::TsTypeRef(node) => {
      let name = serialize_ts_entity_name(ctx, &node.type_name);

      let type_args = node
        .type_params
        .clone()
        .map(|param| serialize_ts_param_inst(ctx, &param));

      ctx.write_ts_type_ref(&node.span, name, type_args)
    }
    TsType::TsTypeQuery(node) => {
      let expr_name = match &node.expr_name {
        TsTypeQueryExpr::TsEntityName(entity) => {
          serialize_ts_entity_name(ctx, entity)
        }
        TsTypeQueryExpr::Import(child) => {
          serialize_ts_type(ctx, &TsType::TsImportType(child.clone()))
        }
      };

      let type_args = node
        .type_args
        .clone()
        .map(|param| serialize_ts_param_inst(ctx, &param));

      ctx.write_ts_type_query(&node.span, expr_name, type_args)
    }
    TsType::TsTypeLit(_) => {
      // TODO: Not sure what this is
      todo!()
    }
    TsType::TsArrayType(node) => {
      let elem = serialize_ts_type(ctx, &node.elem_type);
      ctx.write_ts_array_type(&node.span, elem)
    }
    TsType::TsTupleType(node) => {
      let children = node
        .elem_types
        .iter()
        .map(|elem| {
          if let Some(label) = &elem.label {
            let label = serialize_pat(ctx, label);
            let type_id = serialize_ts_type(ctx, elem.ty.as_ref());

            ctx.write_ts_named_tuple_member(&elem.span, label, type_id)
          } else {
            serialize_ts_type(ctx, elem.ty.as_ref())
          }
        })
        .collect::<Vec<_>>();

      ctx.write_ts_tuple_type(&node.span, children)
    }
    TsType::TsOptionalType(_) => todo!(),
    TsType::TsRestType(node) => {
      let type_ann = serialize_ts_type(ctx, &node.type_ann);
      ctx.write_ts_rest_type(&node.span, type_ann)
    }
    TsType::TsUnionOrIntersectionType(node) => match node {
      TsUnionOrIntersectionType::TsUnionType(node) => {
        let children = node
          .types
          .iter()
          .map(|item| serialize_ts_type(ctx, item))
          .collect::<Vec<_>>();

        ctx.write_ts_union_type(&node.span, children)
      }
      TsUnionOrIntersectionType::TsIntersectionType(node) => {
        let children = node
          .types
          .iter()
          .map(|item| serialize_ts_type(ctx, item))
          .collect::<Vec<_>>();

        ctx.write_ts_intersection_type(&node.span, children)
      }
    },
    TsType::TsConditionalType(node) => {
      let check = serialize_ts_type(ctx, &node.check_type);
      let extends = serialize_ts_type(ctx, &node.extends_type);
      let v_true = serialize_ts_type(ctx, &node.true_type);
      let v_false = serialize_ts_type(ctx, &node.false_type);

      ctx.write_ts_conditional_type(&node.span, check, extends, v_true, v_false)
    }
    TsType::TsInferType(node) => {
      let param = serialize_ts_type_param(ctx, &node.type_param);
      ctx.write_ts_infer_type(&node.span, param)
    }
    TsType::TsParenthesizedType(_) => todo!(),
    TsType::TsTypeOperator(node) => {
      let type_ann = serialize_ts_type(ctx, &node.type_ann);

      let op = match node.op {
        TsTypeOperatorOp::KeyOf => "keyof",
        TsTypeOperatorOp::Unique => "unique",
        TsTypeOperatorOp::ReadOnly => "readonly",
      };

      ctx.write_ts_type_op(&node.span, op, type_ann)
    }
    TsType::TsIndexedAccessType(node) => {
      let index = serialize_ts_type(ctx, &node.index_type);
      let obj = serialize_ts_type(ctx, &node.obj_type);

      ctx.write_ts_indexed_access_type(&node.span, index, obj)
    }
    TsType::TsMappedType(node) => {
      let opt_pos =
        create_true_plus_minus_field(ctx, AstProp::Optional, node.optional);
      let readonly_pos =
        create_true_plus_minus_field(ctx, AstProp::Readonly, node.readonly);

      let name = maybe_serialize_ts_type(ctx, &node.name_type);
      let type_ann = maybe_serialize_ts_type(ctx, &node.type_ann);
      let type_param = serialize_ts_type_param(ctx, &node.type_param);

      // FIXME: true plus minus
      write_true_plus_minus(ctx, opt_pos, node.optional);
      write_true_plus_minus(ctx, readonly_pos, node.readonly);

      ctx.write_ts_mapped_type(&node.span, name, type_ann, type_param)
    }
    TsType::TsLitType(node) => serialize_ts_lit_type(ctx, node),
    TsType::TsTypePredicate(node) => {
      let raw = ctx.header(AstNode::TSTypePredicate, parent, &node.span);
      let asserts_pos = ctx.bool_field(AstProp::Asserts);
      let param_name_pos = ctx.ref_field(AstProp::ParameterName);
      let type_ann_pos = ctx.ref_field(AstProp::TypeAnnotation);
      let pos = ctx.commit_schema(raw);

      let param_name = match &node.param_name {
        TsThisTypeOrIdent::TsThisType(node) => {
          ctx.write_ts_this_type(&node.span)
        }
        TsThisTypeOrIdent::Ident(ident) => serialize_ident(ctx, ident),
      };

      let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann);

      ctx.write_bool(asserts_pos, node.asserts);
      ctx.write_ref(param_name_pos, param_name);
      ctx.write_maybe_ref(type_ann_pos, type_ann);

      pos
    }
    TsType::TsImportType(node) => {
      let raw = ctx.header(AstNode::TSTypePredicate, parent, &node.span);
      let arg_pos = ctx.ref_field(AstProp::Argument);
      let type_args_pos = ctx.ref_field(AstProp::TypeArguments);
      let qualifier_pos = ctx.ref_field(AstProp::Qualifier);
      let pos = ctx.commit_schema(raw);

      let arg = serialize_ts_lit_type(
        ctx,
        &TsLitType {
          lit: TsLit::Str(node.arg.clone()),
          span: node.arg.span,
        },
        pos,
      );

      let type_arg = node.type_args.clone().map(|param_node| {
        serialize_ts_param_inst(ctx, param_node.as_ref(), pos)
      });

      let qualifier = node
        .qualifier
        .clone()
        .map_or(NodeRef(0), |quali| serialize_ts_entity_name(ctx, &quali));

      ctx.write_ref(arg_pos, arg);
      ctx.write_ref(qualifier_pos, qualifier);
      ctx.write_maybe_ref(type_args_pos, type_arg);

      pos
    }
  }
}

fn serialize_ts_lit_type(
  ctx: &mut TsEsTreeBuilder,
  node: &TsLitType,
  parent: NodeRef,
) -> NodeRef {
  let lit = match &node.lit {
    TsLit::Number(lit) => serialize_lit(ctx, &Lit::Num(lit.clone())),
    TsLit::Str(lit) => serialize_lit(ctx, &Lit::Str(lit.clone())),
    TsLit::Bool(lit) => serialize_lit(ctx, &Lit::Bool(*lit)),
    TsLit::BigInt(lit) => serialize_lit(ctx, &Lit::BigInt(lit.clone())),
    TsLit::Tpl(lit) => serialize_expr(
      ctx,
      &Expr::Tpl(Tpl {
        span: lit.span,
        exprs: vec![],
        quasis: lit.quasis.clone(),
      }),
    ),
  };

  ctx.write_ts_lit_type(&node.span, lit)
}

fn create_true_plus_minus_field(
  ctx: &mut TsEsTreeBuilder,
  prop: AstProp,
  value: Option<TruePlusMinus>,
) -> NodePos {
  if let Some(v) = value {
    match v {
      TruePlusMinus::True => NodePos::Bool(ctx.bool_field(prop)),
      TruePlusMinus::Plus | TruePlusMinus::Minus => {
        NodePos::Str(ctx.str_field(prop))
      }
    }
  } else {
    NodePos::Undef(ctx.undefined_field(prop))
  }
}

fn extract_pos(pos: NodePos) -> usize {
  match pos {
    NodePos::Bool(bool_pos) => bool_pos.0,
    NodePos::Field(field_pos) => field_pos.0,
    NodePos::FieldArr(field_arr_pos) => field_arr_pos.0,
    NodePos::Str(str_pos) => str_pos.0,
    NodePos::Undef(undef_pos) => undef_pos.0,
    NodePos::Null(null_pos) => null_pos.0,
    NodePos::Num(num_pos) => num_pos.0,
    NodePos::Obj(obj_pos) => obj_pos.0,
    NodePos::Regex(reg_pos) => reg_pos.0,
  }
}

fn write_true_plus_minus(
  ctx: &mut TsEsTreeBuilder,
  pos: NodePos,
  value: Option<TruePlusMinus>,
) {
  if let Some(v) = value {
    match v {
      TruePlusMinus::True => {
        let bool_pos = BoolPos(extract_pos(pos));
        ctx.write_bool(bool_pos, true);
      }
      TruePlusMinus::Plus => {
        let str_pos = StrPos(extract_pos(pos));
        ctx.write_str(str_pos, "+")
      }
      TruePlusMinus::Minus => {
        let str_pos = StrPos(extract_pos(pos));
        ctx.write_str(str_pos, "-")
      }
    }
  }
}

fn serialize_ts_entity_name(
  ctx: &mut TsEsTreeBuilder,
  node: &TsEntityName,
) -> NodeRef {
  match &node {
    TsEntityName::TsQualifiedName(_) => todo!(),
    TsEntityName::Ident(ident) => serialize_ident(ctx, ident),
  }
}

fn maybe_serialize_ts_type_ann(
  ctx: &mut TsEsTreeBuilder,
  node: &Option<Box<TsTypeAnn>>,
) -> Option<NodeRef> {
  node
    .as_ref()
    .map(|type_ann| serialize_ts_type_ann(ctx, type_ann))
}

fn serialize_ts_type_ann(
  ctx: &mut TsEsTreeBuilder,
  node: &TsTypeAnn,
  parent: NodeRef,
) -> NodeRef {
  let v_type = serialize_ts_type(ctx, &node.type_ann);
  ctx.write_ts_type_ann(&node.span, v_type)
}

fn maybe_serialize_ts_type(
  ctx: &mut TsEsTreeBuilder,
  node: &Option<Box<TsType>>,
) -> Option<NodeRef> {
  node.as_ref().map(|item| serialize_ts_type(ctx, item))
}

fn serialize_ts_type_param(
  ctx: &mut TsEsTreeBuilder,
  node: &TsTypeParam,
  parent: NodeRef,
) -> NodeRef {
  let raw = ctx.header(AstNode::TSTypeParameter, parent, &node.span);
  let name_pos = ctx.ref_field(AstProp::Name);
  let constraint_pos = ctx.ref_field(AstProp::Constraint);
  let default_pos = ctx.ref_field(AstProp::Default);
  let const_pos = ctx.bool_field(AstProp::Const);
  let in_pos = ctx.bool_field(AstProp::In);
  let out_pos = ctx.bool_field(AstProp::Out);
  let pos = ctx.commit_schema(raw);

  let name = serialize_ident(ctx, &node.name);
  let constraint = maybe_serialize_ts_type(ctx, &node.constraint);
  let default = maybe_serialize_ts_type(ctx, &node.default);

  ctx.write_bool(const_pos, node.is_const);
  ctx.write_bool(in_pos, node.is_in);
  ctx.write_bool(out_pos, node.is_out);
  ctx.write_ref(name_pos, name);
  ctx.write_maybe_ref(constraint_pos, constraint);
  ctx.write_maybe_ref(default_pos, default);

  pos
}

fn maybe_serialize_ts_type_param(
  ctx: &mut TsEsTreeBuilder,
  node: &Option<Box<TsTypeParamDecl>>,
  parent: NodeRef,
) -> Option<NodeRef> {
  node.as_ref().map(|node| {
    let raw =
      ctx.header(AstNode::TSTypeParameterDeclaration, parent, &node.span);
    let params_pos = ctx.ref_vec_field(AstProp::Params, node.params.len());
    let pos = ctx.commit_schema(raw);

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
  ctx: &mut TsEsTreeBuilder,
  node: &TsFnParam,
) -> NodeRef {
  match node {
    TsFnParam::Ident(ident) => serialize_ident(ctx, ident),
    TsFnParam::Array(pat) => serialize_pat(ctx, &Pat::Array(pat.clone())),
    TsFnParam::Rest(pat) => serialize_pat(ctx, &Pat::Rest(pat.clone())),
    TsFnParam::Object(pat) => serialize_pat(ctx, &Pat::Object(pat.clone())),
  }
}
