// Copyright 2018-2025 the Deno authors. MIT license.

use deno_ast::swc::ast::AssignTarget;
use deno_ast::swc::ast::AssignTargetPat;
use deno_ast::swc::ast::BindingIdent;
use deno_ast::swc::ast::BlockStmtOrExpr;
use deno_ast::swc::ast::Callee;
use deno_ast::swc::ast::ClassMember;
use deno_ast::swc::ast::Decl;
use deno_ast::swc::ast::Decorator;
use deno_ast::swc::ast::DefaultDecl;
use deno_ast::swc::ast::ExportSpecifier;
use deno_ast::swc::ast::Expr;
use deno_ast::swc::ast::ExprOrSpread;
use deno_ast::swc::ast::FnExpr;
use deno_ast::swc::ast::ForHead;
use deno_ast::swc::ast::Function;
use deno_ast::swc::ast::Ident;
use deno_ast::swc::ast::IdentName;
use deno_ast::swc::ast::ImportSpecifier;
use deno_ast::swc::ast::JSXAttrName;
use deno_ast::swc::ast::JSXAttrOrSpread;
use deno_ast::swc::ast::JSXAttrValue;
use deno_ast::swc::ast::JSXElement;
use deno_ast::swc::ast::JSXElementChild;
use deno_ast::swc::ast::JSXElementName;
use deno_ast::swc::ast::JSXExpr;
use deno_ast::swc::ast::JSXExprContainer;
use deno_ast::swc::ast::JSXFragment;
use deno_ast::swc::ast::JSXMemberExpr;
use deno_ast::swc::ast::JSXNamespacedName;
use deno_ast::swc::ast::JSXObject;
use deno_ast::swc::ast::JSXOpeningElement;
use deno_ast::swc::ast::Key;
use deno_ast::swc::ast::Lit;
use deno_ast::swc::ast::MemberExpr;
use deno_ast::swc::ast::MemberProp;
use deno_ast::swc::ast::ModuleDecl;
use deno_ast::swc::ast::ModuleExportName;
use deno_ast::swc::ast::ModuleItem;
use deno_ast::swc::ast::ObjectLit;
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
use deno_ast::swc::ast::TsEntityName;
use deno_ast::swc::ast::TsEnumMemberId;
use deno_ast::swc::ast::TsExprWithTypeArgs;
use deno_ast::swc::ast::TsFnOrConstructorType;
use deno_ast::swc::ast::TsFnParam;
use deno_ast::swc::ast::TsIndexSignature;
use deno_ast::swc::ast::TsLit;
use deno_ast::swc::ast::TsLitType;
use deno_ast::swc::ast::TsModuleName;
use deno_ast::swc::ast::TsModuleRef;
use deno_ast::swc::ast::TsNamespaceBody;
use deno_ast::swc::ast::TsParamPropParam;
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
use deno_ast::view::MetaPropKind;
use deno_ast::view::MethodKind;
use deno_ast::view::TsKeywordTypeKind;
use deno_ast::view::TsTypeOperatorOp;
use deno_ast::view::UnaryOp;
use deno_ast::view::UpdateOp;
use deno_ast::view::VarDeclKind;
use deno_ast::ParsedSource;

use super::buffer::AstBufSerializer;
use super::buffer::NodeRef;
use super::ts_estree::AstNode;
use super::ts_estree::MethodKind as TsEstreeMethodKind;
use super::ts_estree::PropertyKind;
use super::ts_estree::SourceKind;
use super::ts_estree::TsEsTreeBuilder;
use super::ts_estree::TsKeywordKind;
use super::ts_estree::TsModuleKind;
use crate::util::text_encoding::Utf16Map;

pub fn serialize_swc_to_buffer(
  parsed_source: &ParsedSource,
  utf16_map: &Utf16Map,
) -> Vec<u8> {
  let mut ctx = TsEsTreeBuilder::new();

  let program = &parsed_source.program();

  match program.as_ref() {
    Program::Module(module) => {
      let children = module
        .body
        .iter()
        .map(|item| match item {
          ModuleItem::ModuleDecl(module_decl) => {
            serialize_module_decl(&mut ctx, module_decl)
          }
          ModuleItem::Stmt(stmt) => serialize_stmt(&mut ctx, stmt),
        })
        .collect::<Vec<_>>();

      ctx.write_program(&module.span, SourceKind::Module, children);
    }
    Program::Script(script) => {
      let children = script
        .body
        .iter()
        .map(|stmt| serialize_stmt(&mut ctx, stmt))
        .collect::<Vec<_>>();

      ctx.write_program(&script.span, SourceKind::Script, children);
    }
  }

  ctx.map_utf8_spans_to_utf16(utf16_map);
  ctx.serialize()
}

fn serialize_module_decl(
  ctx: &mut TsEsTreeBuilder,
  module_decl: &ModuleDecl,
) -> NodeRef {
  match module_decl {
    ModuleDecl::Import(node) => {
      let src = serialize_lit(ctx, &Lit::Str(node.src.as_ref().clone()));
      let attrs = serialize_import_attrs(ctx, &node.with);

      let specifiers = node
        .specifiers
        .iter()
        .map(|spec| match spec {
          ImportSpecifier::Named(spec) => {
            let local = serialize_ident(ctx, &spec.local, None);
            let imported = spec
              .imported
              .as_ref()
              .map_or(serialize_ident(ctx, &spec.local, None), |v| {
                serialize_module_export_name(ctx, v)
              });
            ctx.write_import_spec(
              &spec.span,
              spec.is_type_only,
              local,
              imported,
            )
          }
          ImportSpecifier::Default(spec) => {
            let local = serialize_ident(ctx, &spec.local, None);
            ctx.write_import_default_spec(&spec.span, local)
          }
          ImportSpecifier::Namespace(spec) => {
            let local = serialize_ident(ctx, &spec.local, None);
            ctx.write_import_ns_spec(&spec.span, local)
          }
        })
        .collect::<Vec<_>>();

      ctx.write_import_decl(&node.span, node.type_only, src, specifiers, attrs)
    }
    ModuleDecl::ExportDecl(node) => {
      let is_type_only = match &node.decl {
        Decl::Class(_) => false,
        Decl::Fn(_) => false,
        Decl::Var(_) => false,
        Decl::Using(_) => false,
        Decl::TsInterface(_) => true,
        Decl::TsTypeAlias(_) => true,
        Decl::TsEnum(_) => true,
        Decl::TsModule(_) => true,
      };
      let decl = serialize_decl(ctx, &node.decl);

      ctx.write_export_named_decl(
        &node.span,
        is_type_only,
        vec![],
        None,
        vec![],
        Some(decl),
      )
    }
    ModuleDecl::ExportNamed(node) => {
      let attrs = serialize_import_attrs(ctx, &node.with);
      let source = node
        .src
        .as_ref()
        .map(|src| serialize_lit(ctx, &Lit::Str(*src.clone())));

      if let Some(ExportSpecifier::Namespace(ns)) = node.specifiers.first() {
        let exported = serialize_module_export_name(ctx, &ns.name);
        ctx.write_export_all_decl(
          &node.span,
          node.type_only,
          // Namespaced export must always have a source, so this
          // scenario where it's optional can't happen. I think
          // it's just the way SWC stores things internally, since they
          // don't have a dedicated node for namespace exports.
          source.unwrap_or(NodeRef(0)),
          Some(exported),
          attrs,
        )
      } else {
        let specifiers = node
          .specifiers
          .iter()
          .map(|spec| {
            match spec {
              ExportSpecifier::Named(spec) => {
                let local = serialize_module_export_name(ctx, &spec.orig);

                let exported = spec.exported.as_ref().map_or(
                  serialize_module_export_name(ctx, &spec.orig),
                  |exported| serialize_module_export_name(ctx, exported),
                );

                ctx.write_export_spec(
                  &spec.span,
                  spec.is_type_only,
                  local,
                  exported,
                )
              }

              // Already handled earlier
              ExportSpecifier::Namespace(_) => unreachable!(),
              // this is not syntactically valid
              ExportSpecifier::Default(_) => {
                // Ignore syntax errors
                NodeRef(0)
              }
            }
          })
          .collect::<Vec<_>>();

        ctx.write_export_named_decl(
          &node.span,
          node.type_only,
          specifiers,
          source,
          attrs,
          None,
        )
      }
    }
    ModuleDecl::ExportDefaultDecl(node) => {
      let (is_type_only, decl) = match &node.decl {
        DefaultDecl::Class(node) => {
          let ident = node
            .ident
            .as_ref()
            .map(|ident| serialize_ident(ctx, ident, None));

          let super_class = node
            .class
            .super_class
            .as_ref()
            .map(|expr| serialize_expr(ctx, expr.as_ref()));

          let implements = node
            .class
            .implements
            .iter()
            .map(|item| serialize_ts_expr_with_type_args(ctx, item))
            .collect::<Vec<_>>();

          let members = node
            .class
            .body
            .iter()
            .filter_map(|member| serialize_class_member(ctx, member))
            .collect::<Vec<_>>();

          let body = ctx.write_class_body(&node.class.span, members);

          let decorators = node
            .class
            .decorators
            .iter()
            .map(|deco| serialize_decorator(ctx, deco))
            .collect::<Vec<_>>();

          let decl = ctx.write_class_decl(
            &node.class.span,
            false,
            node.class.is_abstract,
            ident,
            super_class,
            implements,
            body,
            decorators,
          );

          (false, decl)
        }
        DefaultDecl::Fn(node) => {
          let ident = node
            .ident
            .as_ref()
            .map(|ident| serialize_ident(ctx, ident, None));

          let fn_obj = node.function.as_ref();

          let type_params =
            maybe_serialize_ts_type_param_decl(ctx, &fn_obj.type_params);

          let params = fn_obj
            .params
            .iter()
            .map(|param| {
              let decorators = param
                .decorators
                .iter()
                .map(|deco| serialize_decorator(ctx, deco))
                .collect::<Vec<_>>();

              serialize_pat(ctx, &param.pat, Some(decorators))
            })
            .collect::<Vec<_>>();

          let return_type =
            maybe_serialize_ts_type_ann(ctx, &fn_obj.return_type);
          let body = fn_obj
            .body
            .as_ref()
            .map(|block| serialize_stmt(ctx, &Stmt::Block(block.clone())));

          let decl = if let Some(body) = body {
            ctx.write_fn_decl(
              &fn_obj.span,
              false,
              fn_obj.is_async,
              fn_obj.is_generator,
              ident,
              type_params,
              return_type,
              body,
              params,
            )
          } else {
            ctx.write_ts_decl_fn(
              &fn_obj.span,
              false,
              fn_obj.is_async,
              fn_obj.is_generator,
              ident,
              type_params,
              return_type,
              params,
            )
          };

          (false, decl)
        }
        DefaultDecl::TsInterfaceDecl(node) => {
          let ident_id = serialize_ident(ctx, &node.id, None);
          let type_param =
            maybe_serialize_ts_type_param_decl(ctx, &node.type_params);

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
            .map(|item| serialize_ts_type_elem(ctx, item))
            .collect::<Vec<_>>();

          let body_pos =
            ctx.write_ts_interface_body(&node.body.span, body_elem_ids);

          let decl = ctx.write_ts_interface_decl(
            &node.span,
            node.declare,
            ident_id,
            type_param,
            extend_ids,
            body_pos,
          );

          (true, decl)
        }
      };

      ctx.write_export_default_decl(&node.span, is_type_only, decl)
    }
    ModuleDecl::ExportDefaultExpr(node) => {
      let expr = serialize_expr(ctx, &node.expr);
      ctx.write_export_default_decl(&node.span, false, expr)
    }
    ModuleDecl::ExportAll(node) => {
      let src = serialize_lit(ctx, &Lit::Str(node.src.as_ref().clone()));
      let attrs = serialize_import_attrs(ctx, &node.with);

      ctx.write_export_all_decl(&node.span, node.type_only, src, None, attrs)
    }
    ModuleDecl::TsImportEquals(node) => {
      let ident = serialize_ident(ctx, &node.id, None);
      let module_ref = match &node.module_ref {
        TsModuleRef::TsEntityName(entity) => {
          serialize_ts_entity_name(ctx, entity)
        }
        TsModuleRef::TsExternalModuleRef(external) => {
          let expr = serialize_lit(ctx, &Lit::Str(external.expr.clone()));
          ctx.write_ts_external_mod_ref(&external.span, expr)
        }
      };

      ctx.write_export_ts_import_equals(
        &node.span,
        node.is_type_only,
        ident,
        module_ref,
      )
    }
    ModuleDecl::TsExportAssignment(node) => {
      let expr = serialize_expr(ctx, &node.expr);
      ctx.write_export_assign(&node.span, expr)
    }
    ModuleDecl::TsNamespaceExport(node) => {
      let decl = serialize_ident(ctx, &node.id, None);
      ctx.write_export_ts_namespace(&node.span, decl)
    }
  }
}

fn serialize_import_attrs(
  ctx: &mut TsEsTreeBuilder,
  raw_attrs: &Option<Box<ObjectLit>>,
) -> Vec<NodeRef> {
  raw_attrs.as_ref().map_or(vec![], |obj| {
    obj
      .props
      .iter()
      .map(|prop| {
        let (key, value) = match prop {
          // Invalid syntax
          PropOrSpread::Spread(_) => {
            // Ignore syntax errors
            (NodeRef(0), NodeRef(0))
          }
          PropOrSpread::Prop(prop) => {
            match prop.as_ref() {
              Prop::Shorthand(ident) => (
                serialize_ident(ctx, ident, None),
                serialize_ident(ctx, ident, None),
              ),
              Prop::KeyValue(kv) => (
                serialize_prop_name(ctx, &kv.key),
                serialize_expr(ctx, kv.value.as_ref()),
              ),
              // Invalid syntax
              Prop::Assign(_)
              | Prop::Getter(_)
              | Prop::Setter(_)
              | Prop::Method(_) => {
                // Ignore syntax errors
                (NodeRef(0), NodeRef(0))
              }
            }
          }
        };

        ctx.write_import_attr(&prop.span(), key, value)
      })
      .collect::<Vec<_>>()
  })
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
      let ident = serialize_ident(ctx, &node.label, None);
      let stmt = serialize_stmt(ctx, &node.body);

      ctx.write_labeled_stmt(&node.span, ident, stmt)
    }
    Stmt::Break(node) => {
      let arg = node
        .label
        .as_ref()
        .map(|label| serialize_ident(ctx, label, None));
      ctx.write_break_stmt(&node.span, arg)
    }
    Stmt::Continue(node) => {
      let arg = node
        .label
        .as_ref()
        .map(|label| serialize_ident(ctx, label, None));

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
        let param = catch
          .param
          .as_ref()
          .map(|param| serialize_pat(ctx, param, None));

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

      let ident = node
        .ident
        .as_ref()
        .map(|ident| serialize_ident(ctx, ident, None));

      let type_params =
        maybe_serialize_ts_type_param_decl(ctx, &fn_obj.type_params);

      let params = fn_obj
        .params
        .iter()
        .map(|param| {
          let decorators = param
            .decorators
            .iter()
            .map(|deco| serialize_decorator(ctx, deco))
            .collect::<Vec<_>>();

          serialize_pat(ctx, &param.pat, Some(decorators))
        })
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
        BinaryOp::NotEqEq => (AstNode::BinaryExpression, "!=="),
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
              serialize_binding_ident(ctx, target, None)
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
            SimpleAssignTarget::Invalid(_) => {
              // Ignore syntax errors
              NodeRef(0)
            }
          }
        }
        AssignTarget::Pat(target) => match target {
          AssignTargetPat::Array(array_pat) => {
            serialize_pat(ctx, &Pat::Array(array_pat.clone()), None)
          }
          AssignTargetPat::Object(object_pat) => {
            serialize_pat(ctx, &Pat::Object(object_pat.clone()), None)
          }
          AssignTargetPat::Invalid(_) => {
            // Ignore syntax errors
            NodeRef(0)
          }
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
          .map(|arg| serialize_expr_or_spread(ctx, arg));

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
    Expr::Ident(node) => serialize_ident(ctx, node, None),
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
      let type_param =
        maybe_serialize_ts_type_param_decl(ctx, &node.type_params);

      let params = node
        .params
        .iter()
        .map(|param| serialize_pat(ctx, param, None))
        .collect::<Vec<_>>();

      let body = match node.body.as_ref() {
        BlockStmtOrExpr::BlockStmt(block_stmt) => {
          serialize_stmt(ctx, &Stmt::Block(block_stmt.clone()))
        }
        BlockStmtOrExpr::Expr(expr) => serialize_expr(ctx, expr.as_ref()),
      };

      let return_type = maybe_serialize_ts_type_ann(ctx, &node.return_type);

      ctx.write_arrow_fn_expr(
        &node.span,
        node.is_async,
        node.is_generator,
        type_param,
        params,
        return_type,
        body,
      )
    }
    Expr::Class(node) => {
      let ident = node
        .ident
        .as_ref()
        .map(|ident| serialize_ident(ctx, ident, None));

      let type_params =
        maybe_serialize_ts_type_param_decl(ctx, &node.class.type_params);

      let super_class = node
        .class
        .super_class
        .as_ref()
        .map(|expr| serialize_expr(ctx, expr.as_ref()));

      let super_type_args = node
        .class
        .super_type_params
        .as_ref()
        .map(|param| serialize_ts_param_inst(ctx, param.as_ref()));

      let implements = node
        .class
        .implements
        .iter()
        .map(|item| serialize_ts_expr_with_type_args(ctx, item))
        .collect::<Vec<_>>();

      let members = node
        .class
        .body
        .iter()
        .filter_map(|member| serialize_class_member(ctx, member))
        .collect::<Vec<_>>();

      let body = ctx.write_class_body(&node.class.span, members);

      ctx.write_class_expr(
        &node.class.span,
        false,
        node.class.is_abstract,
        ident,
        super_class,
        super_type_args,
        type_params,
        implements,
        body,
      )
    }
    Expr::Yield(node) => {
      let arg = node
        .arg
        .as_ref()
        .map(|arg| serialize_expr(ctx, arg.as_ref()));

      ctx.write_yield_expr(&node.span, node.delegate, arg)
    }
    Expr::MetaProp(node) => {
      let (meta, prop) = match node.kind {
        MetaPropKind::NewTarget => (
          ctx.write_identifier(&node.span, "new", false, None, None),
          ctx.write_identifier(&node.span, "target", false, None, None),
        ),
        MetaPropKind::ImportMeta => (
          ctx.write_identifier(&node.span, "import", false, None, None),
          ctx.write_identifier(&node.span, "meta", false, None, None),
        ),
      };
      ctx.write_meta_prop(&node.span, meta, prop)
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
      let expr = serialize_expr(ctx, &node.expr);
      let type_ann = serialize_ts_type(ctx, &node.type_ann);

      ctx.write_ts_type_assertion(&node.span, expr, type_ann)
    }
    Expr::TsConstAssertion(node) => {
      let expr = serialize_expr(ctx, node.expr.as_ref());

      let type_name =
        ctx.write_identifier(&node.span, "const", false, None, None);
      let type_ann = ctx.write_ts_type_ref(&node.span, type_name, None);

      ctx.write_ts_as_expr(&node.span, expr, type_ann)
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
    Expr::TsInstantiation(node) => {
      let expr = serialize_expr(ctx, &node.expr);
      let type_args = serialize_ts_param_inst(ctx, node.type_args.as_ref());
      ctx.write_ts_inst_expr(&node.span, expr, type_args)
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
      // Ignore syntax errors
      NodeRef(0)
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
      let mut kind = PropertyKind::Init;

      let (key, value) = match prop.as_ref() {
        Prop::Shorthand(ident) => {
          shorthand = true;

          let value = serialize_ident(ctx, ident, None);
          let value2 = serialize_ident(ctx, ident, None);
          (value, value2)
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
          let left = serialize_ident(ctx, &assign_prop.key, None);
          let right = serialize_expr(ctx, assign_prop.value.as_ref());

          let child_pos =
            ctx.write_assign_pat(&assign_prop.span, left, right, None);

          (left, child_pos)
        }
        Prop::Getter(getter_prop) => {
          kind = PropertyKind::Get;

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
                type_params: None,
                return_type: getter_prop.type_ann.clone(),
              }),
            }),
          );

          (key, value)
        }
        Prop::Setter(setter_prop) => {
          kind = PropertyKind::Set;

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

fn serialize_ident(
  ctx: &mut TsEsTreeBuilder,
  ident: &Ident,
  type_ann: Option<NodeRef>,
) -> NodeRef {
  ctx.write_identifier(
    &ident.span,
    ident.sym.as_str(),
    ident.optional,
    type_ann,
    None,
  )
}

fn serialize_module_export_name(
  ctx: &mut TsEsTreeBuilder,
  name: &ModuleExportName,
) -> NodeRef {
  match &name {
    ModuleExportName::Ident(ident) => serialize_ident(ctx, ident, None),
    ModuleExportName::Str(lit) => serialize_lit(ctx, &Lit::Str(lit.clone())),
  }
}

fn serialize_decl(ctx: &mut TsEsTreeBuilder, decl: &Decl) -> NodeRef {
  match decl {
    Decl::Class(node) => {
      let ident = serialize_ident(ctx, &node.ident, None);

      let super_class = node
        .class
        .super_class
        .as_ref()
        .map(|expr| serialize_expr(ctx, expr.as_ref()));

      let implements = node
        .class
        .implements
        .iter()
        .map(|item| serialize_ts_expr_with_type_args(ctx, item))
        .collect::<Vec<_>>();

      let members = node
        .class
        .body
        .iter()
        .filter_map(|member| serialize_class_member(ctx, member))
        .collect::<Vec<_>>();

      let body = ctx.write_class_body(&node.class.span, members);

      let decorators = node
        .class
        .decorators
        .iter()
        .map(|deco| serialize_decorator(ctx, deco))
        .collect::<Vec<_>>();

      ctx.write_class_decl(
        &node.class.span,
        false,
        node.class.is_abstract,
        Some(ident),
        super_class,
        implements,
        body,
        decorators,
      )
    }
    Decl::Fn(node) => {
      let ident_id = serialize_ident(ctx, &node.ident, None);
      let type_param_id =
        maybe_serialize_ts_type_param_decl(ctx, &node.function.type_params);
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
        .map(|param| serialize_pat(ctx, &param.pat, None))
        .collect::<Vec<_>>();

      if let Some(body) = body {
        ctx.write_fn_decl(
          &node.function.span,
          node.declare,
          node.function.is_async,
          node.function.is_generator,
          Some(ident_id),
          type_param_id,
          return_type,
          body,
          params,
        )
      } else {
        ctx.write_ts_decl_fn(
          &node.function.span,
          node.declare,
          node.function.is_async,
          node.function.is_generator,
          Some(ident_id),
          type_param_id,
          return_type,
          params,
        )
      }
    }
    Decl::Var(node) => {
      let children = node
        .decls
        .iter()
        .map(|decl| {
          let ident = serialize_pat(ctx, &decl.name, None);
          let init = decl
            .init
            .as_ref()
            .map(|init| serialize_expr(ctx, init.as_ref()));

          ctx.write_var_declarator(&decl.span, ident, init, decl.definite)
        })
        .collect::<Vec<_>>();

      let kind = match node.kind {
        VarDeclKind::Var => "var",
        VarDeclKind::Let => "let",
        VarDeclKind::Const => "const",
      };

      ctx.write_var_decl(&node.span, node.declare, kind, children)
    }
    Decl::Using(node) => {
      let kind = if node.is_await {
        "await using"
      } else {
        "using"
      };

      let children = node
        .decls
        .iter()
        .map(|decl| {
          let ident = serialize_pat(ctx, &decl.name, None);
          let init = decl
            .init
            .as_ref()
            .map(|init| serialize_expr(ctx, init.as_ref()));

          ctx.write_var_declarator(&decl.span, ident, init, decl.definite)
        })
        .collect::<Vec<_>>();

      ctx.write_var_decl(&node.span, false, kind, children)
    }
    Decl::TsInterface(node) => {
      let ident_id = serialize_ident(ctx, &node.id, None);
      let type_param =
        maybe_serialize_ts_type_param_decl(ctx, &node.type_params);

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
        .map(|item| serialize_ts_type_elem(ctx, item))
        .collect::<Vec<_>>();

      let body_pos =
        ctx.write_ts_interface_body(&node.body.span, body_elem_ids);
      ctx.write_ts_interface_decl(
        &node.span,
        node.declare,
        ident_id,
        type_param,
        extend_ids,
        body_pos,
      )
    }
    Decl::TsTypeAlias(node) => {
      let ident = serialize_ident(ctx, &node.id, None);
      let type_ann = serialize_ts_type(ctx, &node.type_ann);
      let type_param =
        maybe_serialize_ts_type_param_decl(ctx, &node.type_params);

      ctx.write_ts_type_alias(
        &node.span,
        node.declare,
        ident,
        type_param,
        type_ann,
      )
    }
    Decl::TsEnum(node) => {
      let id = serialize_ident(ctx, &node.id, None);

      let members = node
        .members
        .iter()
        .map(|member| {
          let ident = match &member.id {
            TsEnumMemberId::Ident(ident) => serialize_ident(ctx, ident, None),
            TsEnumMemberId::Str(lit_str) => {
              serialize_lit(ctx, &Lit::Str(lit_str.clone()))
            }
          };

          let init = member.init.as_ref().map(|init| serialize_expr(ctx, init));

          ctx.write_ts_enum_member(&member.span, ident, init)
        })
        .collect::<Vec<_>>();

      let body = ctx.write_ts_enum_body(&node.span, members);
      ctx.write_ts_enum(&node.span, node.declare, node.is_const, id, body)
    }
    Decl::TsModule(node) => {
      let ident = match &node.id {
        TsModuleName::Ident(ident) => serialize_ident(ctx, ident, None),
        TsModuleName::Str(str_lit) => {
          serialize_lit(ctx, &Lit::Str(str_lit.clone()))
        }
      };

      let body = node
        .body
        .as_ref()
        .map(|body| serialize_ts_namespace_body(ctx, body));

      ctx.write_ts_module_decl(
        &node.span,
        node.declare,
        if node.global {
          TsModuleKind::Global
        } else {
          TsModuleKind::Module
        },
        ident,
        body,
      )
    }
  }
}

fn serialize_ts_namespace_body(
  ctx: &mut TsEsTreeBuilder,
  node: &TsNamespaceBody,
) -> NodeRef {
  match node {
    TsNamespaceBody::TsModuleBlock(mod_block) => {
      let items = mod_block
        .body
        .iter()
        .map(|item| match item {
          ModuleItem::ModuleDecl(decl) => serialize_module_decl(ctx, decl),
          ModuleItem::Stmt(stmt) => serialize_stmt(ctx, stmt),
        })
        .collect::<Vec<_>>();

      ctx.write_ts_module_block(&mod_block.span, items)
    }
    TsNamespaceBody::TsNamespaceDecl(node) => {
      let ident = serialize_ident(ctx, &node.id, None);
      let body = serialize_ts_namespace_body(ctx, &node.body);

      ctx.write_ts_module_decl(
        &node.span,
        node.declare,
        TsModuleKind::Namespace,
        ident,
        Some(body),
      )
    }
  }
}

fn serialize_ts_type_elem(
  ctx: &mut TsEsTreeBuilder,
  node: &TsTypeElement,
) -> NodeRef {
  match node {
    TsTypeElement::TsCallSignatureDecl(ts_call) => {
      let type_ann =
        maybe_serialize_ts_type_param_decl(ctx, &ts_call.type_params);
      let return_type = maybe_serialize_ts_type_ann(ctx, &ts_call.type_ann);
      let params = ts_call
        .params
        .iter()
        .map(|param| serialize_ts_fn_param(ctx, param))
        .collect::<Vec<_>>();

      ctx.write_ts_call_sig_decl(&ts_call.span, type_ann, params, return_type)
    }
    TsTypeElement::TsConstructSignatureDecl(sig) => {
      let type_params =
        maybe_serialize_ts_type_param_decl(ctx, &sig.type_params);

      let params = sig
        .params
        .iter()
        .map(|param| serialize_ts_fn_param(ctx, param))
        .collect::<Vec<_>>();

      // Must be present
      let return_type =
        maybe_serialize_ts_type_ann(ctx, &sig.type_ann).unwrap();

      ctx.write_ts_construct_sig(&sig.span, type_params, params, return_type)
    }
    TsTypeElement::TsPropertySignature(sig) => {
      let key = serialize_expr(ctx, &sig.key);
      let type_ann = maybe_serialize_ts_type_ann(ctx, &sig.type_ann);

      ctx.write_ts_property_sig(
        &sig.span,
        sig.computed,
        sig.optional,
        sig.readonly,
        key,
        type_ann,
      )
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
      let key = serialize_expr(ctx, &sig.key);
      let type_parms =
        maybe_serialize_ts_type_param_decl(ctx, &sig.type_params);
      let params = sig
        .params
        .iter()
        .map(|param| serialize_ts_fn_param(ctx, param))
        .collect::<Vec<_>>();
      let return_type = maybe_serialize_ts_type_ann(ctx, &sig.type_ann);

      ctx.write_ts_method_sig(
        &sig.span,
        sig.computed,
        sig.optional,
        key,
        type_parms,
        params,
        return_type,
      )
    }
    TsTypeElement::TsIndexSignature(sig) => serialize_ts_index_sig(ctx, sig),
  }
}

fn serialize_ts_index_sig(
  ctx: &mut TsEsTreeBuilder,
  node: &TsIndexSignature,
) -> NodeRef {
  let params = node
    .params
    .iter()
    .map(|param| serialize_ts_fn_param(ctx, param))
    .collect::<Vec<_>>();
  let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann);

  ctx.write_ts_index_sig(
    &node.span,
    node.is_static,
    node.readonly,
    params,
    type_ann,
  )
}

fn accessibility_to_str(accessibility: &Accessibility) -> String {
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

  let type_args = node
    .type_args
    .as_ref()
    .map(|arg| serialize_ts_param_inst(ctx, arg));

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

  ctx.write_jsx_opening_elem(
    &node.span,
    node.self_closing,
    name,
    attrs,
    type_args,
  )
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

fn serialize_pat(
  ctx: &mut TsEsTreeBuilder,
  pat: &Pat,
  decorators: Option<Vec<NodeRef>>,
) -> NodeRef {
  match pat {
    Pat::Ident(node) => serialize_binding_ident(ctx, node, decorators),
    Pat::Array(node) => {
      let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann);

      let children = node
        .elems
        .iter()
        .map(|pat| {
          pat
            .as_ref()
            .map_or(NodeRef(0), |v| serialize_pat(ctx, v, None))
        })
        .collect::<Vec<_>>();

      ctx.write_arr_pat(
        &node.span,
        node.optional,
        type_ann,
        children,
        decorators,
      )
    }
    Pat::Rest(node) => {
      let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann);
      let arg = serialize_pat(ctx, &node.arg, None);

      ctx.write_rest_elem(&node.span, type_ann, arg, decorators)
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
            let value = serialize_pat(ctx, key_value_prop.value.as_ref(), None);

            ctx.write_property(
              &key_value_prop.span(),
              false,
              computed,
              false,
              PropertyKind::Init,
              key,
              value,
            )
          }
          ObjectPatProp::Assign(assign_pat_prop) => {
            let key = serialize_binding_ident(ctx, &assign_pat_prop.key, None);
            let mut value =
              serialize_binding_ident(ctx, &assign_pat_prop.key, None);

            let shorthand = assign_pat_prop.value.is_none();

            if let Some(assign) = &assign_pat_prop.value {
              let expr = serialize_expr(ctx, assign);
              value =
                ctx.write_assign_pat(&assign_pat_prop.span, value, expr, None);
            }

            ctx.write_property(
              &node.span,
              shorthand,
              false,
              false,
              PropertyKind::Init,
              key,
              value,
            )
          }
          ObjectPatProp::Rest(rest_pat) => {
            serialize_pat(ctx, &Pat::Rest(rest_pat.clone()), None)
          }
        })
        .collect::<Vec<_>>();

      ctx.write_obj_pat(
        &node.span,
        node.optional,
        type_ann,
        children,
        decorators,
      )
    }
    Pat::Assign(node) => {
      let left = serialize_pat(ctx, &node.left, None);
      let right = serialize_expr(ctx, &node.right);

      ctx.write_assign_pat(&node.span, left, right, decorators)
    }
    Pat::Invalid(_) => {
      // Ignore syntax errors
      NodeRef(0)
    }
    Pat::Expr(node) => serialize_expr(ctx, node),
  }
}

fn serialize_for_head(
  ctx: &mut TsEsTreeBuilder,
  for_head: &ForHead,
) -> NodeRef {
  match for_head {
    ForHead::VarDecl(var_decl) => {
      serialize_decl(ctx, &Decl::Var(var_decl.clone()))
    }
    ForHead::UsingDecl(using_decl) => {
      serialize_decl(ctx, &Decl::Using(using_decl.clone()))
    }
    ForHead::Pat(pat) => serialize_pat(ctx, pat, None),
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
  ctx.write_identifier(
    &ident_name.span,
    ident_name.sym.as_str(),
    false,
    None,
    None,
  )
}

fn serialize_prop_name(
  ctx: &mut TsEsTreeBuilder,
  prop_name: &PropName,
) -> NodeRef {
  match prop_name {
    PropName::Ident(ident_name) => serialize_ident_name(ctx, ident_name),
    PropName::Str(str_prop) => serialize_lit(ctx, &Lit::Str(str_prop.clone())),
    PropName::Num(number) => serialize_lit(ctx, &Lit::Num(number.clone())),
    PropName::Computed(node) => serialize_expr(ctx, &node.expr),
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
    Lit::JSXText(node) => {
      ctx.write_jsx_text(&node.span, &node.raw, &node.value)
    }
  }
}

fn serialize_class_member(
  ctx: &mut TsEsTreeBuilder,
  member: &ClassMember,
) -> Option<NodeRef> {
  match member {
    ClassMember::Constructor(node) => {
      let a11y = node.accessibility.as_ref().map(accessibility_to_str);

      let key = serialize_prop_name(ctx, &node.key);
      let params = node
        .params
        .iter()
        .map(|param| match param {
          ParamOrTsParamProp::TsParamProp(prop) => {
            let a11y = node.accessibility.as_ref().map(accessibility_to_str);

            let decorators = prop
              .decorators
              .iter()
              .map(|deco| serialize_decorator(ctx, deco))
              .collect::<Vec<_>>();

            let paramter = match &prop.param {
              TsParamPropParam::Ident(binding_ident) => {
                serialize_binding_ident(ctx, binding_ident, None)
              }
              TsParamPropParam::Assign(assign_pat) => {
                serialize_pat(ctx, &Pat::Assign(assign_pat.clone()), None)
              }
            };

            ctx.write_ts_param_prop(
              &prop.span,
              prop.is_override,
              prop.readonly,
              a11y,
              decorators,
              paramter,
            )
          }
          ParamOrTsParamProp::Param(param) => {
            serialize_pat(ctx, &param.pat, None)
          }
        })
        .collect::<Vec<_>>();

      let body = node
        .body
        .as_ref()
        .map(|body| serialize_stmt(ctx, &Stmt::Block(body.clone())));

      let value = ctx.write_fn_expr(
        &node.span, false, false, None, None, params, None, body,
      );

      Some(ctx.write_class_method(
        &node.span,
        false,
        false,
        node.is_optional,
        false,
        false,
        TsEstreeMethodKind::Constructor,
        a11y,
        key,
        value,
        vec![],
      ))
    }
    ClassMember::Method(node) => {
      let key = serialize_prop_name(ctx, &node.key);

      Some(serialize_class_method(
        ctx,
        &node.span,
        node.is_abstract,
        node.is_override,
        node.is_optional,
        node.is_static,
        node.accessibility,
        &node.kind,
        key,
        &node.function,
      ))
    }
    ClassMember::PrivateMethod(node) => {
      let key = serialize_private_name(ctx, &node.key);

      Some(serialize_class_method(
        ctx,
        &node.span,
        node.is_abstract,
        node.is_override,
        node.is_optional,
        node.is_static,
        node.accessibility,
        &node.kind,
        key,
        &node.function,
      ))
    }
    ClassMember::ClassProp(node) => {
      let a11y = node.accessibility.as_ref().map(accessibility_to_str);

      let key = serialize_prop_name(ctx, &node.key);
      let value = node.value.as_ref().map(|expr| serialize_expr(ctx, expr));

      let decorators = node
        .decorators
        .iter()
        .map(|deco| serialize_decorator(ctx, deco))
        .collect::<Vec<_>>();

      let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann);

      let out = if node.is_abstract {
        ctx.write_ts_abstract_prop_def(
          &node.span,
          false,
          node.is_optional,
          node.is_override,
          node.is_static,
          node.definite,
          node.readonly,
          node.declare,
          a11y,
          decorators,
          key,
          type_ann,
        )
      } else {
        ctx.write_class_prop(
          &node.span,
          node.declare,
          false,
          node.is_optional,
          node.is_override,
          node.readonly,
          node.is_static,
          a11y,
          decorators,
          key,
          value,
          type_ann,
        )
      };

      Some(out)
    }
    ClassMember::PrivateProp(node) => {
      let a11y = node.accessibility.as_ref().map(accessibility_to_str);

      let decorators = node
        .decorators
        .iter()
        .map(|deco| serialize_decorator(ctx, deco))
        .collect::<Vec<_>>();

      let key = serialize_private_name(ctx, &node.key);

      let value = node.value.as_ref().map(|expr| serialize_expr(ctx, expr));

      let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann);

      Some(ctx.write_class_prop(
        &node.span,
        false,
        false,
        node.is_optional,
        node.is_override,
        node.readonly,
        node.is_static,
        a11y,
        decorators,
        key,
        value,
        type_ann,
      ))
    }
    ClassMember::TsIndexSignature(node) => {
      Some(serialize_ts_index_sig(ctx, node))
    }
    ClassMember::Empty(_) => None,
    ClassMember::StaticBlock(node) => {
      let body = serialize_stmt(ctx, &Stmt::Block(node.body.clone()));
      Some(ctx.write_static_block(&node.span, body))
    }
    ClassMember::AutoAccessor(node) => {
      let a11y = node.accessibility.as_ref().map(accessibility_to_str);
      let decorators = node
        .decorators
        .iter()
        .map(|deco| serialize_decorator(ctx, deco))
        .collect::<Vec<_>>();

      let key = match &node.key {
        Key::Private(private_name) => serialize_private_name(ctx, private_name),
        Key::Public(prop_name) => serialize_prop_name(ctx, prop_name),
      };

      let value = node.value.as_ref().map(|expr| serialize_expr(ctx, expr));

      Some(ctx.write_accessor_property(
        &node.span,
        false,
        false,
        false,
        node.is_override,
        false,
        node.is_static,
        a11y,
        decorators,
        key,
        value,
      ))
    }
  }
}

#[allow(clippy::too_many_arguments)]
fn serialize_class_method(
  ctx: &mut TsEsTreeBuilder,
  span: &Span,
  is_abstract: bool,
  is_override: bool,
  is_optional: bool,
  is_static: bool,
  accessibility: Option<Accessibility>,
  method_kind: &MethodKind,
  key: NodeRef,
  function: &Function,
) -> NodeRef {
  let kind = match method_kind {
    MethodKind::Method => TsEstreeMethodKind::Method,
    MethodKind::Getter => TsEstreeMethodKind::Get,
    MethodKind::Setter => TsEstreeMethodKind::Set,
  };

  let type_params =
    maybe_serialize_ts_type_param_decl(ctx, &function.type_params);
  let params = function
    .params
    .iter()
    .map(|param| {
      let decorators = param
        .decorators
        .iter()
        .map(|deco| serialize_decorator(ctx, deco))
        .collect::<Vec<_>>();

      serialize_pat(ctx, &param.pat, Some(decorators))
    })
    .collect::<Vec<_>>();

  let return_type = maybe_serialize_ts_type_ann(ctx, &function.return_type);

  let body = function
    .body
    .as_ref()
    .map(|body| serialize_stmt(ctx, &Stmt::Block(body.clone())));

  let value = if let Some(body) = body {
    ctx.write_fn_expr(
      &function.span,
      function.is_async,
      function.is_generator,
      None,
      type_params,
      params,
      return_type,
      Some(body),
    )
  } else {
    ctx.write_ts_empty_body_fn_expr(
      span,
      false,
      false,
      function.is_async,
      function.is_generator,
      type_params,
      params,
      return_type,
    )
  };

  let a11y = accessibility.as_ref().map(accessibility_to_str);

  if is_abstract {
    ctx.write_ts_abstract_method_def(
      span,
      false,
      is_optional,
      is_override,
      false,
      a11y,
      key,
      value,
    )
  } else {
    let decorators = function
      .decorators
      .iter()
      .map(|deco| serialize_decorator(ctx, deco))
      .collect::<Vec<_>>();

    ctx.write_class_method(
      span,
      false,
      false,
      is_optional,
      is_override,
      is_static,
      kind,
      a11y,
      key,
      value,
      decorators,
    )
  }
}

fn serialize_ts_expr_with_type_args(
  ctx: &mut TsEsTreeBuilder,
  node: &TsExprWithTypeArgs,
) -> NodeRef {
  let expr = serialize_expr(ctx, &node.expr);
  let type_args = node
    .type_args
    .as_ref()
    .map(|arg| serialize_ts_param_inst(ctx, arg));

  ctx.write_ts_class_implements(&node.span, expr, type_args)
}

fn serialize_decorator(ctx: &mut TsEsTreeBuilder, node: &Decorator) -> NodeRef {
  let expr = serialize_expr(ctx, &node.expr);
  ctx.write_decorator(&node.span, expr)
}

fn serialize_binding_ident(
  ctx: &mut TsEsTreeBuilder,
  node: &BindingIdent,
  decorators: Option<Vec<NodeRef>>,
) -> NodeRef {
  let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann);
  ctx.write_identifier(
    &node.span,
    &node.sym,
    node.optional,
    type_ann,
    decorators,
  )
}

fn serialize_ts_param_inst(
  ctx: &mut TsEsTreeBuilder,
  node: &TsTypeParamInstantiation,
) -> NodeRef {
  let params = node
    .params
    .iter()
    .map(|param| serialize_ts_type(ctx, param))
    .collect::<Vec<_>>();

  ctx.write_ts_type_param_inst(&node.span, params)
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
        let param_ids = node
          .params
          .iter()
          .map(|param| serialize_ts_fn_param(ctx, param))
          .collect::<Vec<_>>();

        let type_params = node
          .type_params
          .as_ref()
          .map(|param| serialize_ts_type_param_decl(ctx, param));
        let return_type = serialize_ts_type_ann(ctx, node.type_ann.as_ref());

        ctx.write_ts_fn_type(
          &node.span,
          type_params,
          param_ids,
          Some(return_type),
        )
      }
      TsFnOrConstructorType::TsConstructorType(node) => {
        // interface Foo { new<T>(arg1: any): any }
        let type_params = node
          .type_params
          .as_ref()
          .map(|param| serialize_ts_type_param_decl(ctx, param));

        let params = node
          .params
          .iter()
          .map(|param| serialize_ts_fn_param(ctx, param))
          .collect::<Vec<_>>();

        let return_type = serialize_ts_type_ann(ctx, node.type_ann.as_ref());

        ctx.write_ts_construct_sig(&node.span, type_params, params, return_type)
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
    TsType::TsTypeLit(node) => {
      let members = node
        .members
        .iter()
        .map(|member| serialize_ts_type_elem(ctx, member))
        .collect::<Vec<_>>();

      ctx.write_ts_type_lit(&node.span, members)
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
            let optional = match label {
              Pat::Ident(binding_ident) => binding_ident.optional,
              Pat::Array(array_pat) => array_pat.optional,
              Pat::Object(object_pat) => object_pat.optional,
              _ => false,
            };
            let label = serialize_pat(ctx, label, None);
            let type_id = serialize_ts_type(ctx, elem.ty.as_ref());

            ctx
              .write_ts_named_tuple_member(&elem.span, label, type_id, optional)
          } else {
            serialize_ts_type(ctx, elem.ty.as_ref())
          }
        })
        .collect::<Vec<_>>();

      ctx.write_ts_tuple_type(&node.span, children)
    }
    TsType::TsOptionalType(node) => {
      let type_ann = serialize_ts_type(ctx, &node.type_ann);
      ctx.write_ts_optional_type(&node.span, type_ann)
    }
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
    TsType::TsParenthesizedType(node) => {
      // Not materialized in TSEstree
      serialize_ts_type(ctx, &node.type_ann)
    }
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
      let name = maybe_serialize_ts_type(ctx, &node.name_type);
      let type_ann = maybe_serialize_ts_type(ctx, &node.type_ann);
      let key = serialize_ident(ctx, &node.type_param.name, None);
      let constraint = node
        .type_param
        .constraint
        .as_ref()
        .map_or(NodeRef(0), |node| serialize_ts_type(ctx, node));

      ctx.write_ts_mapped_type(
        &node.span,
        node.readonly,
        node.optional,
        name,
        type_ann,
        key,
        constraint,
      )
    }
    TsType::TsLitType(node) => serialize_ts_lit_type(ctx, node),
    TsType::TsTypePredicate(node) => {
      let param_name = match &node.param_name {
        TsThisTypeOrIdent::TsThisType(node) => {
          ctx.write_ts_this_type(&node.span)
        }
        TsThisTypeOrIdent::Ident(ident) => serialize_ident(ctx, ident, None),
      };

      let type_ann = maybe_serialize_ts_type_ann(ctx, &node.type_ann);

      ctx.write_ts_type_predicate(
        &node.span,
        node.asserts,
        param_name,
        type_ann,
      )
    }
    TsType::TsImportType(node) => {
      let arg = serialize_ts_lit_type(
        ctx,
        &TsLitType {
          lit: TsLit::Str(node.arg.clone()),
          span: node.arg.span,
        },
      );

      let type_arg = node
        .type_args
        .clone()
        .map(|param_node| serialize_ts_param_inst(ctx, param_node.as_ref()));

      let qualifier = node
        .qualifier
        .clone()
        .map(|quali| serialize_ts_entity_name(ctx, &quali));

      ctx.write_ts_import_type(&node.span, arg, qualifier, type_arg)
    }
  }
}

fn serialize_ts_lit_type(
  ctx: &mut TsEsTreeBuilder,
  node: &TsLitType,
) -> NodeRef {
  match &node.lit {
    TsLit::Number(lit) => {
      let lit = serialize_lit(ctx, &Lit::Num(lit.clone()));
      ctx.write_ts_lit_type(&node.span, lit)
    }
    TsLit::Str(lit) => {
      let lit = serialize_lit(ctx, &Lit::Str(lit.clone()));
      ctx.write_ts_lit_type(&node.span, lit)
    }
    TsLit::Bool(lit) => {
      let lit = serialize_lit(ctx, &Lit::Bool(*lit));
      ctx.write_ts_lit_type(&node.span, lit)
    }
    TsLit::BigInt(lit) => {
      let lit = serialize_lit(ctx, &Lit::BigInt(lit.clone()));
      ctx.write_ts_lit_type(&node.span, lit)
    }
    TsLit::Tpl(lit) => {
      let quasis = lit
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
      let types = lit
        .types
        .iter()
        .map(|ts_type| serialize_ts_type(ctx, ts_type))
        .collect::<Vec<_>>();

      ctx.write_ts_tpl_lit(&node.span, quasis, types)
    }
  }
}

fn serialize_ts_entity_name(
  ctx: &mut TsEsTreeBuilder,
  node: &TsEntityName,
) -> NodeRef {
  match &node {
    TsEntityName::TsQualifiedName(node) => {
      let left = serialize_ts_entity_name(ctx, &node.left);
      let right = serialize_ident_name(ctx, &node.right);

      ctx.write_ts_qualified_name(&node.span, left, right)
    }
    TsEntityName::Ident(ident) => serialize_ident(ctx, ident, None),
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
) -> NodeRef {
  let name = serialize_ident(ctx, &node.name, None);
  let constraint = maybe_serialize_ts_type(ctx, &node.constraint);
  let default = maybe_serialize_ts_type(ctx, &node.default);

  ctx.write_ts_type_param(
    &node.span,
    node.is_in,
    node.is_out,
    node.is_const,
    name,
    constraint,
    default,
  )
}

fn maybe_serialize_ts_type_param_decl(
  ctx: &mut TsEsTreeBuilder,
  node: &Option<Box<TsTypeParamDecl>>,
) -> Option<NodeRef> {
  node
    .as_ref()
    .map(|node| serialize_ts_type_param_decl(ctx, node))
}

fn serialize_ts_type_param_decl(
  ctx: &mut TsEsTreeBuilder,
  node: &TsTypeParamDecl,
) -> NodeRef {
  let params = node
    .params
    .iter()
    .map(|param| serialize_ts_type_param(ctx, param))
    .collect::<Vec<_>>();

  ctx.write_ts_type_param_decl(&node.span, params)
}

fn serialize_ts_fn_param(
  ctx: &mut TsEsTreeBuilder,
  node: &TsFnParam,
) -> NodeRef {
  match node {
    TsFnParam::Ident(ident) => serialize_binding_ident(ctx, ident, None),
    TsFnParam::Array(pat) => serialize_pat(ctx, &Pat::Array(pat.clone()), None),
    TsFnParam::Rest(pat) => serialize_pat(ctx, &Pat::Rest(pat.clone()), None),
    TsFnParam::Object(pat) => {
      serialize_pat(ctx, &Pat::Object(pat.clone()), None)
    }
  }
}
