use deno_ast::{
  swc::{
    ast::{
      AssignTarget, BlockStmtOrExpr, Callee, Decl, Expr, ForHead, Lit,
      MemberProp, ModuleDecl, ModuleItem, Program, Prop, PropOrSpread,
      SimpleAssignTarget, Stmt, VarDeclOrExpr,
    },
    common::Span,
  },
  ParsedSource,
};

enum AstNode {
  Invalid,
  //
  Program,

  // Module declarations
  Import,
  ImportDecl,
  ExportDecl,
  ExportNamed,
  ExportDefaultDecl,
  ExportDefaultExpr,
  ExportAll,
  TsImportEquals,
  TsExportAssignment,
  TsNamespaceExport,

  // Decls
  Class,
  Fn,
  Var,
  Using,
  TsInterface,
  TsTypeAlias,
  TsEnum,
  TsModule,

  // Statements
  Block,
  Empty,
  Debugger,
  With,
  Return,
  Labeled,
  Break,
  Continue,
  If,
  Switch,
  SwitchCase,
  Throw,
  Try,
  While,
  DoWhile,
  For,
  ForIn,
  ForOf,
  Decl,
  Expr,

  // Expressions
  This,
  Array,
  Object,
  FnExpr,
  Unary,
  Update,
  Bin,
  Assign,
  Member,
  SuperProp,
  Cond,
  Call,
  New,
  Seq,
  Ident,
  Tpl,
  TaggedTpl,
  Arrow,
  ClassExpr,
  Yield,
  MetaProp,
  Await,
  Paren,
  TsTypeAssertion,
  TsConstAssertion,
  TsNonNull,
  TsAs,
  TsInstantiation,
  TsSatisfies,
  PrivateName,
  OptChain,

  // Literals
  StringLiteral,
  Bool,
  Null,
  Num,
  BigInt,
  Regex,

  // JSX
  JSXMember,
  JSXNamespacedName,
  JSXEmpty,
  JSXElement,
  JSXFragment,
  JSXText,

  // Custom
  EmptyExpr,
  Spread,
  ObjProperty,
}

impl From<AstNode> for u8 {
  fn from(m: AstNode) -> u8 {
    m as u8
  }
}

pub fn serialize_ast_bin(parsed_source: &ParsedSource) -> Vec<u8> {
  let mut result: Vec<u8> = vec![];

  let program = &parsed_source.program();
  match program.as_ref() {
    Program::Module(module) => {
      push_node(&mut result, AstNode::Program.into(), &module.span);
      for item in &module.body {
        match item {
          ModuleItem::ModuleDecl(module_decl) => {
            serialize_module_decl(&mut result, module_decl)
          }
          ModuleItem::Stmt(stmt) => serialize_stmt(&mut result, stmt),
        }
      }
    }
    Program::Script(script) => {
      push_node(&mut result, AstNode::Program.into(), &script.span);

      for stmt in &script.body {
        serialize_stmt(&mut result, stmt)
      }
    }
  }

  result
}

fn push_node(result: &mut Vec<u8>, kind: u8, span: &Span) {
  result.push(kind);

  // FIXME: Span
  result.push(0);
  result.push(0);
  result.push(0);
  result.push(0);
}

fn serialize_module_decl(result: &mut Vec<u8>, module_decl: &ModuleDecl) {
  match module_decl {
    ModuleDecl::Import(import_decl) => {
      push_node(result, AstNode::Import.into(), &import_decl.span);
    }
    ModuleDecl::ExportDecl(export_decl) => {
      push_node(result, AstNode::ExportDecl.into(), &export_decl.span);
    }
    ModuleDecl::ExportNamed(named_export) => {
      push_node(result, AstNode::ExportNamed.into(), &named_export.span);
    }
    ModuleDecl::ExportDefaultDecl(export_default_decl) => {
      push_node(
        result,
        AstNode::ExportDefaultDecl.into(),
        &export_default_decl.span,
      );
    }
    ModuleDecl::ExportDefaultExpr(export_default_expr) => {
      push_node(
        result,
        AstNode::ExportDefaultExpr.into(),
        &export_default_expr.span,
      );
    }
    ModuleDecl::ExportAll(export_all) => {
      push_node(result, AstNode::ExportAll.into(), &export_all.span);
    }
    ModuleDecl::TsImportEquals(ts_import_equals_decl) => {
      push_node(
        result,
        AstNode::TsImportEquals.into(),
        &ts_import_equals_decl.span,
      );
    }
    ModuleDecl::TsExportAssignment(ts_export_assignment) => {
      push_node(
        result,
        AstNode::TsExportAssignment.into(),
        &ts_export_assignment.span,
      );
    }
    ModuleDecl::TsNamespaceExport(ts_namespace_export_decl) => {
      push_node(
        result,
        AstNode::TsNamespaceExport.into(),
        &ts_namespace_export_decl.span,
      );
    }
  }
}

fn serialize_stmt(result: &mut Vec<u8>, stmt: &Stmt) {
  match stmt {
    Stmt::Block(block_stmt) => {
      push_node(result, AstNode::Block.into(), &block_stmt.span);

      for child in &block_stmt.stmts {
        serialize_stmt(result, child);
      }
    }
    Stmt::Empty(empty_stmt) => {
      push_node(result, AstNode::Empty.into(), &empty_stmt.span);
    }
    Stmt::Debugger(debugger_stmt) => {
      push_node(result, AstNode::Debugger.into(), &debugger_stmt.span);
    }
    Stmt::With(_) => todo!(),
    Stmt::Return(return_stmt) => {
      push_node(result, AstNode::Return.into(), &return_stmt.span);
    }
    Stmt::Labeled(labeled_stmt) => {
      push_node(result, AstNode::Labeled.into(), &labeled_stmt.span);
    }
    Stmt::Break(break_stmt) => {
      push_node(result, AstNode::Break.into(), &break_stmt.span);
    }
    Stmt::Continue(continue_stmt) => {
      push_node(result, AstNode::Continue.into(), &continue_stmt.span);
    }
    Stmt::If(if_stmt) => {
      push_node(result, AstNode::If.into(), &if_stmt.span);

      serialize_expr(result, if_stmt.test.as_ref());
      serialize_stmt(result, if_stmt.cons.as_ref());

      if let Some(alt) = &if_stmt.alt {
        serialize_stmt(result, &alt);
      }
    }
    Stmt::Switch(switch_stmt) => {
      push_node(result, AstNode::Switch.into(), &switch_stmt.span);

      for case in &switch_stmt.cases {
        push_node(result, AstNode::SwitchCase.into(), &case.span);
      }
    }
    Stmt::Throw(throw_stmt) => {
      push_node(result, AstNode::Throw.into(), &throw_stmt.span);
    }
    Stmt::Try(try_stmt) => {
      push_node(result, AstNode::Try.into(), &try_stmt.span);

      serialize_stmt(result, &Stmt::Block(try_stmt.block.clone()));
    }
    Stmt::While(while_stmt) => {
      push_node(result, AstNode::While.into(), &while_stmt.span);

      serialize_expr(result, while_stmt.test.as_ref());
      serialize_stmt(result, while_stmt.body.as_ref());
    }
    Stmt::DoWhile(do_while_stmt) => {
      push_node(result, AstNode::DoWhile.into(), &do_while_stmt.span);
    }
    Stmt::For(for_stmt) => {
      push_node(result, AstNode::For.into(), &for_stmt.span);

      if let Some(init) = &for_stmt.init {
        match init {
          VarDeclOrExpr::VarDecl(var_decl) => {
            serialize_stmt(result, &Stmt::Decl(Decl::Var(var_decl.clone())));
          }
          VarDeclOrExpr::Expr(expr) => {
            serialize_expr(result, expr);
          }
        }
      } else {
        push_node(result, AstNode::EmptyExpr.into(), &for_stmt.span);
      }

      if let Some(test_expr) = &for_stmt.test {
        serialize_expr(result, test_expr.as_ref());
      } else {
        push_node(result, AstNode::EmptyExpr.into(), &for_stmt.span);
      }

      if let Some(update_expr) = &for_stmt.update {
        serialize_expr(result, update_expr.as_ref());
      } else {
        push_node(result, AstNode::EmptyExpr.into(), &for_stmt.span);
      }

      serialize_stmt(result, &for_stmt.body.as_ref());
    }
    Stmt::ForIn(for_in_stmt) => {
      push_node(result, AstNode::ForIn.into(), &for_in_stmt.span);

      match &for_in_stmt.left {
        ForHead::VarDecl(var_decl) => {}
        ForHead::UsingDecl(using_decl) => {}
        ForHead::Pat(pat) => {}
      }

      serialize_expr(result, for_in_stmt.right.as_ref());
      serialize_stmt(result, for_in_stmt.body.as_ref());
    }
    Stmt::ForOf(for_of_stmt) => {
      push_node(result, AstNode::ForOf.into(), &for_of_stmt.span);
    }
    Stmt::Decl(decl) => serialize_decl(result, decl),
    Stmt::Expr(expr_stmt) => {
      push_node(result, AstNode::Expr.into(), &expr_stmt.span);
      serialize_expr(result, expr_stmt.expr.as_ref());
    }
  }
}

fn serialize_decl(result: &mut Vec<u8>, decl: &Decl) {
  match decl {
    Decl::Class(class_decl) => {
      push_node(result, AstNode::Class.into(), &class_decl.class.span);

      //
    }
    Decl::Fn(fn_decl) => {
      push_node(result, AstNode::Fn.into(), &fn_decl.function.span);

      if let Some(body) = &fn_decl.function.as_ref().body {
        serialize_stmt(result, &Stmt::Block(body.clone()));
      }
    }
    Decl::Var(var_decl) => {
      push_node(result, AstNode::Var.into(), &var_decl.span);
    }
    Decl::Using(using_decl) => {
      push_node(result, AstNode::Using.into(), &using_decl.span);
    }
    Decl::TsInterface(ts_interface_decl) => {
      push_node(result, AstNode::TsInterface.into(), &ts_interface_decl.span);
    }
    Decl::TsTypeAlias(ts_type_alias_decl) => {
      push_node(
        result,
        AstNode::TsTypeAlias.into(),
        &ts_type_alias_decl.span,
      );
    }
    Decl::TsEnum(ts_enum_decl) => {
      push_node(result, AstNode::TsEnum.into(), &ts_enum_decl.span);
    }
    Decl::TsModule(ts_module_decl) => {
      push_node(result, AstNode::TsModule.into(), &ts_module_decl.span);
    }
  }
}

fn serialize_expr(result: &mut Vec<u8>, expr: &Expr) {
  match expr {
    Expr::This(this_expr) => {
      push_node(result, AstNode::This.into(), &this_expr.span);
    }
    Expr::Array(array_lit) => {
      push_node(result, AstNode::Array.into(), &array_lit.span);
      for item in &array_lit.elems {

        // FIXME
      }
    }
    Expr::Object(object_lit) => {
      push_node(result, AstNode::Object.into(), &object_lit.span);

      for prop in &object_lit.props {
        match prop {
          PropOrSpread::Spread(spread_element) => {
            push_node(
              result,
              AstNode::Spread.into(),
              &spread_element.dot3_token,
            );
            serialize_expr(result, spread_element.expr.as_ref());
          }
          PropOrSpread::Prop(prop) => match prop.as_ref() {
            Prop::Shorthand(ident) => {
              serialize_expr(result, &Expr::Ident(ident.clone()));
            }
            Prop::KeyValue(key_value_prop) => {
              serialize_expr(result, key_value_prop.value.as_ref())
            }
            Prop::Assign(assign_prop) => {
              push_node(result, AstNode::Assign.into(), &assign_prop.span);
              serialize_expr(result, assign_prop.value.as_ref())
            }
            Prop::Getter(getter_prop) => {
              // TODO
              if let Some(stmt) = &getter_prop.body {
                serialize_stmt(result, &Stmt::Block(stmt.clone()));
              }
            }
            Prop::Setter(setter_prop) => {
              // TODO
              if let Some(body) = &setter_prop.body {
                serialize_stmt(result, &Stmt::Block(body.clone()));
              }
            }
            Prop::Method(method_prop) => {
              if let Some(body) = &method_prop.function.body {
                serialize_stmt(result, &Stmt::Block(body.clone()));
              }
            }
          },
        }
      }
    }
    Expr::Fn(fn_expr) => {
      let fn_obj = fn_expr.function.as_ref();
      push_node(result, AstNode::FnExpr.into(), &fn_obj.span);
    }
    Expr::Unary(unary_expr) => {
      push_node(result, AstNode::Unary.into(), &unary_expr.span);
    }
    Expr::Update(update_expr) => {
      push_node(result, AstNode::Update.into(), &update_expr.span);
      serialize_expr(result, update_expr.arg.as_ref());
    }
    Expr::Bin(bin_expr) => {
      push_node(result, AstNode::Bin.into(), &bin_expr.span);
      serialize_expr(result, bin_expr.left.as_ref());
      serialize_expr(result, bin_expr.right.as_ref());
    }
    Expr::Assign(assign_expr) => {
      push_node(result, AstNode::Assign.into(), &assign_expr.span);

      match &assign_expr.left {
        AssignTarget::Simple(simple_assign_target) => {
          match simple_assign_target {
            SimpleAssignTarget::Ident(binding_ident) => {
              serialize_expr(result, &Expr::Ident(binding_ident.id.clone()));
            }
            SimpleAssignTarget::Member(member_expr) => {
              serialize_expr(result, &Expr::Member(member_expr.clone()));
            }
            SimpleAssignTarget::SuperProp(super_prop_expr) => {}
            SimpleAssignTarget::Paren(paren_expr) => {}
            SimpleAssignTarget::OptChain(opt_chain_expr) => {}
            SimpleAssignTarget::TsAs(ts_as_expr) => {}
            SimpleAssignTarget::TsSatisfies(ts_satisfies_expr) => {}
            SimpleAssignTarget::TsNonNull(ts_non_null_expr) => {}
            SimpleAssignTarget::TsTypeAssertion(ts_type_assertion) => {}
            SimpleAssignTarget::TsInstantiation(ts_instantiation) => {}
            SimpleAssignTarget::Invalid(invalid) => {}
          }
        }
        AssignTarget::Pat(assign_target_pat) => {}
      }
    }
    Expr::Member(member_expr) => {
      push_node(result, AstNode::Member.into(), &member_expr.span);
      serialize_expr(result, member_expr.obj.as_ref());

      match &member_expr.prop {
        MemberProp::Ident(ident_name) => {}
        MemberProp::PrivateName(private_name) => {}
        MemberProp::Computed(computed_prop_name) => {
          serialize_expr(result, computed_prop_name.expr.as_ref());
        }
      }
    }
    Expr::SuperProp(super_prop_expr) => {
      push_node(result, AstNode::SuperProp.into(), &super_prop_expr.span);
    }
    Expr::Cond(cond_expr) => {
      push_node(result, AstNode::Cond.into(), &cond_expr.span);

      serialize_expr(result, cond_expr.test.as_ref());
      serialize_expr(result, cond_expr.cons.as_ref());
      serialize_expr(result, cond_expr.alt.as_ref());
    }
    Expr::Call(call_expr) => {
      push_node(result, AstNode::Call.into(), &call_expr.span);

      match &call_expr.callee {
        Callee::Super(_) => {}
        Callee::Import(import) => {}
        Callee::Expr(expr) => {
          serialize_expr(result, expr);
        }
      }

      for arg in &call_expr.args {
        if let Some(spread) = &arg.spread {
          push_node(result, AstNode::Spread.into(), spread);
        }

        serialize_expr(result, arg.expr.as_ref());
      }
    }
    Expr::New(new_expr) => {
      push_node(result, AstNode::New.into(), &new_expr.span);
      serialize_expr(result, new_expr.callee.as_ref());

      if let Some(args) = &new_expr.args {
        for arg in args {
          if let Some(spread) = &arg.spread {
            push_node(result, AstNode::Spread.into(), spread);
          }

          serialize_expr(result, arg.expr.as_ref());
        }
      }
    }
    Expr::Seq(seq_expr) => {
      push_node(result, AstNode::Seq.into(), &seq_expr.span);

      for expr in &seq_expr.exprs {
        serialize_expr(result, expr);
      }
    }
    Expr::Ident(ident) => {
      push_node(result, AstNode::Ident.into(), &ident.span);
    }
    Expr::Lit(lit) => {
      serialize_lit(result, lit);
    }
    Expr::Tpl(tpl) => {
      push_node(result, AstNode::Tpl.into(), &tpl.span);
    }
    Expr::TaggedTpl(tagged_tpl) => {
      push_node(result, AstNode::TaggedTpl.into(), &tagged_tpl.span);
    }
    Expr::Arrow(arrow_expr) => {
      push_node(result, AstNode::Arrow.into(), &arrow_expr.span);

      match arrow_expr.body.as_ref() {
        BlockStmtOrExpr::BlockStmt(block_stmt) => {
          serialize_stmt(result, &Stmt::Block(block_stmt.clone()));
        }
        BlockStmtOrExpr::Expr(expr) => {
          serialize_expr(result, expr.as_ref());
        }
      }
    }
    Expr::Class(class_expr) => {
      // push_node(result, AstNode::Class.into(), &class_expr.span);
    }
    Expr::Yield(yield_expr) => {
      push_node(result, AstNode::Yield.into(), &yield_expr.span);

      if let Some(arg) = &yield_expr.arg {
        serialize_expr(result, arg.as_ref());
      }
    }
    Expr::MetaProp(meta_prop_expr) => {
      push_node(result, AstNode::MetaProp.into(), &meta_prop_expr.span);
    }
    Expr::Await(await_expr) => {
      push_node(result, AstNode::Await.into(), &await_expr.span);

      serialize_expr(result, await_expr.arg.as_ref());
    }
    Expr::Paren(paren_expr) => {
      push_node(result, AstNode::Paren.into(), &paren_expr.span);

      serialize_expr(result, paren_expr.expr.as_ref());
    }
    Expr::JSXMember(jsxmember_expr) => {
      push_node(result, AstNode::JSXMember.into(), &jsxmember_expr.span);
    }
    Expr::JSXNamespacedName(jsxnamespaced_name) => {
      push_node(
        result,
        AstNode::JSXNamespacedName.into(),
        &jsxnamespaced_name.span,
      );
    }
    Expr::JSXEmpty(jsxempty_expr) => {
      push_node(result, AstNode::JSXEmpty.into(), &jsxempty_expr.span);
    }
    Expr::JSXElement(jsxelement) => {
      push_node(result, AstNode::JSXElement.into(), &jsxelement.span);
    }
    Expr::JSXFragment(jsxfragment) => {
      push_node(result, AstNode::JSXFragment.into(), &jsxfragment.span);
    }
    Expr::TsTypeAssertion(ts_type_assertion) => {
      push_node(
        result,
        AstNode::TsTypeAssertion.into(),
        &ts_type_assertion.span,
      );
    }
    Expr::TsConstAssertion(ts_const_assertion) => {
      push_node(
        result,
        AstNode::TsConstAssertion.into(),
        &ts_const_assertion.span,
      );
    }
    Expr::TsNonNull(ts_non_null_expr) => {
      push_node(result, AstNode::TsNonNull.into(), &ts_non_null_expr.span);
    }
    Expr::TsAs(ts_as_expr) => {
      push_node(result, AstNode::TsAs.into(), &ts_as_expr.span);
    }
    Expr::TsInstantiation(ts_instantiation) => {
      push_node(
        result,
        AstNode::TsInstantiation.into(),
        &ts_instantiation.span,
      );
    }
    Expr::TsSatisfies(ts_satisfies_expr) => {
      push_node(result, AstNode::TsSatisfies.into(), &ts_satisfies_expr.span);
    }
    Expr::PrivateName(private_name) => {
      push_node(result, AstNode::PrivateName.into(), &private_name.span);
    }
    Expr::OptChain(opt_chain_expr) => {
      push_node(result, AstNode::OptChain.into(), &opt_chain_expr.span);
    }
    Expr::Invalid(invalid) => {
      // push_node(result, AstNode::Invalid.into(), &invalid.span);
    }
  }
}

fn serialize_lit(result: &mut Vec<u8>, lit: &Lit) {
  match lit {
    Lit::Str(lit_str) => {
      push_node(result, AstNode::StringLiteral.into(), &lit_str.span)
    }
    Lit::Bool(lit_bool) => {
      push_node(result, AstNode::Bool.into(), &lit_bool.span)
    }
    Lit::Null(null) => push_node(result, AstNode::Null.into(), &null.span),
    Lit::Num(number) => push_node(result, AstNode::Num.into(), &number.span),
    Lit::BigInt(big_int) => {
      push_node(result, AstNode::BigInt.into(), &big_int.span)
    }
    Lit::Regex(regex) => push_node(result, AstNode::Regex.into(), &regex.span),
    Lit::JSXText(jsxtext) => {
      push_node(result, AstNode::JSXText.into(), &jsxtext.span)
    }
  }
}
