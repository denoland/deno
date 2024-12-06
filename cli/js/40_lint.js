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
    return this.#source;
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
  SuperProp: 49,
  ConditionalExpression: 50,
  CallExpression: 51,
  NewExpression: 52,
  SequenceExpression: 53,
  Identifier: 54,
  Tpl: 55,
  TaggedTemplateExpression: 56,
  ArrowFunctionExpression: 57,
  Yield: 59,
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
   * @param {Deno.Range} range
   * @param {number} flags
   */
  constructor(ctx, range, flags) {
    this.#ctx = ctx;
    this.name = getString(ctx, flags);
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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

// Literals

/** @implements {Deno.BooleanLiteral} */
class BooleanLiteral {
  type = /** @type {const} */ ("BooleanLiteral");
  range;
  value = false;
  #ctx;

  /**
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
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
   * @param {ParseContext} ctx
   * @param {Deno.Range} range
   * @param {number} flags
   */
  constructor(ctx, range, flags) {
    this.#ctx = ctx;
    this.range = range;
    this.value = getString(ctx, flags);
  }
}

const DECODER = new TextDecoder();

/**
 * @typedef {{
 *   buf: Uint8Array,
 *   strTable: Map<number, string>,
 *   idTable: number[]
 * }} ParseContext
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
 * @param {ParseContext} ctx
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
 * @param {ParseContext} ctx
 * @param {number} id
 * @returns {Deno.AstNode}
 */
function createAstNode(ctx, id) {
  const { buf, idTable } = ctx;
  const i = idTable[id];
  /** @type {AstNodeId} */
  const kind = buf[i];

  const flags = buf[i];
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
      throw new Error("TODO");
    case AstNodeId.NewExpression:
      throw new Error("TODO");
    case AstNodeId.ObjectExpression:
      throw new Error("TODO");
    case AstNodeId.StaticBlock:
      throw new Error("TODO");
    case AstNodeId.SequenceExpression:
      throw new Error("TODO");
    case AstNodeId.Super:
      throw new Error("TODO");
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
      throw new RegExpLiteral(ctx, range, flags); // FIXME
    case AstNodeId.StringLiteral:
      throw new StringLiteral(ctx, range, flags); // FIXME
    default:
      throw new Error(`Unknown ast node ${kind}`);
  }
}

/**
 * @param {Uint8Array} ast
 */
function buildAstFromBinary(ast) {
  console.log(ast);

  // Extract string table
  /** @type {Map<number, string>} */
  const strTable = new Map();

  let start = 0;
  const stringCount = (ast[0] << 24) + (ast[1] << 16) + (ast[2] << 8) +
    ast[3];
  start += 4;

  let id = 0;
  while (id < stringCount) {
    const len = (ast[start] << 24) + (ast[start + 1] << 16) +
      (ast[start + 2] << 8) +
      ast[start + 3];
    start += 4;

    const strBytes = ast.slice(start, start + len);
    console.log({ strBytes });
    start += len;
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

  const counts = [];
  const stack = [];
  for (let i = start; i < ast.length; i += 14) {
    const kind = ast[i];
    const flags = ast[i + 1];
    let count = (ast[i + 2] << 24) + (ast[i + 3] << 16) + (ast[i + 4] << 8) +
      ast[i + 5];
    const start = (ast[i + 6] << 24) + (ast[i + 7] << 16) + (ast[i + 8] << 8) +
      ast[i + 9];
    const end = (ast[i + 10] << 24) + (ast[i + 11] << 16) +
      (ast[i + 12] << 8) +
      ast[i + 13];

    const span = [start, end];

    let node = null;
    switch (kind) {
      case AstNodeId.Program:
        node = new Program(span, flags === 1 ? "module" : "script");
        break;
      case AstNodeId.Var:
        node = new VariableDeclaration(span);
        break;
      case AstNodeId.VarDeclarator:
        node = new VariableDeclarator(span);
        break;
      case AstNodeId.ExpressionStatement:
        node = new ExpressionStatement(span);
        break;
      case AstNodeId.This:
        node = new ThisExpression(span);
        break;
      case AstNodeId.ArrayExpression:
        node = new ArrayExpression(span);
        break;
      case AstNodeId.ObjectExpression:
        node = new ObjectExpression(span);
        break;
      case AstNodeId.AssignmentExpression:
        node = new ObjectExpression(span);
        break;
      case AstNodeId.MemberExpression:
        node = new MemberExpression(span);
        break;
      case AstNodeId.CallExpression:
        node = new CallExpression(span);
        break;
      case AstNodeId.SequenceExpression:
        node = new SequenceExpression(span);
        break;
      case AstNodeId.ObjProperty:
        node = new ObjectProperty(span);
        break;
      case AstNodeId.ArrowFunctionExpression:
        node = new ArrowFunctionExpression(span);
        break;
      case AstNodeId.BlockStatement:
        node = new BlockStatement(span);
        break;
      case AstNodeId.StringLiteral:
        node = new StringLiteral(span);
        break;
      case AstNodeId.Identifier:
        node = new Identifier(span, strTable.get(count));
        count = 0;
        break;
      case AstNodeId.Fn:
        node = new FunctionDeclaration(span);
        break;
      case AstNodeId.ReturnStatement:
        node = new ReturnStatement(span);
        break;
      case AstNodeId.IfStatement:
        node = new IfStatement(span);
        break;
      case AstNodeId.BinaryExpression:
        node = new LogicalExpression(span);
        break;
      case AstNodeId.Unary:
        node = new UnaryExpression(span);
        break;
      case AstNodeId.Update:
        node = new UpdateExpression(span);
        break;
      case AstNodeId.ForStatement:
        node = new ForStatement(span);
        break;
      case AstNodeId.BooleanLiteral:
        node = new BooleanLiteral(span, flags === 1);
        break;
      case AstNodeId.NullLiteral:
        node = new NullLiteral(span);
        break;
      case AstNodeId.NumericLiteral:
        node = new NumericLiteral(span);
        break;
      case AstNodeId.RegExpLiteral:
        node = new RegExpLiteral(span);
        break;
      case AstNodeId.ForInStatement:
        node = new ForInStatement(span);
        break;
      case AstNodeId.ForOfStatement:
        node = new ForOfStatement(span);
        break;
      case AstNodeId.WhileStatement:
        node = new WhileStatement(span);
        break;
      case AstNodeId.Yield:
        node = new YieldExpression(span);
        break;
      case AstNodeId.ContinueStatement:
        node = new ContinueStatement(span);
        break;
      case AstNodeId.BreakStatement:
        node = new BreakStatement(span);
        break;
      case AstNodeId.ConditionalExpression:
        node = new ConditionalExpression(span);
        break;
      case AstNodeId.SwitchStatement:
        node = new SwitchStatement(span);
        break;
      case AstNodeId.SwitchCase:
        node = new SwitchCase(span);
        break;
      case AstNodeId.LabeledStatement:
        node = new LabeledStatement(span);
        break;
      case AstNodeId.DoWhileStatement:
        node = new DoWhileStatement(span);
        break;
      case AstNodeId.Spread:
        node = new SpreadElement(span);
        break;
      case AstNodeId.ThrowStatement:
        node = new ThrowStatement(span);
        break;
      case AstNodeId.DebuggerStatement:
        node = new DebuggerStatement(span);
        break;
      case AstNodeId.Tpl:
        node = new TemplateLiteral(span);
        break;
      case AstNodeId.NewExpression:
        node = new NewExpression(span);
        break;
      case AstNodeId.Class:
        node = new ClassDeclaration(span);
        break;
      case AstNodeId.TryStatement:
        node = new TryStatement(span);
        break;
      case AstNodeId.CatchClause:
        node = new CatchClause(span);
        break;
      case AstNodeId.TaggedTemplateExpression:
        node = new TaggedTemplateExpression(span);
        break;
      case AstNodeId.FunctionExpression:
        node = new FunctionExpression(span);
        break;
      case AstNodeId.Empty:
        // Ignore empty statements
        break;
      case AstNodeId.EmptyExpr:
        // Nothing, AST defaults to null
        break;
      default:
        throw new Error(`Unknown node: ${kind}`);
    }

    // append node
    if (stack.length > 0) {
      const last = stack[stack.length - 1];
      const id = last[_ID];
      const lastCount = counts[counts.length - 1];

      // console.log({ last, node });
      switch (id) {
        case AstNodeId.Program:
        case AstNodeId.BlockStatement:
          last.body.push(node);
          break;
        case AstNodeId.ExpressionStatement:
          last.expression = node;
          break;
        case AstNodeId.ObjProperty:
          if (lastCount > 1) {
            last.value = node;
          } else {
            last.key = node;
          }
          break;
        case AstNodeId.MemberExpression:
          if (lastCount > 1) {
            last.property = node;
          } else {
            last.object = node;
          }
          break;
        case AstNodeId.CallExpression:
          if (lastCount > 1) {
            last.arguments.push(node);
          } else {
            last.callee = node;
          }
          break;
        case AstNodeId.SequenceExpression:
          last.expressions.push(node);
          break;
        case AstNodeId.ArrowFunctionExpression:
          // FIXME
          break;
        case AstNodeId.ReturnStatement:
        case AstNodeId.Spread:
        case AstNodeId.ThrowStatement:
        case AstNodeId.Unary:
        case AstNodeId.Update:
        case AstNodeId.Yield:
          last.argument = node;
          break;
        case AstNodeId.IfStatement:
        case AstNodeId.ConditionalExpression:
          if (lastCount === 3) {
            last.alternate = node;
          } else if (lastCount === 2) {
            last.consequent = node;
          } else {
            last.test = node;
          }
          break;
        case AstNodeId.BinaryExpression:
          if (lastCount === 2) {
            last.right = node;
          } else {
            last.left = node;
          }
          break;
        case AstNodeId.ForStatement:
          if (lastCount === 4) {
            last.body = node;
          } else if (lastCount === 3) {
            last.update = node;
          } else if (lastCount === 2) {
            last.test = node;
          } else if (lastCount === 1) {
            last.init = node;
          }
          break;
        case AstNodeId.ForInStatement:
        case AstNodeId.ForOfStatement:
          if (lastCount === 3) {
            last.body = node;
          } else if (lastCount === 2) {
            last.right = node;
          } else {
            last.left = node;
          }
          break;
        case AstNodeId.DoWhileStatement:
        case AstNodeId.WhileStatement:
          if (lastCount === 2) {
            last.body = node;
          } else {
            last.test = node;
          }
          break;
        case AstNodeId.BreakStatement:
        case AstNodeId.ContinueStatement:
          last.label = node;
          break;
        case AstNodeId.SwitchStatement:
          if (lastCount > 1) {
            last.cases.push(node);
          } else {
            last.discriminant = node;
          }
          break;
        case AstNodeId.SwitchCase:
          if (lastCount > 1) {
            last.consequent = node;
          } else {
            last.test = node;
          }
          break;
        case AstNodeId.LabeledStatement:
          last.body = node;
          break;
        case AstNodeId.VarDeclarator:
          if (lastCount > 1) {
            last.init = node;
          } else {
            last.id = node;
          }
          break;
        case AstNodeId.NewExpression:
          // FIXME
          break;
        case AstNodeId.Class:
          // FIXME
          break;
        case AstNodeId.TryStatement:
          // FIXME
          break;
        case AstNodeId.CatchClause:
          // FIXME
          break;
        // Can't happen
        case AstNodeId.Identifier:
        case AstNodeId.StringLiteral:
        case AstNodeId.BigInt:
        case AstNodeId.BooleanLiteral:
        case AstNodeId.NullLiteral:
        case AstNodeId.NumericLiteral:
        case AstNodeId.RegExpLiteral:
        case AstNodeId.This:
        case AstNodeId.DebuggerStatement:
          break;
      }

      // console.log("APPENDED");
      // console.log(last);
      // console.log("======");

      // Decrease count
      const newCount = lastCount - 1;
      counts[counts.length - 1] = newCount;
    }

    if (count > 0) {
      stack.push(node);
      counts.push(count);
    } else if (stack.length > 0) {
      let lastCount = counts[counts.length - 1];
      while (stack.length > 1 && lastCount === 0) {
        // console.log({ counts, s: stack.map((x) => x.type) });
        const l = stack.pop();
        // console.log("POP", l);
        lastCount = counts.pop();
      }
    }
  }

  // console.log(JSON.stringify(stack, null, 2));
  return stack[0];
}

export function runPluginsForFile(fileName, serializedAst) {
  const ast = buildAstFromBinary(serializedAst);
  console.log(JSON.stringify(ast, null, 2));

  /** @type {Record<string, (node: any) => void} */
  const mergedVisitor = {};
  const destroyFns = [];

  // Instantiate and merge visitors. This allows us to only traverse
  // the AST once instead of per plugin.
  for (let i = 0; i < state.plugins; i++) {
    const plugin = state.plugins[i];

    for (const name of Object.keys(plugin)) {
      const rule = plugin.rules[name];
      const id = `${plugin.name}/${ruleName}`;
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
  traverse(ast, mergedVisitor, null);

  // Optional: Destroy rules
  for (let i = 0; i < destroyFns.length; i++) {
    destroyFns[i]();
  }
}

/**
 * @param {Record<string, any>} ast
 * @param {*} visitor
 * @param {any | null} parent
 * @returns {void}
 */
function traverse(ast, visitor, parent) {
  if (!ast || typeof ast !== "object") {
    return;
  }

  // Get node type, accounting for SWC's type property naming
  const nodeType = ast.type || (ast.nodeType ? ast.nodeType : null);

  // Skip if not a valid AST node
  if (!nodeType) {
    return;
  }

  ast.parent = parent;

  // Call visitor if it exists for this node type
  visitor[nodeType]?.(ast);

  // Traverse child nodes
  for (const key in ast) {
    if (key === "parent" || key === "type") {
      continue;
    }

    const child = ast[key];

    if (Array.isArray(child)) {
      for (let i = 0; i < child.length; i++) {
        const item = child[i];
        traverse(item, visitor, ast);
      }
    } else if (child !== null && typeof child === "object") {
      traverse(child, visitor, ast);
    }
  }
}
