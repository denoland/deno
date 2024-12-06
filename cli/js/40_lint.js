// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check

import exp from "constants";
import { core } from "ext:core/mod.js";
import { deno } from "../tsc/dts/typescript.d.ts";
const {
  op_lint_get_rule,
  op_lint_get_source,
  op_lint_report,
} = core.ops;

/** @typedef {{ plugins: Array<{ name: string, rules: Record<string, Deno.LintRule}> }} LintState */

/** @type {LintState} */
const state = {
  plugins: [],
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

export function installPlugin(plugin) {
  console.log("plugin", plugin);
  if (typeof plugin !== "object") {
    throw new Error("Linter plugin must be an object");
  }
  if (typeof plugin.name !== "string") {
    throw new Error("Linter plugin name must be a string");
  }
  if (typeof plugin.rules !== "object") {
    throw new Error("Linter plugin rules must be an object");
  }
  if (typeof state.plugins[plugin.name] !== "undefined") {
    throw new Error(`Linter plugin ${plugin.name} has already been registered`);
  }
  state.plugins[plugin.name] = plugin.rules;
  console.log("Installed plugin", plugin.name, plugin.rules);
}

// Keep in sync with Rust
/**
 * @enum {number}
 */
const AstNodeId = {
  Invalid: 0,
  Program: 1,

  Import: 2,

  // Decls
  Class: 12,
  Fn: 13,
  Var: 14,

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
  Unary: 44,
  Update: 45,
  BinaryExpression: 46,
  AssignmentExpression: 47,
  MemberExpression: 48,
  Super: 49,
  ConditionalExpression: 50,
  CallExpression: 51,
  NewExpression: 52,
  SequenceExpression: 53,
  Identifier: 54,
  TemplateLiteral: 55,
  TaggedTemplateExpression: 56,
  ArrowFunctionExpression: 57,
  Yield: 59,
  MetaProperty: 60,
  AwaitExpression: 61,

  StringLiteral: 70,
  BooleanLiteral: 71,
  NullLiteral: 72,
  NumericLiteral: 73,
  BigInt: 74,
  RegExpLiteral: 75,

  // Custom
  EmptyExpr: 82,
  Spread: 83,
  ObjProperty: 84,
  VarDeclarator: 85,
  CatchClause: 86,

  // JSX
  // FIXME
  JSXAttribute: Infinity,
  JSXClosingElement: Infinity,
  JSXClosingFragment: Infinity,
  JSXElement: Infinity,
  JSXExpressionContainer: Infinity,
  JSXFragment: Infinity,
  JSXIdentifier: Infinity,
  JSXMemberExpression: Infinity,
  JSXNamespacedName: Infinity,
  JSXOpeningElement: Infinity,
  JSXOpeningFragment: Infinity,
  JSXSpreadAttribute: Infinity,
  JSXSpreadChild: Infinity,
  JSXText: Infinity,
};

const _ID = Symbol.for("__astId");

/** @implements {Deno.Program} */
class Program {
  type = /** @type {const} */ ("Program");
  range;
  get body() {
    return [];
  }
  #ctx;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {Deno.Program["sourceType"]} sourceType
   */
  constructor(ctx, range, sourceType) {
    this.#ctx = ctx;
    this.range = range;
    this.sourceType = sourceType;
  }
}

/** @implements {Deno.BlockStatement} */
class BlockStatement {
  type = /** @type {const} */ ("BlockStatement");
  body = [];
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

/** @implements {Deno.BreakStatement} */
class BreakStatement {
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
   * @param {Deno.Range} range
   * @param {number} labelId
   */
  constructor(ctx, range, labelId) {
    this.#ctx = ctx;
    this.#labelId = labelId;
    this.range = range;
  }
}

/** @implements {Deno.ContinueStatement} */
class ContinueStatement {
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
   * @param {Deno.Range} range
   * @param {number} labelId
   */
  constructor(ctx, range, labelId) {
    this.#ctx = ctx;
    this.#labelId = labelId;
    this.range = range;
  }
}

/** @implements {Deno.DebuggerStatement} */
class DebuggerStatement {
  type = /** @type {const} */ ("DebuggerStatement");
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

/** @implements {Deno.DoWhileStatement} */
class DoWhileStatement {
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
   * @param {Deno.Range} range
   * @param {number} exprId
   * @param {number} bodyId
   */
  constructor(ctx, range, exprId, bodyId) {
    this.#ctx = ctx;
    this.#exprId = exprId;
    this.#bodyId = bodyId;
    this.range = range;
  }
}

/** @implements {Deno.ExpressionStatement} */
class ExpressionStatement {
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
   * @param {Deno.Range} range
   * @param {number} exprId
   */
  constructor(ctx, range, exprId) {
    this.#ctx = ctx;
    this.#exprId = exprId;
    this.range = range;
  }
}

/** @implements {Deno.ForInStatement} */
class ForInStatement {
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
   * @param {Deno.Range} range
   * @param {number} leftId
   * @param {number} rightId
   * @param {number} bodyId
   */
  constructor(ctx, range, leftId, rightId, bodyId) {
    this.#ctx = ctx;
    this.#leftId = leftId;
    this.#rightId = rightId;
    this.#bodyId = bodyId;
    this.range = range;
  }
}

/** @implements {Deno.ForOfStatement} */
class ForOfStatement {
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
   * @param {Deno.Range} range
   * @param {boolean} isAwait
   * @param {number} leftId
   * @param {number} rightId
   * @param {number} bodyId
   */
  constructor(ctx, range, isAwait, leftId, rightId, bodyId) {
    this.#ctx = ctx;
    this.#leftId = leftId;
    this.#rightId = rightId;
    this.#bodyId = bodyId;
    this.range = range;
    this.await = isAwait;
  }
}

/** @implements {Deno.ForStatement} */
class ForStatement {
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
   * @param {Deno.Range} range
   * @param {number} initId
   * @param {number} testId
   * @param {number} updateId
   * @param {number} bodyId
   */
  constructor(ctx, range, initId, testId, updateId, bodyId) {
    this.#ctx = ctx;
    this.#initId = initId;
    this.#testId = testId;
    this.#updateId = updateId;
    this.#bodyId = bodyId;
    this.range = range;
  }
}

/** @implements {Deno.IfStatement} */
class IfStatement {
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
   * @param {Deno.Range} range
   * @param {number} testId
   * @param {number} updateId
   * @param {number} alternateId
   */
  constructor(ctx, range, testId, updateId, alternateId) {
    this.#ctx = ctx;
    this.#testId = testId;
    this.#consequentId = updateId;
    this.#alternateId = alternateId;
    this.range = range;
  }
}

/** @implements {Deno.LabeledStatement} */
class LabeledStatement {
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
   * @param {Deno.Range} range
   * @param {number} testId
   * @param {number} bodyId
   */
  constructor(ctx, range, testId, bodyId) {
    this.#ctx = ctx;
    this.#labelId = testId;
    this.#bodyId = bodyId;
    this.range = range;
  }
}

/** @implements {Deno.ReturnStatement} */
class ReturnStatement {
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
   * @param {Deno.Range} range
   * @param {number} argId
   */
  constructor(ctx, range, argId) {
    this.#ctx = ctx;
    this.#exprId = argId;
    this.range = range;
  }
}

/** @implements {Deno.SwitchStatement} */
class SwitchStatement {
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
   * @param {Deno.Range} range
   * @param {number} discriminantId
   */
  constructor(ctx, range, discriminantId) {
    this.#ctx = ctx;
    this.#discriminantId = discriminantId;
    this.range = range;
  }
}

/** @implements {Deno.ThrowStatement} */
class ThrowStatement {
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
   * @param {Deno.Range} range
   * @param {number} argId
   */
  constructor(ctx, range, argId) {
    this.#ctx = ctx;
    this.#argId = argId;
    this.range = range;
  }
}

/** @implements {Deno.TryStatement} */
class TryStatement {
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
   * @param {Deno.Range} range
   * @param {number} blockId
   * @param {number} finalizerId
   * @param {number} handlerId
   */
  constructor(ctx, range, blockId, finalizerId, handlerId) {
    this.#ctx = ctx;
    this.#blockId = blockId;
    this.#finalizerId = finalizerId;
    this.#handlerId = handlerId;
    this.range = range;
  }
}

/** @implements {Deno.WhileStatement} */
class WhileStatement {
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
   * @param {Deno.Range} range
   * @param {number} testId
   * @param {number} bodyId
   */
  constructor(ctx, range, testId, bodyId) {
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
class ArrayExpression {
  type = /** @type {const} */ ("ArrayExpression");
  range;
  get elements() {
    return []; // FIXME
  }

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

/** @implements {Deno.ArrowFunctionExpression} */
class ArrowFunctionExpression {
  type = /** @type {const} */ ("ArrowFunctionExpression");
  range;
  async = false;
  generator = false;

  get body() {
    return /** @type {Deno.BlockStatement| Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#bodyId,
    ));
  }

  get params() {
    return []; // FIXME
  }

  get returnType() {
    return null; // FIXME
  }

  get typeParameters() {
    return null; // FIXME
  }

  #ctx;
  #bodyId;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {boolean} isAsync
   * @param {boolean} isGenerator
   * @param {number} bodyId
   */
  constructor(ctx, range, isAsync, isGenerator, bodyId) {
    this.#ctx = ctx;
    this.#bodyId = bodyId;
    this.asnyc = isAsync;
    this.generator = isGenerator;
    this.range = range;
  }
}

/** @implements {Deno.AssignmentExpression} */
class AssignmentExpression {
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
   * @param {Deno.Range} range
   * @param {number} flags
   * @param {number} leftId
   * @param {number} rightId
   */
  constructor(ctx, range, flags, leftId, rightId) {
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
      return "&&=";
    case 1:
      return "&=";
    case 2:
      return "**=";
    case 3:
      return "*=";
    case 4:
      return "||=";
    case 5:
      return "|=";
    case 6:
      return "^=";
    case 7:
      return "=";
    case 8:
      return ">>=";
    case 9:
      return ">>>=";
    case 10:
      return "<<=";
    case 11:
      return "-=";
    case 12:
      return "%=";
    case 13:
      return "+=";
    case 14:
      return "??=";
    case 15:
      return "/=";
    default:
      throw new Error(`Unknown operator: ${n}`);
  }
}

/** @implements {Deno.AwaitExpression} */
class AwaitExpression {
  type = /** @type {const} */ ("AwaitExpression");
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
   * @param {Deno.Range} range
   * @param {number} argId
   */
  constructor(ctx, range, argId) {
    this.#ctx = ctx;
    this.#argId = argId;
    this.range = range;
  }
}

/** @implements {Deno.BinaryExpression} */
class BinaryExpression {
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
   * @param {Deno.Range} range
   * @param {number} flags
   * @param {number} leftId
   * @param {number} rightId
   */
  constructor(ctx, range, flags, leftId, rightId) {
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
    case 0:
      return "&";
    case 1:
      return "**";
    case 2:
      return "*";
    case 3:
      return "|";
    case 4:
      return "^";
    case 5:
      return "===";
    case 6:
      return "==";
    case 7:
      return "!==";
    case 8:
      return "!=";
    case 9:
      return ">=";
    case 10:
      return ">>>";
    case 11:
      return ">>";
    case 12:
      return ">";
    case 13:
      return "in";
    case 14:
      return "instanceof";
    case 15:
      return "<=";
    case 16:
      return "<<";
    case 17:
      return "<";
    case 18:
      return "-";
    case 19:
      return "%";
    case 20:
      return "+";
    case 21:
      return "/";
    default:
      throw new Error(`Unknown operator: ${n}`);
  }
}

/** @implements {Deno.CallExpression} */
class CallExpression {
  type = /** @type {const} */ ("CallExpression");
  range;
  get callee() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#calleeId,
    ));
  }
  get arguments() {
    return []; // FIXME
  }
  get typeArguments() {
    return null; // FIXME
  }

  optional = false; // FIXME

  #ctx;
  #calleeId;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} calleeId
   */
  constructor(ctx, range, calleeId) {
    this.#ctx = ctx;
    this.#calleeId = calleeId;
    this.range = range;
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
class ConditionalExpression {
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
   * @param {Deno.Range} range
   * @param {number} testId
   * @param {number} consequentId
   * @param {number} alternateId
   */
  constructor(ctx, range, testId, consequentId, alternateId) {
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
class Identifier {
  type = /** @type {const} */ ("Identifier");
  range;
  name = "";

  #ctx;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} nameId
   */
  constructor(ctx, range, nameId) {
    this.#ctx = ctx;
    this.name = getString(ctx, nameId);
    this.range = range;
  }
}

/** @implements {Deno.LogicalExpression} */
class LogicalExpression {
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
   * @param {Deno.Range} range
   * @param {number} flags
   * @param {number} leftId
   * @param {number} rightId
   */
  constructor(ctx, range, flags, leftId, rightId) {
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
  switch (n) {
    case 0:
      return "&&";
    case 1:
      return "||";
    case 2:
      return "??";
    default:
      throw new Error(`Unknown operator: ${n}`);
  }
}

/** @implements {Deno.MemberExpression} */
class MemberExpression {
  type = /** @type {const} */ ("MemberExpression");
  range;
  get object() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#objId,
    ));
  }
  get property() {
    return /** @type {Deno.Expression | Deno.Identifier | Deno.PrivateIdentifier} */ (createAstNode(
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
   * @param {Deno.Range} range
   * @param {number} flags
   * @param {number} objId
   * @param {number} propId
   */
  constructor(ctx, range, flags, objId, propId) {
    this.#ctx = ctx;
    this.computed = flags === 1;
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
class NewExpression {
  type = /** @type {const} */ ("NewExpression");
  range;
  get arguments() {
    return []; // FIXME
  }
  get callee() {
    return /** @type {Deno.Expression} */ (createAstNode(
      this.#ctx,
      this.#calleeId,
    ));
  }

  #ctx;
  #calleeId;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} calleeId
   */
  constructor(ctx, range, calleeId) {
    this.#ctx = ctx;
    this.#calleeId = calleeId;
    this.range = range;
  }
}

/** @implements {Deno.Super} */
class Super {
  type = /** @type {const} */ ("Super");
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

// Literals

/** @implements {Deno.BooleanLiteral} */
class BooleanLiteral {
  type = /** @type {const} */ ("BooleanLiteral");
  range;
  value = false;
  #ctx;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} flags
   */
  constructor(ctx, range, flags) {
    this.#ctx = ctx;
    this.value = flags === 1;
    this.range = range;
  }
}

/** @implements {Deno.NullLiteral} */
class NullLiteral {
  type = /** @type {const} */ ("NullLiteral");
  range;
  value = null;

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

/** @implements {Deno.NumericLiteral} */
class NumericLiteral {
  type = /** @type {const} */ ("NumericLiteral");
  range;
  value = 0;

  #ctx;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} flags
   */
  constructor(ctx, range, flags) {
    this.#ctx = ctx;
    this.range = range;
    this.value = flags;
  }
}

/** @implements {Deno.RegExpLiteral} */
class RegExpLiteral {
  type = /** @type {const} */ ("RegExpLiteral");
  range;
  pattern = "";
  flags = "";

  #ctx;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} patternId
   * @param {number} flagsId
   */
  constructor(ctx, range, patternId, flagsId) {
    this.#ctx = ctx;
    this.range = range;
    this.pattern = getString(ctx, patternId);
    this.flags = getString(ctx, flagsId);
  }
}

/** @implements {Deno.StringLiteral} */
class StringLiteral {
  type = /** @type {const} */ ("StringLiteral");
  range;
  value = "";

  #ctx;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} valueId
   */
  constructor(ctx, range, valueId) {
    this.#ctx = ctx;
    this.range = range;
    this.value = getString(ctx, valueId);
  }
}

// JSX

/** @implements {Deno.JSXAttribute} */
class JSXAttribute {
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
   * @param {Deno.Range} range
   * @param {number} nameId
   * @param {number} valueId
   */
  constructor(ctx, range, nameId, valueId) {
    this.#ctx = ctx;
    this.range = range;
    this.#nameId = nameId;
    this.#valueId = valueId;
  }
}

/** @implements {Deno.JSXClosingElement} */
class JSXClosingElement {
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
   * @param {Deno.Range} range
   * @param {number} nameId
   */
  constructor(ctx, range, nameId) {
    this.#ctx = ctx;
    this.range = range;
    this.#nameId = nameId;
  }
}

/** @implements {Deno.JSXClosingFragment} */
class JSXClosingFragment {
  type = /** @type {const} */ ("JSXClosingFragment");
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

/** @implements {Deno.JSXElement} */
class JSXElement {
  type = /** @type {const} */ ("JSXElement");
  range;
  get children() {
    return []; // FIXME
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

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} openId
   * @param {number} closingId
   */
  constructor(ctx, range, openId, closingId) {
    this.#ctx = ctx;
    this.range = range;
    this.#openId = openId;
    this.#closingId = closingId;
  }
}

/** @implements {Deno.JSXExpressionContainer} */
class JSXExpressionContainer {
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
   * @param {Deno.Range} range
   * @param {number} exprId
   */
  constructor(ctx, range, exprId) {
    this.#ctx = ctx;
    this.range = range;
    this.#exprId = exprId;
  }
}

/** @implements {Deno.JSXFragment} */
class JSXFragment {
  type = /** @type {const} */ ("JSXFragment");
  range;
  get children() {
    return []; // FIXME
  }
  get closingFragment() {
    return /** @type {Deno.JSXClosingFragment} */ (createAstNode(
      this.#ctx,
      this.#closingId,
    ));
  }
  get openingFragment() {
    return /** @type {Deno.JSXOpeningFragment} */ (createAstNode(
      this.#ctx,
      this.#openingId,
    ));
  }

  #ctx;
  #closingId;
  #openingId;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} closingId
   * @param {number} openingId
   */
  constructor(ctx, range, closingId, openingId) {
    this.#ctx = ctx;
    this.range = range;
    this.#closingId = closingId;
    this.#openingId = openingId;
  }
}

/** @implements {Deno.JSXIdentifier} */
class JSXIdentifier {
  type = /** @type {const} */ ("JSXIdentifier");
  range;
  name;

  #ctx;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} nameId
   */
  constructor(ctx, range, nameId) {
    this.#ctx = ctx;
    this.range = range;
    this.name = getString(ctx, nameId);
  }
}

/** @implements {Deno.JSXMemberExpression} */
class JSXMemberExpression {
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
   * @param {Deno.Range} range
   * @param {number} objId
   * @param {number} propId
   */
  constructor(ctx, range, objId, propId) {
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
class JSXOpeningElement {
  type = /** @type {const} */ ("JSXOpeningElement");
  range;
  get attributes() {
    return []; // FIXME
  }
  get name() {
    return /** @type {Deno.JSXIdentifier} */ (createAstNode(
      this.#ctx,
      this.#nameId,
    ));
  }

  #ctx;
  #nameId;
  selfClosing = false;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} flags
   * @param {number} nameId
   */
  constructor(ctx, range, flags, nameId) {
    this.#ctx = ctx;
    this.selfClosing = flags === 1;
    this.#nameId = nameId;
    this.range = range;
  }
}

/** @implements {Deno.JSXOpeningFragment} */
class JSXOpeningFragment {
  type = /** @type {const} */ ("JSXOpeningFragment");
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

/** @implements {Deno.JSXSpreadAttribute} */
class JSXSpreadAttribute {
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
   * @param {Deno.Range} range
   * @param {number} argId
   */
  constructor(ctx, range, argId) {
    this.#ctx = ctx;
    this.range = range;
    this.#argId = argId;
  }
}

/** @implements {Deno.JSXSpreadChild} */
class JSXSpreadChild {
  type = /** @type {const} */ ("JSXSpreadChild");
  range;

  get expression() {
    return /** @type {Deno.Expression | Deno.JSXEmptyExpression} */ (createAstNode(
      this.#ctx,
      this.#exprid,
    ));
  }

  #ctx;
  #exprid;

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} exprId
   */
  constructor(ctx, range, exprId) {
    this.#ctx = ctx;
    this.range = range;
    this.#exprid = exprId;
  }
}

/** @implements {Deno.JSXText} */
class JSXText {
  type = /** @type {const} */ ("JSXText");
  range;

  #ctx;
  value = "";
  raw = "";

  /**
   * @param {AstContext} ctx
   * @param {Deno.Range} range
   * @param {number} valueId
   * @param {number} rawId
   */
  constructor(ctx, range, valueId, rawId) {
    this.#ctx = ctx;
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
 *   astStart: number,
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
  const i = idTable[id];
  /** @type {AstNodeId} */
  const kind = buf[i];

  const flags = buf[i + 1];
  const count = readU32(buf, i + 2);
  const rangeStart = readU32(buf, i + 6);
  const rangeEnd = readU32(buf, i + 10);
  const range = /** @type {Deno.Range} */ ([rangeStart, rangeEnd]);

  switch (kind) {
    case AstNodeId.Program:
      return new Program(ctx, range, flags === 1 ? "module" : "script");
    case AstNodeId.Import:
      // case AstNodeId.ImportDecl:
      // case AstNodeId.ExportDecl:
      // case AstNodeId.ExportNamed:
      // case AstNodeId.ExportDefaultDecl:
      // case AstNodeId.ExportDefaultExpr:
      // case AstNodeId.ExportAll:

    // Statements
    case AstNodeId.BlockStatement:
      throw new BlockStatement(ctx, range); // FIXME
    case AstNodeId.BreakStatement:
      throw new BreakStatement(ctx, range, 0); // FIXME
    case AstNodeId.ContinueStatement:
      throw new ContinueStatement(ctx, range, 0); // FIXME
    case AstNodeId.DebuggerStatement:
      throw new DebuggerStatement(ctx, range);
    case AstNodeId.DoWhileStatement:
      throw new DoWhileStatement(ctx, range, 0, 0); // FIXME
    case AstNodeId.ExpressionStatement:
      return new ExpressionStatement(ctx, range, 0); // FIXME
    case AstNodeId.ForInStatement:
      throw new ForInStatement(ctx, range, 0, 0, 0); // FIXME
    case AstNodeId.ForOfStatement:
      throw new ForOfStatement(ctx, range, false, 0, 0, 0); // FIXME
    case AstNodeId.ForStatement:
      throw new ForStatement(ctx, range, 0, 0, 0, 0); // FIXME
    case AstNodeId.IfStatement:
      throw new IfStatement(ctx, range, 0, 0, 0); // FIXME
    case AstNodeId.LabeledStatement:
      throw new LabeledStatement(ctx, range, 0, 0); // FIXME
    case AstNodeId.ReturnStatement:
      throw new ReturnStatement(ctx, range, 0); // FIXME
    case AstNodeId.SwitchStatement:
      throw new SwitchStatement(ctx, range, 0); // FIXME
    case AstNodeId.ThrowStatement:
      throw new ThrowStatement(ctx, range, 0); // FIXME
    case AstNodeId.TryStatement:
      throw new TryStatement(ctx, range, 0, 0, 0); // FIXME
    case AstNodeId.WhileStatement:
      throw new WhileStatement(ctx, range, 0, 0); // FIXME
    case AstNodeId.WithStatement:
      throw new WithStatement(ctx, range, 0, 0);

    // Expressions
    case AstNodeId.ArrayExpression:
      throw new ArrayExpression(ctx, range); // FIXME
    case AstNodeId.ArrowFunctionExpression:
      throw new ArrowFunctionExpression(ctx, range, false, false, 0); // FIXME
    case AstNodeId.AssignmentExpression:
      throw new AssignmentExpression(ctx, range, flags, 0, 0); // FIXME
    case AstNodeId.AwaitExpression:
      throw new AwaitExpression(ctx, range, 0); // FIXME
    case AstNodeId.BinaryExpression:
      throw new BinaryExpression(ctx, range, flags, 0, 0); // FIXME
    case AstNodeId.CallExpression:
      throw new CallExpression(ctx, range, 0); // FIXME
    case AstNodeId.ChainExpression:
      throw new ChainExpression(ctx, range); // FIXME
    case AstNodeId.ConditionalExpression:
      throw new ConditionalExpression(ctx, range, 0, 0, 0); // FIXME
    case AstNodeId.FunctionExpression:
      throw new FunctionExpression(ctx, range); // FIXME
    case AstNodeId.Identifier:
      throw new Identifier(ctx, range, flags); // FIXME
    case AstNodeId.LogicalExpression:
      throw new LogicalExpression(ctx, range, flags, 0, 0); // FIXME
    case AstNodeId.MemberExpression:
      throw new MemberExpression(ctx, range, flags, 0, 0); // FIXME
    case AstNodeId.MetaProperty:
      throw new MetaProperty(ctx, range, 0, 0); // FIXME
    case AstNodeId.NewExpression:
      throw new NewExpression(ctx, range, 0); // FIXME
    case AstNodeId.ObjectExpression:
      throw new Error("TODO");
    case AstNodeId.StaticBlock:
      throw new Error("TODO");
    case AstNodeId.SequenceExpression:
      throw new Error("TODO");
    case AstNodeId.Super:
      throw new Super(ctx, range); // FIXME
    case AstNodeId.TaggedTemplateExpression:
      throw new Error("TODO");
    case AstNodeId.TemplateLiteral:
      throw new Error("TODO");

    // Literals
    case AstNodeId.BooleanLiteral:
      throw new BooleanLiteral(ctx, range, flags); // FIXME
    case AstNodeId.NullLiteral:
      throw new NullLiteral(ctx, range); // FIXME
    case AstNodeId.NumericLiteral:
      throw new NumericLiteral(ctx, range, flags); // FIXME
    case AstNodeId.RegExpLiteral:
      throw new RegExpLiteral(ctx, range, 0, 0); // FIXME
    case AstNodeId.StringLiteral:
      throw new StringLiteral(ctx, range, flags);

      // JSX
      // FIXME
    case AstNodeId.JSXAttribute:
      throw new JSXAttribute(ctx, range, 0, 0); // FIXME
    case AstNodeId.JSXClosingElement:
      throw new JSXClosingElement(ctx, range, 0); // FIXME
    case AstNodeId.JSXClosingFragment:
      throw new JSXClosingFragment(ctx, range); // FIXME
    case AstNodeId.JSXElement:
      throw new JSXElement(ctx, range, 0, 0); // FIXME
    case AstNodeId.JSXExpressionContainer:
      throw new JSXExpressionContainer(ctx, range, 0); // FIXME
    case AstNodeId.JSXFragment:
      throw new JSXFragment(ctx, range, 0, 0); // FIXME
    case AstNodeId.JSXIdentifier:
      throw new JSXIdentifier(ctx, range, 0); // FIXME
    case AstNodeId.JSXMemberExpression:
      throw new JSXMemberExpression(ctx, range, 0, 0); // FIXME
    case AstNodeId.JSXNamespacedName:
      throw new JSXNamespacedName(ctx, range, 0, 0); // FIXME
    case AstNodeId.JSXOpeningElement:
      throw new JSXOpeningElement(ctx, range, flags, 0); // FIXME
    case AstNodeId.JSXOpeningFragment:
      throw new JSXOpeningFragment(ctx, range); // FIXME
    case AstNodeId.JSXSpreadAttribute:
      throw new JSXSpreadAttribute(ctx, range, flags); // FIXME
    case AstNodeId.JSXSpreadChild:
      throw new JSXSpreadChild(ctx, range, flags); // FIXME
    case AstNodeId.JSXText:
      throw new JSXText(ctx, range, 0, 0); // FIXME

    default:
      throw new Error(`Unknown ast node ${kind}`);
  }
}

/**
 * @param {Uint8Array} buf
 * @param {AstContext} buf
 */
function createAstContext(buf) {
  console.log(buf);

  // Extract string table
  /** @type {Map<number, string>} */
  const strTable = new Map();

  let i = 0;
  const stringCount = (buf[0] << 24) + (buf[1] << 16) + (buf[2] << 8) +
    buf[3];
  i += 4;

  let id = 0;
  while (id < stringCount) {
    const len = readU32(buf, i);
    i += 4;

    const strBytes = buf.slice(i, i + len);
    console.log({ strBytes });
    i += len;
    const s = DECODER.decode(strBytes);
    strTable.set(id, s);
    id++;
  }

  console.log({ stringCount, strTable });

  if (strTable.size !== stringCount) {
    throw new Error(
      `Could not deserialize string table. Expected ${stringCount} items, but got ${strTable.size}`,
    );
  }

  /** @type {AstContext} */
  const ctx = { buf, idTable: [], strTable, astStart: i };

  return ctx;
}

/**
 * @param {string} fileName
 * @param {Uint8Array} serializedAst
 */
export function runPluginsForFile(fileName, serializedAst) {
  const ctx = createAstContext(serializedAst);
  console.log(JSON.stringify(ctx, null, 2));

  /** @type {Record<string, (node: any) => void} */
  const mergedVisitor = {};
  const destroyFns = [];

  // Instantiate and merge visitors. This allows us to only traverse
  // the AST once instead of per plugin.
  for (let i = 0; i < state.plugins.length; i++) {
    const plugin = state.plugins[i];

    for (const name of Object.keys(plugin)) {
      const rule = plugin.rules[name];
      const id = `${plugin.name}/${name}`;
      const ctx = new Context(id, fileName);
      const visitor = rule.create(ctx);

      for (const name in visitor) {
        const prev = mergedVisitor[name];
        mergedVisitor[name] = (node) => {
          if (typeof prev === "function") {
            prev(node);
          }

          try {
            visitor[name](node);
          } catch (err) {
            throw new Error(`Visitor "${name}" of plugin "${id}" errored`, {
              cause: err,
            });
          }
        };
      }
      mergedVisitor.push({ ctx, visitor, rule });

      if (typeof rule.destroy === "function") {
        destroyFns.push(() => {
          try {
            rule.destroy(ctx);
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
  const { astStart, buf } = ctx;
  const visitingTypes = new Map();

  // TODO: create visiting types

  // All nodes are in depth first sorted order, so we can just loop
  // forward in an iterative style to visit all nodes in the correct
  // order.
  for (let i = astStart; i < buf.length; i++) {
    const id = buf[i];
    const nodeLen = buf[i + 1];
    const type = buf[i + 1];

    const name = visitingTypes.get(type);
    if (name !== undefined) {
      const node = createAstNode(ctx, id);
      visitor[name](node);
    }

    i += nodeLen;
  }
}
