use deno_ast::{
  swc::common::{Span, DUMMY_SP},
  view::AssignOp,
};
use indexmap::IndexMap;

pub enum Flag {
  ProgramModule,
  FnAsync,
  FnGenerator,
  FnDeclare,
  FnOptional,
  MemberComputed,
  MemberOptional,
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

  TsDeclare,
  TsConst,
  TsTrue,
  TsPlus,
  TsMinus,
  TsReadonly,
}

pub fn assign_op_to_flag(m: AssignOp) -> u8 {
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
      Flag::FnOptional => 0b00001000,
      Flag::MemberComputed => 0b00000001,
      Flag::MemberOptional => 0b00000010,
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

      Flag::TsDeclare => 0b000000001,
      Flag::TsConst => 0b000000010,
      Flag::TsTrue => 0b000000100,
      Flag::TsPlus => 0b000001000,
      Flag::TsMinus => 0b000010000,
      Flag::TsReadonly => 0b000100000,
    }
  }
}

// Keep in sync with JS
#[derive(Debug, PartialEq)]
pub enum AstNode {
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
  TSInterface,
  TsTypeAlias,
  TSEnumDeclaration,
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
  WhileStatement,
  DoWhileStatement,
  ForStatement,
  ForInStatement,
  ForOfStatement,
  Decl,
  ExpressionStatement,

  // Expressions
  This,
  ArrayExpression,
  Object,
  FunctionExpression,
  UnaryExpression,
  UpdateExpression,
  BinaryExpression,
  Assign,
  MemberExpression,
  Super,
  ConditionalExpression,
  CallExpression,
  New,
  Paren,
  SequenceExpression,
  Identifier,
  TemplateLiteral,
  TaggedTemplateExpression,
  ArrowFunctionExpression,
  ClassExpr,
  YieldExpression,
  MetaProp,
  AwaitExpression,
  LogicalExpression,
  TSTypeAssertion,
  TsConstAssertion,
  TSNonNullExpression,
  TSAsExpression,
  TsInstantiation,
  TSSatisfiesExpression,
  PrivateIdentifier,
  ChainExpression,

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

  TSTypeAnnotation,
  TSTypeParameterDeclaration,
  TSTypeParameter,
  TSEnumMember,
  TSInterfaceBody,
  TSInterfaceHeritage,
  TSTypeReference,
  TSThisType,
  TSLiteralType,
  TSInferType,
  TSConditionalType,
  TSUnionType,
  TSIntersectionType,
  TSMappedType,
  TSTypeQuery,
  TSTupleType,
  TSFunctionType,
  TsCallSignatureDeclaration,

  TSAnyKeyword,
  TSBigIntKeyword,
  TSBooleanKeyword,
  TSIntrinsicKeyword,
  TSNeverKeyword,
  TSNullKeyword,
  TSNumberKeyword,
  TSObjectKeyword,
  TSStringKeyword,
  TSSymbolKeyword,
  TSUndefinedKeyword,
  TSUnknownKeyword,
  TSVoidKeyword,
  TSEnumBody,
}

impl From<AstNode> for u8 {
  fn from(m: AstNode) -> u8 {
    m as u8
  }
}

pub enum AstProp {
  // Base
  Parent,
  Range,
  Type,
  _InternalFlags, // Private

  // Node
  Abstract,
  Alternate,
  Argument,
  Arguments,
  Async,
  Attributes,
  Await,
  Block,
  Body,
  Callee,
  Cases,
  Children,
  CheckType,
  ClosingElement,
  ClosingFragment,
  Computed,
  Consequent,
  Const,
  Cooked,
  Declarations,
  Declare,
  Definite,
  Delegate,
  Discriminant,
  Elements,
  ElementTypes,
  ExprName,
  Expression,
  Expressions,
  Exported,
  ExtendsType,
  FalseType,
  Finalizer,
  Flags,
  Generator,
  Handler,
  Id,
  Init,
  Initializer,
  Key,
  Kind,
  Label,
  Left,
  Local,
  Members,
  Meta,
  Method,
  Name,
  Namespace,
  Object,
  OpeningElement,
  OpeningFragment,
  Operator,
  Optional,
  Param,
  Params,
  Pattern,
  Prefix,
  Properties,
  Property,
  Quasi,
  Quasis,
  Raw,
  ReturnType,
  Right,
  SelfClosing,
  Shorthand,
  Source,
  SourceType,
  Specifiers,
  Tag,
  Tail,
  Test,
  TrueType,
  TypeAnnotation,
  TypeArguments,
  TypeName,
  TypeParameter,
  TypeParameters,
  Types,
  Update,
  Value,
}

impl From<AstProp> for u8 {
  fn from(m: AstProp) -> u8 {
    m as u8
  }
}

#[derive(Debug, PartialEq)]
pub enum PropFlags {
  Ref,
  RefArr,
  String,
  Bool,
  AssignOp,
  BinOp,
  LogicalOp,
  UnaryOp,
  VarKind,
}

impl From<PropFlags> for u8 {
  fn from(m: PropFlags) -> u8 {
    m as u8
  }
}

impl TryFrom<u8> for PropFlags {
  type Error = &'static str;

  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      0 => Ok(PropFlags::Ref),
      1 => Ok(PropFlags::RefArr),
      2 => Ok(PropFlags::String),
      3 => Ok(PropFlags::Bool),
      _ => Err("Unknown Prop flag"),
    }
  }
}

const MASK_U32_1: u32 = 0b11111111_00000000_00000000_00000000;
const MASK_U32_2: u32 = 0b00000000_11111111_00000000_00000000;
const MASK_U32_3: u32 = 0b00000000_00000000_11111111_00000000;
const MASK_U32_4: u32 = 0b00000000_00000000_00000000_11111111;

pub fn append_u32(result: &mut Vec<u8>, value: u32) {
  let v1: u8 = ((value & MASK_U32_1) >> 24) as u8;
  let v2: u8 = ((value & MASK_U32_2) >> 16) as u8;
  let v3: u8 = ((value & MASK_U32_3) >> 8) as u8;
  let v4: u8 = (value & MASK_U32_4) as u8;

  result.push(v1);
  result.push(v2);
  result.push(v3);
  result.push(v4);
}

pub fn append_usize(result: &mut Vec<u8>, value: usize) {
  let raw = u32::try_from(value).unwrap();
  append_u32(result, raw);
}

pub fn write_usize(result: &mut [u8], value: usize, idx: usize) {
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

#[derive(Debug, Clone)]
pub struct FlagValue(pub u8);

impl FlagValue {
  pub fn new() -> Self {
    Self(0)
  }

  pub fn set(&mut self, flag: Flag) {
    let value: u8 = flag.into();
    self.0 |= value;
  }
}

#[derive(Debug)]
pub struct StringTable {
  id: usize,
  table: IndexMap<String, usize>,
}

impl StringTable {
  pub fn new() -> Self {
    Self {
      id: 0,
      table: IndexMap::new(),
    }
  }

  pub fn insert(&mut self, s: &str) -> usize {
    if let Some(id) = self.table.get(s) {
      return *id;
    }

    let id = self.id;
    self.id += 1;
    self.table.insert(s.to_string(), id);
    id
  }

  pub fn serialize(&mut self) -> Vec<u8> {
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

#[derive(Debug, Copy, PartialEq)]
pub struct NodeRef(pub usize);

#[derive(Debug)]
pub struct SerializeCtx {
  pub id: usize,
  pub id_to_offset: IndexMap<usize, usize>,
  pub buf: Vec<u8>,
  pub str_table: StringTable,
}

impl SerializeCtx {
  pub fn new() -> Self {
    let mut ctx = Self {
      id: 0,
      id_to_offset: IndexMap::new(),
      buf: vec![],
      str_table: StringTable::new(),
    };

    ctx.str_table.insert("");

    // Placeholder node
    // fIXME
    ctx.push_node(AstNode::Invalid, NodeRef(0), &DUMMY_SP);

    ctx
  }

  /// Begin writing a node
  pub fn header(
    &mut self,
    kind: AstNode,
    parent: NodeRef,
    span: &Span,
    prop_count: usize,
  ) -> NodeRef {
    let offset = self.buf.len();

    let kind_value: u8 = kind.into();
    self.buf.push(kind_value);

    append_usize(&mut self.buf, parent.0);

    // Span
    append_u32(&mut self.buf, span.lo.0);
    append_u32(&mut self.buf, span.hi.0);

    // No node has more than <10 properties
    self.buf.push(prop_count as u8);

    NodeRef(offset)
  }

  pub fn ref_field(&mut self, prop: AstProp) -> usize {
    self.field(prop, PropFlags::Ref)
  }

  pub fn ref_vec_field(&mut self, prop: AstProp) -> usize {
    self.field(prop, PropFlags::RefArr)
  }

  pub fn str_field(&mut self, prop: AstProp) -> usize {
    self.field(prop, PropFlags::String)
  }

  pub fn bool_field(&mut self, prop: AstProp) -> usize {
    self.field(prop, PropFlags::Bool)
  }

  pub fn field(&mut self, prop: AstProp, prop_flags: PropFlags) -> usize {
    let offset = self.buf.len();

    let kind: u8 = prop.into();
    self.buf.push(kind);

    let flags: u8 = prop_flags.into();
    self.buf.push(flags);

    append_usize(&mut self.buf, 0);

    offset
  }

  pub fn write_ref(&mut self, field_offset: usize, value: NodeRef) {
    #[cfg(debug_assertions)]
    {
      let value_kind = self.buf[field_offset + 1];
      if PropFlags::try_from(value_kind).unwrap() != PropFlags::Ref {
        panic!("Trying to write a ref into a non-ref field")
      }
    }

    write_usize(&mut self.buf, value.0, field_offset + 2);
  }

  pub fn write_maybe_ref(
    &mut self,
    field_offset: usize,
    value: Option<NodeRef>,
  ) {
    #[cfg(debug_assertions)]
    {
      let value_kind = self.buf[field_offset + 1];
      if PropFlags::try_from(value_kind).unwrap() != PropFlags::Ref {
        panic!("Trying to write a ref into a non-ref field")
      }
    }

    let ref_value = if let Some(v) = value { v } else { NodeRef(0) };
    write_usize(&mut self.buf, ref_value.0, field_offset + 2);
  }

  pub fn write_refs(&mut self, field_offset: usize, value: Vec<NodeRef>) {
    #[cfg(debug_assertions)]
    {
      let value_kind = self.buf[field_offset + 1];
      if PropFlags::try_from(value_kind).unwrap() != PropFlags::RefArr {
        panic!("Trying to write a ref into a non-ref array field")
      }
    }

    let mut offset = field_offset + 2;
    write_usize(&mut self.buf, value.len(), offset);
    offset += 4;

    for item in value {
      write_usize(&mut self.buf, item.0, offset);
      offset += 4;
    }
  }

  pub fn write_str(&mut self, field_offset: usize, value: &str) {
    #[cfg(debug_assertions)]
    {
      let value_kind = self.buf[field_offset + 1];
      if PropFlags::try_from(value_kind).unwrap() != PropFlags::String {
        panic!("Trying to write a ref into a non-string field")
      }
    }

    let id = self.str_table.insert(value);
    write_usize(&mut self.buf, id, field_offset + 2);
  }

  pub fn write_bool(&mut self, field_offset: usize, value: bool) {
    #[cfg(debug_assertions)]
    {
      let value_kind = self.buf[field_offset + 1];
      if PropFlags::try_from(value_kind).unwrap() != PropFlags::Bool {
        panic!("Trying to write a ref into a non-bool field")
      }
    }

    self.buf[field_offset + 2] = if value { 1 } else { 0 };
  }

  pub fn write_flags(&mut self, field_offset: usize, value: &FlagValue) {
    self.buf[field_offset + 2] = value.0;
  }

  pub fn next_id(&mut self) -> usize {
    let id = self.id;
    self.id += 1;
    id
  }

  pub fn write_u8(&mut self, value: u8) {
    self.buf.push(value);
  }

  pub fn write_node(
    &mut self,
    id: usize,
    kind: AstNode,
    parent_id: usize,
    span: &Span,
    prop_count: usize,
  ) {
    self.id_to_offset.insert(id, self.buf.len());

    let kind_value: u8 = kind.into();
    self.buf.push(kind_value);
    append_usize(&mut self.buf, parent_id);

    // Span
    append_u32(&mut self.buf, span.lo.0);
    append_u32(&mut self.buf, span.hi.0);

    // No node has more than <10 properties
    self.buf.push(prop_count as u8);
  }

  pub fn write_ids<I>(&mut self, prop: AstProp, ids: I)
  where
    I: IntoIterator<Item = usize>,
  {
    self.buf.push(prop.into());

    let mut count = 0;
    let idx = self.buf.len();
    append_usize(&mut self.buf, 0);

    for id in ids {
      append_usize(&mut self.buf, id);
      count += 1;
    }

    write_usize(&mut self.buf, count, idx);
  }

  pub fn write_id(&mut self, id: usize) {
    append_usize(&mut self.buf, id);
  }

  pub fn write_prop(&mut self, prop: AstProp, id: usize) {
    self.buf.push(prop.into());
    append_usize(&mut self.buf, id);
  }

  pub fn push_node(
    &mut self,
    kind: AstNode,
    parent: NodeRef,
    span: &Span,
  ) -> NodeRef {
    self.header(kind, parent, span, 0)
  }
}
