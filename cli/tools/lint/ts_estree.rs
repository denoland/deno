use std::fmt::{self, Debug, Display};

use deno_ast::swc::common::Span;

use super::ast_buf::{
  AstBufSerializer, BoolPos, FieldArrPos, FieldPos, NodeRef, NullPos,
  SerializeCtx, StrPos, UndefPos,
};

// Keep in sync with JS
#[derive(Debug, Clone, PartialEq)]
pub enum AstNode {
  Invalid,
  //
  Program,

  // Module declarations
  ExportAllDeclaration,
  ExportDefaultDeclaration,
  ExportNamedDeclaration,
  ImportDeclaration,
  TsExportAssignment,
  TsImportEquals,
  TsNamespaceExport,

  // Decls
  ClassDeclaration,
  FunctionDeclaration,
  TSEnumDeclaration,
  TSInterface,
  TsModule,
  TsTypeAlias,
  Using,
  VariableDeclaration,

  // Statements
  BlockStatement,
  BreakStatement,
  ContinueStatement,
  DebuggerStatement,
  DoWhileStatement,
  EmptyStatement,
  ExpressionStatement,
  ForInStatement,
  ForOfStatement,
  ForStatement,
  IfStatement,
  LabeledStatement,
  ReturnStatement,
  SwitchCase,
  SwitchStatement,
  ThrowStatement,
  TryStatement,
  WhileStatement,
  WithStatement,

  // Expressions
  ArrayExpression,
  ArrowFunctionExpression,
  AssignmentExpression,
  AwaitExpression,
  BinaryExpression,
  CallExpression,
  ChainExpression,
  ClassExpression,
  ConditionalExpression,
  FunctionExpression,
  Identifier,
  ImportExpression,
  LogicalExpression,
  MemberExpression,
  MetaProp,
  NewExpression,
  ObjectExpression,
  PrivateIdentifier,
  SequenceExpression,
  Super,
  TaggedTemplateExpression,
  TemplateLiteral,
  ThisExpression,
  TSAsExpression,
  TsConstAssertion,
  TsInstantiation,
  TSNonNullExpression,
  TSSatisfiesExpression,
  TSTypeAssertion,
  UnaryExpression,
  UpdateExpression,
  YieldExpression,

  // Literals
  StringLiteral,
  Bool,
  Null,
  NumericLiteral,
  BigIntLiteral,
  RegExpLiteral,

  // Custom
  EmptyExpr,
  SpreadElement,
  Property,
  VariableDeclarator,
  CatchClause,
  RestElement,
  ExportSpecifier,
  TemplateElement,
  MethodDefinition,
  ClassBody,

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
  TSTypeParameterInstantiation,
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
  TSNamedTupleMember,
  TSFunctionType,
  TsCallSignatureDeclaration,
  TSPropertySignature,
  TSMethodSignature,
  TSIndexSignature,
  TSIndexedAccessType,
  TSTypeOperator,
  TSTypePredicate,
  TSImportType,
  TSRestType,
  TSArrayType,
  TSClassImplements,

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
  TSEnumBody, // Last value is used for max value
}

impl Display for AstNode {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    Debug::fmt(self, f)
  }
}

impl From<AstNode> for u8 {
  fn from(m: AstNode) -> u8 {
    m as u8
  }
}

#[derive(Debug, Clone)]
pub enum AstProp {
  // Base, these three must be in sync with JS
  Type,
  Parent,
  Range,

  // Node
  Abstract,
  Accessibility,
  Alternate,
  Argument,
  Arguments,
  Asserts,
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
  Declaration,
  Declarations,
  Declare,
  Default,
  Definite,
  Delegate,
  Discriminant,
  Elements,
  ElementType,
  ElementTypes,
  ExprName,
  Expression,
  Expressions,
  Exported,
  Extends,
  ExtendsType,
  FalseType,
  Finalizer,
  Flags,
  Generator,
  Handler,
  Id,
  In,
  IndexType,
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
  ObjectType,
  OpeningElement,
  OpeningFragment,
  Operator,
  Optional,
  Out,
  Param,
  ParameterName,
  Params,
  Pattern,
  Prefix,
  Properties,
  Property,
  Qualifier,
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
  Static,
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
  Value, // Last value is used for max value
}

impl Display for AstProp {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let s = match self {
      AstProp::Parent => "parent",
      AstProp::Range => "range",
      AstProp::Type => "type",
      AstProp::Abstract => "abstract",
      AstProp::Accessibility => "accessibility",
      AstProp::Alternate => "alternate",
      AstProp::Argument => "argument",
      AstProp::Arguments => "arguments",
      AstProp::Asserts => "asserts",
      AstProp::Async => "async",
      AstProp::Attributes => "attributes",
      AstProp::Await => "await",
      AstProp::Block => "block",
      AstProp::Body => "body",
      AstProp::Callee => "callee",
      AstProp::Cases => "cases",
      AstProp::Children => "children",
      AstProp::CheckType => "checkType",
      AstProp::ClosingElement => "closingElement",
      AstProp::ClosingFragment => "closingFragment",
      AstProp::Computed => "computed",
      AstProp::Consequent => "consequent",
      AstProp::Const => "const",
      AstProp::Constraint => "constraint",
      AstProp::Cooked => "cooked",
      AstProp::Declaration => "declaration",
      AstProp::Declarations => "declarations",
      AstProp::Declare => "declare",
      AstProp::Default => "default",
      AstProp::Definite => "definite",
      AstProp::Delegate => "delegate",
      AstProp::Discriminant => "discriminant",
      AstProp::Elements => "elements",
      AstProp::ElementType => "elementType",
      AstProp::ElementTypes => "elementTypes",
      AstProp::ExprName => "exprName",
      AstProp::Expression => "expression",
      AstProp::Expressions => "expressions",
      AstProp::Exported => "exported",
      AstProp::Extends => "extends",
      AstProp::ExtendsType => "extendsType",
      AstProp::FalseType => "falseType",
      AstProp::Finalizer => "finalizer",
      AstProp::Flags => "flags",
      AstProp::Generator => "generator",
      AstProp::Handler => "handler",
      AstProp::Id => "id",
      AstProp::In => "in",
      AstProp::IndexType => "indexType",
      AstProp::Init => "init",
      AstProp::Initializer => "initializer",
      AstProp::Implements => "implements",
      AstProp::Key => "key",
      AstProp::Kind => "kind",
      AstProp::Label => "label",
      AstProp::Left => "left",
      AstProp::Literal => "literal",
      AstProp::Local => "local",
      AstProp::Members => "members",
      AstProp::Meta => "meta",
      AstProp::Method => "method",
      AstProp::Name => "name",
      AstProp::Namespace => "namespace",
      AstProp::NameType => "nameType",
      AstProp::Object => "object",
      AstProp::ObjectType => "objectType",
      AstProp::OpeningElement => "openingElement",
      AstProp::OpeningFragment => "openingFragment",
      AstProp::Operator => "operator",
      AstProp::Optional => "optional",
      AstProp::Out => "out",
      AstProp::Param => "param",
      AstProp::ParameterName => "parameterName",
      AstProp::Params => "params",
      AstProp::Pattern => "pattern",
      AstProp::Prefix => "prefix",
      AstProp::Properties => "properties",
      AstProp::Property => "property",
      AstProp::Qualifier => "qualifier",
      AstProp::Quasi => "quasi",
      AstProp::Quasis => "quasis",
      AstProp::Raw => "raw",
      AstProp::Readonly => "readonly",
      AstProp::ReturnType => "returnType",
      AstProp::Right => "right",
      AstProp::SelfClosing => "selfClosing",
      AstProp::Shorthand => "shorthand",
      AstProp::Source => "source",
      AstProp::SourceType => "sourceType",
      AstProp::Specifiers => "specifiers",
      AstProp::Static => "static",
      AstProp::SuperClass => "superClass",
      AstProp::SuperTypeArguments => "superTypeArguments",
      AstProp::Tag => "tag",
      AstProp::Tail => "tail",
      AstProp::Test => "test",
      AstProp::TrueType => "trueType",
      AstProp::TypeAnnotation => "typeAnnotation",
      AstProp::TypeArguments => "typeArguments",
      AstProp::TypeName => "typeName",
      AstProp::TypeParameter => "typeParameter",
      AstProp::TypeParameters => "typeParameters",
      AstProp::Types => "types",
      AstProp::Update => "update",
      AstProp::Value => "value",
    };

    write!(f, "{}", s)
  }
}

impl From<AstProp> for u8 {
  fn from(m: AstProp) -> u8 {
    m as u8
  }
}

pub struct TsEsTreeBuilder {
  ctx: SerializeCtx,
}

impl TsEsTreeBuilder {
  pub fn new() -> Self {
    // Max values
    let kind_count: u8 = AstNode::TSEnumBody.into();
    let prop_count: u8 = AstProp::Value.into();
    Self {
      ctx: SerializeCtx::new(kind_count, prop_count),
    }
  }

  pub fn alloc_new_expr(
    &mut self,
    parent: NodeRef,
    span: &Span,
    args_len: usize,
  ) -> (NodeRef, usize, usize, usize) {
    let pos = self.ctx.header(AstNode::NewExpression, parent, span, 3);
    let callee_pos = self.ctx.ref_field(AstProp::Callee);
    let type_args_pos = self.ctx.ref_field(AstProp::TypeArguments);
    let args_pos = self.ctx.ref_vec_field(AstProp::Arguments, args_len);

    (pos, callee_pos, type_args_pos, args_pos)
  }

  pub fn commit_new_expr(
    &mut self,
    opts: (NodeRef, usize, usize, usize),
    callee: NodeRef,
    type_args: Option<NodeRef>,
    args: Vec<NodeRef>,
  ) -> NodeRef {
    self.ctx.write_ref(opts.1, callee);
    self.ctx.write_maybe_ref(opts.2, type_args);
    self.ctx.write_refs(opts.3, args);

    opts.0
  }

  pub fn alloc_seq(
    &mut self,
    parent: NodeRef,
    span: &Span,
    len: usize,
  ) -> (NodeRef, usize) {
    let pos = self
      .ctx
      .header(AstNode::SequenceExpression, parent, &span, 1);
    let exprs_pos = self.ctx.ref_vec_field(AstProp::Expressions, len);

    (pos, exprs_pos)
  }

  pub fn commit_seq(
    &mut self,
    opts: (NodeRef, usize),
    children: Vec<NodeRef>,
  ) -> NodeRef {
    self.ctx.write_refs(opts.1, children);

    opts.0
  }

  pub fn alloc_tpl(
    &mut self,
    parent: NodeRef,
    span: &Span,
    quasis_len: usize,
    exprs_len: usize,
  ) -> (NodeRef, usize, usize) {
    let pos = self.ctx.header(AstNode::TemplateLiteral, parent, &span, 2);
    let quasis_pos = self.ctx.ref_vec_field(AstProp::Quasis, quasis_len);
    let exprs_pos = self.ctx.ref_vec_field(AstProp::Expressions, exprs_len);

    (pos, quasis_pos, exprs_pos)
  }

  pub fn commit_tpl(
    &mut self,
    opts: (NodeRef, usize, usize),
    quasis: Vec<NodeRef>,
    exprs: Vec<NodeRef>,
  ) -> NodeRef {
    self.ctx.write_refs(opts.1, quasis);
    self.ctx.write_refs(opts.2, exprs);

    opts.0
  }

  pub fn alloc_tpl_elem(
    &mut self,
    parent: NodeRef,
    span: &Span,
  ) -> (NodeRef, usize, usize, usize) {
    let tpl_pos = self.ctx.header(AstNode::TemplateElement, parent, span, 3);
    let tail_pos = self.ctx.bool_field(AstProp::Tail);
    let raw_pos = self.ctx.str_field(AstProp::Raw);
    let cooked_pos = self.ctx.str_field(AstProp::Cooked);

    (tpl_pos, tail_pos, raw_pos, cooked_pos)
  }

  pub fn commit_tpl_elem(
    &mut self,
    opts: (NodeRef, usize, usize, usize),
    tail: bool,
    quasi: &str,
    raw: &str,
  ) -> NodeRef {
    self.ctx.write_bool(opts.1, tail);
    self.ctx.write_str(opts.2, raw);
    self.ctx.write_str(opts.3, quasi);

    opts.0
  }

  pub fn alloc_tagged_tpl(
    &mut self,
    parent: NodeRef,
    span: &Span,
  ) -> (NodeRef, usize, usize, usize) {
    let pos =
      self
        .ctx
        .header(AstNode::TaggedTemplateExpression, parent, &span, 3);
    let tag_pos = self.ctx.ref_field(AstProp::Tag);
    let type_arg_pos = self.ctx.ref_field(AstProp::TypeArguments);
    let quasi_pos = self.ctx.ref_field(AstProp::Quasi);

    (pos, tag_pos, type_arg_pos, quasi_pos)
  }

  pub fn commit_tagged_tpl(
    &mut self,
    opts: (NodeRef, usize, usize, usize),
    tag: NodeRef,
    type_param: Option<NodeRef>,
    quasi: NodeRef,
  ) -> NodeRef {
    self.ctx.write_ref(opts.1, tag);
    self.ctx.write_maybe_ref(opts.2, type_param);
    self.ctx.write_ref(opts.3, quasi);

    opts.0
  }
}

impl AstBufSerializer<AstNode, AstProp> for TsEsTreeBuilder {
  fn header(
    &mut self,
    kind: AstNode,
    parent: NodeRef,
    span: &Span,
    prop_count: usize,
  ) -> NodeRef {
    self.ctx.header(kind, parent, span, prop_count)
  }

  fn ref_field(&mut self, prop: AstProp) -> FieldPos {
    FieldPos(self.ctx.ref_field(prop))
  }

  fn ref_vec_field(&mut self, prop: AstProp, len: usize) -> FieldArrPos {
    FieldArrPos(self.ctx.ref_vec_field(prop, len))
  }

  fn str_field(&mut self, prop: AstProp) -> StrPos {
    StrPos(self.ctx.str_field(prop))
  }

  fn bool_field(&mut self, prop: AstProp) -> BoolPos {
    BoolPos(self.ctx.bool_field(prop))
  }

  fn undefined_field(&mut self, prop: AstProp) -> UndefPos {
    UndefPos(self.ctx.undefined_field(prop))
  }

  fn null_field(&mut self, prop: AstProp) -> NullPos {
    NullPos(self.ctx.null_field(prop))
  }

  fn write_ref(&mut self, pos: FieldPos, value: NodeRef) {
    self.ctx.write_ref(pos.0, value);
  }

  fn write_maybe_ref(&mut self, pos: FieldPos, value: Option<NodeRef>) {
    self.ctx.write_maybe_ref(pos.0, value);
  }

  fn write_refs(&mut self, pos: FieldArrPos, value: Vec<NodeRef>) {
    self.ctx.write_refs(pos.0, value);
  }

  fn write_str(&mut self, pos: StrPos, value: &str) {
    self.ctx.write_str(pos.0, value);
  }

  fn write_bool(&mut self, pos: BoolPos, value: bool) {
    self.ctx.write_bool(pos.0, value);
  }

  fn serialize(&mut self) -> Vec<u8> {
    self.ctx.serialize()
  }
}
