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
    const range = data.node ? data.node.range : data.range ? data.range : null;
    if (range == null) {
      throw new Error(
        "Either `node` or `span` must be provided when reporting an error",
      );
    }

    const start = range[0] - 1;
    const end = range[1] - 1;

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
  FnOptional: 0b00001000,
  MemberComputed: 0b00000001,
  MemberOptional: 0b00000010,
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

  UpdatePrefix: 0b000000001,
  UpdatePlusPlus: 0b000000010,
  UpdateMinusMinus: 0b000000100,

  YieldDelegate: 1,
  ParamOptional: 1,

  ClassDeclare: 0b000000001,
  ClassAbstract: 0b000000010,
  ClassConstructor: 0b000000100,
  ClassMethod: 0b000001000,
  ClassPublic: 0b001000000,
  ClassProtected: 0b010000000,
  ClassPrivate: 0b100000000,
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
  ClassDeclaration: 12,
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
  UpdateExpression: 45,
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
  YieldExpression: 60,
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
  ChainExpression: 71,

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
  MethodDefinition: 86,

  // Patterns
  ArrayPattern: 87,
  AssignmentPattern: 88,
  ObjectPattern: 89,

  // JSX
  JSXAttribute: 90,
  JSXClosingElement: 91,
  JSXClosingFragment: 92,
  JSXElement: 93,
  JSXEmptyExpression: 94,
  JSXExpressionContainer: 95,
  JSXFragment: 96,
  JSXIdentifier: 97,
  JSXMemberExpression: 98,
  JSXNamespacedName: 99,
  JSXOpeningElement: 100,
  JSXOpeningFragment: 101,
  JSXSpreadAttribute: 102,
  JSXSpreadChild: 103,
  JSXText: 104,
};

const AstTypeName = Object.keys(AstType);

// Keep in sync with Rust
const AstProp = [
  // Base
  "parent",
  "range",
  "type",
  "_InternalFlags", // Internal

  // Node
  "alternate",
  "argument",
  "arguments",
  "async",
  "attributes",
  "await",
  "block",
  "body",
  "callee",
  "cases",
  "children",
  "closingElement",
  "closingFragment",
  "computed",
  "consequent",
  "cooked",
  "declarations",
  "declare",
  "definite",
  "delegate",
  "discriminant",
  "elements",
  "elementTypes",
  "expression",
  "expressions",
  "exported",
  "finalizer",
  "flags",
  "generator",
  "handler",
  "id",
  "init",
  "key",
  "kind",
  "label",
  "left",
  "local",
  "members",
  "meta",
  "method",
  "name",
  "namespace",
  "object",
  "openingElement",
  "openingFragment",
  "operator",
  "optional",
  "param",
  "params",
  "pattern",
  "prefix",
  "properties",
  "property",
  "quasi",
  "quasis",
  "raw",
  "returnType",
  "right",
  "selfClosing",
  "shorthand",
  "source",
  "specifiers",
  "tag",
  "tail",
  "test",
  "typeAnnotation",
  "typeArguments",
  "typeParameters",
  "types",
  "update",
  "value",
];
// FIXME: this is slow
const AST_PROP_PARENT = AstProp.indexOf("parent");
const AST_PROP_TYPE = AstProp.indexOf("type");
const AST_PROP_BODY = AstProp.indexOf("body");
const AST_PROP_CHILDREN = AstProp.indexOf("children");
const AST_PROP_ELEMENTS = AstProp.indexOf("elements");
const AST_PROP_PROPERTIES = AstProp.indexOf("properties");
const AST_PROP_ARGUMENTS = AstProp.indexOf("arguments");
const AST_PROP_PARAMS = AstProp.indexOf("params");
const AST_PROP_EXPRESSIONS = AstProp.indexOf("expressions");
const AST_PROP_QUASIS = AstProp.indexOf("quasis");
const AST_PROP_RANGE = AstProp.indexOf("range");
const AST_PROP_INTERNAL_FLAGS = AstProp.indexOf("_InternalFlags");
const AST_PROP_ATTRIBUTE = AstProp.indexOf("attribute");
const AST_PROP_CONSEQUENT = AstProp.indexOf("consequent");
const AST_PROP_CASES = AstProp.indexOf("cases");
const AST_PROP_SPECIFIERS = AstProp.indexOf("specifiers");
const AST_PROP_DECLARATIONS = AstProp.indexOf("declarations");
const AST_PROP_MEMBERS = AstProp.indexOf("members");
const AST_PROP_OPERATOR = AstProp.indexOf("operator");
const AST_PROP_PREFIX = AstProp.indexOf("prefix");
const AST_PROP_OPTIONAL = AstProp.indexOf("optional");
const AST_PROP_ASYNC = AstProp.indexOf("async");
const AST_PROP_GENERATOR = AstProp.indexOf("generator");
const AST_PROP_NAME = AstProp.indexOf("name");
const AST_PROP_VALUE = AstProp.indexOf("value");
const AST_PROP_COMPUTED = AstProp.indexOf("computed");
const AST_PROP_DELEGATE = AstProp.indexOf("delegate");
const AST_PROP_SELF_CLOSING = AstProp.indexOf("selfClosing");

/**
 * @param {number} type
 * @param {number} propId
 * @returns {boolean}
 */
function isArrayProp(type, propId) {
  switch (type) {
    case AstType.Program:
    case AstType.BlockStatement:
      // case AstType.StaticBlock:
      return propId === AST_PROP_BODY;
    case AstType.JSXOpeningElement:
      return propId === AST_PROP_ATTRIBUTE;
    case AstType.SwitchCase:
      return propId === AST_PROP_CONSEQUENT;
    default:
      switch (propId) {
        case AST_PROP_CHILDREN:
        case AST_PROP_ELEMENTS:
        case AST_PROP_PROPERTIES:
        case AST_PROP_PARAMS:
        case AST_PROP_ARGUMENTS:
        case AST_PROP_EXPRESSIONS:
        case AST_PROP_QUASIS:
        case AST_PROP_CASES:
        case AST_PROP_SPECIFIERS:
        case AST_PROP_DECLARATIONS:
        case AST_PROP_MEMBERS:
          return true;
        default:
          return false;
      }
  }
}

/**
 * @param {AstContext} ctx
 * @param {number} offset
 * @param {number} propId
 * @returns {*}
 */
function readValue(ctx, offset, propId) {
  const { buf } = ctx;

  if (propId === AST_PROP_TYPE) {
    const type = buf[offset];
    return AstTypeName[type];
  } else if (propId === AST_PROP_RANGE) {
    const start = readU32(buf, offset + 1 + 4);
    const end = readU32(buf, offset + 1 + 4 + 4);
    return [start, end];
  } else if (propId === AST_PROP_PARENT) {
    return readU32(buf, offset + 1);
  }

  const type = buf[offset];

  // type + parentId + SpanLo + SpanHi
  offset += 1 + 4 + 4 + 4;

  const propCount = buf[offset];
  offset += 1;

  for (let i = 0; i < propCount; i++) {
    const searchProp = buf[offset];
    offset += 1;

    if (searchProp === propId) {
      if (propId === AST_PROP_INTERNAL_FLAGS) {
        return buf[offset];
      } else if (isArrayProp(type, searchProp)) {
        const len = readU32(buf, offset);
        offset += 4;

        const ids = new Array(len).fill(null);
        for (let j = 0; j < len; j++) {
          ids[i] = readU32(buf, offset);
          offset += 4;
        }
        return ids;
      }

      return readU32(buf, offset);
    }

    if (searchProp === AST_PROP_INTERNAL_FLAGS) {
      offset += 1;
    } else if (isArrayProp(type, searchProp)) {
      const len = readU32(buf, offset);
      offset += 4 + (len * 4);
    } else {
      offset += 4;
    }
  }

  return 0;
}

/**
 * @param {AstContext} ctx
 * @param {number} offset
 * @returns {number}
 */
function getTypeId(ctx, offset) {
  return ctx.buf[offset];
}

/**
 * @param {AstContext} ctx
 * @param {number} offset
 * @param {Map<number, any>} seen
 * @returns {*}
 */
function toJsValue(ctx, offset, seen) {
  const cached = seen.get(offset);
  if (cached !== undefined) return cached;

  const type = getTypeId(ctx, offset);
  const range = readValue(ctx, offset, AST_PROP_RANGE);

  /** @type {Record<string, any>} */
  const node = {
    type: AstTypeName[type],
    range,
  };

  seen.set(offset, node);

  // console.log("toJSON", AstTypeName[type], offset);

  const { buf, idTable } = ctx;

  // type + parentId + SpanLo + SpanHi
  offset += 1 + 4 + 4 + 4;

  const propCount = buf[offset];
  offset += 1;

  let flags = 0;
  for (let i = 0; i < propCount; i++) {
    const propId = buf[offset];
    offset += 1;

    const name = AstProp[propId];
    // console.log(`reading node ${AstTypeName[type]}, prop ${name}`);

    if (propId === AST_PROP_INTERNAL_FLAGS) {
      flags = buf[offset];
      offset += 1;
    } else if (
      propId === AST_PROP_NAME && (type === AstType.Identifier ||
        type === AstType.JSXIdentifier)
    ) {
      const strId = readU32(buf, offset);
      offset += 4;

      node[name] = getString(ctx, strId);
    } else if (
      propId === AST_PROP_VALUE && (type === AstType.StringLiteral ||
        type === AstType.NumericLiteral)
    ) {
      const strId = readU32(buf, offset);
      offset += 4;

      node[name] = getString(ctx, strId);
    } else if (isArrayProp(type, propId)) {
      const len = readU32(buf, offset);
      offset += 4;

      const elems = new Array(len).fill(null);
      for (let j = 0; j < len; j++) {
        const id = readU32(buf, offset);
        offset += 4;

        if (id === 0) {
          elems[j] = undefined;
        } else {
          const nodeOffset = idTable[id];
          elems[j] = toJsValue(ctx, nodeOffset, seen);
        }
      }

      node[name] = elems;
    } else {
      const id = readU32(buf, offset);
      offset += 4;

      if (id === 0) {
        node[name] = null;
      } else {
        const nodeOffset = idTable[id];
        node[name] = toJsValue(ctx, nodeOffset, seen);
      }
    }
  }

  return node;
}

const INTERNAL_CTX = Symbol("ctx");
const INTERNAL_OFFSET = Symbol("offset");

class Node {
  [INTERNAL_CTX];
  [INTERNAL_OFFSET];

  /**
   * @param {AstContext} ctx
   * @param {number} offset
   */
  constructor(ctx, offset) {
    this[INTERNAL_CTX] = ctx;
    this[INTERNAL_OFFSET] = offset;
  }

  /**
   * @param {*} _
   * @param {*} options
   * @returns {string}
   */
  [Symbol.for("Deno.customInspect")](_, options) {
    const seen = new Map();
    const json = toJsValue(this[INTERNAL_CTX], this[INTERNAL_OFFSET], seen);
    seen.clear();
    return Deno.inspect(json, options);
  }
}

for (let i = 0; i < AstProp.length; i++) {
  const name = AstProp[i];
  Object.defineProperty(Node.prototype, name, {
    get() {
      const ctx = /** @type {AstContext} */ (this[INTERNAL_CTX]);
      const offset = this[INTERNAL_OFFSET];
      const type = getTypeId(ctx, offset);
      const flags = readValue(ctx, offset, AST_PROP_INTERNAL_FLAGS);

      switch (i) {
        case AST_PROP_OPERATOR:
          switch (type) {
            case AstType.AssignmentExpression:
              return getAssignOperator(flags);
            case AstType.BinaryExpression:
              return getBinaryOperator(flags);
            case AstType.UpdateExpression:
              return (flags & Flags.UpdatePlusPlus) !== 0 ? "++" : "--";
            case AstType.LogicalExpression:
              return getLogicalOperator(flags);
            case AstType.UnaryExpression:
              return getUnaryOperator(flags);
          }

          break;
        case AST_PROP_PREFIX:
          return (flags & Flags.UpdatePrefix) !== 0;
        case AST_PROP_OPTIONAL:
          switch (type) {
            case AstType.FunctionExpression:
              return (flags & Flags.FnOptional) !== 0;
            case AstType.MemberExpression:
              return (flags & Flags.MemberOptional) !== 0;
            case AstType.ArrayPattern:
            case AstType.ObjectPattern:
              return (flags & Flags.ParamOptional) !== 0;
          }

          break;
        case AST_PROP_ASYNC:
          return (flags & Flags.FnAsync) !== 0;
        case AST_PROP_GENERATOR:
          return (flags & Flags.FnGenerator) !== 0;
        case AST_PROP_COMPUTED:
          return (flags & Flags.MemberComputed) !== 0;
        case AST_PROP_DELEGATE:
          return (flags & Flags.YieldDelegate) !== 0;
        case AST_PROP_SELF_CLOSING:
          return flags !== 0;
        case AST_PROP_NAME: {
          const value = readValue(ctx, offset, AST_PROP_NAME);

          switch (type) {
            case AstType.Identifier:
            case AstType.JSXIdentifier:
            case AstType.PrivateIdentifier:
              return getString(ctx, value);
            default: {
              const nodeOffset = ctx.idTable[value];
              return new Node(ctx, nodeOffset);
            }
          }
        }
      }

      const value = readValue(ctx, offset, i);
      if (Array.isArray(value)) {
        const nodes = new Array(value.length);
        for (let i = 0; i < value.length; i++) {
          const id = value[i];
          if (id === 0) {
            nodes[i] = undefined;
            continue;
          }
          const nodeOffset = ctx.idTable[id];
          nodes[i] = new Node(ctx, nodeOffset);
        }
        return nodes;
      }

      if (value === 0) return null;
      const nodeOffset = ctx.idTable[value];
      return new Node(ctx, nodeOffset);
    },
  });
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
    default:
      throw new Error(`Unknown operator: ${n}`);
  }
}

// Literals

// /** @implements {Deno.BooleanLiteral} */
// class BooleanLiteral extends Node {
//   type = /** @type {const} */ ("BooleanLiteral");
//   range;
//   value = false;

//   /**
//    * @param {AstContext} ctx
//    * @param {number} parentId
//    * @param {Deno.Range} range
//    * @param {number} flags
//    */
//   constructor(ctx, parentId, range, flags) {
//     super(ctx, parentId);
//     this.value = flags === 1;
//     this.range = range;
//   }
// }

// /** @implements {Deno.BigIntLiteral} */
// class BigIntLiteral extends Node {
//   type = /** @type {const} */ ("BigIntLiteral");
//   range;
//   value;

//   /**
//    * @param {AstContext} ctx
//    * @param {number} parentId
//    * @param {Deno.Range} range
//    * @param {number} strId
//    */
//   constructor(ctx, parentId, range, strId) {
//     super(ctx, parentId);
//     this.range = range;
//     this.value = BigInt(getString(ctx, strId));
//   }
// }

// /** @implements {Deno.NullLiteral} */
// class NullLiteral extends Node {
//   type = /** @type {const} */ ("NullLiteral");
//   range;
//   value = null;

//   /**
//    * @param {AstContext} ctx
//    * @param {number} parentId
//    * @param {Deno.Range} range
//    */
//   constructor(ctx, parentId, range) {
//     super(ctx, parentId);
//     this.range = range;
//   }
// }

// /** @implements {Deno.NumericLiteral} */
// class NumericLiteral extends Node {
//   type = /** @type {const} */ ("NumericLiteral");
//   range;
//   value = 0;

//   /**
//    * @param {AstContext} ctx
//    * @param {number} parentId
//    * @param {Deno.Range} range
//    * @param {number} strId
//    */
//   constructor(ctx, parentId, range, strId) {
//     super(ctx, parentId);
//     this.range = range;
//     this.value = Number(getString(ctx, strId));
//   }
// }

// /** @implements {Deno.RegExpLiteral} */
// class RegExpLiteral extends Node {
//   type = /** @type {const} */ ("RegExpLiteral");
//   range;
//   pattern = "";
//   flags = "";

//   /**
//    * @param {AstContext} ctx
//    * @param {number} parentId
//    * @param {Deno.Range} range
//    * @param {number} patternId
//    * @param {number} flagsId
//    */
//   constructor(ctx, parentId, range, patternId, flagsId) {
//     super(ctx, parentId);

//     this.range = range;
//     this.pattern = getString(ctx, patternId);
//     this.flags = getString(ctx, flagsId);
//   }
// }

// /** @implements {Deno.StringLiteral} */
// class StringLiteral extends Node {
//   type = /** @type {const} */ ("StringLiteral");
//   range;
//   value = "";

//   /**
//    * @param {AstContext} ctx
//    * @param {number} parentId
//    * @param {Deno.Range} range
//    * @param {number} strId
//    */
//   constructor(ctx, parentId, range, strId) {
//     super(ctx, parentId);
//     this.range = range;
//     this.value = getString(ctx, strId);
//   }
// }

// /** @implements {Deno.TemplateElement} */
// class TemplateElement extends Node {
//   type = /** @type {const} */ ("TemplateElement");
//   range;

//   tail = false;
//   value;

//   /**
//    * @param {AstContext} ctx
//    * @param {number} parentId
//    * @param {Deno.Range} range
//    * @param {number} rawId
//    * @param {number} cookedId
//    * @param {boolean} tail
//    */
//   constructor(ctx, parentId, range, rawId, cookedId, tail) {
//     super(ctx, parentId);

//     const raw = getString(ctx, rawId);
//     this.value = {
//       raw,
//       cooked: cookedId === 0 ? raw : getString(ctx, cookedId),
//     };
//     this.tail = tail;
//     this.range = range;
//   }
// }

// /** @implements {Deno.JSXIdentifier} */
// class JSXIdentifier extends Node {
//   type = /** @type {const} */ ("JSXIdentifier");
//   range;
//   name;

//   /**
//    * @param {AstContext} ctx
//    * @param {number} parentId
//    * @param {Deno.Range} range
//    * @param {number} nameId
//    */
//   constructor(ctx, parentId, range, nameId) {
//     super(ctx, parentId);

//     this.range = range;
//     this.name = getString(ctx, nameId);
//   }
// }

const DECODER = new TextDecoder();

/**
 * @typedef {{
 *   buf: Uint8Array,
 *   strTable: Map<number, string>,
 *   idTable: number[],
 *   rootId: number
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
 * @param {Uint8Array} buf
 * @param {AstContext} buf
 */
function createAstContext(buf) {
  /** @type {Map<number, string>} */
  const strTable = new Map();

  // console.log(JSON.stringify(buf, null, 2));

  const strTableOffset = readU32(buf, buf.length - 12);
  const idTableOffset = readU32(buf, buf.length - 8);
  const rootId = readU32(buf, buf.length - 4);
  // console.log({ strTableOffset, idTableOffset, rootId });

  let offset = strTableOffset;
  const stringCount = readU32(buf, offset);
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
  const idCount = readU32(buf, idTableOffset);
  offset += 4;

  const idTable = new Array(idCount);

  for (let i = 0; i < idCount; i++) {
    const id = readU32(buf, offset);
    idTable[i] = id;
    offset += 4;
  }

  if (idTable.length !== idCount) {
    throw new Error(
      `Could not deserialize id table. Expected ${idCount} items, but got ${idTable.length}`,
    );
  }

  /** @type {AstContext} */
  const ctx = { buf, idTable, strTable, rootId };

  // console.log({ strTable, idTable });

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

  // console.log("buffer len", ctx.buf.length, ctx.buf.byteLength);
  console.log("merged visitor", visitor);
  console.log("visiting types", visitTypes);

  traverseInner(ctx, visitTypes, visitor, ctx.rootId);
}

const SKIP_CHILD_TRAVERSAL = new Set([
  AstType.BooleanLiteral,
  AstType.BigIntLiteral,
  AstType.DebuggerStatement,
  AstType.Identifier,
  AstType.JSXClosingFragment,
  AstType.JSXEmptyExpression,
  AstType.JSXOpeningFragment,
  AstType.JSXText,
  AstType.NullLiteral,
  AstType.NumericLiteral,
  AstType.PrivateIdentifier,
  AstType.RegExpLiteral,
  AstType.StringLiteral,
  AstType.TemplateLiteral,
  AstType.This,
]);

/**
 * @param {AstContext} ctx
 * @param {Map<number, string>} visitTypes
 * @param {Record<string, (x: any) => void>} visitor
 * @param {number} id
 */
function traverseInner(ctx, visitTypes, visitor, id) {
  // console.log("traversing id", id);

  // Empty id
  if (id === 0) return;
  const { idTable, buf } = ctx;
  if (id >= idTable.length) {
    throw new Error(`Invalid node  id: ${id}`);
  }

  let offset = idTable[id];
  if (offset === undefined) throw new Error(`Unknown id: ${id}`);

  const type = buf[offset];

  // console.log({ id, type, offset });

  const name = visitTypes.get(type);
  if (name !== undefined) {
    // console.log("--> invoking visitor");
    const node = new Node(ctx, offset);
    visitor[name](node);
  }

  if (SKIP_CHILD_TRAVERSAL.has(type)) return;

  // type + parentId + SpanLo + SpanHi
  offset += 1 + 4 + 4 + 4;

  const propCount = buf[offset];
  offset += 1;
  // console.log({ propCount });

  for (let i = 0; i < propCount; i++) {
    const searchProp = buf[offset];
    offset += 1;

    if (searchProp === AST_PROP_INTERNAL_FLAGS) {
      offset += 1;
      continue;
    }

    if (isArrayProp(type, searchProp)) {
      const len = readU32(buf, offset);
      offset += 4;

      for (let j = 0; j < len; j++) {
        const childId = readU32(buf, offset);
        offset += 4;
        traverseInner(ctx, visitTypes, visitor, childId);
      }
      continue;
    }

    const childId = readU32(buf, offset);
    traverseInner(ctx, visitTypes, visitor, childId);
    offset += 4;
  }
}
