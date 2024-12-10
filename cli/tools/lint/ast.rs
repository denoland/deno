use deno_ast::{
  swc::{
    ast::{
      AssignTarget, BlockStmtOrExpr, Callee, Decl, Expr, ForHead, Ident, Lit,
      MemberProp, ModuleDecl, ModuleItem, Pat, Program, Prop, PropName,
      PropOrSpread, SimpleAssignTarget, Stmt, SuperProp, TsType, VarDeclOrExpr,
    },
    common::{Span, Spanned, DUMMY_SP},
  },
  view::BinaryOp,
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
  Paren,
  Seq,
  Ident,
  Tpl,
  TaggedTpl,
  Arrow,
  ClassExpr,
  Yield,
  MetaProp,
  Await,
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
  Property,
  VarDeclarator,
  CatchClause,

  // Patterns
  ArrayPattern,
  AssignmentPattern,
  ObjectPattern,
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

fn write_usize(result: &mut Vec<u8>, value: usize, idx: usize) {
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
  BoolFalse,
  BoolTrue,
  FnAsync,
  FnGenerator,
  MemberComputed,
  PropShorthand,
  PropComputed,
  PropGetter,
  PropSetter,
}

impl From<Flag> for u8 {
  fn from(m: Flag) -> u8 {
    match m {
      Flag::ProgramModule => 0b00000001,
      Flag::BoolFalse => 0b00000000,
      Flag::BoolTrue => 0b00000001,
      Flag::FnAsync => 0b00000001,
      Flag::FnGenerator => 0b00000010,
      Flag::MemberComputed => 0b00000001,
      Flag::PropShorthand => 0b00000001,
      Flag::PropComputed => 0b00000010,
      Flag::PropGetter => 0b00000100,
      Flag::PropSetter => 0b00001000,
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
      return id.clone();
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

    eprintln!("Serialized string table: {:#?}", result);

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
}

pub fn serialize_ast_bin(parsed_source: &ParsedSource) -> Vec<u8> {
  let mut ctx = SerializeCtx::new();

  let parent_id = 0;

  let program = &parsed_source.program();
  let mut flags = FlagValue::new();

  match program.as_ref() {
    Program::Module(module) => {
      let id = ctx.push_node(AstNode::Program, parent_id, &module.span);

      flags.set(Flag::ProgramModule);
      ctx.result.push(flags.0);

      let offset = ctx.reserve_child_ids_with_count(module.body.len());

      for (i, item) in module.body.iter().enumerate() {
        let child_id = match item {
          ModuleItem::ModuleDecl(module_decl) => {
            serialize_module_decl(&mut ctx, module_decl, parent_id)
          }
          ModuleItem::Stmt(stmt) => serialize_stmt(&mut ctx, stmt, id),
        };

        ctx.set_child(offset, child_id, i);
      }
    }
    Program::Script(script) => {
      let id = ctx.push_node(AstNode::Program, parent_id, &script.span);

      ctx.result.push(flags.0);
      let offset = ctx.reserve_child_ids_with_count(script.body.len());

      for (i, stmt) in script.body.iter().enumerate() {
        let child_id = serialize_stmt(&mut ctx, stmt, id);
        ctx.set_child(offset, child_id, i);
      }
    }
  }

  let mut result: Vec<u8> = vec![];

  // Serialize string table
  eprintln!("STRING {:#?}", ctx.str_table);
  result.append(&mut ctx.str_table.serialize());

  // Serialize ids
  append_usize(&mut result, ctx.id_to_offset.len());

  let offset = result.len() + (ctx.id_to_offset.len() * 4);

  for (i, value) in ctx.id_to_offset {
    append_usize(&mut result, value + offset);
  }

  // Append serialized AST
  result.append(&mut ctx.result);
  result
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
      ctx.push_node(AstNode::ExportNamed, parent_id, &node.span)
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
      let id = ctx.push_node(AstNode::Block, parent_id, &node.span);
      let offset = ctx.reserve_child_ids_with_count(node.stmts.len());

      for (i, child) in node.stmts.iter().enumerate() {
        let child_id = serialize_stmt(ctx, child, id);
        ctx.set_child(offset, child_id, i);
      }

      id
    }
    Stmt::Empty(node) => ctx.push_node(AstNode::Empty, parent_id, &node.span),
    Stmt::Debugger(node) => {
      ctx.push_node(AstNode::Debugger, parent_id, &node.span)
    }
    Stmt::With(_) => todo!(),
    Stmt::Return(node) => {
      let id = ctx.push_node(AstNode::Return, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(1);

      if let Some(arg) = &node.arg {
        let child_id = serialize_expr(ctx, &arg, id);
        ctx.set_child(offset, child_id, 0);
      };

      id
    }
    Stmt::Labeled(node) => {
      let id = ctx.push_node(AstNode::Labeled, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(2);

      let ident_id = serialize_ident(ctx, &node.label, id);
      let stmt_id = serialize_stmt(ctx, &node.body, id);

      ctx.set_child(offset, ident_id, 0);
      ctx.set_child(offset, stmt_id, 1);

      id
    }
    Stmt::Break(node) => {
      let id = ctx.push_node(AstNode::Break, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(1);

      if let Some(arg) = &node.label {
        let child_id = serialize_ident(ctx, &arg, id);
        ctx.set_child(offset, child_id, 0);
      };

      id
    }
    Stmt::Continue(node) => {
      let id = ctx.push_node(AstNode::Continue, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(1);

      if let Some(arg) = &node.label {
        let child_id = serialize_ident(ctx, &arg, id);
        ctx.set_child(offset, child_id, 0);
      };

      id
    }
    Stmt::If(node) => {
      let id = ctx.push_node(AstNode::If, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(3); // Test + Consequent + Alternate

      let test_id = serialize_expr(ctx, node.test.as_ref(), id);
      let cons_id = serialize_stmt(ctx, node.cons.as_ref(), id);

      ctx.set_child(offset, test_id, 0);
      ctx.set_child(offset, cons_id, 1);

      if let Some(alt) = &node.alt {
        let child_id = serialize_stmt(ctx, &alt, id);
        ctx.set_child(offset, child_id, 2);
      }

      id
    }
    Stmt::Switch(node) => {
      let id = ctx.push_node(AstNode::Switch.into(), parent_id, &node.span);

      let offset = ctx.reserve_child_ids_with_count(node.cases.len());

      for (i, case) in node.cases.iter().enumerate() {
        let child_id =
          ctx.push_node(AstNode::SwitchCase.into(), parent_id, &case.span);

        let child_offset =
          ctx.reserve_child_ids_with_count(case.cons.len() + 1);
        // ctx.set_child(child_offset, child_id, i);
        ctx.set_child(offset, child_id, i);
      }

      id
    }
    Stmt::Throw(node) => {
      let id = ctx.push_node(AstNode::Throw.into(), parent_id, &node.span);
      let offset = ctx.reserve_child_ids(1);

      let child_id = serialize_expr(ctx, &node.arg, id);
      ctx.set_child(offset, child_id, 0);

      id
    }
    Stmt::Try(node) => {
      let try_id = ctx.push_node(AstNode::Try.into(), parent_id, &node.span);
      let offset = ctx.reserve_child_ids(3); // Block + Catch + Finalizer

      let block_id =
        serialize_stmt(ctx, &Stmt::Block(node.block.clone()), try_id);
      ctx.set_child(offset, block_id, 0);

      if let Some(catch_clause) = &node.handler {
        let clause_id = ctx.push_node(
          AstNode::CatchClause.into(),
          try_id,
          &catch_clause.span,
        );

        let clause_offset = ctx.reserve_child_ids(2); // Param + Body

        if let Some(param) = &catch_clause.param {
          // FIXME
        }

        let body_id =
          serialize_stmt(ctx, &Stmt::Block(catch_clause.body.clone()), try_id);
        ctx.set_child(clause_offset, body_id, 1);

        ctx.set_child(offset, clause_id, 1);
      }

      if let Some(finalizer) = &node.finalizer {
        let child_id =
          serialize_stmt(ctx, &Stmt::Block(finalizer.clone()), try_id);
        ctx.set_child(offset, child_id, 2);
      }

      try_id
    }
    Stmt::While(node) => {
      let id = ctx.push_node(AstNode::While, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(2);

      let test_id = serialize_expr(ctx, node.test.as_ref(), id);
      let stmt_id = serialize_stmt(ctx, node.body.as_ref(), id);

      ctx.set_child(offset, test_id, 0);
      ctx.set_child(offset, stmt_id, 1);

      id
    }
    Stmt::DoWhile(node) => {
      let id = ctx.push_node(AstNode::DoWhile, parent_id, &node.span);

      let offset = ctx.reserve_child_ids(2);

      let expr_id = serialize_expr(ctx, node.test.as_ref(), id);
      let stmt_id = serialize_stmt(ctx, node.body.as_ref(), id);

      ctx.set_child(offset, expr_id, 0);
      ctx.set_child(offset, stmt_id, 0);

      id
    }
    Stmt::For(node) => {
      let id = ctx.push_node(AstNode::For, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(4); // Init + test + update + body

      if let Some(init) = &node.init {
        match init {
          VarDeclOrExpr::VarDecl(var_decl) => {
            serialize_stmt(ctx, &Stmt::Decl(Decl::Var(var_decl.clone())), id);
          }
          VarDeclOrExpr::Expr(expr) => {
            serialize_expr(ctx, expr, id);
          }
        }
      }

      if let Some(test_expr) = &node.test {
        let child_id = serialize_expr(ctx, test_expr.as_ref(), parent_id);
        ctx.set_child(offset, child_id, 1);
      }

      if let Some(update_expr) = &node.update {
        let child_id = serialize_expr(ctx, update_expr.as_ref(), id);
        ctx.set_child(offset, child_id, 2);
      }

      let child_id = serialize_stmt(ctx, &node.body.as_ref(), id);
      ctx.set_child(offset, child_id, 3);

      id
    }
    Stmt::ForIn(node) => {
      let id = ctx.push_node(AstNode::ForIn, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(3); // Left + Right + Body

      // FIXME
      match &node.left {
        ForHead::VarDecl(var_decl) => {}
        ForHead::UsingDecl(using_decl) => {}
        ForHead::Pat(pat) => {}
      }

      let right_id = serialize_expr(ctx, node.right.as_ref(), id);
      let body_id = serialize_stmt(ctx, node.body.as_ref(), id);

      ctx.set_child(offset, right_id, 1);
      ctx.set_child(offset, body_id, 2);

      id
    }
    Stmt::ForOf(node) => {
      let id = ctx.push_node(AstNode::ForOf, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(3);

      // FIXME
      match &node.left {
        ForHead::VarDecl(var_decl) => {}
        ForHead::UsingDecl(using_decl) => {}
        ForHead::Pat(pat) => {}
      }

      let right_id = serialize_expr(ctx, node.right.as_ref(), id);
      let body_id = serialize_stmt(ctx, node.body.as_ref(), id);

      ctx.set_child(offset, right_id, 1);
      ctx.set_child(offset, body_id, 2);

      id
    }
    Stmt::Decl(node) => serialize_decl(ctx, node, parent_id),
    Stmt::Expr(node) => {
      let id = ctx.push_node(AstNode::Expr, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(1);

      let child_id = serialize_expr(ctx, node.expr.as_ref(), id);
      ctx.set_child(offset, child_id, 0);

      id
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
      let id = ctx.push_node(AstNode::Class, parent_id, &node.class.span);

      // FIXME

      id
    }
    Decl::Fn(node) => {
      let id = ctx.push_node(AstNode::Fn, parent_id, &node.function.span);

      // FIXME

      id
    }
    Decl::Var(node) => {
      let id = ctx.push_node(AstNode::Var, parent_id, &node.span);
      let offset = ctx.reserve_child_ids_with_count(node.decls.len());

      for (i, decl) in node.decls.iter().enumerate() {
        let child_id =
          ctx.push_node(AstNode::VarDeclarator.into(), parent_id, &decl.span);

        ctx.set_child(offset, child_id, i);

        let child_offset = ctx.reserve_child_ids(2); // Name + init

        let decl_id = match &decl.name {
          Pat::Ident(binding_ident) => {
            serialize_ident(ctx, &binding_ident.id, child_id)
          }
          Pat::Array(array_pat) => todo!(),
          Pat::Rest(rest_pat) => todo!(),
          Pat::Object(object_pat) => todo!(),
          Pat::Assign(assign_pat) => todo!(),
          Pat::Invalid(invalid) => todo!(),
          Pat::Expr(expr) => serialize_expr(ctx, expr.as_ref(), child_id),
        };
        ctx.set_child(child_offset, decl_id, 0);

        if let Some(init) = &decl.init {
          let expr_id = serialize_expr(ctx, init.as_ref(), id);
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

      let id =
        ctx.push_node(AstNode::TsInterface.into(), parent_id, &node.span);

      // FIXME

      id
    }
    Decl::TsTypeAlias(node) => {
      // FIXME: Declare flag
      let id =
        ctx.push_node(AstNode::TsTypeAlias.into(), parent_id, &node.span);

      let offset = ctx.reserve_child_ids(1);

      let ident_id = serialize_ident(ctx, &node.id, id);
      ctx.set_child(offset, ident_id, 0);

      // FIXME
      // let foo = ts_type_alias_decl.type_ann

      id
    }
    Decl::TsEnum(node) => {
      let id = ctx.push_node(AstNode::TsEnum.into(), parent_id, &node.span);

      // Ident + member count
      let count = 1 + node.members.len();
      let offset = ctx.reserve_child_ids_with_count(count);

      let ident_id = serialize_ident(ctx, &node.id, parent_id);
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

fn serialize_expr(
  ctx: &mut SerializeCtx,
  expr: &Expr,
  parent_id: usize,
) -> usize {
  match expr {
    Expr::This(node) => ctx.push_node(AstNode::This, parent_id, &node.span),
    Expr::Array(node) => {
      let id = ctx.push_node(AstNode::Array.into(), parent_id, &node.span);
      let offset = ctx.reserve_child_ids_with_count(node.elems.len());

      for (i, maybe_item) in node.elems.iter().enumerate() {
        if let Some(item) = maybe_item {
          // FIXME
          ctx.set_child(offset, 0, i);
        }
      }

      id
    }
    Expr::Object(node) => {
      let id = ctx.push_node(AstNode::Object, parent_id, &node.span);

      let offset = ctx.reserve_child_ids_with_count(node.props.len());

      for (i, prop) in node.props.iter().enumerate() {
        let child_id = match prop {
          PropOrSpread::Spread(spread_element) => {
            let child_id =
              ctx.push_node(AstNode::Spread, id, &spread_element.dot3_token);

            let child_offset = ctx.reserve_child_ids(1);
            let expr_id =
              serialize_expr(ctx, spread_element.expr.as_ref(), child_id);

            ctx.set_child(child_offset, expr_id, 0);

            child_id
          }
          PropOrSpread::Prop(prop) => {
            let mut flags = FlagValue::new();
            let prop_id = ctx.push_node(AstNode::Property, id, &prop.span());

            // Reserve flag
            ctx.result.push(flags.0);
            let flag_offset = ctx.result.len() - 1;

            let child_offset = ctx.reserve_child_ids(2);

            // FIXME: optional
            // computed: boolean;
            // key: PropertyName;
            // kind: 'get' | 'init' | 'set';
            // method: boolean;
            // optional: boolean;
            // shorthand: boolean;
            match prop.as_ref() {
              Prop::Shorthand(ident) => {
                flags.set(Flag::PropShorthand);

                let child_id = serialize_ident(ctx, ident, prop_id);
                ctx.set_child(child_offset, child_id, 0);
                ctx.set_child(child_offset, child_id, 1);
              }
              Prop::KeyValue(key_value_prop) => {
                if let PropName::Computed(_) = key_value_prop.key {
                  flags.set(Flag::PropComputed)
                }

                let key_id =
                  serialize_prop_name(ctx, &key_value_prop.key, prop_id);
                let value_id =
                  serialize_expr(ctx, key_value_prop.value.as_ref(), prop_id);

                ctx.set_child(child_offset, key_id, 0);
                ctx.set_child(child_offset, value_id, 1);
              }
              Prop::Assign(assign_prop) => {
                let child_id =
                  ctx.push_node(AstNode::Assign, prop_id, &assign_prop.span);

                let key_id = serialize_ident(ctx, &assign_prop.key, child_id);
                let value_id =
                  serialize_expr(ctx, assign_prop.value.as_ref(), child_id);

                ctx.set_child(child_id, key_id, 0);
                ctx.set_child(child_id, value_id, 1);
              }
              Prop::Getter(getter_prop) => {
                flags.set(Flag::PropGetter);

                let key_id =
                  serialize_prop_name(ctx, &getter_prop.key, prop_id);

                ctx.set_child(child_offset, key_id, 0);

                if let Some(stmt) = &getter_prop.body {
                  let value_id =
                    serialize_stmt(ctx, &Stmt::Block(stmt.clone()), prop_id);
                  ctx.set_child(child_offset, value_id, 1);
                }
              }
              Prop::Setter(setter_prop) => {
                flags.set(Flag::PropSetter);

                let key_id =
                  serialize_prop_name(ctx, &setter_prop.key, prop_id);

                let child_id =
                  ctx.push_node(AstNode::FnExpr, prop_id, &setter_prop.span);
                // TODO
                // if let Some(body) = &setter_prop.body {
                //   serialize_stmt(ctx, &Stmt::Block(body.clone()));
                // }
                todo!()
              }
              Prop::Method(method_prop) => {
                // if let Some(body) = &method_prop.function.body {
                //   serialize_stmt(ctx, &Stmt::Block(body.clone()));
                // }
                todo!()
              }
            }

            ctx.result[flag_offset] = flags.0;

            prop_id
          }
        };

        ctx.set_child(offset, child_id, i);
      }

      id
    }
    Expr::Fn(node) => {
      let fn_obj = node.function.as_ref();
      let id = ctx.push_node(AstNode::FnExpr, parent_id, &fn_obj.span);

      // FIXME

      id
    }
    Expr::Unary(node) => {
      let id = ctx.push_node(AstNode::Unary, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(1);

      let child_id = serialize_expr(ctx, &node.arg, id);
      ctx.set_child(offset, child_id, 0);

      id
    }
    Expr::Update(node) => {
      let id = ctx.push_node(AstNode::Update, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(1);

      let child_id = serialize_expr(ctx, node.arg.as_ref(), id);
      ctx.set_child(offset, child_id, 0);

      id
    }
    Expr::Bin(node) => {
      let id = match node.op {
        BinaryOp::LogicalOr
        | BinaryOp::LogicalAnd
        | BinaryOp::NullishCoalescing => {
          ctx.push_node(AstNode::LogicalExpression, parent_id, &node.span)
        }
        _ => ctx.push_node(AstNode::Bin, parent_id, &node.span),
      };

      let offset = ctx.reserve_child_ids(2);

      let left_id = serialize_expr(ctx, node.left.as_ref(), id);
      let right_id = serialize_expr(ctx, node.right.as_ref(), id);

      ctx.set_child(offset, left_id, 0);
      ctx.set_child(offset, right_id, 1);

      id
    }
    Expr::Assign(node) => {
      let id = ctx.push_node(AstNode::Assign, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(2);

      let left_id = match &node.left {
        AssignTarget::Simple(simple_assign_target) => {
          match simple_assign_target {
            SimpleAssignTarget::Ident(binding_ident) => {
              serialize_ident(ctx, &binding_ident.id, id)
            }
            SimpleAssignTarget::Member(member_expr) => {
              serialize_expr(ctx, &Expr::Member(member_expr.clone()), id)
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
        AssignTarget::Pat(assign_target_pat) => todo!(),
      };

      let right_id = serialize_expr(ctx, node.right.as_ref(), id);

      ctx.set_child(offset, left_id, 0);
      ctx.set_child(offset, right_id, 1);

      id
    }
    Expr::Member(node) => {
      let id = ctx.push_node(AstNode::Member, parent_id, &node.span);
      let mut flags = FlagValue::new();
      // Reserve space
      ctx.result.push(flags.0);
      let flag_offset = ctx.result.len() - 1;

      let offset = ctx.reserve_child_ids(2);

      let obj_id = serialize_expr(ctx, node.obj.as_ref(), id);

      let prop_id = match &node.prop {
        MemberProp::Ident(ident_name) => {
          let child_id = ctx.push_node(AstNode::Ident, id, &ident_name.span);

          let str_id = ctx.str_table.insert(ident_name.sym.as_str());
          append_usize(&mut ctx.result, str_id);

          child_id
        }
        MemberProp::PrivateName(private_name) => {
          let child_id =
            ctx.push_node(AstNode::PrivateIdentifier, id, &private_name.span);

          let str_id = ctx.str_table.insert(&private_name.name.as_str());
          append_usize(&mut ctx.result, str_id);

          child_id
        }
        MemberProp::Computed(computed_prop_name) => {
          flags.set(Flag::MemberComputed);

          serialize_expr(ctx, computed_prop_name.expr.as_ref(), id)
        }
      };

      ctx.result[flag_offset] = flags.0;

      ctx.set_child(offset, obj_id, 0);
      ctx.set_child(offset, prop_id, 1);

      id
    }
    Expr::SuperProp(node) => {
      let id = ctx.push_node(AstNode::SuperProp, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(2);

      // FIXME
      match &node.prop {
        SuperProp::Ident(ident_name) => {}
        SuperProp::Computed(computed_prop_name) => {}
      }

      id
    }
    Expr::Cond(node) => {
      let id = ctx.push_node(AstNode::Cond, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(3);

      let test_id = serialize_expr(ctx, node.test.as_ref(), id);
      let cons_id = serialize_expr(ctx, node.cons.as_ref(), id);
      let alt_id = serialize_expr(ctx, node.alt.as_ref(), id);

      ctx.set_child(offset, test_id, 0);
      ctx.set_child(offset, cons_id, 1);
      ctx.set_child(offset, alt_id, 2);

      id
    }
    Expr::Call(node) => {
      let id = ctx.push_node(AstNode::Call, parent_id, &node.span);
      // Callee + type args
      let offset = ctx.reserve_child_ids(2);
      let arg_offset = ctx.reserve_child_ids_with_count(node.args.len());

      match &node.callee {
        Callee::Super(_) => todo!(),
        Callee::Import(import) => todo!(),
        Callee::Expr(expr) => {
          let expr_id = serialize_expr(ctx, expr, id);
          ctx.set_child(offset, expr_id, 0);
        }
      }

      if let Some(type_args) = &node.type_args {

        // ctx.set_child(offset, expr_id, 1);
      }

      // FIXME
      for (i, arg) in node.args.iter().enumerate() {
        // if let Some(spread) = &arg.spread {
        //   let spread_id = ctx.push_node(AstNode::Spread, None, parent_id, 1, spread);
        //   let id = serialize_expr(ctx, spread., spread_id);

        //   ctx.set_child(spread_id, spread_id, i);
        //   ctx.set_child(offset, spread_id, i + 2);

        // } else {

        //   serialize_expr(ctx, arg.expr.as_ref());
        // }
      }

      id
    }
    Expr::New(node) => {
      let id = ctx.push_node(AstNode::New.into(), parent_id, &node.span);
      let offset = ctx.reserve_child_ids(2);
      let child_offset = ctx.reserve_child_ids_with_count(
        node.args.as_ref().map_or(0, |args| args.len()),
      );

      let expr_id = serialize_expr(ctx, node.callee.as_ref(), id);
      ctx.set_child(offset, expr_id, 0);

      if let Some(args) = &node.args {
        for arg in args {
          // FIXME
          //   if let Some(spread) = &arg.spread {
          //     ctx.push_node(AstNode::Spread.into(), None)
          //   }

          //   serialize_expr(ctx, arg.expr.as_ref());
        }
      }

      if let Some(type_ann) = &node.type_args {
        // FIXME
        // ctx.set_child(offset, child_id, 1);
      }

      id
    }
    Expr::Seq(node) => {
      let id = ctx.push_node(AstNode::Seq, parent_id, &node.span);

      let offset = ctx.reserve_child_ids_with_count(node.exprs.len());

      for (i, expr) in node.exprs.iter().enumerate() {
        let child_id = serialize_expr(ctx, expr, id);
        ctx.set_child(offset, child_id, i);
      }

      id
    }
    Expr::Ident(node) => serialize_ident(ctx, node, parent_id),
    Expr::Lit(node) => serialize_lit(ctx, node, parent_id),
    Expr::Tpl(node) => {
      let id = ctx.push_node(AstNode::Tpl, parent_id, &node.span);
      //

      id
    }
    Expr::TaggedTpl(tagged_tpl) => {
      let id = ctx.push_node(AstNode::TaggedTpl, parent_id, &tagged_tpl.span);

      id
    }
    Expr::Arrow(node) => {
      let id = ctx.push_node(AstNode::Arrow, parent_id, &node.span);

      let mut flags = FlagValue::new();
      if node.is_async {
        flags.set(Flag::FnAsync);
      }
      if node.is_generator {
        flags.set(Flag::FnGenerator);
      }
      ctx.result.push(flags.0);

      let type_offset = ctx.reserve_child_ids(1);
      if let Some(type_params) = &node.type_params {
        // FIXME;
      }

      let param_offset = ctx.reserve_child_ids_with_count(node.params.len());
      for (i, pat) in node.params.iter().enumerate() {
        let child_id = serialize_pat(ctx, pat, id);
        ctx.set_child(param_offset, child_id, i);
      }

      // FIXME
      let offset = ctx.reserve_child_ids(2);

      let body_id = match node.body.as_ref() {
        BlockStmtOrExpr::BlockStmt(block_stmt) => {
          serialize_stmt(ctx, &Stmt::Block(block_stmt.clone()), id)
        }
        BlockStmtOrExpr::Expr(expr) => serialize_expr(ctx, expr.as_ref(), id),
      };
      ctx.set_child(offset, body_id, 0);

      if let Some(return_type) = &node.return_type {
        // FIXME
        // ctx.set_child(offset, body_id, 1);
      }

      id
    }
    Expr::Class(node) => {
      let id = ctx.push_node(AstNode::ClassExpr, parent_id, &node.class.span);

      // FIXME

      id
    }
    Expr::Yield(node) => {
      let id = ctx.push_node(AstNode::Yield, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(1);

      if let Some(arg) = &node.arg {
        let child_id = serialize_expr(ctx, arg.as_ref(), id);
        ctx.set_child(offset, child_id, 0);
      }

      id
    }
    Expr::MetaProp(node) => {
      ctx.push_node(AstNode::MetaProp.into(), parent_id, &node.span)
    }
    Expr::Await(node) => {
      let id = ctx.push_node(AstNode::Await, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(1);

      let child_id = serialize_expr(ctx, node.arg.as_ref(), id);
      ctx.set_child(offset, child_id, 0);

      id
    }
    Expr::Paren(node) => {
      let id = ctx.push_node(AstNode::Paren, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(1);

      let child_id = serialize_expr(ctx, node.expr.as_ref(), id);
      ctx.set_child(offset, child_id, 0);

      id
    }
    Expr::JSXMember(node) => {
      // FIXME
      let id = ctx.push_node(AstNode::JSXMember, parent_id, &node.span);

      id
    }
    Expr::JSXNamespacedName(node) => {
      let id = ctx.push_node(AstNode::JSXNamespacedName, parent_id, &node.span);

      id
    }
    Expr::JSXEmpty(node) => {
      let id = ctx.push_node(AstNode::JSXEmpty, parent_id, &node.span);

      id
    }
    Expr::JSXElement(node) => {
      let id = ctx.push_node(AstNode::JSXElement, parent_id, &node.span);

      id
    }
    Expr::JSXFragment(node) => {
      let id = ctx.push_node(AstNode::JSXFragment, parent_id, &node.span);

      id
    }
    Expr::TsTypeAssertion(node) => {
      let id = ctx.push_node(AstNode::TsTypeAssertion, parent_id, &node.span);

      id
    }
    Expr::TsConstAssertion(node) => {
      let id = ctx.push_node(AstNode::TsConstAssertion, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(1);

      let expr_id = serialize_expr(ctx, node.expr.as_ref(), id);
      ctx.set_child(offset, expr_id, 0);

      id
    }
    Expr::TsNonNull(node) => {
      let id = ctx.push_node(AstNode::TsNonNull, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(1);

      let child_id = serialize_expr(ctx, node.expr.as_ref(), id);
      ctx.set_child(offset, child_id, 0);

      id
    }
    Expr::TsAs(node) => {
      let id = ctx.push_node(AstNode::TsAs, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(2);

      let expr_id = serialize_expr(ctx, node.expr.as_ref(), id);
      let type_id = serialize_ts_type(ctx, node.type_ann.as_ref(), id);

      ctx.set_child(offset, expr_id, 0);
      ctx.set_child(offset, type_id, 1);

      id
    }
    Expr::TsInstantiation(node) => {
      let id = ctx.push_node(AstNode::TsInstantiation, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(2);
      let expr_id = serialize_expr(ctx, node.expr.as_ref(), id);
      // let expr_id = serialize_expr(ctx, ts_instantiation.type_args.as_ref(), id);
      ctx.set_child(offset, expr_id, 0);

      // FIXME
      id
    }
    Expr::TsSatisfies(node) => {
      let id = ctx.push_node(AstNode::TsSatisfies, parent_id, &node.span);
      let offset = ctx.reserve_child_ids(2);

      let expr_id = serialize_expr(ctx, node.expr.as_ref(), id);
      let type_id = serialize_ts_type(ctx, node.type_ann.as_ref(), id);

      ctx.set_child(offset, expr_id, 0);
      ctx.set_child(offset, type_id, 1);

      id
    }
    Expr::PrivateName(node) => {
      let id = ctx.push_node(AstNode::PrivateIdentifier, parent_id, &node.span);

      id
    }
    Expr::OptChain(node) => {
      let id = ctx.push_node(AstNode::OptChain, parent_id, &node.span);

      id
    }
    Expr::Invalid(invalid) => {
      // ctx.push_node(result, AstNode::Invalid.into(), &invalid.span);
      todo!()
    }
  }
}

fn serialize_pat(ctx: &mut SerializeCtx, pat: &Pat, parent_id: usize) -> usize {
  match pat {
    Pat::Ident(node) => serialize_ident(ctx, &node.id, parent_id),
    Pat::Array(node) => {
      let id = ctx.push_node(AstNode::ArrayPattern, parent_id, &node.span);

      // FIXME: Optional
      // FIXME: Type Ann
      let offset = ctx.reserve_child_ids_with_count(node.elems.len());

      for (i, elem) in node.elems.iter().enumerate() {
        if let Some(pat) = elem {
          let child_id = serialize_pat(ctx, pat, id);
          ctx.set_child(offset, child_id, i);
        }
      }

      id
    }
    Pat::Rest(node) => todo!(),
    Pat::Object(node) => todo!(),
    Pat::Assign(node) => todo!(),
    Pat::Invalid(node) => todo!(),
    Pat::Expr(node) => todo!(),
  }
}

fn serialize_prop_name(
  ctx: &mut SerializeCtx,
  prop_name: &PropName,
  parent_id: usize,
) -> usize {
  match prop_name {
    PropName::Ident(ident_name) => {
      let child_id = ctx.push_node(AstNode::Ident, parent_id, &ident_name.span);

      let str_id = ctx.str_table.insert(ident_name.sym.as_str());
      append_usize(&mut ctx.result, str_id);

      child_id
    }
    PropName::Str(str_prop) => {
      let child_id =
        ctx.push_node(AstNode::StringLiteral, parent_id, &str_prop.span);

      let str_id = ctx.str_table.insert(&str_prop.value.as_str());
      append_usize(&mut ctx.result, str_id);

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

fn serialize_ident(
  ctx: &mut SerializeCtx,
  ident: &Ident,
  parent_id: usize,
) -> usize {
  let id = ctx.push_node(AstNode::Ident, parent_id, &ident.span);

  let str_id = ctx.str_table.insert(ident.sym.as_str());
  append_usize(&mut ctx.result, str_id);

  id
}

fn serialize_lit(ctx: &mut SerializeCtx, lit: &Lit, parent_id: usize) -> usize {
  match lit {
    Lit::Str(node) => {
      let id = ctx.push_node(AstNode::StringLiteral, parent_id, &node.span);
      let str_id = ctx.str_table.insert(&node.value.as_str());
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
      let id = ctx.push_node(AstNode::Num, parent_id, &node.span);

      let value = node.raw.as_ref().unwrap();
      let str_id = ctx.str_table.insert(&value.as_str());
      append_usize(&mut ctx.result, str_id);

      id
    }
    Lit::BigInt(node) => {
      let id = ctx.push_node(AstNode::BigInt, parent_id, &node.span);

      let str_id = ctx.str_table.insert(&node.value.to_string());
      append_usize(&mut ctx.result, str_id);

      id
    }
    Lit::Regex(node) => {
      let id = ctx.push_node(AstNode::Regex, parent_id, &node.span);

      let pattern_id = ctx.str_table.insert(&node.exp.as_str());
      let flag_id = ctx.str_table.insert(&node.flags.as_str());

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
