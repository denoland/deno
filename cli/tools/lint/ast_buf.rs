use deno_ast::{
  swc::common::{Span, DUMMY_SP},
  view::AssignOp,
};
use indexmap::IndexMap;

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

// Keep in sync with JS
pub enum AstProp {
  // Base
  Parent,
  Range,
  Type,
  _InternalFlags, // Private

  // Node
  Abstract,
  Accessibility,
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
  Constraint,
  Cooked,
  Declarations,
  Declare,
  Default,
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
  In,
  Init,
  Initializer,
  Implements,
  Key,
  Kind,
  Label,
  Left,
  Literal,
  Local,
  Members,
  Meta,
  Method,
  Name,
  Namespace,
  NameType,
  Object,
  OpeningElement,
  OpeningFragment,
  Operator,
  Optional,
  Out,
  Param,
  Params,
  Pattern,
  Prefix,
  Properties,
  Property,
  Quasi,
  Quasis,
  Raw,
  Readonly,
  ReturnType,
  Right,
  SelfClosing,
  Shorthand,
  Source,
  SourceType,
  Specifiers,
  SuperClass,
  SuperTypeArguments,
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
  TruePlusMinus,
  Accessibility,
  UpdateOp,
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
      4 => Ok(PropFlags::AssignOp),
      5 => Ok(PropFlags::BinOp),
      6 => Ok(PropFlags::LogicalOp),
      7 => Ok(PropFlags::UnaryOp),
      8 => Ok(PropFlags::VarKind),
      9 => Ok(PropFlags::TruePlusMinus),
      10 => Ok(PropFlags::Accessibility),
      11 => Ok(PropFlags::UpdateOp),
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

  pub fn set(&mut self, flag: impl Into<u8>) {
    let value: u8 = flag.into();
    self.0 |= value;
  }
}

impl From<FlagValue> for u8 {
  fn from(item: FlagValue) -> Self {
    item.0
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeRef(pub usize);

#[derive(Debug)]
pub struct SerializeCtx {
  pub buf: Vec<u8>,
  pub start_buf: NodeRef,
  pub str_table: StringTable,
}

impl SerializeCtx {
  pub fn new() -> Self {
    let mut ctx = Self {
      start_buf: NodeRef(0),
      buf: vec![],
      str_table: StringTable::new(),
    };

    ctx.str_table.insert("");

    // Placeholder node
    ctx.push_node(AstNode::Invalid, NodeRef(0), &DUMMY_SP);
    ctx.start_buf = NodeRef(ctx.buf.len());
    eprintln!("START {:#?} {}", ctx.buf, ctx.buf.len());

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

  pub fn write_flags(&mut self, field_offset: usize, value: impl Into<u8>) {
    self.buf[field_offset + 2] = value.into();
  }

  pub fn push_node(
    &mut self,
    kind: AstNode,
    parent: NodeRef,
    span: &Span,
  ) -> NodeRef {
    self.header(kind, parent, span, 0)
  }

  pub fn serialize(&mut self) -> Vec<u8> {
    let mut buf: Vec<u8> = vec![];

    // Append serialized AST
    buf.append(&mut self.buf);

    let offset_str_table = buf.len();

    // Serialize string table
    // eprintln!("STRING {:#?}", self.str_table);
    buf.append(&mut self.str_table.serialize());

    append_usize(&mut buf, offset_str_table);
    append_usize(&mut buf, self.start_buf.0);

    buf
  }
}
