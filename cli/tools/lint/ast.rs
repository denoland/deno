use deno_ast::{
  swc::{
    ast::{
      AssignTarget, AssignTargetPat, BlockStmtOrExpr, Callee, ClassMember,
      Decl, ExportSpecifier, Expr, ExprOrSpread, FnExpr, ForHead, Function,
      Ident, IdentName, JSXAttrName, JSXAttrOrSpread, JSXAttrValue, JSXElement,
      JSXElementChild, JSXElementName, JSXEmptyExpr, JSXExpr, JSXExprContainer,
      JSXFragment, JSXMemberExpr, JSXNamespacedName, JSXObject,
      JSXOpeningElement, Lit, MemberProp, ModuleDecl, ModuleExportName,
      ModuleItem, ObjectPatProp, Param, ParamOrTsParamProp, Pat, Program, Prop,
      PropName, PropOrSpread, SimpleAssignTarget, Stmt, SuperProp, TsType,
      VarDeclOrExpr,
    },
    common::{Span, Spanned, SyntaxContext, DUMMY_SP},
  },
  view::{Accessibility, AssignOp, BinaryOp, UnaryOp, UpdateOp, VarDeclKind},
  ParsedSource,
};
use indexmap::IndexMap;

// Keep in sync with JS
enum AstNode {
  Invalid,
  //
  Program,

  // Module declarations
  Import,
  ImportDecl,
  ExportDecl,
  ExportNamedDeclaration,
  ExportDefaultDecl,
  ExportDefaultExpr,
  ExportAll,
  TsImportEquals,
  TsExportAssignment,
  TsNamespaceExport,

  // Decls
  ClassDeclaration,
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
  IfStatement,
  Switch,
  SwitchCase,
  Throw,
  TryStatement,
  While,
  DoWhileStatement,
  ForStatement,
  ForInStatement,
  ForOfStatement,
  Decl,
  Expr,

  // Expressions
  This,
  Array,
  Object,
  FunctionExpression,
  Unary,
  UpdateExpression,
  BinaryExpression,
  Assign,
  MemberExpression,
  Super,
  Cond,
  Call,
  New,
  Paren,
  Seq,
  Identifier,
  TemplateLiteral,
  TaggedTpl,
  ArrowFunctionExpression,
  ClassExpr,
  YieldExpression,
  MetaProp,
  AwaitExpression,
  LogicalExpression,
  TsTypeAssertion,
  TsConstAssertion,
  TsNonNull,
  TsAs,
  TsInstantiation,
  TsSatisfies,
  PrivateIdentifier,
  OptChain,

  // Literals
  StringLiteral,
  Bool,
  Null,
  NumericLiteral,
  BigIntLiteral,
  RegExpLiteral,

  // Custom
  EmptyExpr,
  Spread,
  Property,
  VariableDeclarator,
  CatchClause,
  RestElement,
  ExportSpecifier,
  TemplateElement,
  MethodDefinition,

  // Patterns
  ArrayPattern,
  AssignmentPattern,
  ObjectPattern,

  // JSX
  JSXAttribute,
  JSXClosingElement,
  JSXClosingFragment,
  JSXElement,
  JSXEmptyExpression,
  JSXExpressionContainer,
  JSXFragment,
  JSXIdentifier,
  JSXMemberExpression,
  JSXNamespacedName,
  JSXOpeningElement,
  JSXOpeningFragment,
  JSXSpreadAttribute,
  JSXSpreadChild,
  JSXText,
}

impl From<AstNode> for u8 {
  fn from(m: AstNode) -> u8 {
    m as u8
  }
}

const MASK_U32_1: u32 = 0b11111111_00000000_00000000_00000000;
const MASK_U32_2: u32 = 0b00000000_11111111_00000000_00000000;
const MASK_U32_3: u32 = 0b00000000_00000000_11111111_00000000;
const MASK_U32_4: u32 = 0b00000000_00000000_00000000_11111111;

fn append_u32(result: &mut Vec<u8>, value: u32) {
  let v1: u8 = ((value & MASK_U32_1) >> 24) as u8;
  let v2: u8 = ((value & MASK_U32_2) >> 16) as u8;
  let v3: u8 = ((value & MASK_U32_3) >> 8) as u8;
  let v4: u8 = (value & MASK_U32_4) as u8;

  result.push(v1);
  result.push(v2);
  result.push(v3);
  result.push(v4);
}

fn append_usize(result: &mut Vec<u8>, value: usize) {
  let raw = u32::try_from(value).unwrap();
  append_u32(result, raw);
}

fn write_usize(result: &mut [u8], value: usize, idx: usize) {
  let raw = u32::try_from(value).unwrap();

  let v1: u8 = ((raw & MASK_U32_1) >> 24) as u8;
  let v2: u8 = ((raw & MASK_U32_2) >> 16) as u8;
  let v3: u8 = ((raw & MASK_U32_3) >> 8) as u8;
  let v4: u8 = (raw & MASK_U32_4) as u8;

  result[idx] = v1;
  result[idx + 1] = v2;
  result[idx + 2] = v3;
  result[idx + 3] = v4;
}

enum Flag {
  ProgramModule,
  FnAsync,
  FnGenerator,
  FnDeclare,
  MemberComputed,
  PropShorthand,
  PropComputed,
  PropGetter,
  PropSetter,
  PropMethod,
  VarVar,
  VarConst,
  VarLet,
  VarDeclare,
  ExportType,
  TplTail,
  ForAwait,
  LogicalOr,
  LogicalAnd,
  LogicalNullishCoalescin,
  JSXSelfClosing,

  BinEqEq,
  BinNotEq,
  BinEqEqEq,
  BinNotEqEq,
  BinLt,
  BinLtEq,
  BinGt,
  BinGtEq,
  BinLShift,
  BinRShift,
  BinZeroFillRShift,
  BinAdd,
  BinSub,
  BinMul,
  BinDiv,
  BinMod,
  BinBitOr,
  BinBitXor,
  BinBitAnd,
  BinIn,
  BinInstanceOf,
  BinExp,

  UnaryMinus,
  UnaryPlus,
  UnaryBang,
  UnaryTilde,
  UnaryTypeOf,
  UnaryVoid,
  UnaryDelete,

  UpdatePrefix,
  UpdatePlusPlus,
  UpdateMinusMinus,

  YieldDelegate,
  ParamOptional,

  ClassDeclare,
  ClassAbstract,
  ClassConstructor,
  ClassMethod,
  ClassPublic,
  ClassProtected,
  ClassPrivate,
}

fn assign_op_to_flag(m: AssignOp) -> u8 {
  match m {
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
  }
}

impl From<Flag> for u8 {
  fn from(m: Flag) -> u8 {
    match m {
      Flag::ProgramModule => 0b00000001,
      Flag::FnAsync => 0b00000001,
      Flag::FnGenerator => 0b00000010,
      Flag::FnDeclare => 0b00000100,
      Flag::MemberComputed => 0b00000001,
      Flag::PropShorthand => 0b00000001,
      Flag::PropComputed => 0b00000010,
      Flag::PropGetter => 0b00000100,
      Flag::PropSetter => 0b00001000,
      Flag::PropMethod => 0b00010000,
      Flag::VarVar => 0b00000001,
      Flag::VarConst => 0b00000010,
      Flag::VarLet => 0b00000100,
      Flag::VarDeclare => 0b00001000,
      Flag::ExportType => 0b000000001,
      Flag::TplTail => 0b000000001,
      Flag::ForAwait => 0b000000001,
      Flag::LogicalOr => 0b000000001,
      Flag::LogicalAnd => 0b000000010,
      Flag::LogicalNullishCoalescin => 0b000000100,
      Flag::JSXSelfClosing => 0b000000001,
      Flag::BinEqEq => 1,
      Flag::BinNotEq => 2,
      Flag::BinEqEqEq => 3,
      Flag::BinNotEqEq => 4,
      Flag::BinLt => 5,
      Flag::BinLtEq => 6,
      Flag::BinGt => 7,
      Flag::BinGtEq => 8,
      Flag::BinLShift => 9,
      Flag::BinRShift => 10,
      Flag::BinZeroFillRShift => 11,
      Flag::BinAdd => 12,
      Flag::BinSub => 13,
      Flag::BinMul => 14,
      Flag::BinDiv => 15,
      Flag::BinMod => 16,
      Flag::BinBitOr => 17,
      Flag::BinBitXor => 18,
      Flag::BinBitAnd => 19,
      Flag::BinIn => 20,
      Flag::BinInstanceOf => 21,
      Flag::BinExp => 22,

      Flag::UnaryMinus => 1,
      Flag::UnaryPlus => 2,
      Flag::UnaryBang => 3,
      Flag::UnaryTilde => 4,
      Flag::UnaryTypeOf => 5,
      Flag::UnaryVoid => 6,
      Flag::UnaryDelete => 7,

      Flag::UpdatePrefix => 0b000000001,
      Flag::UpdatePlusPlus => 0b000000010,
      Flag::UpdateMinusMinus => 0b000000100,

      Flag::YieldDelegate => 1,
      Flag::ParamOptional => 1,

      Flag::ClassDeclare => 0b000000001,
      Flag::ClassAbstract => 0b000000010,
      Flag::ClassConstructor => 0b000000100,
      Flag::ClassMethod => 0b000001000,
      Flag::ClassPublic => 0b001000000,
      Flag::ClassProtected => 0b010000000,
      Flag::ClassPrivate => 0b10000000,
    }
  }
}

#[derive(Debug, Clone)]
struct FlagValue(u8);

impl FlagValue {
  fn new() -> Self {
    Self(0)
  }

  fn set(&mut self, flag: Flag) {
    let value: u8 = flag.into();
    self.0 |= value;
  }
}

#[derive(Debug)]
struct StringTable {
  id: usize,
  table: IndexMap<String, usize>,
}

impl StringTable {
  fn new() -> Self {
    Self {
      id: 0,
      table: IndexMap::new(),
    }
  }

  fn insert(&mut self, s: &str) -> usize {
    if let Some(id) = self.table.get(s) {
      return *id;
    }

    let id = self.id;
    self.id += 1;
    self.table.insert(s.to_string(), id);
    id
  }

  fn serialize(&mut self) -> Vec<u8> {
    let mut result: Vec<u8> = vec![];
    append_u32(&mut result, self.table.len() as u32);

    // Assume that it's sorted by id
    for (s, _id) in &self.table {
      let bytes = s.as_bytes();
      append_u32(&mut result, bytes.len() as u32);
      result.append(&mut bytes.to_vec());
    }

    // eprintln!("Serialized string table: {:#?}", result);

    result
  }
}

struct SerializeCtx {
  id: usize,
  id_to_offset: IndexMap<usize, usize>,
  result: Vec<u8>,
  str_table: StringTable,
}

impl SerializeCtx {
  fn new() -> Self {
    let mut ctx = Self {
      id: 0,
      id_to_offset: IndexMap::new(),
      result: vec![],
      str_table: StringTable::new(),
    };

    ctx.str_table.insert("");

    ctx.push_node(AstNode::Empty, 0, &DUMMY_SP);

    ctx
  }

  fn reserve_child_ids_with_count(&mut self, count: usize) -> usize {
    append_usize(&mut self.result, count);
    self.reserve_child_ids(count)
  }

  fn reserve_child_ids(&mut self, count: usize) -> usize {
    let offset = self.result.len();

    for _ in 0..count {
      append_usize(&mut self.result, 0);
    }

    offset
  }

  fn next_id(&mut self) -> usize {
    let id = self.id;
    self.id += 1;
    id
  }

  fn write_u8(&mut self, value: u8) {
    self.result.push(value);
  }

  fn write_node(
    &mut self,
    id: usize,
    kind: AstNode,
    parent_id: usize,
    span: &Span,
  ) {
    self.id_to_offset.insert(id, self.result.len());

    let kind_value: u8 = kind.into();
    self.result.push(kind_value);
    append_usize(&mut self.result, parent_id);

    // Span
    append_u32(&mut self.result, span.lo.0);
    append_u32(&mut self.result, span.hi.0);
  }

  fn write_ids<I>(&mut self, ids: I)
  where
    I: IntoIterator<Item = usize>,
  {
    let mut count = 0;
    let idx = self.result.len();
    append_usize(&mut self.result, 0);

    for id in ids {
      append_usize(&mut self.result, id);
      count += 1;
    }

    write_usize(&mut self.result, count, idx);
  }

  fn write_id(&mut self, id: usize) {
    append_usize(&mut self.result, id);
  }

  fn write_flags(&mut self, flags: &FlagValue) {
    self.result.push(flags.0)
  }

  fn push_node(
    &mut self,
    kind: AstNode,
    parent_id: usize,
    span: &Span,
  ) -> usize {
    let id = self.id;
    self.id_to_offset.insert(id, self.result.len());
    self.id += 1;

    let kind_value: u8 = kind.into();
    self.result.push(kind_value);
    append_usize(&mut self.result, parent_id);

    // Span
    append_u32(&mut self.result, span.lo.0);
    append_u32(&mut self.result, span.hi.0);

    id
  }

  fn set_child(&mut self, offset: usize, child_id: usize, idx: usize) {
    let pos = offset + (idx * 4);
    write_usize(&mut self.result, child_id, pos);
  }

  fn serialize_module_decl(
    &mut self,
    module_decl: &ModuleDecl,
    parent_id: usize,
  ) -> usize {
    match module_decl {
      ModuleDecl::Import(node) => {
        self.push_node(AstNode::Import, parent_id, &node.span)
      }
      ModuleDecl::ExportDecl(node) => {
        self.push_node(AstNode::ExportDecl, parent_id, &node.span)
      }
      ModuleDecl::ExportNamed(node) => {
        let id = self.next_id();

        let mut flags = FlagValue::new();
        flags.set(Flag::ExportType);

        let src_id = node
          .src
          .as_ref()
          .map_or(0, |src| serialize_lit(self, &Lit::Str(*src.clone()), id));

        // FIXME: I don't think these are valid
        let attr_id = node.with.as_ref().map_or(0, |attributes| {
          self.serialize_expr(&Expr::Object(*attributes.clone()), id)
        });

        let spec_ids = node
          .specifiers
          .iter()
          .map(|spec| {
            match spec {
              ExportSpecifier::Named(child) => {
                let spec_id = self.next_id();

                let mut flags = FlagValue::new();
                flags.set(Flag::ExportType);

                let org_id =
                  self.serialize_module_exported_name(&child.orig, spec_id);

                let exported_id =
                  child.exported.as_ref().map_or(0, |exported| {
                    self.serialize_module_exported_name(exported, spec_id)
                  });

                self.write_node(
                  spec_id,
                  AstNode::ExportSpecifier,
                  id,
                  &child.span,
                );
                self.write_flags(&flags);
                self.write_id(org_id);
                self.write_id(exported_id);

                spec_id
              }

              // These two aren't syntactically valid
              ExportSpecifier::Namespace(_) => todo!(),
              ExportSpecifier::Default(_) => todo!(),
            }
          })
          .collect::<Vec<_>>();

        self.write_node(
          id,
          AstNode::ExportNamedDeclaration,
          parent_id,
          &node.span,
        );
        self.write_flags(&flags);
        self.write_id(src_id);
        self.write_id(attr_id);
        self.write_ids(spec_ids);

        id
      }
      ModuleDecl::ExportDefaultDecl(node) => {
        self.push_node(AstNode::ExportDefaultDecl, parent_id, &node.span)
      }
      ModuleDecl::ExportDefaultExpr(node) => {
        self.push_node(AstNode::ExportDefaultExpr, parent_id, &node.span)
      }
      ModuleDecl::ExportAll(node) => {
        self.push_node(AstNode::ExportAll, parent_id, &node.span)
      }
      ModuleDecl::TsImportEquals(node) => {
        self.push_node(AstNode::TsImportEquals, parent_id, &node.span)
      }
      ModuleDecl::TsExportAssignment(node) => {
        self.push_node(AstNode::TsExportAssignment, parent_id, &node.span)
      }
      ModuleDecl::TsNamespaceExport(node) => {
        self.push_node(AstNode::TsNamespaceExport, parent_id, &node.span)
      }
    }
  }

  fn serialize_stmt(&mut self, stmt: &Stmt, parent_id: usize) -> usize {
    match stmt {
      Stmt::Block(node) => {
        let id = self.next_id();

        let children = node
          .stmts
          .iter()
          .map(|stmt| self.serialize_stmt(stmt, parent_id))
          .collect::<Vec<_>>();

        self.write_node(id, AstNode::Block, parent_id, &node.span);
        self.write_ids(children);

        id
      }
      Stmt::Empty(_) => 0,
      Stmt::Debugger(node) => {
        self.push_node(AstNode::Debugger, parent_id, &node.span)
      }
      Stmt::With(_) => todo!(),
      Stmt::Return(node) => {
        let id = self.next_id();

        let arg_id = node
          .arg
          .as_ref()
          .map_or(0, |arg| self.serialize_expr(arg, id));

        self.write_node(id, AstNode::Return, parent_id, &node.span);
        self.write_id(arg_id);

        id
      }
      Stmt::Labeled(node) => {
        let id = self.next_id();

        let ident_id = self.serialize_ident(&node.label, id);
        let stmt_id = self.serialize_stmt(&node.body, id);

        self.write_node(id, AstNode::Labeled, parent_id, &node.span);
        self.write_id(ident_id);
        self.write_id(stmt_id);

        id
      }
      Stmt::Break(node) => {
        let id = self.next_id();

        let arg_id = node
          .label
          .as_ref()
          .map_or(0, |label| self.serialize_ident(label, id));

        self.write_node(id, AstNode::Break, parent_id, &node.span);
        self.write_id(arg_id);

        id
      }
      Stmt::Continue(node) => {
        let id = self.next_id();

        let arg_id = node
          .label
          .as_ref()
          .map_or(0, |label| self.serialize_ident(label, id));

        self.write_node(id, AstNode::Continue, parent_id, &node.span);
        self.write_id(arg_id);

        id
      }
      Stmt::If(node) => {
        let id = self.next_id();

        let test_id = self.serialize_expr(node.test.as_ref(), id);
        let cons_id = self.serialize_stmt(node.cons.as_ref(), id);

        let alt_id = node
          .alt
          .as_ref()
          .map_or(0, |alt| self.serialize_stmt(alt, id));

        self.write_node(id, AstNode::IfStatement, parent_id, &node.span);
        self.write_id(test_id);
        self.write_id(cons_id);
        self.write_id(alt_id);

        id
      }
      Stmt::Switch(node) => {
        let id = self.next_id();

        let expr_id = self.serialize_expr(&node.discriminant, id);

        let case_ids = node
          .cases
          .iter()
          .map(|case| {
            let child_id = self.next_id();

            let test_id = case
              .test
              .as_ref()
              .map_or(0, |test| self.serialize_expr(test, child_id));

            let cons = case
              .cons
              .iter()
              .map(|cons| self.serialize_stmt(cons, child_id))
              .collect::<Vec<_>>();

            self.write_node(child_id, AstNode::SwitchCase, id, &case.span);
            self.write_id(test_id);
            self.write_ids(cons);

            child_id
          })
          .collect::<Vec<_>>();

        self.write_node(id, AstNode::Switch, parent_id, &node.span);
        self.write_id(expr_id);
        self.write_ids(case_ids);

        id
      }
      Stmt::Throw(node) => {
        let id = self.next_id();

        let expr_id = self.serialize_expr(&node.arg, id);

        self.write_node(id, AstNode::Throw, parent_id, &node.span);
        self.write_id(expr_id);

        id
      }
      Stmt::Try(node) => {
        let id = self.next_id();

        let block_id =
          self.serialize_stmt(&Stmt::Block(node.block.clone()), id);

        let catch_id = node.handler.as_ref().map_or(0, |catch| {
          let clause_id = self.next_id();

          let param_id = catch
            .param
            .as_ref()
            .map_or(0, |param| serialize_pat(self, param, clause_id));

          let body_id =
            self.serialize_stmt(&Stmt::Block(catch.body.clone()), id);

          self.write_node(clause_id, AstNode::CatchClause, id, &catch.span);
          self.write_id(param_id);
          self.write_id(body_id);

          clause_id
        });

        let final_id = node.finalizer.as_ref().map_or(0, |finalizer| {
          self.serialize_stmt(&Stmt::Block(finalizer.clone()), id)
        });

        self.write_node(id, AstNode::TryStatement, parent_id, &node.span);
        self.write_id(block_id);
        self.write_id(catch_id);
        self.write_id(final_id);

        id
      }
      Stmt::While(node) => {
        let id = self.next_id();

        let test_id = self.serialize_expr(node.test.as_ref(), id);
        let stmt_id = self.serialize_stmt(node.body.as_ref(), id);

        self.write_node(id, AstNode::While, parent_id, &node.span);
        self.write_id(test_id);
        self.write_id(stmt_id);

        id
      }
      Stmt::DoWhile(node) => {
        let id = self.next_id();

        let expr_id = self.serialize_expr(node.test.as_ref(), id);
        let stmt_id = self.serialize_stmt(node.body.as_ref(), id);

        self.write_node(id, AstNode::DoWhileStatement, parent_id, &node.span);
        self.write_id(expr_id);
        self.write_id(stmt_id);

        id
      }
      Stmt::For(node) => {
        let id = self.next_id();

        let init_id = node.init.as_ref().map_or(0, |init| match init {
          VarDeclOrExpr::VarDecl(var_decl) => {
            self.serialize_stmt(&Stmt::Decl(Decl::Var(var_decl.clone())), id)
          }
          VarDeclOrExpr::Expr(expr) => self.serialize_expr(expr, id),
        });

        let test_id = node
          .test
          .as_ref()
          .map_or(0, |expr| self.serialize_expr(expr, id));
        let update_id = node
          .update
          .as_ref()
          .map_or(0, |expr| self.serialize_expr(expr, id));
        let body_id = self.serialize_stmt(node.body.as_ref(), id);

        self.write_node(id, AstNode::ForStatement, parent_id, &node.span);
        self.write_id(init_id);
        self.write_id(test_id);
        self.write_id(update_id);
        self.write_id(body_id);

        id
      }
      Stmt::ForIn(node) => {
        let id = self.next_id();

        let left_id = serialize_for_head(self, &node.left, id);
        let right_id = self.serialize_expr(node.right.as_ref(), id);
        let body_id = self.serialize_stmt(node.body.as_ref(), id);

        self.write_node(id, AstNode::ForInStatement, parent_id, &node.span);
        self.write_id(left_id);
        self.write_id(right_id);
        self.write_id(body_id);

        id
      }
      Stmt::ForOf(node) => {
        let id = self.next_id();

        let mut flags = FlagValue::new();
        flags.set(Flag::ForAwait);

        let left_id = serialize_for_head(self, &node.left, id);
        let right_id = self.serialize_expr(node.right.as_ref(), id);
        let body_id = self.serialize_stmt(node.body.as_ref(), id);

        self.write_node(id, AstNode::ForOfStatement, parent_id, &node.span);
        self.write_flags(&flags);
        self.write_id(left_id);
        self.write_id(right_id);
        self.write_id(body_id);

        id
      }
      Stmt::Decl(node) => serialize_decl(self, node, parent_id),
      Stmt::Expr(node) => {
        let id = self.next_id();

        let child_id = self.serialize_expr(node.expr.as_ref(), id);
        self.write_node(id, AstNode::Expr, parent_id, &node.span);
        self.write_id(child_id);

        id
      }
    }
  }

  fn serialize_expr(&mut self, expr: &Expr, parent_id: usize) -> usize {
    match expr {
      Expr::This(node) => self.push_node(AstNode::This, parent_id, &node.span),
      Expr::Array(node) => {
        let id = self.next_id();

        let elem_ids = node
          .elems
          .iter()
          .map(|item| {
            item
              .as_ref()
              .map_or(0, |item| self.serialize_expr_or_spread(item, id))
          })
          .collect::<Vec<_>>();

        self.write_node(id, AstNode::Array, parent_id, &node.span);
        self.write_ids(elem_ids);

        id
      }
      Expr::Object(node) => {
        let id = self.next_id();

        let prop_ids = node
          .props
          .iter()
          .map(|prop| {
            match prop {
              PropOrSpread::Spread(spread_element) => serialize_spread(
                self,
                spread_element.expr.as_ref(),
                &spread_element.dot3_token,
                parent_id,
              ),
              PropOrSpread::Prop(prop) => {
                let mut flags = FlagValue::new();
                let prop_id = self.next_id();

                // FIXME: optional
                let (key_id, value_id) = match prop.as_ref() {
                  Prop::Shorthand(ident) => {
                    flags.set(Flag::PropShorthand);

                    let child_id = self.serialize_ident(ident, prop_id);
                    (child_id, child_id)
                  }
                  Prop::KeyValue(key_value_prop) => {
                    if let PropName::Computed(_) = key_value_prop.key {
                      flags.set(Flag::PropComputed)
                    }

                    let key_id =
                      serialize_prop_name(self, &key_value_prop.key, prop_id);
                    let value_id = self
                      .serialize_expr(key_value_prop.value.as_ref(), prop_id);

                    (key_id, value_id)
                  }
                  Prop::Assign(assign_prop) => {
                    let child_id = self.next_id();

                    let key_id =
                      self.serialize_ident(&assign_prop.key, prop_id);
                    let value_id =
                      self.serialize_expr(assign_prop.value.as_ref(), prop_id);

                    self.write_node(
                      child_id,
                      AstNode::AssignmentPattern,
                      prop_id,
                      &assign_prop.span,
                    );
                    self.write_id(key_id);
                    self.write_id(value_id);

                    (key_id, value_id)
                  }
                  Prop::Getter(getter_prop) => {
                    flags.set(Flag::PropGetter);

                    let key_id =
                      serialize_prop_name(self, &getter_prop.key, prop_id);

                    let value_id = self.serialize_expr(
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
                      serialize_prop_name(self, &setter_prop.key, prop_id);

                    let param = Param::from(*setter_prop.param.clone());

                    let value_id = self.serialize_expr(
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
                      serialize_prop_name(self, &method_prop.key, prop_id);

                    let value_id = self.serialize_expr(
                      &Expr::Fn(FnExpr {
                        ident: None,
                        function: method_prop.function.clone(),
                      }),
                      prop_id,
                    );

                    (key_id, value_id)
                  }
                };

                self.write_node(prop_id, AstNode::Property, id, &prop.span());
                self.write_flags(&flags);
                self.write_id(key_id);
                self.write_id(value_id);

                prop_id
              }
            }
          })
          .collect::<Vec<_>>();

        self.write_node(id, AstNode::Object, parent_id, &node.span);
        self.write_ids(prop_ids);

        id
      }
      Expr::Fn(node) => {
        let id = self.next_id();
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
          .map_or(0, |ident| self.serialize_ident(ident, id));

        let type_param_id = fn_obj.type_params.as_ref().map_or(0, |param| {
          todo!() // FIXME
        });

        let param_ids = fn_obj
          .params
          .iter()
          .map(|param| serialize_pat(self, &param.pat, id))
          .collect::<Vec<_>>();

        let return_id = fn_obj.return_type.as_ref().map_or(0, |ret_type| {
          todo!() // FIXME
        });

        let block_id = fn_obj.body.as_ref().map_or(0, |block| {
          self.serialize_stmt(&Stmt::Block(block.clone()), id)
        });

        self.write_node(
          id,
          AstNode::FunctionExpression,
          parent_id,
          &fn_obj.span,
        );
        self.write_flags(&flags);
        self.write_id(ident_id);
        self.write_id(type_param_id);
        self.write_ids(param_ids);
        self.write_id(return_id);
        self.write_id(block_id);

        id
      }
      Expr::Unary(node) => {
        let id = self.next_id();

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

        let child_id = self.serialize_expr(&node.arg, id);

        self.write_node(id, AstNode::Unary, parent_id, &node.span);
        self.write_flags(&flags);
        self.write_id(child_id);

        id
      }
      Expr::Update(node) => {
        let id = self.next_id();

        let mut flags = FlagValue::new();
        if node.prefix {
          flags.set(Flag::UpdatePrefix);
        }
        flags.set(match node.op {
          UpdateOp::PlusPlus => Flag::UpdatePlusPlus,
          UpdateOp::MinusMinus => Flag::UpdateMinusMinus,
        });

        let child_id = self.serialize_expr(node.arg.as_ref(), id);

        self.write_node(id, AstNode::UpdateExpression, parent_id, &node.span);
        self.write_flags(&flags);
        self.write_id(child_id);

        id
      }
      Expr::Bin(node) => {
        let (node_type, flag) = match node.op {
          BinaryOp::LogicalOr => (AstNode::LogicalExpression, Flag::LogicalOr),
          BinaryOp::LogicalAnd => {
            (AstNode::LogicalExpression, Flag::LogicalAnd)
          }
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

        let id = self.next_id();

        let mut flags = FlagValue::new();
        flags.set(flag);

        let left_id = self.serialize_expr(node.left.as_ref(), id);
        let right_id = self.serialize_expr(node.right.as_ref(), id);

        self.write_node(id, node_type, parent_id, &node.span);
        self.write_flags(&flags);
        self.write_id(left_id);
        self.write_id(right_id);

        id
      }
      Expr::Assign(node) => {
        let id = self.next_id();

        let left_id = match &node.left {
          AssignTarget::Simple(simple_assign_target) => {
            match simple_assign_target {
              SimpleAssignTarget::Ident(binding_ident) => {
                self.serialize_ident(&binding_ident.id, id)
              }
              SimpleAssignTarget::Member(member_expr) => {
                self.serialize_expr(&Expr::Member(member_expr.clone()), id)
              }
              SimpleAssignTarget::SuperProp(super_prop_expr) => todo!(),
              SimpleAssignTarget::Paren(paren_expr) => todo!(),
              SimpleAssignTarget::OptChain(opt_chain_expr) => todo!(),
              SimpleAssignTarget::TsAs(ts_as_expr) => todo!(),
              SimpleAssignTarget::TsSatisfies(ts_satisfies_expr) => todo!(),
              SimpleAssignTarget::TsNonNull(ts_non_null_expr) => todo!(),
              SimpleAssignTarget::TsTypeAssertion(ts_type_assertion) => todo!(),
              SimpleAssignTarget::TsInstantiation(ts_instantiation) => todo!(),
              SimpleAssignTarget::Invalid(invalid) => todo!(),
            }
          }
          AssignTarget::Pat(target) => match target {
            AssignTargetPat::Array(array_pat) => {
              serialize_pat(self, &Pat::Array(array_pat.clone()), id)
            }
            AssignTargetPat::Object(object_pat) => {
              serialize_pat(self, &Pat::Object(object_pat.clone()), id)
            }
            AssignTargetPat::Invalid(invalid) => todo!(),
          },
        };

        let right_id = self.serialize_expr(node.right.as_ref(), id);

        self.write_node(id, AstNode::Assign, parent_id, &node.span);
        self.write_u8(assign_op_to_flag(node.op));
        self.write_id(left_id);
        self.write_id(right_id);

        id
      }
      Expr::Member(node) => {
        let id = self.next_id();

        let mut flags = FlagValue::new();
        let obj_id = self.serialize_expr(node.obj.as_ref(), id);

        let prop_id = match &node.prop {
          MemberProp::Ident(ident_name) => {
            serialize_ident_name(self, ident_name, id)
          }
          MemberProp::PrivateName(private_name) => {
            let child_id = self.push_node(
              AstNode::PrivateIdentifier,
              id,
              &private_name.span,
            );

            let str_id = self.str_table.insert(private_name.name.as_str());
            append_usize(&mut self.result, str_id);

            child_id
          }
          MemberProp::Computed(computed_prop_name) => {
            flags.set(Flag::MemberComputed);
            self.serialize_expr(computed_prop_name.expr.as_ref(), id)
          }
        };

        self.write_node(id, AstNode::MemberExpression, parent_id, &node.span);
        self.write_flags(&flags);
        self.write_id(obj_id);
        self.write_id(prop_id);

        id
      }
      Expr::SuperProp(node) => {
        let id = self.next_id();

        let mut flags = FlagValue::new();
        if let SuperProp::Computed(_) = node.prop {
          flags.set(Flag::MemberComputed)
        }

        let super_id = self.push_node(AstNode::Super, id, &node.obj.span);

        let child_id = match &node.prop {
          SuperProp::Ident(ident_name) => {
            serialize_ident_name(self, ident_name, id)
          }
          SuperProp::Computed(prop) => self.serialize_expr(&prop.expr, id),
        };

        self.write_node(id, AstNode::MemberExpression, parent_id, &node.span);
        self.write_flags(&flags);
        self.write_id(super_id);
        self.write_id(child_id);

        id
      }
      Expr::Cond(node) => {
        let id = self.next_id();

        let test_id = self.serialize_expr(node.test.as_ref(), id);
        let cons_id = self.serialize_expr(node.cons.as_ref(), id);
        let alt_id = self.serialize_expr(node.alt.as_ref(), id);

        self.write_node(id, AstNode::Cond, parent_id, &node.span);
        self.write_id(test_id);
        self.write_id(cons_id);
        self.write_id(alt_id);

        id
      }
      Expr::Call(node) => {
        let id = self.next_id();

        let callee_id = match &node.callee {
          Callee::Super(super_node) => {
            self.push_node(AstNode::Super, id, &super_node.span)
          }
          Callee::Import(import) => todo!(),
          Callee::Expr(expr) => self.serialize_expr(expr, id),
        };

        let type_id = node.type_args.as_ref().map_or(0, |type_arg| {
          todo!() // FIXME
        });

        let arg_ids = node
          .args
          .iter()
          .map(|arg| self.serialize_expr_or_spread(arg, id))
          .collect::<Vec<_>>();

        self.write_node(id, AstNode::Call, parent_id, &node.span);
        self.write_id(callee_id);
        self.write_id(type_id);
        self.write_ids(arg_ids);

        id
      }
      Expr::New(node) => {
        let id = self.next_id();

        let callee_id = self.serialize_expr(node.callee.as_ref(), id);

        let arg_ids: Vec<usize> = node.args.as_ref().map_or(vec![], |args| {
          args
            .iter()
            .map(|arg| self.serialize_expr_or_spread(arg, id))
            .collect::<Vec<_>>()
        });

        let type_arg_id = node.type_args.as_ref().map_or(0, |type_arg| {
          todo!() // FIXME
        });

        self.write_node(id, AstNode::New, parent_id, &node.span);
        self.write_id(callee_id);
        self.write_id(type_arg_id);
        self.write_ids(arg_ids);

        id
      }
      Expr::Seq(node) => {
        let id = self.next_id();

        let children = node
          .exprs
          .iter()
          .map(|expr| self.serialize_expr(expr, id))
          .collect::<Vec<_>>();

        self.write_node(id, AstNode::Seq, parent_id, &node.span);
        self.write_ids(children);

        id
      }
      Expr::Ident(node) => self.serialize_ident(node, parent_id),
      Expr::Lit(node) => serialize_lit(self, node, parent_id),
      Expr::Tpl(node) => {
        let id = self.next_id();

        let quasi_ids = node
          .quasis
          .iter()
          .map(|quasi| {
            let tpl_id = self.next_id();

            let mut flags = FlagValue::new();
            flags.set(Flag::TplTail);

            let raw_str_id = self.str_table.insert(quasi.raw.as_str());

            let cooked_str_id = quasi
              .cooked
              .as_ref()
              .map_or(0, |cooked| self.str_table.insert(cooked.as_str()));

            self.write_node(tpl_id, AstNode::TemplateElement, id, &quasi.span);
            self.write_flags(&flags);
            append_usize(&mut self.result, raw_str_id);
            append_usize(&mut self.result, cooked_str_id);

            tpl_id
          })
          .collect::<Vec<_>>();

        let expr_ids = node
          .exprs
          .iter()
          .map(|expr| self.serialize_expr(expr, id))
          .collect::<Vec<_>>();

        self.write_node(id, AstNode::TemplateLiteral, parent_id, &node.span);
        self.write_ids(quasi_ids);
        self.write_ids(expr_ids);

        id
      }
      Expr::TaggedTpl(tagged_tpl) => {
        let id =
          self.push_node(AstNode::TaggedTpl, parent_id, &tagged_tpl.span);

        // FIXME

        id
      }
      Expr::Arrow(node) => {
        let id = self.next_id();

        let mut flags = FlagValue::new();
        if node.is_async {
          flags.set(Flag::FnAsync);
        }
        if node.is_generator {
          flags.set(Flag::FnGenerator);
        }

        let type_param_id =
          node.type_params.as_ref().map_or(0, |param| todo!());

        let param_ids = node
          .params
          .iter()
          .map(|param| serialize_pat(self, param, id))
          .collect::<Vec<_>>();

        let body_id = match node.body.as_ref() {
          BlockStmtOrExpr::BlockStmt(block_stmt) => {
            self.serialize_stmt(&Stmt::Block(block_stmt.clone()), id)
          }
          BlockStmtOrExpr::Expr(expr) => self.serialize_expr(expr.as_ref(), id),
        };

        let return_type_id = node.return_type.as_ref().map_or(0, |arg| {
          todo!() // FIXME
        });

        self.write_node(
          id,
          AstNode::ArrowFunctionExpression,
          parent_id,
          &node.span,
        );
        self.write_flags(&flags);
        self.write_id(type_param_id);
        self.write_ids(param_ids);
        self.write_id(body_id);
        self.write_id(return_type_id);

        id
      }
      Expr::Class(node) => {
        let id =
          self.push_node(AstNode::ClassExpr, parent_id, &node.class.span);

        // FIXME

        id
      }
      Expr::Yield(node) => {
        let id = self.next_id();

        let mut flags = FlagValue::new();
        if node.delegate {
          flags.set(Flag::YieldDelegate)
        }

        let arg_id = node
          .arg
          .as_ref()
          .map_or(0, |arg| self.serialize_expr(arg.as_ref(), id));

        self.write_node(id, AstNode::YieldExpression, parent_id, &node.span);
        self.write_flags(&flags);
        self.write_id(arg_id);

        id
      }
      Expr::MetaProp(node) => {
        self.push_node(AstNode::MetaProp, parent_id, &node.span)
      }
      Expr::Await(node) => {
        let id = self.next_id();
        let arg_id = self.serialize_expr(node.arg.as_ref(), id);

        self.write_node(id, AstNode::AwaitExpression, parent_id, &node.span);
        self.write_id(arg_id);

        id
      }
      Expr::Paren(node) => {
        // Paren nodes are treated as a syntax only thing in TSEStree
        // and are never materialized to actual AST nodes.
        self.serialize_expr(&node.expr, parent_id)
      }
      Expr::JSXMember(node) => serialize_jsx_member_expr(self, node, parent_id),
      Expr::JSXNamespacedName(node) => {
        serialize_jsx_namespaced_name(self, node, parent_id)
      }
      Expr::JSXEmpty(node) => serialize_jsx_empty_expr(self, node, parent_id),
      Expr::JSXElement(node) => serialize_jsx_element(self, node, parent_id),
      Expr::JSXFragment(node) => serialize_jsx_fragment(self, node, parent_id),
      Expr::TsTypeAssertion(node) => {
        let id =
          self.push_node(AstNode::TsTypeAssertion, parent_id, &node.span);

        // FIXME

        id
      }
      Expr::TsConstAssertion(node) => {
        let id = self.next_id();
        let expr_id = self.serialize_expr(node.expr.as_ref(), id);

        self.write_node(id, AstNode::TsConstAssertion, parent_id, &node.span);
        self.write_id(expr_id);

        id
      }
      Expr::TsNonNull(node) => {
        let id = self.next_id();
        let expr_id = self.serialize_expr(node.expr.as_ref(), id);

        self.write_node(id, AstNode::TsNonNull, parent_id, &node.span);
        self.write_id(expr_id);

        id
      }
      Expr::TsAs(node) => {
        let id = self.next_id();

        let expr_id = self.serialize_expr(node.expr.as_ref(), id);
        let type_id = serialize_ts_type(self, node.type_ann.as_ref(), id);

        self.write_node(id, AstNode::TsAs, parent_id, &node.span);
        self.write_id(expr_id);
        self.write_id(type_id);

        id
      }
      Expr::TsInstantiation(node) => {
        let id = self.next_id();

        let expr_id = self.serialize_expr(node.expr.as_ref(), id);
        // FIXME
        // let expr_id = ctx.serialize_expr( ts_instantiation.type_args.as_ref(), id);
        self.write_node(id, AstNode::TsInstantiation, parent_id, &node.span);
        self.write_id(expr_id);

        id
      }
      Expr::TsSatisfies(node) => {
        let id = self.next_id();

        let expr_id = self.serialize_expr(node.expr.as_ref(), id);
        let type_id = serialize_ts_type(self, node.type_ann.as_ref(), id);

        self.write_node(id, AstNode::TsSatisfies, parent_id, &node.span);
        self.write_id(expr_id);
        self.write_id(type_id);

        id
      }
      Expr::PrivateName(node) => {
        let id =
          self.push_node(AstNode::PrivateIdentifier, parent_id, &node.span);

        // FIXME

        id
      }
      Expr::OptChain(node) => {
        let id = self.push_node(AstNode::OptChain, parent_id, &node.span);

        // FIXME

        id
      }
      Expr::Invalid(invalid) => {
        // ctx.push_node(result, AstNode::Invalid.into(), &invalid.span);
        todo!()
      }
    }
  }

  fn serialize_ident(&mut self, ident: &Ident, parent_id: usize) -> usize {
    let id = self.push_node(AstNode::Identifier, parent_id, &ident.span);

    let str_id = self.str_table.insert(ident.sym.as_str());
    append_usize(&mut self.result, str_id);

    id
  }

  fn serialize_expr_or_spread(
    &mut self,
    arg: &ExprOrSpread,
    parent_id: usize,
  ) -> usize {
    if let Some(spread) = &arg.spread {
      serialize_spread(self, &arg.expr, spread, parent_id)
    } else {
      self.serialize_expr(arg.expr.as_ref(), parent_id)
    }
  }

  fn serialize_module_exported_name(
    &mut self,
    name: &ModuleExportName,
    parent_id: usize,
  ) -> usize {
    match &name {
      ModuleExportName::Ident(ident) => self.serialize_ident(ident, parent_id),
      ModuleExportName::Str(lit) => {
        serialize_lit(self, &Lit::Str(lit.clone()), parent_id)
      }
    }
  }
}

pub fn serialize_ast_bin(parsed_source: &ParsedSource) -> Vec<u8> {
  let mut ctx = SerializeCtx::new();

  let parent_id = 0;

  let program = &parsed_source.program();
  let mut flags = FlagValue::new();

  // eprintln!("SWC {:#?}", program);

  match program.as_ref() {
    Program::Module(module) => {
      let id = ctx.next_id();

      flags.set(Flag::ProgramModule);

      let child_ids = module
        .body
        .iter()
        .map(|item| match item {
          ModuleItem::ModuleDecl(module_decl) => {
            ctx.serialize_module_decl(module_decl, parent_id)
          }
          ModuleItem::Stmt(stmt) => ctx.serialize_stmt(stmt, id),
        })
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::Program, parent_id, &module.span);
      ctx.write_flags(&flags);
      ctx.write_ids(child_ids);
    }
    Program::Script(script) => {
      let id = ctx.next_id();

      let child_ids = script
        .body
        .iter()
        .map(|stmt| ctx.serialize_stmt(stmt, id))
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::Program, parent_id, &script.span);
      ctx.write_flags(&flags);
      ctx.write_ids(child_ids);
    }
  }

  let mut result: Vec<u8> = vec![];

  // Serialize string table
  // eprintln!("STRING {:#?}", ctx.str_table);
  result.append(&mut ctx.str_table.serialize());

  // Serialize ids
  append_usize(&mut result, ctx.id_to_offset.len());

  let offset = result.len() + (ctx.id_to_offset.len() * 4);

  for (_i, value) in ctx.id_to_offset {
    append_usize(&mut result, value + offset);
  }

  // Append serialized AST
  result.append(&mut ctx.result);
  result
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

      let ident_id = ctx.serialize_ident(&node.ident, id);
      let type_param_id =
        node.class.type_params.as_ref().map_or(0, |type_params| {
          // FIXME
          todo!()
        });

      let super_class_id = node
        .class
        .super_class
        .as_ref()
        .map_or(0, |super_class| ctx.serialize_expr(super_class, id));

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
                ctx.serialize_stmt(&Stmt::Block(body.clone()), member_id)
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
              );
              ctx.write_flags(&flags);
              ctx.write_id(key_id);
              ctx.write_id(body_id);
              ctx.write_ids(params);

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
                ctx.serialize_stmt(&Stmt::Block(body.clone()), member_id)
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
              );
              ctx.write_flags(&flags);
              ctx.write_id(key_id);
              ctx.write_id(body_id);
              ctx.write_ids(params);

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
      );
      ctx.write_flags(&flags);
      ctx.write_id(ident_id);
      ctx.write_id(type_param_id);
      ctx.write_id(super_class_id);
      ctx.write_id(super_type_params);
      ctx.write_ids(implement_ids);
      ctx.write_ids(member_ids);

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

      let ident_id = ctx.serialize_ident(&node.ident, parent_id);

      let type_param_id =
        node.function.type_params.as_ref().map_or(0, |type_param| {
          // FIXME
          todo!()
        });

      let return_type =
        node.function.return_type.as_ref().map_or(0, |return_type| {
          // FIXME
          todo!()
        });

      let body_id = node
        .function
        .body
        .as_ref()
        .map_or(0, |body| ctx.serialize_stmt(&Stmt::Block(body.clone()), id));

      let params = node
        .function
        .params
        .iter()
        .map(|param| serialize_pat(ctx, &param.pat, id))
        .collect::<Vec<_>>();

      ctx.write_node(id, AstNode::Fn, parent_id, &node.function.span);
      ctx.write_flags(&flags);
      ctx.write_id(ident_id);
      ctx.write_id(type_param_id);
      ctx.write_id(return_type);
      ctx.write_id(body_id);
      ctx.write_ids(params);

      id
    }
    Decl::Var(node) => {
      let id = ctx.push_node(AstNode::Var, parent_id, &node.span);
      let mut flags = FlagValue::new();
      if node.declare {
        flags.set(Flag::VarDeclare)
      }
      match node.kind {
        VarDeclKind::Var => flags.set(Flag::VarVar),
        VarDeclKind::Let => flags.set(Flag::VarLet),
        VarDeclKind::Const => flags.set(Flag::VarConst),
      }
      ctx.result.push(flags.0);

      let offset = ctx.reserve_child_ids_with_count(node.decls.len());

      for (i, decl) in node.decls.iter().enumerate() {
        let child_id =
          ctx.push_node(AstNode::VariableDeclarator, id, &decl.span);

        ctx.set_child(offset, child_id, i);
        // FIXME: Definite?

        let child_offset = ctx.reserve_child_ids(2); // Name + init

        let decl_id = serialize_pat(ctx, &decl.name, child_id);
        ctx.set_child(child_offset, decl_id, 0);

        if let Some(init) = &decl.init {
          let expr_id = ctx.serialize_expr(init.as_ref(), child_id);
          ctx.set_child(child_offset, expr_id, 1);
        }
      }

      id
    }
    Decl::Using(node) => {
      let id = ctx.push_node(AstNode::Using, parent_id, &node.span);
      let offset = ctx.reserve_child_ids_with_count(node.decls.len());

      for (i, decl) in node.decls.iter().enumerate() {
        // FIXME
      }

      id
    }
    Decl::TsInterface(node) => {
      // ident + body + type_ann + extends(Vec)
      let count = 3 + node.extends.len();

      let id = ctx.push_node(AstNode::TsInterface, parent_id, &node.span);

      // FIXME

      id
    }
    Decl::TsTypeAlias(node) => {
      // FIXME: Declare flag
      let id = ctx.push_node(AstNode::TsTypeAlias, parent_id, &node.span);

      let offset = ctx.reserve_child_ids(1);

      let ident_id = ctx.serialize_ident(&node.id, id);
      ctx.set_child(offset, ident_id, 0);

      // FIXME
      // let foo = ts_type_alias_decl.type_ann

      id
    }
    Decl::TsEnum(node) => {
      let id = ctx.push_node(AstNode::TsEnum, parent_id, &node.span);

      // Ident + member count
      let count = 1 + node.members.len();
      let offset = ctx.reserve_child_ids_with_count(count);

      let ident_id = ctx.serialize_ident(&node.id, parent_id);
      ctx.set_child(offset, ident_id, id);

      for (i, member) in node.members.iter().enumerate() {
        // FIXME
        // let member_id = member.
        // ctx.set_child(id, member_id, id);
      }

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

fn serialize_jsx_element(
  ctx: &mut SerializeCtx,
  node: &JSXElement,
  parent_id: usize,
) -> usize {
  let id = ctx.push_node(AstNode::JSXElement, parent_id, &node.span);
  let offset = ctx.reserve_child_ids(2);
  let child_offset = ctx.reserve_child_ids_with_count(node.children.len());

  let opening_id = serialize_jsx_opening_element(ctx, &node.opening, id);
  ctx.set_child(offset, opening_id, 0);

  if let Some(closing) = &node.closing {
    let closing_id =
      ctx.push_node(AstNode::JSXClosingElement, id, &closing.span);
    ctx.set_child(offset, closing_id, 1);

    let closing_offset = ctx.reserve_child_ids(1);
    let child_id = serialize_jsx_element_name(ctx, &closing.name, id);
    ctx.set_child(closing_offset, child_id, 1);
  }

  serialize_jsx_children(ctx, &node.children, id, child_offset);

  id
}

fn serialize_jsx_fragment(
  ctx: &mut SerializeCtx,
  node: &JSXFragment,
  parent_id: usize,
) -> usize {
  let id = ctx.push_node(AstNode::JSXFragment, parent_id, &node.span);

  let offset = ctx.reserve_child_ids(2);
  let child_offset = ctx.reserve_child_ids_with_count(node.children.len());

  let opening_id =
    ctx.push_node(AstNode::JSXOpeningFragment, id, &node.opening.span);
  let closing_id =
    ctx.push_node(AstNode::JSXClosingFragment, id, &node.closing.span);

  ctx.set_child(offset, opening_id, 0);
  ctx.set_child(offset, closing_id, 1);

  serialize_jsx_children(ctx, &node.children, id, child_offset);

  id
}

fn serialize_jsx_children(
  ctx: &mut SerializeCtx,
  children: &[JSXElementChild],
  parent_id: usize,
  offset: usize,
) {
  for (i, child) in children.iter().enumerate() {
    let child_id = match child {
      JSXElementChild::JSXText(text) => {
        let id = ctx.push_node(AstNode::JSXText, parent_id, &text.span);

        let raw_id = ctx.str_table.insert(text.raw.as_str());
        let value_id = ctx.str_table.insert(text.value.as_str());

        append_usize(&mut ctx.result, raw_id);
        append_usize(&mut ctx.result, value_id);

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
    };
    ctx.set_child(offset, child_id, i);
  }
}

fn serialize_jsx_member_expr(
  ctx: &mut SerializeCtx,
  node: &JSXMemberExpr,
  parent_id: usize,
) -> usize {
  let id = ctx.push_node(AstNode::JSXMemberExpression, parent_id, &node.span);
  let offset = ctx.reserve_child_ids(2);

  let obj_id = match &node.obj {
    JSXObject::JSXMemberExpr(member) => {
      serialize_jsx_member_expr(ctx, member, id)
    }
    JSXObject::Ident(ident) => serialize_jsx_identifier(ctx, ident, parent_id),
  };

  let prop_id = serialize_ident_name_as_jsx_identifier(ctx, &node.prop, id);

  ctx.set_child(offset, obj_id, 0);
  ctx.set_child(offset, prop_id, 1);

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
  let id = ctx.push_node(AstNode::JSXOpeningElement, parent_id, &node.span);

  let mut flags = FlagValue::new();
  if node.self_closing {
    flags.set(Flag::JSXSelfClosing);
  }
  ctx.result.push(flags.0);

  let offset = ctx.reserve_child_ids(1);
  let child_offset = ctx.reserve_child_ids_with_count(node.attrs.len());

  let name_id = serialize_jsx_element_name(ctx, &node.name, id);
  ctx.set_child(offset, name_id, 0);

  // FIXME: type args

  for (i, attr) in node.attrs.iter().enumerate() {
    let child_id = match attr {
      JSXAttrOrSpread::JSXAttr(jsxattr) => {
        let attr_id = ctx.push_node(AstNode::JSXAttribute, id, &jsxattr.span);
        let offset = ctx.reserve_child_ids(2);

        let name_id = match &jsxattr.name {
          JSXAttrName::Ident(name) => {
            serialize_ident_name_as_jsx_identifier(ctx, name, attr_id)
          }
          JSXAttrName::JSXNamespacedName(node) => {
            serialize_jsx_namespaced_name(ctx, node, attr_id)
          }
        };
        ctx.set_child(offset, name_id, 0);

        if let Some(value) = &jsxattr.value {
          let value_id = match value {
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
          };

          ctx.set_child(offset, value_id, 1);
        }

        attr_id
      }
      JSXAttrOrSpread::SpreadElement(spread) => {
        let attr_id =
          ctx.push_node(AstNode::JSXSpreadAttribute, id, &spread.dot3_token);

        let offset = ctx.reserve_child_ids(1);
        let child_id = ctx.serialize_expr(&spread.expr, attr_id);
        ctx.set_child(offset, child_id, 0);

        attr_id
      }
    };

    ctx.set_child(child_offset, child_id, i);
  }

  id
}

fn serialize_jsx_container_expr(
  ctx: &mut SerializeCtx,
  node: &JSXExprContainer,
  parent_id: usize,
) -> usize {
  let id =
    ctx.push_node(AstNode::JSXExpressionContainer, parent_id, &node.span);

  let offset = ctx.reserve_child_ids(1);

  let child_id = match &node.expr {
    JSXExpr::JSXEmptyExpr(expr) => serialize_jsx_empty_expr(ctx, expr, id),
    JSXExpr::Expr(expr) => ctx.serialize_expr(expr, id),
  };

  ctx.set_child(offset, child_id, 0);

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
  let id = ctx.push_node(AstNode::JSXNamespacedName, parent_id, &node.span);

  let offset = ctx.reserve_child_ids(2);
  let ns_id = serialize_ident_name_as_jsx_identifier(ctx, &node.ns, id);
  let name_id = serialize_ident_name_as_jsx_identifier(ctx, &node.name, id);

  ctx.set_child(offset, ns_id, 0);
  ctx.set_child(offset, name_id, 1);

  id
}

fn serialize_ident_name_as_jsx_identifier(
  ctx: &mut SerializeCtx,
  node: &IdentName,
  parent_id: usize,
) -> usize {
  let id = ctx.push_node(AstNode::JSXIdentifier, parent_id, &node.span);

  let str_id = ctx.str_table.insert(node.sym.as_str());
  append_usize(&mut ctx.result, str_id);

  id
}

fn serialize_jsx_identifier(
  ctx: &mut SerializeCtx,
  node: &Ident,
  parent_id: usize,
) -> usize {
  let id = ctx.push_node(AstNode::JSXIdentifier, parent_id, &node.span);

  let str_id = ctx.str_table.insert(node.sym.as_str());
  append_usize(&mut ctx.result, str_id);

  id
}

fn serialize_pat(ctx: &mut SerializeCtx, pat: &Pat, parent_id: usize) -> usize {
  match pat {
    Pat::Ident(node) => ctx.serialize_ident(&node.id, parent_id),
    Pat::Array(node) => {
      let id = ctx.push_node(AstNode::ArrayPattern, parent_id, &node.span);

      let mut flags = FlagValue::new();
      if node.optional {
        flags.set(Flag::ParamOptional);
      }
      ctx.result.push(flags.0);

      let offset = ctx.reserve_child_ids(1);
      let elem_offset = ctx.reserve_child_ids_with_count(node.elems.len());

      // FIXME: Type Ann
      if let Some(type_ann) = &node.type_ann {
        // ctx.set_child(offset, child_id, 0);
      }

      for (i, elem) in node.elems.iter().enumerate() {
        if let Some(pat) = elem {
          let child_id = serialize_pat(ctx, pat, id);
          ctx.set_child(elem_offset, child_id, i);
        }
      }

      id
    }
    Pat::Rest(node) => {
      let id = ctx.push_node(AstNode::RestElement, parent_id, &node.span);

      let offset = ctx.reserve_child_ids(2);
      if let Some(type_ann) = &node.type_ann {
        // FIXME
        // ctx.set_child(offset, type_id, 0);
      }

      let arg_id = serialize_pat(ctx, &node.arg, parent_id);
      ctx.set_child(offset, arg_id, 1);

      id
    }
    Pat::Object(node) => {
      let id = ctx.push_node(AstNode::ObjectPattern, parent_id, &node.span);

      let mut flags = FlagValue::new();
      if node.optional {
        flags.set(Flag::ParamOptional);
      }
      ctx.result.push(flags.0);

      let offset = ctx.reserve_child_ids(1);
      let props_offset = ctx.reserve_child_ids_with_count(node.props.len());

      // FIXME: Type Ann
      if let Some(type_ann) = &node.type_ann {
        // ctx.set_child(offset, child_id, 0);
      }

      for (i, prop) in node.props.iter().enumerate() {
        let child_id = match prop {
          ObjectPatProp::KeyValue(key_value_prop) => {
            let child_id =
              ctx.push_node(AstNode::Property, id, &key_value_prop.span());
            let mut flags = FlagValue::new();
            if let PropName::Computed(_) = key_value_prop.key {
              flags.set(Flag::PropComputed)
            }
            ctx.result.push(flags.0);

            let child_offset = ctx.reserve_child_ids(2);

            let key_id =
              serialize_prop_name(ctx, &key_value_prop.key, child_id);
            let value_id =
              serialize_pat(ctx, key_value_prop.value.as_ref(), child_id);

            ctx.set_child(child_offset, key_id, 0);
            ctx.set_child(child_offset, value_id, 1);

            child_id
          }
          ObjectPatProp::Assign(assign_pat_prop) => {
            let child_id =
              ctx.push_node(AstNode::Property, id, &assign_pat_prop.span);
            ctx.result.push(0); // No flags

            let child_offset = ctx.reserve_child_ids(2);

            let ident_id =
              ctx.serialize_ident(&assign_pat_prop.key.id, parent_id);
            ctx.set_child(child_offset, ident_id, 0);

            if let Some(value) = &assign_pat_prop.value {
              let value_id = ctx.serialize_expr(value, child_id);
              ctx.set_child(child_offset, value_id, 1);
            }

            child_id
          }
          ObjectPatProp::Rest(rest_pat) => {
            serialize_pat(ctx, &Pat::Rest(rest_pat.clone()), parent_id)
          }
        };

        ctx.set_child(props_offset, child_id, i);
      }

      id
    }
    Pat::Assign(node) => {
      let id = ctx.push_node(AstNode::AssignmentPattern, parent_id, &node.span);

      let offset = ctx.reserve_child_ids(2);
      let left_id = serialize_pat(ctx, &node.left, id);
      let right_id = ctx.serialize_expr(&node.right, id);

      ctx.set_child(offset, left_id, 0);
      ctx.set_child(offset, right_id, 1);

      id
    }
    Pat::Invalid(node) => todo!(),
    Pat::Expr(node) => ctx.serialize_expr(node, parent_id),
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
    ForHead::UsingDecl(using_decl) => todo!(),
    ForHead::Pat(pat) => serialize_pat(ctx, pat, parent_id),
  }
}

fn serialize_spread(
  ctx: &mut SerializeCtx,
  expr: &Expr,
  span: &Span,
  parent_id: usize,
) -> usize {
  let id = ctx.push_node(AstNode::Spread, parent_id, span);

  let child_offset = ctx.reserve_child_ids(1);
  let expr_id = ctx.serialize_expr(expr, id);

  ctx.set_child(child_offset, expr_id, 0);

  id
}

fn serialize_ident_name(
  ctx: &mut SerializeCtx,
  ident_name: &IdentName,
  parent_id: usize,
) -> usize {
  let id = ctx.push_node(AstNode::Identifier, parent_id, &ident_name.span);

  let str_id = ctx.str_table.insert(ident_name.sym.as_str());
  append_usize(&mut ctx.result, str_id);

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
      append_usize(&mut ctx.result, str_id);

      child_id
    }
    PropName::Num(number) => {
      serialize_lit(ctx, &Lit::Num(number.clone()), parent_id)
    }
    PropName::Computed(node) => ctx.serialize_expr(&node.expr, parent_id),
    PropName::BigInt(big_int) => {
      serialize_lit(ctx, &Lit::BigInt(big_int.clone()), parent_id)
    }
  }
}

fn serialize_lit(ctx: &mut SerializeCtx, lit: &Lit, parent_id: usize) -> usize {
  match lit {
    Lit::Str(node) => {
      let id = ctx.push_node(AstNode::StringLiteral, parent_id, &node.span);
      let str_id = ctx.str_table.insert(node.value.as_str());
      append_usize(&mut ctx.result, str_id);

      id
    }
    Lit::Bool(lit_bool) => {
      let id = ctx.push_node(AstNode::Bool, parent_id, &lit_bool.span);

      let value: u8 = if lit_bool.value { 1 } else { 0 };
      ctx.result.push(value);

      id
    }
    Lit::Null(node) => ctx.push_node(AstNode::Null, parent_id, &node.span),
    Lit::Num(node) => {
      let id = ctx.push_node(AstNode::NumericLiteral, parent_id, &node.span);

      let value = node.raw.as_ref().unwrap();
      let str_id = ctx.str_table.insert(value.as_str());
      append_usize(&mut ctx.result, str_id);

      id
    }
    Lit::BigInt(node) => {
      let id = ctx.push_node(AstNode::BigIntLiteral, parent_id, &node.span);

      let str_id = ctx.str_table.insert(&node.value.to_string());
      append_usize(&mut ctx.result, str_id);

      id
    }
    Lit::Regex(node) => {
      let id = ctx.push_node(AstNode::RegExpLiteral, parent_id, &node.span);

      let pattern_id = ctx.str_table.insert(node.exp.as_str());
      let flag_id = ctx.str_table.insert(node.flags.as_str());

      append_usize(&mut ctx.result, pattern_id);
      append_usize(&mut ctx.result, flag_id);

      id
    }
    Lit::JSXText(jsxtext) => {
      ctx.push_node(AstNode::JSXText, parent_id, &jsxtext.span)
    }
  }
}

fn serialize_ts_type(
  ctx: &mut SerializeCtx,
  ts_type: &TsType,
  parent_id: usize,
) -> usize {
  todo!();
}
