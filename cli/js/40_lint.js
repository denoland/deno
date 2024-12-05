// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core } from "ext:core/mod.js";
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
const AstNode = {
  Invalid: 0,
  Program: 1,

  Import: 2,

  // Decls
  Class: 12,
  Fn: 13,
  Var: 14,

  // Statements
  Block: 20,
  Empty: 21,
  Debugger: 22,
  With: 23,
  Return: 24,
  Labeled: 25,
  Break: 26,
  Continue: 27,
  If: 28,
  Switch: 29,
  SwitchCase: 30,
  Throw: 31,
  Try: 32,
  While: 33,
  DoWhile: 34,
  For: 35,
  ForIn: 36,
  ForOf: 37,
  Decl: 38,
  Expr: 39,

  // Expressions
  This: 40,
  Array: 41,
  Object: 42,
  FnExpr: 43,
  Unary: 44,
  Update: 45,
  Bin: 46,
  Assign: 47,
  Member: 48,
  SuperProp: 49,
  Cond: 50,
  Call: 51,
  New: 52,
  Seq: 53,
  Ident: 54,
  Tpl: 55,
  TaggedTpl: 56,
  Arrow: 57,
  Yield: 59,

  StringLiteral: 70,
  Bool: 71,
  Null: 72,
  Num: 73,
  BigInt: 74,
  Regex: 75,

  // Custom
  EmptyExpr: 82,
  Spread: 83,
  ObjProperty: 84,
  VarDeclarator: 85,
  CatchClause: 86,
};

const _ID = Symbol.for("__astId");

class Program {
  type = "Program";
  body = [];
  [_ID] = AstNode.Program;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class VariableDeclaration {
  type = "VariableDeclaration";
  declarations = [];
  [_ID] = AstNode.Var;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class VariableDeclarator {
  type = "VariableDeclarator";
  id = null;
  init = null;
  [_ID] = AstNode.VarDeclarator;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class FunctionDeclaration {
  type = "FunctionDeclaration";
  [_ID] = AstNode.Fn;
  generator = false;
  async = false;
  id = null;
  params = [];
  body = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class ReturnStatement {
  type = "ReturnStatement";
  [_ID] = AstNode.Return;
  argument = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class IfStatement {
  type = "IfStatement";
  [_ID] = AstNode.If;
  test = null;
  consequent = null;
  alternate = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class LabeledStatement {
  type = "LabeledStatement";
  [_ID] = AstNode.Labeled;
  body = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class ThrowStatement {
  type = "ThrowStatement";
  [_ID] = AstNode.Throw;
  argument = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class ForStatement {
  type = "ForStatement";
  [_ID] = AstNode.For;
  init = null;
  test = null;
  update = null;
  body = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class ForInStatement {
  type = "ForInStatement";
  [_ID] = AstNode.ForIn;
  left = null;
  right = null;
  body = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class ForOfStatement {
  type = "ForOfStatement";
  [_ID] = AstNode.ForOf;
  await = false;
  left = null;
  right = null;
  body = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class WhileStatement {
  type = "WhileStatement";
  [_ID] = AstNode.While;
  test = null;
  body = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class ClassDeclaration {
  type = "ClassDeclaration";
  [_ID] = AstNode.Class;
  id = null;
  superClass = null;
  body = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class TryStatement {
  type = "TryStatement";
  [_ID] = AstNode.Try;
  block = null;
  handler = null;
  finalizer = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class CatchClause {
  type = "CatchClause";
  [_ID] = AstNode.CatchClause;
  param = null;
  body = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class DoWhileStatement {
  type = "DoWhileStatement";
  [_ID] = AstNode.DoWhile;
  test = null;
  body = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class SwitchStatement {
  type = "SwitchStatement";
  [_ID] = AstNode.Switch;
  discriminant = null;
  cases = [];
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class SwitchCase {
  type = "SwitchCase";
  [_ID] = AstNode.SwitchCase;
  test = null;
  consequent = null;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class ExpressionStatement {
  type = "ExpressionStatement";
  expression = null;
  [_ID] = AstNode.Expr;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class NewExpression {
  type = "NewExpression";
  callee = null;
  typeArguments = null;
  arguments = [];
  [_ID] = AstNode.New;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class UnaryExpression {
  type = "UnaryExpression";
  argument = null;
  operator = null;
  [_ID] = AstNode.Unary;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class UpdateExpression {
  type = "UpdateExpression";
  argument = null;
  operator = null;
  prefix = false;
  [_ID] = AstNode.Update;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class ThisExpression {
  type = "ThisExpression";
  [_ID] = AstNode.This;
  loc;

  constructor(span) {
    this.loc = span;
  }
}

class ArrayExpression {
  type = "ArrayExpression";
  elements = [];
  [_ID] = AstNode.Array;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class ObjectExpression {
  type = "ObjectExpression";
  properties = [];
  [_ID] = AstNode.Object;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class LogicalExpression {
  type = "LogicalExpression";
  [_ID] = AstNode.Bin;
  operator = null;
  left = null;
  right = null;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class SequenceExpression {
  type = "SequenceExpression";
  expressions = [];
  [_ID] = AstNode.Seq;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class BlockStatement {
  type = "BlockStatement";
  body = [];
  [_ID] = AstNode.Block;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class ContinueStatement {
  type = "ContinueStatement";
  label = null;
  [_ID] = AstNode.Continue;

  loc;
  constructor(span) {
    this.loc = span;
  }
}
class BreakStatement {
  type = "BreakStatement";
  label = null;
  [_ID] = AstNode.Break;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class DebuggerStatement {
  type = "DebuggerStatement";
  [_ID] = AstNode.Debugger;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class MemberExpression {
  type = "MemberExpression";
  [_ID] = AstNode.Member;
  computed = false;
  property = null;
  object = null;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class CallExpression {
  type = "CallExpression";
  properties = [];
  [_ID] = AstNode.Call;
  callee = null;
  arguments = [];

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class ArrowFunctionExpression {
  type = "ArrowFunctionExpression";
  [_ID] = AstNode.Arrow;
  generator = false;
  async = false;
  id = null;
  params = [];
  body = null;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class YieldExpression {
  type = "YieldExpression";
  [_ID] = AstNode.Yield;
  delegate = false;
  argument = null;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class ConditionalExpression {
  type = "ConditionalExpression";
  [_ID] = AstNode.Cond;
  test = null;
  consequent = null;
  alternate = null;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class StringLiteral {
  type = "StringLiteral";
  [_ID] = AstNode.StringLiteral;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class BooleanLiteral {
  type = "BooleanLiteral";
  value = false;
  [_ID] = AstNode.Bool;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class NullLiteral {
  type = "NullLiteral";
  value = false;
  [_ID] = AstNode.Null;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class NumericLiteral {
  type = "NumericLiteral";
  value = 0;
  [_ID] = AstNode.Num;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class RegExpLiteral {
  type = "RegExpLiteral";
  value = null;
  pattern = "";
  flags = "";
  [_ID] = AstNode.Regex;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class TemplateLiteral {
  type = "TemplateLiteral";
  expressions = [];
  quasis = [];
  [_ID] = AstNode.Tpl;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class TaggedTemplateExpression {
  type = "TaggedTemplateExpression";
  tag = null;
  quasi = null;
  [_ID] = AstNode.TaggedTpl;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class FunctionExpression {
  type = "FunctionExpression";
  generator = false;
  async = false;
  id = null;
  params = [];
  body = null;
  [_ID] = AstNode.FnExpr;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class Identifier {
  type = "Identifier";
  [_ID] = AstNode.Ident;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class ObjectProperty {
  type = "ObjectProperty";
  method = false;
  computed = false;
  shorthand = false;
  key = null;
  value = null;
  [_ID] = AstNode.ObjProperty;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

class SpreadElement {
  type = "SpreadElement";
  argument = null;
  [_ID] = AstNode.Spread;

  loc;
  constructor(span) {
    this.loc = span;
  }
}

/**
 * @param {Uint8Array} ast
 */
function buildAstFromBinary(ast) {
  // console.log(ast);
  const counts = [];
  const stack = [];
  for (let i = 0; i < ast.length; i += 14) {
    const kind = ast[i];
    const flags = ast[i + 1];
    const count = ast[i + 2] << ast[i + 3] << ast[i + 4] << ast[i + 5];
    const start = ast[i + 6] << ast[i + 7] << ast[i + 8] << ast[i + 9];
    const end = ast[i + 10] << ast[i + 11] << ast[i + 12] << ast[i + 13];

    const span = [start, end];

    let node = null;
    switch (kind) {
      case AstNode.Program:
        node = new Program(span);
        break;
      case AstNode.Var:
        node = new VariableDeclaration(span);
        break;
      case AstNode.VarDeclarator:
        node = new VariableDeclarator(span);
        break;
      case AstNode.Expr:
        node = new ExpressionStatement(span);
        break;
      case AstNode.This:
        node = new ThisExpression(span);
        break;
      case AstNode.Array:
        node = new ArrayExpression(span);
        break;
      case AstNode.Object:
        node = new ObjectExpression(span);
        break;
      case AstNode.Assign:
        node = new ObjectExpression(span);
        break;
      case AstNode.Member:
        node = new MemberExpression(span);
        break;
      case AstNode.Call:
        node = new CallExpression(span);
        break;
      case AstNode.Seq:
        node = new SequenceExpression(span);
        break;
      case AstNode.ObjProperty:
        node = new ObjectProperty(span);
        break;
      case AstNode.Arrow:
        node = new ArrowFunctionExpression(span);
        break;
      case AstNode.Block:
        node = new BlockStatement(span);
        break;
      case AstNode.StringLiteral:
        node = new StringLiteral(span);
        break;
      case AstNode.Ident:
        node = new Identifier(span);
        break;
      case AstNode.Fn:
        node = new FunctionDeclaration(span);
        break;
      case AstNode.Return:
        node = new ReturnStatement(span);
        break;
      case AstNode.If:
        node = new IfStatement(span);
        break;
      case AstNode.Bin:
        node = new LogicalExpression(span);
        break;
      case AstNode.Unary:
        node = new UnaryExpression(span);
        break;
      case AstNode.Update:
        node = new UpdateExpression(span);
        break;
      case AstNode.For:
        node = new ForStatement(span);
        break;
      case AstNode.Bool:
        node = new BooleanLiteral(span);
        break;
      case AstNode.Null:
        node = new NullLiteral(span);
        break;
      case AstNode.Num:
        node = new NumericLiteral(span);
        break;
      case AstNode.Regex:
        node = new RegExpLiteral(span);
        break;
      case AstNode.ForIn:
        node = new ForInStatement(span);
        break;
      case AstNode.ForOf:
        node = new ForOfStatement(span);
        break;
      case AstNode.While:
        node = new WhileStatement(span);
        break;
      case AstNode.Yield:
        node = new YieldExpression(span);
        break;
      case AstNode.Continue:
        node = new ContinueStatement(span);
        break;
      case AstNode.Break:
        node = new BreakStatement(span);
        break;
      case AstNode.Cond:
        node = new ConditionalExpression(span);
        break;
      case AstNode.Switch:
        node = new SwitchStatement(span);
        break;
      case AstNode.SwitchCase:
        node = new SwitchCase(span);
        break;
      case AstNode.Labeled:
        node = new LabeledStatement(span);
        break;
      case AstNode.DoWhile:
        node = new DoWhileStatement(span);
        break;
      case AstNode.Spread:
        node = new SpreadElement(span);
        break;
      case AstNode.Throw:
        node = new ThrowStatement(span);
        break;
      case AstNode.Debugger:
        node = new DebuggerStatement(span);
        break;
      case AstNode.Tpl:
        node = new TemplateLiteral(span);
        break;
      case AstNode.New:
        node = new NewExpression(span);
        break;
      case AstNode.Class:
        node = new ClassDeclaration(span);
        break;
      case AstNode.Try:
        node = new TryStatement(span);
        break;
      case AstNode.CatchClause:
        node = new CatchClause(span);
        break;
      case AstNode.TaggedTpl:
        node = new TaggedTemplateExpression(span);
        break;
      case AstNode.FnExpr:
        node = new FunctionExpression(span);
        break;
      case AstNode.Empty:
        // Ignore empty statements
        break;
      case AstNode.EmptyExpr:
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
        case AstNode.Program:
        case AstNode.Block:
          last.body.push(node);
          break;
        case AstNode.Expr:
          last.expression = node;
          break;
        case AstNode.ObjProperty:
          if (lastCount > 1) {
            last.value = node;
          } else {
            last.key = node;
          }
          break;
        case AstNode.Member:
          if (lastCount > 1) {
            last.property = node;
          } else {
            last.object = node;
          }
          break;
        case AstNode.Call:
          if (lastCount > 1) {
            last.arguments.push(node);
          } else {
            last.callee = node;
          }
          break;
        case AstNode.Seq:
          last.expressions.push(node);
          break;
        case AstNode.Arrow:
          // FIXME
          break;
        case AstNode.Return:
        case AstNode.Spread:
        case AstNode.Throw:
        case AstNode.Unary:
        case AstNode.Update:
        case AstNode.Yield:
          last.argument = node;
          break;
        case AstNode.If:
        case AstNode.Cond:
          if (lastCount === 3) {
            last.alternate = node;
          } else if (lastCount === 2) {
            last.consequent = node;
          } else {
            last.test = node;
          }
          break;
        case AstNode.Bin:
          if (lastCount === 2) {
            last.right = node;
          } else {
            last.left = node;
          }
          break;
        case AstNode.For:
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
        case AstNode.ForIn:
        case AstNode.ForOf:
          if (lastCount === 3) {
            last.body = node;
          } else if (lastCount === 2) {
            last.right = node;
          } else {
            last.left = node;
          }
          break;
        case AstNode.DoWhile:
        case AstNode.While:
          if (lastCount === 2) {
            last.body = node;
          } else {
            last.test = node;
          }
          break;
        case AstNode.Break:
        case AstNode.Continue:
          last.label = node;
          break;
        case AstNode.Switch:
          if (lastCount > 1) {
            last.cases.push(node);
          } else {
            last.discriminant = node;
          }
          break;
        case AstNode.SwitchCase:
          if (lastCount > 1) {
            last.consequent = node;
          } else {
            last.test = node;
          }
          break;
        case AstNode.Labeled:
          last.body = node;
          break;
        case AstNode.VarDeclarator:
          if (lastCount > 1) {
            last.init = node;
          } else {
            last.id = node;
          }
          break;
        case AstNode.New:
          // FIXME
          break;
        case AstNode.Class:
          // FIXME
          break;
        case AstNode.Try:
          // FIXME
          break;
        case AstNode.CatchClause:
          // FIXME
          break;
        // Can't happen
        case AstNode.Ident:
        case AstNode.StringLiteral:
        case AstNode.BigInt:
        case AstNode.Bool:
        case AstNode.Null:
        case AstNode.Num:
        case AstNode.Regex:
        case AstNode.This:
        case AstNode.Debugger:
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

export function runPluginsForFile(fileName, serializedAst, binary) {
  const ast = buildAstFromBinary(binary);
  // console.log(ast);
  // const ast = JSON.parse(serializedAst, (key, value) => {
  //   if (key === "ctxt") {
  //     return undefined;
  //   }
  //   return value;
  // });

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
