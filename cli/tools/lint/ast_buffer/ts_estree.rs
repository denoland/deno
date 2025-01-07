// Copyright 2018-2025 the Deno authors. MIT license.

use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;

use deno_ast::swc::common::Span;

use super::buffer::AstBufSerializer;
use super::buffer::NodeRef;
use super::buffer::SerializeCtx;

#[derive(Debug, Clone, PartialEq)]
pub enum AstNode {
  // First node must always be the empty/invalid node
  Invalid,
  RefArray,
  // Typically the
  Program,

  // Module declarations
  ExportAllDeclaration,
  ExportDefaultDeclaration,
  ExportNamedDeclaration,
  ImportDeclaration,
  ImportSpecifier,
  ImportDefaultSpecifier,
  ImportNamespaceSpecifier,
  TsExportAssignment,
  TsImportEquals,
  TsNamespaceExport,

  // Decls
  ClassDeclaration,
  FunctionDeclaration,
  TSEnumDeclaration,
  TSInterface,
  TsModule,
  TSTypeAliasDeclaration,
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
  TSNonNullExpression,
  TSSatisfiesExpression,
  TSTypeAssertion,
  UnaryExpression,
  UpdateExpression,
  YieldExpression,

  // TODO: TSEsTree uses a single literal node
  // Literals
  Literal,

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
  // Base, these must be in sync with JS in the same order.
  Invalid,
  Type,
  Parent,
  Range,
  Length, // Not used in AST, but can be used in attr selectors

  // Starting from here the order doesn't matter.
  // Following are all possible AST node properties.
  Abstract,
  Accessibility,
  Alternate,
  Argument,
  Arguments,
  Asserts,
  Async,
  Attributes,
  Await,
  BigInt,
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
  ExportKind,
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
  Imported,
  ImportKind,
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
  Options,
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
  Regex,
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

// TODO: Feels like there should be an easier way to iterater over an
// enum in Rust and lowercase the first letter.
impl Display for AstProp {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let s = match self {
      AstProp::Invalid => "__invalid__", // unused
      AstProp::Parent => "parent",
      AstProp::Range => "range",
      AstProp::Type => "type",
      AstProp::Length => "length",
      AstProp::Abstract => "abstract",
      AstProp::Accessibility => "accessibility",
      AstProp::Alternate => "alternate",
      AstProp::Argument => "argument",
      AstProp::Arguments => "arguments",
      AstProp::Asserts => "asserts",
      AstProp::Async => "async",
      AstProp::Attributes => "attributes",
      AstProp::Await => "await",
      AstProp::BigInt => "bigint",
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
      AstProp::ExportKind => "exportKind",
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
      AstProp::Imported => "imported",
      AstProp::ImportKind => "importKind",
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
      AstProp::Options => "options",
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
      AstProp::Regex => "regex",
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

impl AstBufSerializer for TsEsTreeBuilder {
  fn serialize(&mut self) -> Vec<u8> {
    self.ctx.serialize()
  }
}

// TODO: Add a builder API to make it easier to convert from different source
// ast formats.
impl TsEsTreeBuilder {
  pub fn new() -> Self {
    // Max values
    // TODO: Maybe there is a rust macro to grab the last enum value?
    let kind_max_count: u8 = u8::from(AstNode::TSEnumBody) + 1;
    let prop_max_count: u8 = u8::from(AstProp::Value) + 1;
    Self {
      ctx: SerializeCtx::new(kind_max_count, prop_max_count),
    }
  }

  pub fn write_program(
    &mut self,
    span: &Span,
    source_kind: &str,
    body: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::Program, span);

    self.ctx.write_str(AstProp::SourceType, source_kind);
    self.ctx.write_ref_vec(AstProp::Body, &id, body);

    self.ctx.set_root_idx(id.0);

    self.ctx.commit_node(id)
  }

  pub fn write_import_decl(
    &mut self,
    span: &Span,
    type_only: bool,
    source: &str,
    specifiers: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ImportDeclaration, span);

    let kind = if type_only { "type" } else { "value" };
    self.ctx.write_str(AstProp::ImportKind, kind);
    self.ctx.write_str(AstProp::Source, source);
    self.ctx.write_ref_vec(AstProp::Specifiers, &id, specifiers);

    self.ctx.commit_node(id)
  }

  pub fn write_import_spec(
    &mut self,
    span: &Span,
    type_only: bool,
    local: NodeRef,
    imported: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ImportSpecifier, span);

    let kind = if type_only { "type" } else { "value" };
    self.ctx.write_str(AstProp::ImportKind, kind);

    let actual = imported.unwrap_or(local.clone());

    self.ctx.write_ref(AstProp::Imported, &id, actual);
    self.ctx.write_ref(AstProp::Local, &id, local);

    self.ctx.commit_node(id)
  }

  pub fn write_import_default_spec(
    &mut self,
    span: &Span,
    local: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ImportDefaultSpecifier, span);
    self.ctx.write_ref(AstProp::Local, &id, local);
    self.ctx.commit_node(id)
  }

  pub fn write_import_ns_spec(
    &mut self,
    span: &Span,
    local: NodeRef,
  ) -> NodeRef {
    let id = self
      .ctx
      .append_node(AstNode::ImportNamespaceSpecifier, span);
    self.ctx.write_ref(AstProp::Local, &id, local);
    self.ctx.commit_node(id)
  }

  pub fn write_export_decl(&mut self, span: &Span, decl: NodeRef) -> NodeRef {
    // TSEstree mixes these types :/
    let id = self.ctx.append_node(AstNode::ExportNamedDeclaration, span);
    self.ctx.write_ref(AstProp::Declaration, &id, decl);
    self.ctx.commit_node(id)
  }

  pub fn write_export_named_decl(
    &mut self,
    span: &Span,
    specifiers: Vec<NodeRef>,
    source: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ExportNamedDeclaration, span);

    self.ctx.write_ref_vec(AstProp::Specifiers, &id, specifiers);
    self.ctx.write_maybe_ref(AstProp::Source, &id, source);

    self.ctx.commit_node(id)
  }

  pub fn write_export_spec(
    &mut self,
    span: &Span,
    type_only: bool,
    local: NodeRef,
    exported: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ExportSpecifier, span);

    let kind = if type_only { "type" } else { "value" };
    self.ctx.write_str(AstProp::ExportKind, kind);

    let actual = exported.unwrap_or(local.clone());
    self.ctx.write_ref(AstProp::Exported, &id, actual);
    self.ctx.write_ref(AstProp::Local, &id, local);

    self.ctx.commit_node(id)
  }

  pub fn write_var_decl(
    &mut self,
    span: &Span,
    declare: bool,
    kind: &str,
    decls: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::VariableDeclaration, span);

    self.ctx.write_bool(AstProp::Declare, declare);
    self.ctx.write_str(AstProp::Kind, kind);
    self.ctx.write_ref_vec(AstProp::Declarations, &id, decls);

    self.ctx.commit_node(id)
  }

  pub fn write_var_declarator(
    &mut self,
    span: &Span,
    ident: NodeRef,
    init: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::VariableDeclarator, span);

    self.ctx.write_ref(AstProp::Id, &id, ident);
    self.ctx.write_maybe_ref(AstProp::Init, &id, init);

    self.ctx.commit_node(id)
  }

  pub fn write_fn_decl(
    &mut self,
    span: &Span,
    is_declare: bool,
    is_async: bool,
    is_generator: bool,
    ident: NodeRef,
    type_param: Option<NodeRef>,
    return_type: Option<NodeRef>,
    body: Option<NodeRef>,
    params: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::FunctionDeclaration, span);

    self.ctx.write_bool(AstProp::Declare, is_declare);
    self.ctx.write_bool(AstProp::Async, is_async);
    self.ctx.write_bool(AstProp::Generator, is_generator);
    self.ctx.write_ref(AstProp::Id, &id, ident);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeParameters, &id, type_param);
    self
      .ctx
      .write_maybe_ref(AstProp::ReturnType, &id, return_type);
    self.ctx.write_maybe_ref(AstProp::Body, &id, body);
    self.ctx.write_ref_vec(AstProp::Params, &id, params);

    self.ctx.commit_node(id)
  }

  pub fn write_block_stmt(
    &mut self,
    span: &Span,
    body: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::BlockStatement, span);
    self.ctx.write_ref_vec(AstProp::Body, &id, body);
    self.ctx.commit_node(id)
  }

  pub fn write_debugger_stmt(&mut self, span: &Span) -> NodeRef {
    let id = self.ctx.append_node(AstNode::DebuggerStatement, span);
    self.ctx.commit_node(id)
  }

  pub fn write_with_stmt(
    &mut self,
    span: &Span,
    obj: NodeRef,
    body: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::WithStatement, span);

    self.ctx.write_ref(AstProp::Object, &id, obj);
    self.ctx.write_ref(AstProp::Body, &id, body);

    self.ctx.commit_node(id)
  }

  pub fn write_return_stmt(
    &mut self,
    span: &Span,
    arg: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ReturnStatement, span);
    self.ctx.write_maybe_ref(AstProp::Argument, &id, arg);
    self.ctx.commit_node(id)
  }

  pub fn write_labeled_stmt(
    &mut self,
    span: &Span,
    label: NodeRef,
    body: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::LabeledStatement, span);

    self.ctx.write_ref(AstProp::Label, &id, label);
    self.ctx.write_ref(AstProp::Body, &id, body);

    self.ctx.commit_node(id)
  }

  pub fn write_break_stmt(
    &mut self,
    span: &Span,
    label: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::LabeledStatement, span);
    self.ctx.write_maybe_ref(AstProp::Label, &id, label);
    self.ctx.commit_node(id)
  }

  pub fn write_continue_stmt(
    &mut self,
    span: &Span,
    label: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ContinueStatement, span);
    self.ctx.write_maybe_ref(AstProp::Label, &id, label);
    self.ctx.commit_node(id)
  }

  pub fn write_if_stmt(
    &mut self,
    span: &Span,
    test: NodeRef,
    consequent: NodeRef,
    alternate: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::IfStatement, span);

    self.ctx.write_ref(AstProp::Test, &id, test);
    self.ctx.write_ref(AstProp::Consequent, &id, consequent);
    self.ctx.write_maybe_ref(AstProp::Alternate, &id, alternate);

    self.ctx.commit_node(id)
  }

  pub fn write_switch_stmt(
    &mut self,
    span: &Span,
    discriminant: NodeRef,
    cases: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::SwitchStatement, span);

    self.ctx.write_ref(AstProp::Discriminant, &id, discriminant);
    self.ctx.write_ref_vec(AstProp::Cases, &id, cases);

    self.ctx.commit_node(id)
  }

  pub fn write_switch_case(
    &mut self,
    span: &Span,
    test: Option<NodeRef>,
    consequent: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::SwitchCase, span);

    self.ctx.write_maybe_ref(AstProp::Test, &id, test);
    self.ctx.write_ref_vec(AstProp::Consequent, &id, consequent);

    self.ctx.commit_node(id)
  }

  pub fn write_throw_stmt(&mut self, span: &Span, arg: NodeRef) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ThrowStatement, span);
    self.ctx.write_ref(AstProp::Argument, &id, arg);
    self.ctx.commit_node(id)
  }

  pub fn write_while_stmt(
    &mut self,
    span: &Span,
    test: NodeRef,
    body: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::WhileStatement, span);

    self.ctx.write_ref(AstProp::Test, &id, test);
    self.ctx.write_ref(AstProp::Body, &id, body);

    self.ctx.commit_node(id)
  }

  pub fn write_do_while_stmt(
    &mut self,
    span: &Span,
    test: NodeRef,
    body: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::DoWhileStatement, span);

    self.ctx.write_ref(AstProp::Test, &id, test);
    self.ctx.write_ref(AstProp::Body, &id, body);

    self.ctx.commit_node(id)
  }

  pub fn write_for_stmt(
    &mut self,
    span: &Span,
    init: Option<NodeRef>,
    test: Option<NodeRef>,
    update: Option<NodeRef>,
    body: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ForStatement, span);

    self.ctx.write_maybe_ref(AstProp::Init, &id, init);
    self.ctx.write_maybe_ref(AstProp::Test, &id, test);
    self.ctx.write_maybe_ref(AstProp::Update, &id, update);
    self.ctx.write_ref(AstProp::Body, &id, body);

    self.ctx.commit_node(id)
  }

  pub fn write_for_in_stmt(
    &mut self,
    span: &Span,
    left: NodeRef,
    right: NodeRef,
    body: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ForInStatement, span);

    self.ctx.write_ref(AstProp::Left, &id, left);
    self.ctx.write_ref(AstProp::Right, &id, right);
    self.ctx.write_ref(AstProp::Body, &id, body);

    self.ctx.commit_node(id)
  }

  pub fn write_for_of_stmt(
    &mut self,
    span: &Span,
    is_await: bool,
    left: NodeRef,
    right: NodeRef,
    body: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ForOfStatement, span);

    self.ctx.write_bool(AstProp::Await, is_await);
    self.ctx.write_ref(AstProp::Left, &id, left);
    self.ctx.write_ref(AstProp::Right, &id, right);
    self.ctx.write_ref(AstProp::Body, &id, body);

    self.ctx.commit_node(id)
  }

  pub fn write_expr_stmt(&mut self, span: &Span, expr: NodeRef) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ExpressionStatement, span);
    self.ctx.write_ref(AstProp::Expression, &id, expr);
    self.ctx.commit_node(id)
  }

  pub fn write_try_stmt(
    &mut self,
    span: &Span,
    block: NodeRef,
    handler: Option<NodeRef>,
    finalizer: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TryStatement, span);

    self.ctx.write_ref(AstProp::Block, &id, block);
    self.ctx.write_maybe_ref(AstProp::Handler, &id, handler);
    self.ctx.write_maybe_ref(AstProp::Finalizer, &id, finalizer);

    self.ctx.commit_node(id)
  }

  pub fn write_catch_clause(
    &mut self,
    span: &Span,
    param: Option<NodeRef>,
    body: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::CatchClause, span);

    self.ctx.write_maybe_ref(AstProp::Param, &id, param);
    self.ctx.write_ref(AstProp::Body, &id, body);

    self.ctx.commit_node(id)
  }

  pub fn write_arr_expr(
    &mut self,
    span: &Span,
    elems: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ArrayExpression, span);
    self.ctx.write_ref_vec(AstProp::Elements, &id, elems);
    self.ctx.commit_node(id)
  }

  pub fn write_obj_expr(
    &mut self,
    span: &Span,
    props: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ObjectExpression, span);
    self.ctx.write_ref_vec(AstProp::Properties, &id, props);
    self.ctx.commit_node(id)
  }

  pub fn write_bin_expr(
    &mut self,
    span: &Span,
    operator: &str,
    left: NodeRef,
    right: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::BinaryExpression, span);

    self.ctx.write_str(AstProp::Operator, operator);
    self.ctx.write_ref(AstProp::Left, &id, left);
    self.ctx.write_ref(AstProp::Right, &id, right);

    self.ctx.commit_node(id)
  }

  pub fn write_logical_expr(
    &mut self,
    span: &Span,
    operator: &str,
    left: NodeRef,
    right: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::LogicalExpression, span);

    self.ctx.write_str(AstProp::Operator, operator);
    self.ctx.write_ref(AstProp::Left, &id, left);
    self.ctx.write_ref(AstProp::Right, &id, right);

    self.ctx.commit_node(id)
  }

  pub fn write_fn_expr(
    &mut self,
    span: &Span,
    is_async: bool,
    is_generator: bool,
    ident: Option<NodeRef>,
    type_params: Option<NodeRef>,
    params: Vec<NodeRef>,
    return_type: Option<NodeRef>,
    body: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::FunctionExpression, span);

    self.ctx.write_bool(AstProp::Async, is_async);
    self.ctx.write_bool(AstProp::Generator, is_generator);
    self.ctx.write_maybe_ref(AstProp::Id, &id, ident);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeParameters, &id, type_params);
    self.ctx.write_ref_vec(AstProp::Params, &id, params);
    self
      .ctx
      .write_maybe_ref(AstProp::ReturnType, &id, return_type);
    self.ctx.write_maybe_ref(AstProp::Body, &id, body);

    self.ctx.commit_node(id)
  }

  pub fn write_arrow_fn_expr(
    &mut self,
    span: &Span,
    is_async: bool,
    is_generator: bool,
    type_params: Option<NodeRef>,
    params: Vec<NodeRef>,
    return_type: Option<NodeRef>,
    body: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ArrowFunctionExpression, span);

    self.ctx.write_bool(AstProp::Async, is_async);
    self.ctx.write_bool(AstProp::Generator, is_generator);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeParameters, &id, type_params);
    self.ctx.write_ref_vec(AstProp::Params, &id, params);
    self
      .ctx
      .write_maybe_ref(AstProp::ReturnType, &id, return_type);
    self.ctx.write_ref(AstProp::Body, &id, body);

    self.ctx.commit_node(id)
  }

  pub fn write_this_expr(&mut self, span: &Span) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ThisExpression, span);
    self.ctx.commit_node(id)
  }

  pub fn write_super(&mut self, span: &Span) -> NodeRef {
    let id = self.ctx.append_node(AstNode::Super, span);
    self.ctx.commit_node(id)
  }

  pub fn write_unary_expr(
    &mut self,
    span: &Span,
    operator: &str,
    arg: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::UnaryExpression, span);

    self.ctx.write_str(AstProp::Operator, operator);
    self.ctx.write_ref(AstProp::Argument, &id, arg);

    self.ctx.commit_node(id)
  }

  pub fn write_new_expr(
    &mut self,
    span: &Span,
    callee: NodeRef,
    type_args: Option<NodeRef>,
    args: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::NewExpression, span);

    self.ctx.write_ref(AstProp::Callee, &id, callee);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeArguments, &id, type_args);
    self.ctx.write_ref_vec(AstProp::Arguments, &id, args);

    self.ctx.commit_node(id)
  }

  pub fn write_import_expr(
    &mut self,
    span: &Span,
    source: NodeRef,
    options: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ImportExpression, span);

    self.ctx.write_ref(AstProp::Source, &id, source);
    self.ctx.write_ref(AstProp::Options, &id, options);

    self.ctx.commit_node(id)
  }

  pub fn write_call_expr(
    &mut self,
    span: &Span,
    optional: bool,
    callee: NodeRef,
    type_args: Option<NodeRef>,
    args: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::CallExpression, span);

    self.ctx.write_bool(AstProp::Optional, optional);
    self.ctx.write_ref(AstProp::Callee, &id, callee);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeArguments, &id, type_args);
    self.ctx.write_ref_vec(AstProp::Arguments, &id, args);

    self.ctx.commit_node(id)
  }

  pub fn write_update_expr(
    &mut self,
    span: &Span,
    prefix: bool,
    operator: &str,
    arg: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::UpdateExpression, span);

    self.ctx.write_bool(AstProp::Prefix, prefix);
    self.ctx.write_str(AstProp::Operator, operator);
    self.ctx.write_ref(AstProp::Argument, &id, arg);

    self.ctx.commit_node(id)
  }

  pub fn write_assignment_expr(
    &mut self,
    span: &Span,
    operator: &str,
    left: NodeRef,
    right: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::AssignmentExpression, span);

    self.ctx.write_str(AstProp::Operator, operator);
    self.ctx.write_ref(AstProp::Left, &id, left);
    self.ctx.write_ref(AstProp::Right, &id, right);

    self.ctx.commit_node(id)
  }

  pub fn write_conditional_expr(
    &mut self,
    span: &Span,
    test: NodeRef,
    consequent: NodeRef,
    alternate: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ConditionalExpression, span);

    self.ctx.write_ref(AstProp::Test, &id, test);
    self.ctx.write_ref(AstProp::Consequent, &id, consequent);
    self.ctx.write_ref(AstProp::Alternate, &id, alternate);

    self.ctx.commit_node(id)
  }

  pub fn write_member_expr(
    &mut self,
    span: &Span,
    optional: bool,
    computed: bool,
    obj: NodeRef,
    prop: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::MemberExpression, span);

    self.ctx.write_bool(AstProp::Optional, optional);
    self.ctx.write_bool(AstProp::Computed, computed);
    self.ctx.write_ref(AstProp::Object, &id, obj);
    self.ctx.write_ref(AstProp::Property, &id, prop);

    self.ctx.commit_node(id)
  }

  pub fn write_chain_expr(&mut self, span: &Span, expr: NodeRef) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ChainExpression, span);
    self.ctx.write_ref(AstProp::Expression, &id, expr);
    self.ctx.commit_node(id)
  }

  pub fn write_sequence_expr(
    &mut self,
    span: &Span,
    exprs: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::SequenceExpression, span);
    self.ctx.write_ref_vec(AstProp::Expressions, &id, exprs);
    self.ctx.commit_node(id)
  }

  pub fn write_template_lit(
    &mut self,
    span: &Span,
    quasis: Vec<NodeRef>,
    exprs: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TemplateLiteral, span);

    self.ctx.write_ref_vec(AstProp::Quasis, &id, quasis);
    self.ctx.write_ref_vec(AstProp::Expressions, &id, exprs);

    self.ctx.commit_node(id)
  }

  pub fn write_template_elem(
    &mut self,
    span: &Span,
    tail: bool,
    raw: &str,
    cooked: &str,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TemplateElement, span);

    self.ctx.write_bool(AstProp::Tail, tail);
    self.ctx.write_str(AstProp::Raw, raw);
    self.ctx.write_str(AstProp::Cooked, cooked);

    self.ctx.commit_node(id)
  }

  pub fn write_tagged_template_expr(
    &mut self,
    span: &Span,
    tag: NodeRef,
    type_args: Option<NodeRef>,
    quasi: NodeRef,
  ) -> NodeRef {
    let id = self
      .ctx
      .append_node(AstNode::TaggedTemplateExpression, span);

    self.ctx.write_ref(AstProp::Tag, &id, tag);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeArguments, &id, type_args);
    self.ctx.write_ref(AstProp::Quasi, &id, quasi);

    self.ctx.commit_node(id)
  }

  pub fn write_yield_expr(
    &mut self,
    span: &Span,
    delegate: bool,
    arg: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::YieldExpression, span);

    self.ctx.write_bool(AstProp::Delegate, delegate);
    self.ctx.write_maybe_ref(AstProp::Argument, &id, arg);

    self.ctx.commit_node(id)
  }

  pub fn write_await_expr(&mut self, span: &Span, arg: NodeRef) -> NodeRef {
    let id = self.ctx.append_node(AstNode::AwaitExpression, span);
    self.ctx.write_ref(AstProp::Argument, &id, arg);
    self.ctx.commit_node(id)
  }

  pub fn write_identifier(
    &mut self,
    span: &Span,
    name: &str,
    optional: bool,
    type_annotation: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::Identifier, span);

    self.ctx.write_str(AstProp::Name, name);
    self.ctx.write_bool(AstProp::Optional, optional);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeAnnotation, &id, type_annotation);

    self.ctx.commit_node(id)
  }

  pub fn write_private_identifier(
    &mut self,
    span: &Span,
    name: &str,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::PrivateIdentifier, span);
    self.ctx.write_str(AstProp::Name, name);
    self.ctx.commit_node(id)
  }

  pub fn write_assign_pat(
    &mut self,
    span: &Span,
    left: NodeRef,
    right: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::AssignmentPattern, span);

    self.ctx.write_ref(AstProp::Left, &id, left);
    self.ctx.write_ref(AstProp::Right, &id, right);

    self.ctx.commit_node(id)
  }

  pub fn write_arr_pat(
    &mut self,
    span: &Span,
    optional: bool,
    type_ann: Option<NodeRef>,
    elems: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ArrayPattern, span);

    self.ctx.write_bool(AstProp::Optional, optional);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeAnnotation, &id, type_ann);
    self.ctx.write_ref_vec(AstProp::Elements, &id, elems);

    self.ctx.commit_node(id)
  }

  pub fn write_obj_pat(
    &mut self,
    span: &Span,
    optional: bool,
    type_ann: Option<NodeRef>,
    props: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::ObjectPattern, span);

    self.ctx.write_bool(AstProp::Optional, optional);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeAnnotation, &id, type_ann);
    self.ctx.write_ref_vec(AstProp::Properties, &id, props);

    self.ctx.commit_node(id)
  }

  pub fn write_rest_elem(
    &mut self,
    span: &Span,
    type_ann: Option<NodeRef>,
    arg: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::RestElement, span);

    self
      .ctx
      .write_maybe_ref(AstProp::TypeAnnotation, &id, type_ann);
    self.ctx.write_ref(AstProp::Argument, &id, arg);

    self.ctx.commit_node(id)
  }

  pub fn write_spread(&mut self, span: &Span, arg: NodeRef) -> NodeRef {
    let id = self.ctx.append_node(AstNode::SpreadElement, span);
    self.ctx.write_ref(AstProp::Argument, &id, arg);
    self.ctx.commit_node(id)
  }

  pub fn write_property(
    &mut self,
    span: &Span,
    shorthand: bool,
    computed: bool,
    method: bool,
    kind: &str,
    key: NodeRef,
    value: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::Property, span);

    self.ctx.write_bool(AstProp::Shorthand, shorthand);
    self.ctx.write_bool(AstProp::Computed, computed);
    self.ctx.write_bool(AstProp::Method, method);
    self.ctx.write_str(AstProp::Kind, kind);
    self.ctx.write_ref(AstProp::Key, &id, key);
    self.ctx.write_ref(AstProp::Value, &id, value);

    self.ctx.commit_node(id)
  }

  pub fn write_str_lit(
    &mut self,
    span: &Span,
    value: &str,
    raw: &str,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::Literal, span);

    self.ctx.write_str(AstProp::Value, value);
    self.ctx.write_str(AstProp::Raw, raw);

    self.ctx.commit_node(id)
  }

  pub fn write_bool_lit(&mut self, span: &Span, value: bool) -> NodeRef {
    let id = self.ctx.append_node(AstNode::Literal, span);

    let raw = &format!("{}", value);
    self.ctx.write_str(AstProp::Raw, raw);
    self.ctx.write_bool(AstProp::Value, value);

    self.ctx.commit_node(id)
  }

  pub fn write_null_lit(&mut self, span: &Span) -> NodeRef {
    let id = self.ctx.append_node(AstNode::Literal, span);

    self.ctx.write_null(AstProp::Value);
    self.ctx.write_str(AstProp::Raw, "null");

    self.ctx.commit_node(id)
  }

  pub fn write_num_lit(
    &mut self,
    span: &Span,
    value: &str,
    raw: &str,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::Literal, span);

    self.ctx.write_num(AstProp::Value, value);
    self.ctx.write_str(AstProp::Raw, raw);

    self.ctx.commit_node(id)
  }

  pub fn write_bigint_lit(
    &mut self,
    span: &Span,
    value: &str,
    raw: &str,
    bigint_value: &str,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::Literal, span);

    self.ctx.write_bigint(AstProp::Value, value);
    self.ctx.write_str(AstProp::Raw, raw);
    self.ctx.write_str(AstProp::BigInt, bigint_value);

    self.ctx.commit_node(id)
  }

  pub fn write_regex_lit(
    &mut self,
    span: &Span,
    pattern: &str,
    flags: &str,
    value: &str,
    raw: &str,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::Literal, span);

    self.ctx.write_obj(AstProp::Regex, 2);
    self.ctx.write_str(AstProp::Flags, flags);
    self.ctx.write_str(AstProp::Pattern, pattern);
    self.ctx.write_regex(AstProp::Value, value);
    self.ctx.write_str(AstProp::Raw, raw);

    self.ctx.commit_node(id)
  }

  pub fn write_jsx_identifier(&mut self, span: &Span, name: &str) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXIdentifier, span);
    self.ctx.write_str(AstProp::Name, name);
    self.ctx.commit_node(id)
  }

  pub fn write_jsx_namespaced_name(
    &mut self,
    span: &Span,
    namespace: NodeRef,
    name: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXNamespacedName, span);

    self.ctx.write_ref(AstProp::Namespace, &id, namespace);
    self.ctx.write_ref(AstProp::Name, &id, name);

    self.ctx.commit_node(id)
  }

  pub fn write_jsx_empty_expr(&mut self, span: &Span) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXEmptyExpression, span);
    self.ctx.commit_node(id)
  }

  pub fn write_jsx_elem(
    &mut self,
    span: &Span,
    opening: NodeRef,
    closing: Option<NodeRef>,
    children: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXElement, span);

    self.ctx.write_ref(AstProp::OpeningElement, &id, opening);
    self
      .ctx
      .write_maybe_ref(AstProp::ClosingElement, &id, closing);
    self.ctx.write_ref_vec(AstProp::Children, &id, children);

    self.ctx.commit_node(id)
  }

  pub fn write_jsx_opening_elem(
    &mut self,
    span: &Span,
    self_closing: bool,
    name: NodeRef,
    attrs: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXOpeningElement, span);

    self.ctx.write_bool(AstProp::SelfClosing, self_closing);
    self.ctx.write_ref(AstProp::Name, &id, name);
    self.ctx.write_ref_vec(AstProp::Attributes, &id, attrs);

    self.ctx.commit_node(id)
  }

  pub fn write_jsx_attr(
    &mut self,
    span: &Span,
    name: NodeRef,
    value: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXAttribute, span);

    self.ctx.write_ref(AstProp::Name, &id, name);
    self.ctx.write_maybe_ref(AstProp::Value, &id, value);

    self.ctx.commit_node(id)
  }

  pub fn write_jsx_spread_attr(
    &mut self,
    span: &Span,
    arg: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXSpreadAttribute, span);
    self.ctx.write_ref(AstProp::Argument, &id, arg);
    self.ctx.commit_node(id)
  }

  pub fn write_jsx_closing_elem(
    &mut self,
    span: &Span,
    name: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXClosingElement, span);
    self.ctx.write_ref(AstProp::Name, &id, name);
    self.ctx.commit_node(id)
  }

  pub fn write_jsx_frag(
    &mut self,
    span: &Span,
    opening: NodeRef,
    closing: NodeRef,
    children: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXFragment, span);

    self.ctx.write_ref(AstProp::OpeningFragment, &id, opening);
    self.ctx.write_ref(AstProp::ClosingFragment, &id, closing);
    self.ctx.write_ref_vec(AstProp::Children, &id, children);

    self.ctx.commit_node(id)
  }

  pub fn write_jsx_opening_frag(&mut self, span: &Span) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXOpeningFragment, span);
    self.ctx.commit_node(id)
  }

  pub fn write_jsx_closing_frag(&mut self, span: &Span) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXClosingFragment, span);
    self.ctx.commit_node(id)
  }

  pub fn write_jsx_expr_container(
    &mut self,
    span: &Span,
    expr: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXExpressionContainer, span);
    self.ctx.write_ref(AstProp::Expression, &id, expr);
    self.ctx.commit_node(id)
  }

  pub fn write_jsx_text(
    &mut self,
    span: &Span,
    raw: &str,
    value: &str,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXText, span);

    self.ctx.write_str(AstProp::Raw, raw);
    self.ctx.write_str(AstProp::Value, value);

    self.ctx.commit_node(id)
  }

  pub fn write_jsx_member_expr(
    &mut self,
    span: &Span,
    obj: NodeRef,
    prop: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::JSXMemberExpression, span);

    self.ctx.write_ref(AstProp::Object, &id, obj);
    self.ctx.write_ref(AstProp::Property, &id, prop);

    self.ctx.commit_node(id)
  }

  pub fn _write_ts_class_implements(
    &mut self,
    span: &Span,
    expr: NodeRef,
    type_args: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSClassImplements, span);

    self.ctx.write_ref(AstProp::Expression, &id, expr);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeArguments, &id, type_args);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_call_sig_decl(
    &mut self,
    span: &Span,
    type_ann: Option<NodeRef>,
    params: Vec<NodeRef>,
    return_type: Option<NodeRef>,
  ) -> NodeRef {
    let id = self
      .ctx
      .append_node(AstNode::TsCallSignatureDeclaration, span);

    self
      .ctx
      .write_maybe_ref(AstProp::TypeAnnotation, &id, type_ann);
    self.ctx.write_ref_vec(AstProp::Params, &id, params);
    self
      .ctx
      .write_maybe_ref(AstProp::ReturnType, &id, return_type);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_property_sig(
    &mut self,
    span: &Span,
    computed: bool,
    optional: bool,
    readonly: bool,
    key: NodeRef,
    type_ann: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSPropertySignature, span);

    self.ctx.write_bool(AstProp::Computed, computed);
    self.ctx.write_bool(AstProp::Optional, optional);
    self.ctx.write_bool(AstProp::Readonly, readonly);
    // TODO(@marvinhagemeister) not sure where this is coming from
    self.ctx.write_bool(AstProp::Static, false);

    self.ctx.write_ref(AstProp::Key, &id, key);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeAnnotation, &id, type_ann);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_enum(
    &mut self,
    span: &Span,
    declare: bool,
    is_const: bool,
    ident: NodeRef,
    body: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSEnumDeclaration, span);

    self.ctx.write_bool(AstProp::Declare, declare);
    self.ctx.write_bool(AstProp::Const, is_const);
    self.ctx.write_ref(AstProp::Id, &id, ident);
    self.ctx.write_ref(AstProp::Body, &id, body);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_enum_body(
    &mut self,
    span: &Span,
    members: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSEnumBody, span);
    self.ctx.write_ref_vec(AstProp::Members, &id, members);
    self.ctx.commit_node(id)
  }

  pub fn write_ts_enum_member(
    &mut self,
    span: &Span,
    ident: NodeRef,
    init: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSEnumMember, span);

    self.ctx.write_ref(AstProp::Id, &id, ident);
    self.ctx.write_maybe_ref(AstProp::Initializer, &id, init);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_type_assertion(
    &mut self,
    span: &Span,
    expr: NodeRef,
    type_ann: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSTypeAssertion, span);

    self.ctx.write_ref(AstProp::Expression, &id, expr);
    self.ctx.write_ref(AstProp::TypeAnnotation, &id, type_ann);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_type_param_inst(
    &mut self,
    span: &Span,
    params: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self
      .ctx
      .append_node(AstNode::TSTypeParameterInstantiation, span);

    self.ctx.write_ref_vec(AstProp::Params, &id, params);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_type_alias(
    &mut self,
    span: &Span,
    declare: bool,
    ident: NodeRef,
    type_param: Option<NodeRef>,
    type_ann: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSTypeAliasDeclaration, span);

    self.ctx.write_bool(AstProp::Declare, declare);
    self.ctx.write_ref(AstProp::Id, &id, ident);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeParameters, &id, type_param);
    self.ctx.write_ref(AstProp::TypeAnnotation, &id, type_ann);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_satisfies_expr(
    &mut self,
    span: &Span,
    expr: NodeRef,
    type_ann: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSSatisfiesExpression, span);

    self.ctx.write_ref(AstProp::Expression, &id, expr);
    self.ctx.write_ref(AstProp::TypeAnnotation, &id, type_ann);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_as_expr(
    &mut self,
    span: &Span,
    expr: NodeRef,
    type_ann: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSAsExpression, span);

    self.ctx.write_ref(AstProp::Expression, &id, expr);
    self.ctx.write_ref(AstProp::TypeAnnotation, &id, type_ann);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_non_null(&mut self, span: &Span, expr: NodeRef) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSNonNullExpression, span);
    self.ctx.write_ref(AstProp::Expression, &id, expr);
    self.ctx.commit_node(id)
  }

  pub fn write_ts_this_type(&mut self, span: &Span) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSThisType, span);
    self.ctx.commit_node(id)
  }

  pub fn write_ts_interface(
    &mut self,
    span: &Span,
    declare: bool,
    ident: NodeRef,
    type_param: Option<NodeRef>,
    extends: Vec<NodeRef>,
    body: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSInterface, span);

    self.ctx.write_bool(AstProp::Declare, declare);
    self.ctx.write_ref(AstProp::Id, &id, ident);
    self.ctx.write_maybe_ref(AstProp::Extends, &id, type_param);
    self
      .ctx
      .write_ref_vec(AstProp::TypeParameters, &id, extends);
    self.ctx.write_ref(AstProp::Body, &id, body);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_interface_body(
    &mut self,
    span: &Span,
    body: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSInterfaceBody, span);
    self.ctx.write_ref_vec(AstProp::Body, &id, body);
    self.ctx.commit_node(id)
  }

  pub fn write_ts_getter_sig(
    &mut self,
    span: &Span,
    key: NodeRef,
    return_type: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSMethodSignature, span);

    self.ctx.write_bool(AstProp::Computed, false);
    self.ctx.write_bool(AstProp::Optional, false);
    self.ctx.write_bool(AstProp::Readonly, false);
    self.ctx.write_bool(AstProp::Static, false);
    self.ctx.write_str(AstProp::Kind, "getter");
    self.ctx.write_ref(AstProp::Key, &id, key);
    self
      .ctx
      .write_maybe_ref(AstProp::ReturnType, &id, return_type);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_setter_sig(
    &mut self,
    span: &Span,
    key: NodeRef,
    param: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSMethodSignature, span);

    self.ctx.write_bool(AstProp::Computed, false);
    self.ctx.write_bool(AstProp::Optional, false);
    self.ctx.write_bool(AstProp::Readonly, false);
    self.ctx.write_bool(AstProp::Static, false);
    self.ctx.write_str(AstProp::Kind, "setter");
    self.ctx.write_ref(AstProp::Key, &id, key);
    self.ctx.write_ref_vec(AstProp::Params, &id, vec![param]);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_interface_heritage(
    &mut self,
    span: &Span,
    expr: NodeRef,
    type_args: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSInterfaceHeritage, span);

    self.ctx.write_ref(AstProp::Expression, &id, expr);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeArguments, &id, type_args);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_union_type(
    &mut self,
    span: &Span,
    types: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSUnionType, span);
    self.ctx.write_ref_vec(AstProp::Types, &id, types);
    self.ctx.commit_node(id)
  }

  pub fn write_ts_intersection_type(
    &mut self,
    span: &Span,
    types: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSIntersectionType, span);
    self.ctx.write_ref_vec(AstProp::Types, &id, types);
    self.ctx.commit_node(id)
  }

  pub fn write_ts_infer_type(
    &mut self,
    span: &Span,
    type_param: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSInferType, span);
    self.ctx.write_ref(AstProp::TypeParameter, &id, type_param);
    self.ctx.commit_node(id)
  }

  pub fn write_ts_type_op(
    &mut self,
    span: &Span,
    op: &str,
    type_ann: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSTypeOperator, span);

    self.ctx.write_str(AstProp::Operator, op);
    self.ctx.write_ref(AstProp::TypeAnnotation, &id, type_ann);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_indexed_access_type(
    &mut self,
    span: &Span,
    index: NodeRef,
    obj: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSIndexedAccessType, span);

    self.ctx.write_ref(AstProp::IndexType, &id, index);
    self.ctx.write_ref(AstProp::ObjectType, &id, obj);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_keyword(
    &mut self,
    kind: TsKeywordKind,
    span: &Span,
  ) -> NodeRef {
    let kind = match kind {
      TsKeywordKind::Any => AstNode::TSAnyKeyword,
      TsKeywordKind::Unknown => AstNode::TSUnknownKeyword,
      TsKeywordKind::Number => AstNode::TSNumberKeyword,
      TsKeywordKind::Object => AstNode::TSObjectKeyword,
      TsKeywordKind::Boolean => AstNode::TSBooleanKeyword,
      TsKeywordKind::BigInt => AstNode::TSBigIntKeyword,
      TsKeywordKind::String => AstNode::TSStringKeyword,
      TsKeywordKind::Symbol => AstNode::TSSymbolKeyword,
      TsKeywordKind::Void => AstNode::TSVoidKeyword,
      TsKeywordKind::Undefined => AstNode::TSUndefinedKeyword,
      TsKeywordKind::Null => AstNode::TSNullKeyword,
      TsKeywordKind::Never => AstNode::TSNeverKeyword,
      TsKeywordKind::Intrinsic => AstNode::TSIntrinsicKeyword,
    };

    let id = self.ctx.append_node(kind, span);
    self.ctx.commit_node(id)
  }

  pub fn write_ts_rest_type(
    &mut self,
    span: &Span,
    type_ann: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSRestType, span);
    self.ctx.write_ref(AstProp::TypeAnnotation, &id, type_ann);
    self.ctx.commit_node(id)
  }

  pub fn write_ts_conditional_type(
    &mut self,
    span: &Span,
    check: NodeRef,
    extends: NodeRef,
    true_type: NodeRef,
    false_type: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSConditionalType, span);

    self.ctx.write_ref(AstProp::CheckType, &id, check);
    self.ctx.write_ref(AstProp::ExtendsType, &id, extends);
    self.ctx.write_ref(AstProp::TrueType, &id, true_type);
    self.ctx.write_ref(AstProp::FalseType, &id, false_type);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_mapped_type(
    &mut self,
    span: &Span,
    name: Option<NodeRef>,
    type_ann: Option<NodeRef>,
    type_param: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSMappedType, span);

    self.ctx.write_maybe_ref(AstProp::NameType, &id, name);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeAnnotation, &id, type_ann);
    self.ctx.write_ref(AstProp::TypeParameter, &id, type_param);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_lit_type(&mut self, span: &Span, lit: NodeRef) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSLiteralType, span);
    self.ctx.write_ref(AstProp::Literal, &id, lit);
    self.ctx.commit_node(id)
  }

  pub fn write_ts_type_ann(
    &mut self,
    span: &Span,
    type_ann: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSTypeAnnotation, span);
    self.ctx.write_ref(AstProp::TypeAnnotation, &id, type_ann);
    self.ctx.commit_node(id)
  }

  pub fn write_ts_array_type(
    &mut self,
    span: &Span,
    elem_type: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSArrayType, span);
    self.ctx.write_ref(AstProp::ElementType, &id, elem_type);
    self.ctx.commit_node(id)
  }

  pub fn write_ts_type_query(
    &mut self,
    span: &Span,
    expr_name: NodeRef,
    type_arg: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSTypeQuery, span);

    self.ctx.write_ref(AstProp::ExprName, &id, expr_name);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeArguments, &id, type_arg);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_type_ref(
    &mut self,
    span: &Span,
    type_name: NodeRef,
    type_arg: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSTypeReference, span);

    self.ctx.write_ref(AstProp::TypeName, &id, type_name);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeArguments, &id, type_arg);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_type_predicate(
    &mut self,
    span: &Span,
    asserts: bool,
    param_name: NodeRef,
    type_ann: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSTypePredicate, span);

    self.ctx.write_bool(AstProp::Asserts, asserts);
    self.ctx.write_ref(AstProp::ParameterName, &id, param_name);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeAnnotation, &id, type_ann);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_tuple_type(
    &mut self,
    span: &Span,
    elem_types: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSTupleType, span);

    self
      .ctx
      .write_ref_vec(AstProp::ElementTypes, &id, elem_types);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_named_tuple_member(
    &mut self,
    span: &Span,
    label: NodeRef,
    elem_type: NodeRef,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSNamedTupleMember, span);

    self.ctx.write_ref(AstProp::Label, &id, label);
    self.ctx.write_ref(AstProp::ElementType, &id, elem_type);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_type_param_decl(
    &mut self,
    span: &Span,
    params: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self
      .ctx
      .append_node(AstNode::TSTypeParameterDeclaration, span);

    self.ctx.write_ref_vec(AstProp::Params, &id, params);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_type_param(
    &mut self,
    span: &Span,
    is_in: bool,
    is_out: bool,
    is_const: bool,
    name: NodeRef,
    constraint: Option<NodeRef>,
    default: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSTypeParameter, span);

    self.ctx.write_bool(AstProp::In, is_in);
    self.ctx.write_bool(AstProp::Out, is_out);
    self.ctx.write_bool(AstProp::Const, is_const);
    self.ctx.write_ref(AstProp::Name, &id, name);
    self
      .ctx
      .write_maybe_ref(AstProp::Constraint, &id, constraint);
    self.ctx.write_maybe_ref(AstProp::Default, &id, default);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_import_type(
    &mut self,
    span: &Span,
    arg: NodeRef,
    qualifier: Option<NodeRef>,
    type_args: Option<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSImportType, span);

    self.ctx.write_ref(AstProp::Argument, &id, arg);
    self.ctx.write_maybe_ref(AstProp::Qualifier, &id, qualifier);
    self
      .ctx
      .write_maybe_ref(AstProp::TypeArguments, &id, type_args);

    self.ctx.commit_node(id)
  }

  pub fn write_ts_fn_type(
    &mut self,
    span: &Span,
    params: Vec<NodeRef>,
  ) -> NodeRef {
    let id = self.ctx.append_node(AstNode::TSFunctionType, span);
    self.ctx.write_ref_vec(AstProp::Params, &id, params);
    self.ctx.commit_node(id)
  }
}

#[derive(Debug)]
pub enum TsKeywordKind {
  Any,
  Unknown,
  Number,
  Object,
  Boolean,
  BigInt,
  String,
  Symbol,
  Void,
  Undefined,
  Null,
  Never,
  Intrinsic,
}
