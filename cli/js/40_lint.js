// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check

import { core } from "ext:core/mod.js";
const {
  op_lint_get_rule,
  op_lint_get_source,
  op_lint_report,
} = core.ops;

/** @typedef {{ plugins: Deno.LintPlugin[], installedPlugins: Set<string> }} LintState */

/** @type {LintState} */
const state = {
  plugins: [],
  installedPlugins: new Set(),
};

/** @implements {Deno.LintRuleContext} */
export class Context {
  id;

  fileName;

  #source = null;

  /**
   * @param {string} id
   * @param {string} fileName
   */
  constructor(id, fileName) {
    this.id = id;
    this.fileName = fileName;
  }

  source() {
    if (this.#source === null) {
      this.#source = op_lint_get_source();
    }
    return /** @type {*} */ (this.#source);
  }

  report(data) {
    let start, end;

    if (data.node) {
      start = data.node.span.start - 1;
      end = data.node.span.end - 1;
    } else if (data.span) {
      start = data.span.start - 1;
      end = data.span.end - 1;
    } else {
      throw new Error(
        "Either `node` or `span` must be provided when reporting an error",
      );
    }

    op_lint_report(
      this.id,
      this.fileName,
      data.message,
      start,
      end,
    );
  }
}

/**
 * @param {Deno.LintPlugin} plugin
 */
export function installPlugin(plugin) {
  if (typeof plugin !== "object") {
    throw new Error("Linter plugin must be an object");
  }
  if (typeof plugin.name !== "string") {
    throw new Error("Linter plugin name must be a string");
  }
  if (typeof plugin.rules !== "object") {
    throw new Error("Linter plugin rules must be an object");
  }
  if (state.installedPlugins.has(plugin.name)) {
    throw new Error(`Linter plugin ${plugin.name} has already been registered`);
  }
  state.plugins.push(plugin);
  state.installedPlugins.add(plugin.name);
}

// Keep in sync with Rust
/**
 * @enum {number}
 */
const Flags = {
  ProgramModule: 0b00000001,
  FnAsync: 0b00000001,
  FnGenerator: 0b00000010,
  FnDeclare: 0b00000100,
  MemberComputed: 0b00000001,
  PropShorthand: 0b00000001,
  PropComputed: 0b00000010,
  PropGetter: 0b00000100,
  PropSetter: 0b00001000,
  PropMethod: 0b00010000,
  VarVar: 0b00000001,
  VarConst: 0b00000010,
  VarLet: 0b00000100,
  VarDeclare: 0b00001000,
  ExportType: 0b000000001,
  TplTail: 0b000000001,
  ForAwait: 0b000000001,
  LogicalOr: 0b000000001,
  LogicalAnd: 0b000000010,
  LogicalNullishCoalescin: 0b000000100,
  JSXSelfClosing: 0b000000001,

  BinEqEq: 1,
  BinNotEq: 2,
  BinEqEqEq: 3,
  BinNotEqEq: 4,
  BinLt: 5,
  BinLtEq: 6,
  BinGt: 7,
  BinGtEq: 8,
  BinLShift: 9,
  BinRShift: 10,
  BinZeroFillRShift: 11,
  BinAdd: 12,
  BinSub: 13,
  BinMul: 14,
  BinDiv: 15,
  BinMod: 16,
  BinBitOr: 17,
  BinBitXor: 18,
  BinBitAnd: 19,
  BinIn: 20,
  BinInstanceOf: 21,
  BinExp: 22,

  UnaryMinus: 1,
  UnaryPlus: 2,
  UnaryBang: 3,
  UnaryTilde: 4,
  UnaryTypeOf: 5,
  UnaryVoid: 6,
  UnaryDelete: 7,
};

// Keep in sync with Rust
/**
 * @enum {number}
 */
const AstType = {
  Invalid: 0,
  Program: 1,

  Import: 2,
  ImportDecl: 3,
  ExportDecl: 4,
  ExportNamed: 5,
  ExportDefaultDecl: 6,
  ExportDefaultExpr: 7,
  ExportAll: 8,
  TSImportEquals: 9,
  TSExportAssignment: 10,
  TSNamespaceExport: 11,

  // Decls
  Class: 12,
  FunctionDeclaration: 13,
  VariableDeclaration: 14,
  Using: 15,
  TsInterface: 16,
  TsTypeAlias: 17,
  TsEnum: 18,
  TsModule: 19,

  // Statements
  BlockStatement: 20,
  Empty: 21,
  DebuggerStatement: 22,
  WithStatement: 23,
  ReturnStatement: 24,
  LabeledStatement: 25,
  BreakStatement: 26,
  ContinueStatement: 27,
  IfStatement: 28,
  SwitchStatement: 29,
  SwitchCase: 30,
  ThrowStatement: 31,
  TryStatement: 32,
  WhileStatement: 33,
  DoWhileStatement: 34,
  ForStatement: 35,
  ForInStatement: 36,
  ForOfStatement: 37,
  Decl: 38,
  ExpressionStatement: 39,

  // Expressions
  This: 40,
  ArrayExpression: 41,
  ObjectExpression: 42,
  FunctionExpression: 43,
  UnaryExpression: 44,
  Update: 45,
  BinaryExpression: 46,
  AssignmentExpression: 47,
  MemberExpression: 48,
  Super: 49,
  ConditionalExpression: 50,
  CallExpression: 51,
  NewExpression: 52,
  ParenthesisExpression: 53,
  SequenceExpression: 54,
  Identifier: 55,
  TemplateLiteral: 56,
  TaggedTemplateExpression: 57,
  ArrowFunctionExpression: 58,
  ClassExpr: 59,
  Yield: 60,
  MetaProperty: 61,
  AwaitExpression: 62,
  LogicalExpression: 63,
  TSTypeAssertion: 64,
  TSConstAssertion: 65,
  TSNonNull: 66,
  TSAs: 67,
  TSInstantiation: 68,
  TSSatisfies: 69,
  PrivateIdentifier: 70,
  OptChain: 71,

  StringLiteral: 72,
  BooleanLiteral: 73,
  NullLiteral: 74,
  NumericLiteral: 75,
  BigIntLiteral: 76,
  RegExpLiteral: 77,

  // Custom
  EmptyExpr: 78,
  SpreadElement: 79,
  Property: 80,
  VariableDeclarator: 81,
  CatchClause: 82,
  RestElement: 83,
  ExportSpecifier: 84,
  TemplateElement: 85,

  // Patterns
  ArrayPattern: 86,
  AssignmentPattern: 87,
  ObjectPattern: 88,

  // JSX
  JSXAttribute: 89,
  JSXClosingElement: 90,
  JSXClosingFragment: 91,
  JSXElement: 92,
  JSXEmptyExpression: 93,
  JSXExpressionContainer: 94,
  JSXFragment: 95,
  JSXIdentifier: 96,
  JSXMemberExpression: 97,
  JSXNamespacedName: 98,
  JSXOpeningElement: 99,
  JSXOpeningFragment: 100,
  JSXSpreadAttribute: 101,
  JSXSpreadChild: 102,
  JSXText: 103,
};

const AstNodeById = Object.keys(AstType);

/**
 * @param {AstContext} ctx
 * @param {number[]} ids
 * @returns {any[]}
 */
function createChildNodes(ctx, ids) {
  /** @type {any[]} */
  const out = [];
  for (let i = 0; i < ids.length; i++) {
    const id = ids[i];
    out.push(createAstNode(ctx, id));
  }

  return out;
}

class BaseNode {
  #ctx;
  #parentId;

  get parent() {
    return /** @type {*} */ (createAstNode(
      this.#ctx,
      this.#parentId,
    ));
  }

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   */
  constructor(ctx, parentId) {
    this.#ctx = ctx;
    this.#parentId = parentId;
  }
}

/** @implements {Deno.Program} */
class Program extends BaseNode {
  type = /** @type {const} */ ("Program");
  range;
  get body() {
    return createChildNodes(this.#ctx, this.#childIds);
  }

  #ctx;
  #childIds;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {Deno.Program["sourceType"]} sourceType
   * @param {number[]} childIds
   */
  constructor(ctx, parentId, range, sourceType, childIds) {
    super(ctx, parentId);
    this.#ctx = ctx;
    this.range = range;
    this.sourceType = sourceType;
    this.#childIds = childIds;
  }
}

// Declarations

/** @implements {Deno.VariableDeclaration} */
class VariableDeclaration extends BaseNode {
  type = /** @type {const} */ ("VariableDeclaration");
  range;
  get declarations() {
    return createChildNodes(this.#ctx, this.#childIds);
  }

  #ctx;
  #childIds;
  kind;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} flags
   * @param {number[]} childIds
   */
  constructor(ctx, parentId, range, flags, childIds) {
    super(ctx, parentId);
    this.#ctx = ctx;
    this.range = range;
    this.kind = (Flags.VarConst & flags) != 0
      ? /** @type {const} */ ("const")
      : (Flags.VarLet & flags) != 0
      ? /** @type {const} */ ("let")
      : /** @type {const} */ ("var");

    // FIXME: Declare
    this.#childIds = childIds;
  }
}

/** @implements {Deno.VariableDeclarator} */
class VariableDeclarator extends BaseNode {
  type = /** @type {const} */ ("VariableDeclarator");
  range;

  get id() {
    return /** @type {*} */ (createAstNode(this.#ctx, this.#nameId));
  }

  get init() {
    if (this.#initId === 0) return null;
    return /** @type {*} */ (createAstNode(this.#ctx, this.#initId));
  }

  #ctx;
  #nameId;
  #initId;
  definite = false;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} nameId
   * @param {number} initId
   */
  constructor(ctx, parentId, range, nameId, initId) {
    // FIXME: Definite
    super(ctx, parentId);
    this.#ctx = ctx;
    this.range = range;
    this.#nameId = nameId;
    this.#initId = initId;
  }
}

// Statements

/** @implements {Deno.BlockStatement} */
class BlockStatement extends BaseNode {
  type = /** @type {const} */ ("BlockStatement");
  get body() {
    return createChildNodes(this.#ctx, this.#childIds);
  }
  range;

  #ctx;
  #childIds;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number[]} childIds
   */
  constructor(ctx, parentId, range, childIds) {
    super(ctx, parentId);
    this.#ctx = ctx;
    this.range = range;
    this.#childIds = childIds;
  }
}

/** @implements {Deno.BreakStatement} */
class BreakStatement extends BaseNode {
  type = /** @type {const} */ ("BreakStatement");
  get label() {
    if (this.#labelId === 0) return null;
    return /** @type {Deno.Identifier} */ (createAstNode(
      this.#ctx,
      this.#labelId,
    ));
  }
  range;

  #ctx;
  #labelId;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} labelId
   */
  constructor(ctx, parentId, range, labelId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#labelId = labelId;
    this.range = range;
  }
}

/** @implements {Deno.ContinueStatement} */
class ContinueStatement extends BaseNode {
  type = /** @type {const} */ ("ContinueStatement");
  range;
  get label() {
    return /** @type {Deno.Identifier} */ (createAstNode(
      this.#ctx,
      this.#labelId,
    ));
  }

  #ctx;
  #labelId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} labelId
   */
  constructor(ctx, parentId, range, labelId) {
    super(ctx, parentId);
    this.#ctx = ctx;
    this.#labelId = labelId;
    this.range = range;
  }
}

/** @implements {Deno.DebuggerStatement} */
class DebuggerStatement extends BaseNode {
  type = /** @type {const} */ ("DebuggerStatement");
  range;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   */
  constructor(ctx, parentId, range) {
    super(ctx, parentId);
    this.range = range;
  }
}

/** @implements {Deno.DoWhileStatement} */
class DoWhileStatement extends BaseNode {
  type = /** @type {const} */ ("DoWhileStatement");
  range;
  get test() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#exprId,
    ));
  }
  get body() {
    return /** @type {Deno.Statement} */ (createAstNode(
      this.#ctx,
      this.#bodyId,
    ));
  }

  #ctx;
  #exprId = 0;
  #bodyId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} exprId
   * @param {number} bodyId
   */
  constructor(ctx, parentId, range, exprId, bodyId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#exprId = exprId;
    this.#bodyId = bodyId;
    this.range = range;
  }
}

/** @implements {Deno.ExpressionStatement} */
class ExpressionStatement extends BaseNode {
  type = /** @type {const} */ ("ExpressionStatement");
  range;
  get expression() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#exprId,
    ));
  }

  #ctx;
  #exprId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} exprId
   */
  constructor(ctx, parentId, range, exprId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#exprId = exprId;
    this.range = range;
  }
}

/** @implements {Deno.ForInStatement} */
class ForInStatement extends BaseNode {
  type = /** @type {const} */ ("ForInStatement");
  range;
  get left() {
    return /** @type {Deno.Expression | Deno.VariableDeclaration} */ (createAstNode(
      this.#ctx,
      this.#leftId,
    ));
  }
  get right() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#rightId,
    ));
  }
  get body() {
    return /** @type {Deno.Statement} */ (createAstNode(
      this.#ctx,
      this.#bodyId,
    ));
  }

  #ctx;
  #leftId = 0;
  #rightId = 0;
  #bodyId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} leftId
   * @param {number} rightId
   * @param {number} bodyId
   */
  constructor(ctx, parentId, range, leftId, rightId, bodyId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#leftId = leftId;
    this.#rightId = rightId;
    this.#bodyId = bodyId;
    this.range = range;
  }
}

/** @implements {Deno.ForOfStatement} */
class ForOfStatement extends BaseNode {
  type = /** @type {const} */ ("ForOfStatement");
  range;
  get left() {
    return /** @type {Deno.Expression | Deno.VariableDeclaration | Deno.UsingDeclaration} */ (createAstNode(
      this.#ctx,
      this.#leftId,
    ));
  }
  get right() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#rightId,
    ));
  }
  get body() {
    return /** @type {Deno.Statement} */ (createAstNode(
      this.#ctx,
      this.#bodyId,
    ));
  }

  await;

  #ctx;
  #leftId = 0;
  #rightId = 0;
  #bodyId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {boolean} isAwait
   * @param {number} leftId
   * @param {number} rightId
   * @param {number} bodyId
   */
  constructor(ctx, parentId, range, isAwait, leftId, rightId, bodyId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#leftId = leftId;
    this.#rightId = rightId;
    this.#bodyId = bodyId;
    this.range = range;
    this.await = isAwait;
  }
}

/** @implements {Deno.ForStatement} */
class ForStatement extends BaseNode {
  type = /** @type {const} */ ("ForStatement");
  range;
  get init() {
    if (this.#initId === 0) return null;

    return /** @type {Deno.Expression | Deno.VariableDeclaration} */ (createAstNode(
      this.#ctx,
      this.#initId,
    ));
  }
  get test() {
    if (this.#initId === 0) return null;
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#testId,
    ));
  }
  get update() {
    if (this.#updateId === 0) return null;
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#updateId,
    ));
  }
  get body() {
    return /** @type {Deno.Statement} */ (createAstNode(
      this.#ctx,
      this.#bodyId,
    ));
  }

  #ctx;
  #initId = 0;
  #testId = 0;
  #updateId = 0;
  #bodyId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} initId
   * @param {number} testId
   * @param {number} updateId
   * @param {number} bodyId
   */
  constructor(ctx, parentId, range, initId, testId, updateId, bodyId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#initId = initId;
    this.#testId = testId;
    this.#updateId = updateId;
    this.#bodyId = bodyId;
    this.range = range;
  }
}

/** @implements {Deno.IfStatement} */
class IfStatement extends BaseNode {
  type = /** @type {const} */ ("IfStatement");
  range;
  get test() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#testId,
    ));
  }
  get consequent() {
    return /** @type {Deno.Statement} */ (createAstNode(
      this.#ctx,
      this.#consequentId,
    ));
  }
  get alternate() {
    if (this.#alternateId === 0) return null;
    return /** @type {Deno.Statement} */ (createAstNode(
      this.#ctx,
      this.#alternateId,
    ));
  }

  #ctx;
  #testId = 0;
  #consequentId = 0;
  #alternateId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} testId
   * @param {number} updateId
   * @param {number} alternateId
   */
  constructor(ctx, parentId, range, testId, updateId, alternateId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#testId = testId;
    this.#consequentId = updateId;
    this.#alternateId = alternateId;
    this.range = range;
  }
}

/** @implements {Deno.LabeledStatement} */
class LabeledStatement extends BaseNode {
  type = /** @type {const} */ ("LabeledStatement");
  range;
  get label() {
    return /** @type {Deno.Identifier} */ (createAstNode(
      this.#ctx,
      this.#labelId,
    ));
  }
  get body() {
    return /** @type {Deno.Statement} */ (createAstNode(
      this.#ctx,
      this.#bodyId,
    ));
  }

  #ctx;
  #labelId = 0;
  #bodyId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} testId
   * @param {number} bodyId
   */
  constructor(ctx, parentId, range, testId, bodyId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#labelId = testId;
    this.#bodyId = bodyId;
    this.range = range;
  }
}

/** @implements {Deno.ReturnStatement} */
class ReturnStatement extends BaseNode {
  type = /** @type {const} */ ("ReturnStatement");
  range;
  get argument() {
    if (this.#exprId === 0) return null;
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#exprId,
    ));
  }

  #ctx;
  #exprId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} argId
   */
  constructor(ctx, parentId, range, argId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#exprId = argId;
    this.range = range;
  }
}

/** @implements {Deno.SwitchStatement} */
class SwitchStatement extends BaseNode {
  type = /** @type {const} */ ("SwitchStatement");
  range;
  get discriminant() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#discriminantId,
    ));
  }
  get cases() {
    return []; // FIXME
  }

  #ctx;
  #discriminantId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} discriminantId
   */
  constructor(ctx, parentId, range, discriminantId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#discriminantId = discriminantId;
    this.range = range;
  }
}

/** @implements {Deno.ThrowStatement} */
class ThrowStatement extends BaseNode {
  type = /** @type {const} */ ("ThrowStatement");
  range;
  get argument() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#argId,
    ));
  }

  #ctx;
  #argId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} argId
   */
  constructor(ctx, parentId, range, argId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#argId = argId;
    this.range = range;
  }
}

/** @implements {Deno.TryStatement} */
class TryStatement extends BaseNode {
  type = /** @type {const} */ ("TryStatement");
  range;
  get block() {
    return /** @type {Deno.BlockStatement} */ (createAstNode(
      this.#ctx,
      this.#blockId,
    ));
  }
  get finalizer() {
    if (this.#finalizerId === 0) return null;
    return /** @type {Deno.BlockStatement} */ (createAstNode(
      this.#ctx,
      this.#finalizerId,
    ));
  }
  get handler() {
    if (this.#handlerId === 0) return null;
    return /** @type {Deno.CatchClause} */ (createAstNode(
      this.#ctx,
      this.#handlerId,
    ));
  }

  #ctx;
  #blockId = 0;
  #finalizerId = 0;
  #handlerId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} blockId
   * @param {number} finalizerId
   * @param {number} handlerId
   */
  constructor(ctx, parentId, range, blockId, finalizerId, handlerId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#blockId = blockId;
    this.#finalizerId = finalizerId;
    this.#handlerId = handlerId;
    this.range = range;
  }
}

/** @implements {Deno.WhileStatement} */
class WhileStatement extends BaseNode {
  type = /** @type {const} */ ("WhileStatement");
  range;
  get test() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#testId,
    ));
  }
  get body() {
    return /** @type {Deno.Statement} */ (createAstNode(
      this.#ctx,
      this.#bodyId,
    ));
  }

  #ctx;
  #testId = 0;
  #bodyId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} testId
   * @param {number} bodyId
   */
  constructor(ctx, parentId, range, testId, bodyId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#testId = testId;
    this.#bodyId = bodyId;
    this.range = range;
  }
}

/** @implements {Deno.WithStatement} */
class WithStatement {
  type = /** @type {const} */ ("WithStatement");
  range;
  get body() {
    return /** @type {Deno.Statement} */ (createAstNode(
      this.#ctx,
      this.#bodyId,
    ));
  }
  get object() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#objectId,
    ));
  }

  #ctx;
  #bodyId = 0;
  #objectId = 0;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} bodyId
   * @param {number} objectId
   */
  constructor(ctx, range, bodyId, objectId) {
    this.#ctx = ctx;
    this.#bodyId = bodyId;
    this.#objectId = objectId;
    this.range = range;
  }
}

// Expressions

/** @implements {Deno.ArrayExpression} */
class ArrayExpression extends BaseNode {
  type = /** @type {const} */ ("ArrayExpression");
  range;
  get elements() {
    return createChildNodes(this.#ctx, this.#elemIds);
  }

  #ctx;
  #elemIds;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number[]} elemIds
   */
  constructor(ctx, parentId, range, elemIds) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.#elemIds = elemIds;
  }
}

/** @implements {Deno.ArrowFunctionExpression} */
class ArrowFunctionExpression extends BaseNode {
  type = /** @type {const} */ ("ArrowFunctionExpression");
  range;
  async = false;
  generator = false;

  get body() {
    return /** @type {*} */ (createAstNode(
      this.#ctx,
      this.#bodyId,
    ));
  }

  get params() {
    return createChildNodes(this.#ctx, this.#paramIds);
  }

  get returnType() {
    if (this.#returnTypeId === 0) return null;
    return /** @type {*} */ (createAstNode(
      this.#ctx,
      this.#returnTypeId,
    ));
  }

  get typeParameters() {
    if (this.#typeParamId === 0) return null;
    return /** @type {*} */ (createAstNode(
      this.#ctx,
      this.#typeParamId,
    ));
  }

  #ctx;
  #bodyId;
  #typeParamId;
  #paramIds;
  #returnTypeId;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {boolean} isAsync
   * @param {boolean} isGenerator
   * @param {number} typeParamId
   * @param {number[]} paramIds
   * @param {number} bodyId
   * @param {number} returnTypeId
   */
  constructor(
    ctx,
    parentId,
    range,
    isAsync,
    isGenerator,
    typeParamId,
    paramIds,
    bodyId,
    returnTypeId,
  ) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#bodyId = bodyId;
    this.#typeParamId = typeParamId;
    this.#paramIds = paramIds;
    this.#returnTypeId = returnTypeId;
    this.asnyc = isAsync;
    this.generator = isGenerator;
    this.range = range;
  }
}

/** @implements {Deno.AssignmentExpression} */
class AssignmentExpression extends BaseNode {
  type = /** @type {const} */ ("AssignmentExpression");
  range;
  get left() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#leftId,
    ));
  }
  get right() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#rightId,
    ));
  }

  operator;

  #ctx;
  #leftId = 0;
  #rightId = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} flags
   * @param {number} leftId
   * @param {number} rightId
   */
  constructor(ctx, parentId, range, flags, leftId, rightId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#leftId = leftId;
    this.#rightId = rightId;
    this.range = range;
    this.operator = getAssignOperator(flags);
  }
}

/**
 * @param {number} n
 * @returns {Deno.AssignmentExpression["operator"]}
 */
function getAssignOperator(n) {
  switch (n) {
    case 0:
      return "=";
    case 1:
      return "+=";
    case 2:
      return "-=";
    case 3:
      return "*=";
    case 4:
      return "/=";
    case 5:
      return "%=";
    case 6:
      return "<<=";
    case 7:
      return ">>=";
    case 8:
      return ">>>=";
    case 9:
      return "|=";
    case 10:
      return "^=";
    case 11:
      return "&=";
    case 12:
      return "**=";
    case 13:
      return "&&=";
    case 14:
      return "||=";
    case 15:
      return "??=";
    default:
      throw new Error(`Unknown operator: ${n}`);
  }
}

/** @implements {Deno.AwaitExpression} */
class AwaitExpression extends BaseNode {
  type = /** @type {const} */ ("AwaitExpression");
  range;
  get argument() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#argId,
    ));
  }

  #ctx;
  #argId;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} argId
   */
  constructor(ctx, parentId, range, argId) {
    super(ctx, parentId);
    this.#ctx = ctx;
    this.#argId = argId;
    this.range = range;
  }
}

/** @implements {Deno.BinaryExpression} */
class BinaryExpression extends BaseNode {
  type = /** @type {const} */ ("BinaryExpression");
  range;
  get left() {
    return /** @type {Deno.Expression | Deno.PrivateIdentifier} */ (createAstNode(
      this.#ctx,
      this.#leftId,
    ));
  }
  get right() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#rightId,
    ));
  }

  operator;
  #ctx;
  #leftId;
  #rightId;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} flags
   * @param {number} leftId
   * @param {number} rightId
   */
  constructor(ctx, parentId, range, flags, leftId, rightId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#leftId = leftId;
    this.#rightId = rightId;
    this.operator = getBinaryOperator(flags);
    this.range = range;
  }
}

/**
 * @param {number} n
 * @returns {Deno.BinaryExpression["operator"]}
 */
function getBinaryOperator(n) {
  switch (n) {
    case 1:
      return "==";
    case 2:
      return "!=";
    case 3:
      return "===";
    case 4:
      return "!==";
    case 5:
      return "<";
    case 6:
      return "<=";
    case 7:
      return ">";
    case 8:
      return ">=";
    case 9:
      return "<<";
    case 10:
      return ">>";
    case 11:
      return ">>>";
    case 12:
      return "+";
    case 13:
      return "-";
    case 14:
      return "*";
    case 15:
      return "/";
    case 16:
      return "%";
    case 17:
      return "|";
    case 18:
      return "^";
    case 19:
      return "&";
    case 20:
      return "in";
    case 21:
      return "instanceof";
    case 22:
      return "**";
    default:
      throw new Error(`Unknown operator: ${n}`);
  }
}

/** @implements {Deno.CallExpression} */
class CallExpression extends BaseNode {
  type = /** @type {const} */ ("CallExpression");
  range;
  get callee() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#calleeId,
    ));
  }
  get arguments() {
    return createChildNodes(this.#ctx, this.#argumentIds);
  }
  get typeArguments() {
    if (this.#typeArgId === 0) return null;
    return createAstNode(this.#ctx, this.#typeArgId);
  }

  optional = false; // FIXME

  #ctx;
  #calleeId;
  #typeArgId;
  #argumentIds;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} calleeId
   * @param {number} typeArgId
   * @param {number[]} argumentIds
   */
  constructor(ctx, parentId, range, calleeId, typeArgId, argumentIds) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#calleeId = calleeId;
    this.range = range;
    this.#typeArgId = typeArgId;
    this.#argumentIds = argumentIds;
  }
}

/** @implements {Deno.ChainExpression} */
class ChainExpression {
  type = /** @type {const} */ ("ChainExpression");
  range;

  #ctx;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   */
  constructor(ctx, range) {
    this.#ctx = ctx;
    this.range = range;
  }
}

/** @implements {Deno.ConditionalExpression} */
class ConditionalExpression extends BaseNode {
  type = /** @type {const} */ ("ConditionalExpression");
  range;
  get test() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#testId,
    ));
  }
  get consequent() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#consequentId,
    ));
  }
  get alternate() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#alternateId,
    ));
  }

  #ctx;
  #testId;
  #consequentId;
  #alternateId;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} testId
   * @param {number} consequentId
   * @param {number} alternateId
   */
  constructor(ctx, parentId, range, testId, consequentId, alternateId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#testId = testId;
    this.#consequentId = consequentId;
    this.#alternateId = alternateId;
    this.range = range;
  }
}

/** @implements {Deno.FunctionExpression} */
class FunctionExpression {
  type = /** @type {const} */ ("FunctionExpression");
  range;

  #ctx;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   */
  constructor(ctx, range) {
    this.#ctx = ctx;
    this.range = range;
  }
}

/** @implements {Deno.Identifier} */
class Identifier extends BaseNode {
  type = /** @type {const} */ ("Identifier");
  range;
  name = "";

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} nameId
   */
  constructor(ctx, parentId, range, nameId) {
    super(ctx, parentId);

    this.name = getString(ctx, nameId);
    this.range = range;
  }
}

/** @implements {Deno.LogicalExpression} */
class LogicalExpression extends BaseNode {
  type = /** @type {const} */ ("LogicalExpression");
  range;
  get left() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#leftId,
    ));
  }
  get right() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#rightId,
    ));
  }

  #ctx;
  #leftId;
  #rightId;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} flags
   * @param {number} leftId
   * @param {number} rightId
   */
  constructor(ctx, parentId, range, flags, leftId, rightId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.operator = getLogicalOperator(flags);
    this.#leftId = leftId;
    this.#rightId = rightId;
    this.range = range;
  }
}

/**
 * @param {number} n
 * @returns {Deno.LogicalExpression["operator"]}
 */
function getLogicalOperator(n) {
  if ((n & Flags.LogicalAnd) !== 0) {
    return "&&";
  } else if ((n & Flags.LogicalOr) !== 0) {
    return "||";
  } else if ((n & Flags.LogicalNullishCoalescin) !== 0) {
    return "??";
  }

  throw new Error(`Unknown operator: ${n}`);
}

/** @implements {Deno.MemberExpression} */
class MemberExpression extends BaseNode {
  type = /** @type {const} */ ("MemberExpression");
  range;
  get object() {
    return /** @type {*} */ (createAstNode(
      this.#ctx,
      this.#objId,
    ));
  }
  get property() {
    return /** @type {*} */ (createAstNode(
      this.#ctx,
      this.#propId,
    ));
  }
  optional = false; // FIXME
  computed = false; // FIXME

  #ctx;
  #objId;
  #propId;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} flags
   * @param {number} objId
   * @param {number} propId
   */
  constructor(ctx, parentId, range, flags, objId, propId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.computed = (flags & Flags.MemberComputed) !== 0;
    this.#objId = objId;
    this.#propId = propId;
    this.range = range;
  }
}

/** @implements {Deno.MetaProperty} */
class MetaProperty {
  type = /** @type {const} */ ("MetaProperty");
  range;
  get meta() {
    return /** @type {Deno.Identifier} */ (createAstNode(
      this.#ctx,
      this.#metaId,
    ));
  }
  get property() {
    return /** @type {Deno.Identifier} */ (createAstNode(
      this.#ctx,
      this.#propId,
    ));
  }

  #ctx;
  #metaId;
  #propId;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} metaId
   * @param {number} propId
   */
  constructor(ctx, range, metaId, propId) {
    this.#ctx = ctx;
    this.#metaId = metaId;
    this.#propId = propId;
    this.range = range;
  }
}

/** @implements {Deno.NewExpression} */
class NewExpression extends BaseNode {
  type = /** @type {const} */ ("NewExpression");
  range;
  get arguments() {
    return createChildNodes(this.#ctx, this.#childIds);
  }
  get callee() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#calleeId,
    ));
  }

  #ctx;
  #calleeId;
  #childIds;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} calleeId
   * @param {number[]} childIds
   */
  constructor(ctx, parentId, range, calleeId, childIds) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.#calleeId = calleeId;
    this.#childIds = childIds;
    this.range = range;
  }
}

/** @implements {Deno.ObjectExpression} */
class ObjectExpression extends BaseNode {
  type = /** @type {const} */ ("ObjectExpression");
  range;
  get properties() {
    return createChildNodes(this.#ctx, this.#elemIds);
  }

  #ctx;
  #elemIds;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number[]} elemIds
   */
  constructor(ctx, parentId, range, elemIds) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.#elemIds = elemIds;
  }
}

/** @implements {Deno.ParenthesisExpression} */
class ParenthesisExpression extends BaseNode {
  type = /** @type {const} */ ("ParenthesisExpression");
  range;
  #ctx;
  #exprId;

  get expression() {
    return /** @type {*} */ (createAstNode(this.#ctx, this.#exprId));
  }

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} exprId
   */
  constructor(ctx, parentId, range, exprId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.#exprId = exprId;
  }
}

/** @implements {Deno.PrivateIdentifier} */
class PrivateIdentifier extends BaseNode {
  type = /** @type {const} */ ("PrivateIdentifier");
  range;
  #ctx;
  name;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} nameId
   */
  constructor(ctx, parentId, range, nameId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.name = getString(ctx, nameId);
  }
}

/** @implements {Deno.Property} */
class Property extends BaseNode {
  type = /** @type {const} */ ("Property");
  range;
  #ctx;

  get key() {
    return /** @type {*} */ (createAstNode(this.#ctx, this.#keyId));
  }

  get value() {
    return /** @type {*} */ (createAstNode(this.#ctx, this.#valueId));
  }

  #keyId;
  #valueId;

  // FIXME
  computed = false;
  method = false;
  shorthand = false;
  kind = /** @type {const} */ ("get");

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} keyId
   * @param {number} valueId
   */
  constructor(ctx, parentId, range, keyId, valueId) {
    super(ctx, parentId);

    // FIXME flags

    this.#ctx = ctx;
    this.range = range;
    this.#keyId = keyId;
    this.#valueId = valueId;
  }
}

/** @implements {Deno.SequenceExpression} */
class SequenceExpression extends BaseNode {
  type = /** @type {const} */ ("SequenceExpression");
  range;
  #ctx;
  #childIds;

  get expressions() {
    return createChildNodes(this.#ctx, this.#childIds);
  }

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number[]} childIds
   */
  constructor(ctx, parentId, range, childIds) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.#childIds = childIds;
  }
}

/** @implements {Deno.SpreadElement} */
class SpreadElement extends BaseNode {
  type = /** @type {const} */ ("SpreadElement");
  range;
  #ctx;
  #exprId;

  get argument() {
    return /** @type {*} */ (createAstNode(this.#ctx, this.#exprId));
  }

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} exprId
   */
  constructor(ctx, parentId, range, exprId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.#exprId = exprId;
  }
}

/** @implements {Deno.Super} */
class Super extends BaseNode {
  type = /** @type {const} */ ("Super");
  range;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   */
  constructor(ctx, parentId, range) {
    super(ctx, parentId);
    this.range = range;
  }
}

/** @implements {Deno.UnaryExpression} */
class UnaryExpression extends BaseNode {
  type = /** @type {const} */ ("UnaryExpression");
  range;

  get argument() {
    return /** @type {*} */ (createAstNode(this.#ctx, this.#exprId));
  }

  #ctx;
  #exprId;
  operator;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} flags
   * @param {number} exprId
   */
  constructor(ctx, parentId, range, flags, exprId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.operator = getUnaryOperator(flags);
    this.#exprId = exprId;
  }
}

/**
 * @param {number} n
 * @returns {Deno.UnaryExpression["operator"]}
 */
function getUnaryOperator(n) {
  switch (n) {
    case 1:
      return "-";
    case 2:
      return "+";
    case 3:
      return "!";
    case 4:
      return "~";
    case 5:
      return "typeof";
    case 6:
      return "void";
    case 7:
      return "delete";
  }
}

// Literals

/** @implements {Deno.BooleanLiteral} */
class BooleanLiteral extends BaseNode {
  type = /** @type {const} */ ("BooleanLiteral");
  range;
  value = false;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} flags
   */
  constructor(ctx, parentId, range, flags) {
    super(ctx, parentId);
    this.value = flags === 1;
    this.range = range;
  }
}

/** @implements {Deno.BigIntLiteral} */
class BigIntLiteral extends BaseNode {
  type = /** @type {const} */ ("BigIntLiteral");
  range;
  value;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} strId
   */
  constructor(ctx, parentId, range, strId) {
    super(ctx, parentId);
    this.range = range;
    this.value = BigInt(getString(ctx, strId));
  }
}

/** @implements {Deno.NullLiteral} */
class NullLiteral extends BaseNode {
  type = /** @type {const} */ ("NullLiteral");
  range;
  value = null;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   */
  constructor(ctx, parentId, range) {
    super(ctx, parentId);
    this.range = range;
  }
}

/** @implements {Deno.NumericLiteral} */
class NumericLiteral extends BaseNode {
  type = /** @type {const} */ ("NumericLiteral");
  range;
  value = 0;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} strId
   */
  constructor(ctx, parentId, range, strId) {
    super(ctx, parentId);
    this.range = range;
    this.value = Number(getString(ctx, strId));
  }
}

/** @implements {Deno.RegExpLiteral} */
class RegExpLiteral extends BaseNode {
  type = /** @type {const} */ ("RegExpLiteral");
  range;
  pattern = "";
  flags = "";

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} patternId
   * @param {number} flagsId
   */
  constructor(ctx, parentId, range, patternId, flagsId) {
    super(ctx, parentId);

    this.range = range;
    this.pattern = getString(ctx, patternId);
    this.flags = getString(ctx, flagsId);
  }
}

/** @implements {Deno.StringLiteral} */
class StringLiteral extends BaseNode {
  type = /** @type {const} */ ("StringLiteral");
  range;
  value = "";

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} strId
   */
  constructor(ctx, parentId, range, strId) {
    super(ctx, parentId);
    this.range = range;
    this.value = getString(ctx, strId);
  }
}

/** @implements {Deno.TemplateLiteral} */
class TemplateLiteral extends BaseNode {
  type = /** @type {const} */ ("TemplateLiteral");
  range;

  #ctx;
  #exprIds;
  #quasiIds;

  get expressions() {
    return createChildNodes(this.#ctx, this.#exprIds);
  }

  get quasis() {
    return createChildNodes(this.#ctx, this.#quasiIds);
  }

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number[]} quasiIds
   * @param {number[]} exprIds
   */
  constructor(ctx, parentId, range, quasiIds, exprIds) {
    super(ctx, parentId);
    this.#ctx = ctx;
    this.#quasiIds = quasiIds;
    this.#exprIds = exprIds;
    this.range = range;
  }
}

/** @implements {Deno.TemplateElement} */
class TemplateElement extends BaseNode {
  type = /** @type {const} */ ("TemplateElement");
  range;

  tail = false;
  value;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} rawId
   * @param {number} cookedId
   * @param {boolean} tail
   */
  constructor(ctx, parentId, range, rawId, cookedId, tail) {
    super(ctx, parentId);

    const raw = getString(ctx, rawId);
    this.value = {
      raw,
      cooked: cookedId === 0 ? raw : getString(ctx, cookedId),
    };
    this.tail = tail;
    this.range = range;
  }
}

// JSX

/** @implements {Deno.JSXAttribute} */
class JSXAttribute extends BaseNode {
  type = /** @type {const} */ ("JSXAttribute");
  range;
  get name() {
    return /** @type {Deno.JSXIdentifier | Deno.JSXNamespacedName} */ (createAstNode(
      this.#ctx,
      this.#nameId,
    ));
  }
  get value() {
    if (this.#valueId === 0) return null;
    return /** @type {Deno.JSXElement | Deno.JSXExpressionContainer | Deno.JSXSpreadChild | Deno.Literal} */ (createAstNode(
      this.#ctx,
      this.#valueId,
    ));
  }

  #ctx;
  #nameId;
  #valueId;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} nameId
   * @param {number} valueId
   */
  constructor(ctx, parentId, range, nameId, valueId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.#nameId = nameId;
    this.#valueId = valueId;
  }
}

/** @implements {Deno.JSXClosingElement} */
class JSXClosingElement extends BaseNode {
  type = /** @type {const} */ ("JSXClosingElement");
  range;
  get name() {
    return /** @type {Deno.JSXIdentifier | Deno.JSXMemberExpression | Deno.JSXNamespacedName} */ (createAstNode(
      this.#ctx,
      this.#nameId,
    ));
  }

  #ctx;
  #nameId;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} nameId
   */
  constructor(ctx, parentId, range, nameId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.#nameId = nameId;
  }
}

/** @implements {Deno.JSXClosingFragment} */
class JSXClosingFragment extends BaseNode {
  type = /** @type {const} */ ("JSXClosingFragment");
  range;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   */
  constructor(ctx, parentId, range) {
    super(ctx, parentId);

    this.range = range;
  }
}

/** @implements {Deno.JSXElement} */
class JSXElement extends BaseNode {
  type = /** @type {const} */ ("JSXElement");
  range;
  get children() {
    return createChildNodes(this.#ctx, this.#childIds);
  }

  get openingElement() {
    return /** @type {Deno.JSXOpeningElement} */ (createAstNode(
      this.#ctx,
      this.#openId,
    ));
  }
  get closingElement() {
    return /** @type {Deno.JSXClosingElement} */ (createAstNode(
      this.#ctx,
      this.#closingId,
    ));
  }

  #ctx;
  #openId;
  #closingId;
  #childIds;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} openId
   * @param {number} closingId
   * @param {number[]} childIds
   */
  constructor(ctx, parentId, range, openId, closingId, childIds) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.#openId = openId;
    this.#closingId = closingId;
    this.#childIds = childIds;
  }
}

/** @implements {Deno.JSXEmptyExpression} */
class JSXEmptyExpression extends BaseNode {
  type = /** @type {const} */ ("JSXEmptyExpression");
  range;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   */
  constructor(ctx, parentId, range) {
    super(ctx, parentId);
    this.range = range;
  }
}

/** @implements {Deno.JSXExpressionContainer} */
class JSXExpressionContainer extends BaseNode {
  type = /** @type {const} */ ("JSXExpressionContainer");
  range;
  get expression() {
    return /** @type {Deno.Expression | Deno.JSXEmptyExpression} */ (createAstNode(
      this.#ctx,
      this.#exprId,
    ));
  }

  #ctx;
  #exprId;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} exprId
   */
  constructor(ctx, parentId, range, exprId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.#exprId = exprId;
  }
}

/** @implements {Deno.JSXFragment} */
class JSXFragment extends BaseNode {
  type = /** @type {const} */ ("JSXFragment");
  range;
  get children() {
    return createChildNodes(this.#ctx, this.#childIds);
  }

  get openingFragment() {
    return /** @type {*} */ (createAstNode(
      this.#ctx,
      this.#openId,
    ));
  }
  get closingFragment() {
    return /** @type {*} */ (createAstNode(
      this.#ctx,
      this.#closingId,
    ));
  }

  #ctx;
  #childIds;
  #openId;
  #closingId;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} openId
   * @param {number} closingId
   * @param {number[]} childIds
   */
  constructor(ctx, parentId, range, openId, closingId, childIds) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.#openId = openId;
    this.#closingId = closingId;
    this.#childIds = childIds;
  }
}

/** @implements {Deno.JSXIdentifier} */
class JSXIdentifier extends BaseNode {
  type = /** @type {const} */ ("JSXIdentifier");
  range;
  name;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} nameId
   */
  constructor(ctx, parentId, range, nameId) {
    super(ctx, parentId);

    this.range = range;
    this.name = getString(ctx, nameId);
  }
}

/** @implements {Deno.JSXMemberExpression} */
class JSXMemberExpression extends BaseNode {
  type = /** @type {const} */ ("JSXMemberExpression");
  range;
  get object() {
    return /** @type {Deno.JSXMemberExpression["object"]} */ (createAstNode(
      this.#ctx,
      this.#objId,
    ));
  }
  get property() {
    return /** @type {Deno.JSXIdentifier} */ (createAstNode(
      this.#ctx,
      this.#propertyId,
    ));
  }

  #ctx;
  #objId;
  #propertyId;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} objId
   * @param {number} propId
   */
  constructor(ctx, parentId, range, objId, propId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.#objId = objId;
    this.#propertyId = propId;
  }
}

/** @implements {Deno.JSXNamespacedName} */
class JSXNamespacedName {
  type = /** @type {const} */ ("JSXNamespacedName");
  range;
  get name() {
    return /** @type {Deno.JSXIdentifier} */ (createAstNode(
      this.#ctx,
      this.#nameId,
    ));
  }
  get namespace() {
    return /** @type {Deno.JSXIdentifier} */ (createAstNode(
      this.#ctx,
      this.#namespaceId,
    ));
  }

  #ctx;
  #nameId;
  #namespaceId;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} nameId
   * @param {number} nsId
   */
  constructor(ctx, range, nameId, nsId) {
    this.#ctx = ctx;
    this.range = range;
    this.#nameId = nameId;
    this.#namespaceId = nsId;
  }
}

/** @implements {Deno.JSXOpeningElement} */
class JSXOpeningElement extends BaseNode {
  type = /** @type {const} */ ("JSXOpeningElement");
  range;
  get attributes() {
    return createChildNodes(this.#ctx, this.#attrIds);
  }
  get name() {
    return /** @type {Deno.JSXIdentifier} */ (createAstNode(
      this.#ctx,
      this.#nameId,
    ));
  }

  #ctx;
  #nameId;
  #attrIds;
  selfClosing = false;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {boolean} isSelfClosing
   * @param {number} nameId
   * @param {number[]} attrIds
   */
  constructor(ctx, parentId, range, isSelfClosing, nameId, attrIds) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.selfClosing = isSelfClosing;
    this.#nameId = nameId;
    this.#attrIds = attrIds;
    this.range = range;
  }
}

/** @implements {Deno.JSXOpeningFragment} */
class JSXOpeningFragment extends BaseNode {
  type = /** @type {const} */ ("JSXOpeningFragment");
  range;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   */
  constructor(ctx, parentId, range) {
    super(ctx, parentId);

    this.range = range;
  }
}

/** @implements {Deno.JSXSpreadAttribute} */
class JSXSpreadAttribute extends BaseNode {
  type = /** @type {const} */ ("JSXSpreadAttribute");
  range;

  get argument() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#argId,
    ));
  }

  #ctx;
  #argId;

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} argId
   */
  constructor(ctx, parentId, range, argId) {
    super(ctx, parentId);

    this.#ctx = ctx;
    this.range = range;
    this.#argId = argId;
  }
}

/** @implements {Deno.JSXText} */
class JSXText extends BaseNode {
  type = /** @type {const} */ ("JSXText");
  range;

  value = "";
  raw = "";

  /**
   * @param {AstContext} ctx
   * @param {number} parentId
   * @param {Deno.Range} range
   * @param {number} valueId
   * @param {number} rawId
   */
  constructor(ctx, parentId, range, valueId, rawId) {
    super(ctx, parentId);

    this.range = range;
    this.value = getString(ctx, valueId);
    this.raw = getString(ctx, rawId);
  }
}

const DECODER = new TextDecoder();

/**
 * @typedef {{
 *   buf: Uint8Array,
 *   strTable: Map<number, string>,
 *   idTable: number[],
 * }} AstContext
 */

/**
 * @param {Uint8Array} buf
 * @param {number} i
 * @returns {number}
 */
function readU32(buf, i) {
  return (buf[i] << 24) + (buf[i + 1] << 16) + (buf[i + 2] << 8) +
    buf[i + 3];
}

/**
 * @param {Uint8Array} buf
 * @param {number} offset
 * @returns {number[]}
 */
function readChildIds(buf, offset) {
  const count = readU32(buf, offset);
  offset += 4;
  // console.log("read children count:", count, offset);

  /** @type {number[]} */
  const out = new Array(count);

  for (let i = 0; i < count; i++) {
    out[i] = readU32(buf, offset);
    offset += 4;
  }

  // console.log("read children", out);

  return out;
}

/**
 * @param {AstContext} ctx
 * @param {number} id
 * @returns {string}
 */
function getString(ctx, id) {
  const name = ctx.strTable.get(id);
  if (name === undefined) {
    throw new Error(`Missing string id: ${id}`);
  }

  return name;
}

/**
 * @param {AstContext} ctx
 * @param {number} id
 * @returns {Deno.AstNode}
 */
function createAstNode(ctx, id) {
  const { buf, idTable } = ctx;

  let offset = idTable[id];
  if (offset >= buf.length) {
    throw new Error(
      `Could not find id: ${id}. Offset ${offset} bigger than buffer length: ${buf.length}`,
    );
  }

  // console.log({ id, offset });
  /** @type {AstType} */
  const kind = buf[offset];
  console.log("creating node", id, kind);

  const parentId = readU32(buf, offset + 1);
  const rangeStart = readU32(buf, offset + 5);
  const rangeEnd = readU32(buf, offset + 9);
  const range = /** @type {Deno.Range} */ ([rangeStart, rangeEnd]);

  offset += 13;

  switch (kind) {
    case AstType.Program: {
      const moduleType = buf[offset] === 1 ? "module" : "script";
      const childIds = readChildIds(buf, offset + 1);
      return new Program(ctx, parentId, range, moduleType, childIds);
    }
    case AstType.Import:
    case AstType.ImportDecl:
    case AstType.ExportDecl:
    case AstType.ExportNamed:
    case AstType.ExportDefaultDecl:
    case AstType.ExportDefaultExpr:
    case AstType.ExportAll:
      throw new Error("FIXME");

    // Declarations
    case AstType.VariableDeclaration: {
      const flags = buf[offset];
      const childIds = readChildIds(buf, offset + 1);
      return new VariableDeclaration(ctx, parentId, range, flags, childIds);
    }
    case AstType.VariableDeclarator: {
      const nameId = readU32(buf, offset);
      const initId = readU32(buf, offset + 4);
      return new VariableDeclarator(ctx, parentId, range, nameId, initId);
    }

    // Statements
    case AstType.BlockStatement: {
      const childIds = readChildIds(buf, offset);
      return new BlockStatement(ctx, parentId, range, childIds);
    }
    case AstType.BreakStatement: {
      const labelId = readU32(buf, offset);
      return new BreakStatement(ctx, parentId, range, labelId);
    }
    case AstType.ContinueStatement: {
      const labelId = readU32(buf, offset);
      return new ContinueStatement(ctx, parentId, range, labelId);
    }
    case AstType.DebuggerStatement:
      return new DebuggerStatement(ctx, parentId, range);
    case AstType.DoWhileStatement: {
      const exprId = readU32(buf, offset);
      const bodyId = readU32(buf, offset + 4);
      return new DoWhileStatement(ctx, parentId, range, exprId, bodyId);
    }
    case AstType.ExpressionStatement: {
      const exprId = readU32(buf, offset);
      return new ExpressionStatement(ctx, parentId, range, exprId);
    }
    case AstType.ForInStatement: {
      const leftId = readU32(buf, offset);
      const rightId = readU32(buf, offset + 4);
      const bodyId = readU32(buf, offset + 8);
      return new ForInStatement(ctx, parentId, range, leftId, rightId, bodyId);
    }
    case AstType.ForOfStatement: {
      const flags = buf[offset];
      const isAwait = (flags & Flags.ForAwait) !== 0;
      const leftId = readU32(buf, offset + 1);
      const rightId = readU32(buf, offset + 5);
      const bodyId = readU32(buf, offset + 9);
      return new ForOfStatement(
        ctx,
        parentId,
        range,
        isAwait,
        leftId,
        rightId,
        bodyId,
      );
    }
    case AstType.ForStatement: {
      const initId = readU32(buf, offset);
      const testId = readU32(buf, offset + 4);
      const updateId = readU32(buf, offset + 8);
      const bodyId = readU32(buf, offset + 12);
      return new ForStatement(
        ctx,
        parentId,
        range,
        initId,
        testId,
        updateId,
        bodyId,
      );
    }
    case AstType.IfStatement: {
      const testId = readU32(buf, offset);
      const consequentId = readU32(buf, offset + 4);
      const alternateId = readU32(buf, offset + 8);
      return new IfStatement(
        ctx,
        parentId,
        range,
        testId,
        consequentId,
        alternateId,
      );
    }
    case AstType.LabeledStatement: {
      const labelId = readU32(buf, offset);
      const stmtId = readU32(buf, offset + 4);
      return new LabeledStatement(ctx, parentId, range, labelId, stmtId);
    }
    case AstType.ReturnStatement: {
      const argId = readU32(buf, offset);
      return new ReturnStatement(ctx, parentId, range, argId);
    }
    case AstType.SwitchStatement:
      throw new SwitchStatement(ctx, parentId, range, 0); // FIXME
    case AstType.ThrowStatement: {
      const argId = readU32(buf, offset);
      return new ThrowStatement(ctx, parentId, range, argId);
    }
    case AstType.TryStatement: {
      const blockId = readU32(buf, offset);
      const catchId = readU32(buf, offset + 4);
      const finalId = readU32(buf, offset + 8);
      return new TryStatement(ctx, parentId, range, blockId, catchId, finalId);
    }
    case AstType.WhileStatement: {
      const testId = readU32(buf, offset);
      const stmtId = readU32(buf, offset + 4);
      return new WhileStatement(ctx, parentId, range, testId, stmtId);
    }
    case AstType.WithStatement:
      return new WithStatement(ctx, range, 0, 0);

    // Expressions
    case AstType.ArrayExpression: {
      const elemIds = readChildIds(buf, offset);
      return new ArrayExpression(ctx, parentId, range, elemIds);
    }
    case AstType.ArrowFunctionExpression: {
      const flags = buf[offset];
      offset += 1;

      const isAsync = (flags & Flags.FnAsync) !== 0;
      const isGenerator = (flags & Flags.FnGenerator) !== 0;

      const typeParamId = readU32(buf, offset);
      offset += 4;
      const paramIds = readChildIds(buf, offset);
      offset += 4;
      offset += paramIds.length * 4;

      const bodyId = readU32(buf, offset);
      offset += 4;

      const returnTypeId = readU32(buf, offset);
      offset += 4;

      return new ArrowFunctionExpression(
        ctx,
        parentId,
        range,
        isAsync,
        isGenerator,
        typeParamId,
        paramIds,
        bodyId,
        returnTypeId,
      );
    }
    case AstType.AssignmentExpression: {
      const flags = buf[offset];
      const leftId = readU32(buf, offset + 1);
      const rightId = readU32(buf, offset + 5);
      return new AssignmentExpression(
        ctx,
        parentId,
        range,
        flags,
        leftId,
        rightId,
      );
    }
    case AstType.AwaitExpression: {
      const argId = readU32(buf, offset);
      return new AwaitExpression(ctx, parentId, range, argId);
    }
    case AstType.BinaryExpression: {
      const flags = buf[offset];
      const leftId = readU32(buf, offset + 1);
      const rightId = readU32(buf, offset + 5);
      return new BinaryExpression(ctx, parentId, range, flags, leftId, rightId);
    }
    case AstType.CallExpression: {
      const calleeId = readU32(buf, offset);
      const typeArgId = readU32(buf, offset + 4);
      const childIds = readChildIds(buf, offset + 8);
      return new CallExpression(
        ctx,
        parentId,
        range,
        calleeId,
        typeArgId,
        childIds,
      );
    }
    case AstType.ConditionalExpression: {
      const testId = readU32(buf, offset);
      const consId = readU32(buf, offset + 4);
      const altId = readU32(buf, offset + 8);
      return new ConditionalExpression(
        ctx,
        parentId,
        range,
        testId,
        consId,
        altId,
      );
    }
    case AstType.FunctionExpression:
      throw new FunctionExpression(ctx, range); // FIXME
    case AstType.Identifier: {
      const strId = readU32(buf, offset);
      return new Identifier(ctx, parentId, range, strId);
    }
    case AstType.LogicalExpression: {
      const flags = buf[offset];
      const leftId = readU32(buf, offset + 1);
      const rightId = readU32(buf, offset + 5);
      return new LogicalExpression(
        ctx,
        parentId,
        range,
        flags,
        leftId,
        rightId,
      );
    }
    case AstType.MemberExpression: {
      const flags = buf[offset];
      offset += 1;
      const objId = readU32(buf, offset);
      offset += 4;
      const propId = readU32(buf, offset);
      return new MemberExpression(ctx, parentId, range, flags, objId, propId);
    }
    case AstType.MetaProperty:
      throw new MetaProperty(ctx, parentId, range, 0, 0); // FIXME
    case AstType.NewExpression: {
      const calleeId = readU32(buf, offset);
      const typeId = readU32(buf, offset + 4); // FIXME
      const childIds = readChildIds(buf, offset + 8);
      throw new NewExpression(ctx, parentId, range, calleeId, childIds);
    }
    case AstType.ObjectExpression: {
      const elemIds = readChildIds(buf, offset);
      return new ObjectExpression(ctx, parentId, range, elemIds);
    }
    case AstType.ParenthesisExpression: {
      const exprId = readU32(buf, offset);
      return new ParenthesisExpression(ctx, parentId, range, exprId);
    }
    case AstType.PrivateIdentifier: {
      const strId = readU32(buf, offset);
      return new PrivateIdentifier(ctx, parentId, range, strId);
    }
    case AstType.Property: {
      const flags = buf[offset]; // FIXME
      const keyId = readU32(buf, offset + 1);
      const valueId = readU32(buf, offset + 5);
      return new Property(ctx, parentId, range, keyId, valueId);
    }
    case AstType.SequenceExpression: {
      const childIds = readChildIds(buf, offset);
      return new SequenceExpression(ctx, parentId, range, childIds);
    }
    case AstType.SpreadElement: {
      const exprId = readU32(buf, offset);
      return new SpreadElement(ctx, parentId, range, exprId);
    }
    case AstType.Super:
      throw new Super(ctx, parentId, range);
    case AstType.TaggedTemplateExpression:
      throw new Error("FIXME");

    case AstType.TemplateElement: {
      const flags = buf[offset];
      const tail = (flags & Flags.TplTail) !== 0;
      const rawId = readU32(buf, offset + 1);
      const cookedId = readU32(buf, offset + 5);
      return new TemplateElement(ctx, parentId, range, rawId, cookedId, tail);
    }
    case AstType.TemplateLiteral: {
      const quasiIds = readChildIds(buf, offset);
      offset += 4;
      offset += quasiIds.length * 4;
      const exprIds = readChildIds(buf, offset);
      return new TemplateLiteral(ctx, parentId, range, quasiIds, exprIds);
    }
    case AstType.UnaryExpression: {
      const flags = buf[offset];
      const exprId = readU32(buf, offset + 1);
      return new UnaryExpression(ctx, parentId, range, flags, exprId);
    }

    // Literals
    case AstType.BooleanLiteral: {
      const flags = buf[offset];
      return new BooleanLiteral(ctx, parentId, range, flags);
    }
    case AstType.BigIntLiteral: {
      const strId = readU32(buf, offset);
      return new BigIntLiteral(ctx, parentId, range, strId);
    }
    case AstType.NullLiteral:
      return new NullLiteral(ctx, parentId, range);
    case AstType.NumericLiteral: {
      const strId = readU32(buf, offset);
      return new NumericLiteral(ctx, parentId, range, strId);
    }
    case AstType.RegExpLiteral: {
      const patternId = readU32(buf, offset);
      const flagsId = readU32(buf, offset + 4);
      return new RegExpLiteral(ctx, parentId, range, patternId, flagsId);
    }
    case AstType.StringLiteral: {
      const strId = readU32(buf, offset);
      return new StringLiteral(ctx, parentId, range, strId);
    }

    // JSX
    case AstType.JSXAttribute: {
      const nameId = readU32(buf, offset);
      const valueId = readU32(buf, offset + 4);
      return new JSXAttribute(ctx, parentId, range, nameId, valueId);
    }
    case AstType.JSXClosingElement: {
      const nameId = readU32(buf, offset);
      return new JSXClosingElement(ctx, parentId, range, nameId);
    }
    case AstType.JSXClosingFragment:
      return new JSXClosingFragment(ctx, parentId, range);
    case AstType.JSXElement: {
      const openingId = readU32(buf, offset);
      const closingId = readU32(buf, offset + 4);
      const childIds = readChildIds(buf, offset + 8);
      return new JSXElement(
        ctx,
        parentId,
        range,
        openingId,
        closingId,
        childIds,
      );
    }
    case AstType.JSXEmptyExpression: {
      return new JSXEmptyExpression(ctx, parentId, range);
    }
    case AstType.JSXExpressionContainer: {
      const exprId = readU32(buf, offset);
      return new JSXExpressionContainer(ctx, parentId, range, exprId);
    }
    case AstType.JSXFragment: {
      const openingId = readU32(buf, offset);
      const closingId = readU32(buf, offset + 4);
      const childIds = readChildIds(buf, offset + 8);
      return new JSXFragment(
        ctx,
        parentId,
        range,
        openingId,
        closingId,
        childIds,
      );
    }
    case AstType.JSXIdentifier: {
      const strId = readU32(buf, offset);
      return new JSXIdentifier(ctx, parentId, range, strId);
    }
    case AstType.JSXMemberExpression: {
      const objId = readU32(buf, offset);
      const propId = readU32(buf, offset + 4);
      return new JSXMemberExpression(ctx, parentId, range, objId, propId);
    }
    case AstType.JSXNamespacedName:
      throw new JSXNamespacedName(ctx, range, 0, 0); // FIXME
    case AstType.JSXOpeningElement: {
      const flags = buf[offset];
      const nameId = readU32(buf, offset + 1);
      const attrIds = readChildIds(buf, offset + 5);

      const isSelfClosing = (flags & Flags.JSXSelfClosing) !== 0;

      return new JSXOpeningElement(
        ctx,
        parentId,
        range,
        isSelfClosing,
        nameId,
        attrIds,
      );
    }
    case AstType.JSXOpeningFragment:
      return new JSXOpeningFragment(ctx, parentId, range);
    case AstType.JSXSpreadAttribute: {
      const childId = readU32(buf, offset);
      return new JSXSpreadAttribute(ctx, parentId, range, childId);
    }
    case AstType.JSXSpreadChild:
      throw new Error(`JSXSpreadChild not supported`);
    case AstType.JSXText: {
      const rawId = readU32(buf, offset);
      const valueId = readU32(buf, offset + 4);
      return new JSXText(ctx, parentId, range, rawId, valueId);
    }

    default:
      throw new Error(`Unknown ast node ${kind}`);
  }
}

/**
 * @param {Uint8Array} buf
 * @param {AstContext} buf
 */
function createAstContext(buf) {
  // console.log(buf);

  // Extract string table
  /** @type {Map<number, string>} */
  const strTable = new Map();

  let offset = 0;
  const stringCount = readU32(buf, 0);
  offset += 4;

  let id = 0;
  for (let i = 0; i < stringCount; i++) {
    const len = readU32(buf, offset);
    offset += 4;

    const strBytes = buf.slice(offset, offset + len);
    offset += len;
    const s = DECODER.decode(strBytes);
    strTable.set(id, s);
    id++;
  }

  // console.log({ stringCount, strTable });

  if (strTable.size !== stringCount) {
    throw new Error(
      `Could not deserialize string table. Expected ${stringCount} items, but got ${strTable.size}`,
    );
  }

  // Build id table
  const idCount = readU32(buf, offset);
  offset += 4;

  const idTable = new Array(idCount);

  for (let i = 0; i < idCount; i++) {
    const id = readU32(buf, offset);
    idTable[i] = id;
    offset += 4;
  }

  // console.log({ idCount, idTable });
  if (idTable.length !== idCount) {
    throw new Error(
      `Could not deserialize id table. Expected ${idCount} items, but got ${idTable.length}`,
    );
  }

  /** @type {AstContext} */
  const ctx = { buf, idTable, strTable };

  return ctx;
}

/**
 * @param {string} fileName
 * @param {Uint8Array} serializedAst
 */
export function runPluginsForFile(fileName, serializedAst) {
  const ctx = createAstContext(serializedAst);
  // console.log(JSON.stringify(ctx, null, 2));

  /** @type {Record<string, (node: any) => void>} */
  const mergedVisitor = {};
  const destroyFns = [];

  // console.log(state);

  // Instantiate and merge visitors. This allows us to only traverse
  // the AST once instead of per plugin.
  for (let i = 0; i < state.plugins.length; i++) {
    const plugin = state.plugins[i];

    for (const name of Object.keys(plugin.rules)) {
      const rule = plugin.rules[name];
      const id = `${plugin.name}/${name}`;
      const ctx = new Context(id, fileName);
      const visitor = rule.create(ctx);

      // console.log({ visitor });

      for (const name in visitor) {
        const prev = mergedVisitor[name];
        mergedVisitor[name] = (node) => {
          if (typeof prev === "function") {
            prev(node);
          }

          try {
            visitor[name](node);
          } catch (err) {
            // FIXME: console here doesn't support error cause
            console.log(err);
            throw new Error(`Visitor "${name}" of plugin "${id}" errored`, {
              cause: err,
            });
          }
        };
      }

      if (typeof rule.destroy === "function") {
        const destroyFn = rule.destroy.bind(rule);
        destroyFns.push(() => {
          try {
            destroyFn(ctx);
          } catch (err) {
            throw new Error(`Destroy hook of "${id}" errored`, { cause: err });
          }
        });
      }
    }
  }

  // Traverse ast with all visitors at the same time to avoid traversing
  // multiple times.
  try {
    traverse(ctx, mergedVisitor);
  } finally {
    // Optional: Destroy rules
    for (let i = 0; i < destroyFns.length; i++) {
      destroyFns[i]();
    }
  }
}

/**
 * @param {AstContext} ctx
 * @param {*} visitor
 * @returns {void}
 */
function traverse(ctx, visitor) {
  const visitTypes = new Map();

  // TODO: create visiting types
  for (const name in visitor) {
    const id = AstType[name];
    visitTypes.set(id, name);
  }

  console.log("buffer len", ctx.buf.length, ctx.buf.byteLength);
  console.log("merged visitor", visitor);
  console.log("visiting types", visitTypes);

  // Program is always id 1
  const id = 1;
  traverseInner(ctx, visitTypes, visitor, id);
}

/**
 * @param {AstContext} ctx
 * @param {Map<number, string>} visitTypes
 * @param {Record<string, (x: any) => void>} visitor
 * @param {number} id
 */
function traverseInner(ctx, visitTypes, visitor, id) {
  console.log("traversing id", id);

  // Empty id
  if (id === 0) return;
  const { idTable, buf } = ctx;
  if (id >= idTable.length) {
    throw new Error(`Invalid node  id: ${id}`);
  }

  let offset = idTable[id];
  if (offset === undefined) throw new Error(`Unknown id: ${id}`);

  const type = buf[offset];
  console.log({ id, type, offset });

  const name = visitTypes.get(type);
  if (name !== undefined) {
    const node = createAstNode(ctx, id);
    visitor[name](node);
  }

  // type + parentId + SpanLo + SpanHi
  offset += 1 + 4 + 4 + 4;

  // Children
  switch (type) {
    case AstType.Program: {
      // skip flag reading during traversal
      offset += 1;
      const childIds = readChildIds(buf, offset);
      return traverseChildren(ctx, visitTypes, visitor, childIds);
    }
    case AstType.VariableDeclaration: {
      // Skip flags
      offset += 1;

      const childIds = readChildIds(buf, offset);
      traverseChildren(ctx, visitTypes, visitor, childIds);
      return;
    }

    // Multiple children only
    case AstType.ArrayExpression:
    case AstType.BlockStatement:
    case AstType.ObjectExpression:
    case AstType.SequenceExpression: {
      const stmtsIds = readChildIds(buf, offset);
      return traverseChildren(ctx, visitTypes, visitor, stmtsIds);
    }

    // Expressions
    case AstType.CallExpression: {
      const calleeId = readU32(buf, offset);
      traverseInner(ctx, visitTypes, visitor, calleeId);

      const typeArgId = readU32(buf, offset + 4);
      if (typeArgId > 0) {
        traverseInner(ctx, visitTypes, visitor, typeArgId);
      }

      const childIds = readChildIds(buf, offset + 8);
      return traverseChildren(ctx, visitTypes, visitor, childIds);
    }
    case AstType.ArrowFunctionExpression: {
      // Skip flags
      offset += 1;

      const typeParamId = readU32(buf, offset);
      offset += 4;
      if (typeParamId > 0) {
        traverseInner(ctx, visitTypes, visitor, typeParamId);
      }

      const childIds = readChildIds(buf, offset);
      offset += 4;
      offset += childIds.length * 4;
      traverseChildren(ctx, visitTypes, visitor, childIds);

      const bodyId = readU32(buf, offset);
      offset += 4;
      traverseInner(ctx, visitTypes, visitor, bodyId);

      const returnTypeId = readU32(buf, offset);
      traverseInner(ctx, visitTypes, visitor, returnTypeId);

      return;
    }
    case AstType.MemberExpression: {
      // Skip flags
      offset += 1;

      const objId = readU32(buf, offset);
      const propId = readU32(buf, offset + 4);

      traverseInner(ctx, visitTypes, visitor, objId);
      traverseInner(ctx, visitTypes, visitor, propId);

      return;
    }
    case AstType.AssignmentExpression: {
      // Skip flags
      offset += 1;

      const leftId = readU32(buf, offset);
      const rightId = readU32(buf, offset + 4);

      traverseInner(ctx, visitTypes, visitor, leftId);
      traverseInner(ctx, visitTypes, visitor, rightId);

      return;
    }
    case AstType.BinaryExpression:
    case AstType.LogicalExpression: {
      // Skip flags
      offset += 1;

      const leftId = readU32(buf, offset);
      const rightId = readU32(buf, offset + 4);

      traverseInner(ctx, visitTypes, visitor, leftId);
      traverseInner(ctx, visitTypes, visitor, rightId);

      return;
    }
    case AstType.Property: {
      // Skip flags
      offset += 1;

      const keyId = readU32(buf, offset);
      const valueId = readU32(buf, offset + 4);

      traverseInner(ctx, visitTypes, visitor, keyId);
      traverseInner(ctx, visitTypes, visitor, valueId);

      return;
    }

    case AstType.JSXElement:
    case AstType.JSXFragment: {
      const openingId = readU32(buf, offset);
      const closingId = readU32(buf, offset + 4);
      const childIds = readChildIds(buf, offset + 8);

      traverseInner(ctx, visitTypes, visitor, openingId);
      traverseInner(ctx, visitTypes, visitor, closingId);
      traverseChildren(ctx, visitTypes, visitor, childIds);

      return;
    }
    case AstType.JSXOpeningElement: {
      // Skip flags
      offset += 1;

      const nameId = readU32(buf, offset);
      const childIds = readChildIds(buf, offset + 4);

      traverseInner(ctx, visitTypes, visitor, nameId);
      traverseChildren(ctx, visitTypes, visitor, childIds);

      return;
    }
    case AstType.UnaryExpression: {
      // Skip flags
      offset += 1;

      const exprId = readU32(buf, offset);
      traverseInner(ctx, visitTypes, visitor, exprId);
      return;
    }
    case AstType.ForOfStatement: {
      // Skip flags
      offset += 1;

      const firstId = readU32(buf, offset);
      const secondId = readU32(buf, offset + 4);
      const thirdId = readU32(buf, offset + 8);

      traverseInner(ctx, visitTypes, visitor, firstId);
      traverseInner(ctx, visitTypes, visitor, secondId);
      traverseInner(ctx, visitTypes, visitor, thirdId);
      return;
    }
    case AstType.NewExpression: {
      const calleeId = readU32(buf, offset);
      const typeId = readU32(buf, offset + 4);
      const childIds = readChildIds(buf, offset + 8);

      traverseInner(ctx, visitTypes, visitor, calleeId);
      traverseInner(ctx, visitTypes, visitor, typeId);
      traverseChildren(ctx, visitTypes, visitor, childIds);

      return;
    }

    // Three children
    case AstType.ForInStatement:
    case AstType.IfStatement: {
      const firstId = readU32(buf, offset);
      const secondId = readU32(buf, offset + 4);
      const thirdId = readU32(buf, offset + 8);

      traverseInner(ctx, visitTypes, visitor, firstId);
      traverseInner(ctx, visitTypes, visitor, secondId);
      traverseInner(ctx, visitTypes, visitor, thirdId);
      return;
    }

    // Two children
    case AstType.AssignmentPattern:
    case AstType.JSXAttribute:
    case AstType.JSXMemberExpression:
    case AstType.LabeledStatement:
    case AstType.VariableDeclarator:
    case AstType.WhileStatement: {
      const firstId = readU32(buf, offset);
      const secondId = readU32(buf, offset + 4);

      traverseInner(ctx, visitTypes, visitor, firstId);
      traverseInner(ctx, visitTypes, visitor, secondId);
      return;
    }

    // Single child
    case AstType.AwaitExpression:
    case AstType.BreakStatement:
    case AstType.ContinueStatement:
    case AstType.ExpressionStatement:
    case AstType.JSXClosingElement:
    case AstType.JSXExpressionContainer:
    case AstType.JSXIdentifier:
    case AstType.JSXSpreadAttribute:
    case AstType.ReturnStatement:
    case AstType.SpreadElement:
    case AstType.ParenthesisExpression: {
      const childId = readU32(buf, offset);
      return traverseInner(ctx, visitTypes, visitor, childId);
    }

    // These have no children
    case AstType.BooleanLiteral:
    case AstType.BigIntLiteral:
    case AstType.DebuggerStatement:
    case AstType.Identifier:
    case AstType.JSXClosingFragment:
    case AstType.JSXEmptyExpression:
    case AstType.JSXOpeningFragment:
    case AstType.JSXText:
    case AstType.NullLiteral:
    case AstType.NumericLiteral:
    case AstType.PrivateIdentifier:
    case AstType.RegExpLiteral:
    case AstType.StringLiteral:
    case AstType.TemplateLiteral:
    case AstType.This:
      return;
    default:
      throw new Error(`Unknown ast type: ${type}`);
  }
}

/**
 * @param {AstContext} ctx
 * @param {Map<number, string>} visitTypes
 * @param {Record<string, *>} visitor
 * @param {number[]} ids
 */
function traverseChildren(ctx, visitTypes, visitor, ids) {
  for (let i = 0; i < ids.length; i++) {
    const id = ids[i];
    traverseInner(ctx, visitTypes, visitor, id);
  }
}
