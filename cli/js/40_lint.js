// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check

import { core } from "ext:core/mod.js";
const {
  op_lint_get_rule,
  op_lint_get_source,
  op_lint_report,
} = core.ops;

// Keep in sync with Rust
/**
 * @enum {number}
 */
const AstTypeName = [
  "Invalid",
  "Program",

  "Import",
  "ImportDecl",
  "ExportDecl",
  "ExportNamed",
  "ExportDefaultDecl",
  "ExportDefaultExpr",
  "ExportAll",
  "TSImportEquals",
  "TSExportAssignment",
  "TSNamespaceExport",

  // Decls
  "ClassDeclaration",
  "FunctionDeclaration",
  "VariableDeclaration",
  "Using",
  "TsInterface",
  "TsTypeAlias",
  "TsEnum",
  "TsModule",

  // Statements
  "BlockStatement",
  "Empty",
  "DebuggerStatement",
  "WithStatement",
  "ReturnStatement",
  "LabeledStatement",
  "BreakStatement",
  "ContinueStatement",
  "IfStatement",
  "SwitchStatement",
  "SwitchCase",
  "ThrowStatement",
  "TryStatement",
  "WhileStatement",
  "DoWhileStatement",
  "ForStatement",
  "ForInStatement",
  "ForOfStatement",
  "Decl",
  "ExpressionStatement",

  // Expressions
  "This",
  "ArrayExpression",
  "ObjectExpression",
  "FunctionExpression",
  "UnaryExpression",
  "UpdateExpression",
  "BinaryExpression",
  "AssignmentExpression",
  "MemberExpression",
  "Super",
  "ConditionalExpression",
  "CallExpression",
  "NewExpression",
  "ParenthesisExpression",
  "SequenceExpression",
  "Identifier",
  "TemplateLiteral",
  "TaggedTemplateExpression",
  "ArrowFunctionExpression",
  "ClassExpr",
  "YieldExpression",
  "MetaProperty",
  "AwaitExpression",
  "LogicalExpression",
  "TSTypeAssertion",
  "TSConstAssertion",
  "TSNonNull",
  "TSAs",
  "TSInstantiation",
  "TSSatisfies",
  "PrivateIdentifier",
  "ChainExpression",

  "StringLiteral",
  "BooleanLiteral",
  "NullLiteral",
  "NumericLiteral",
  "BigIntLiteral",
  "RegExpLiteral",

  // Custom
  "EmptyExpr",
  "SpreadElement",
  "Property",
  "VariableDeclarator",
  "CatchClause",
  "RestElement",
  "ExportSpecifier",
  "TemplateElement",
  "MethodDefinition",

  // Patterns
  "ArrayPattern",
  "AssignmentPattern",
  "ObjectPattern",

  // JSX
  "JSXAttribute",
  "JSXClosingElement",
  "JSXClosingFragment",
  "JSXElement",
  "JSXEmptyExpression",
  "JSXExpressionContainer",
  "JSXFragment",
  "JSXIdentifier",
  "JSXMemberExpression",
  "JSXNamespacedName",
  "JSXOpeningElement",
  "JSXOpeningFragment",
  "JSXSpreadAttribute",
  "JSXSpreadChild",
  "JSXText",
];

/** @type {Map<string, number>} */
const AstType = new Map();
for (let i = 0; i < AstTypeName.length; i++) {
  AstType.set(AstTypeName[i], i);
}

// Keep in sync with Rust
const AstPropArr = [
  // Base
  "parent",
  "range",
  "type",
  "_InternalFlags",

  // Node
  "abstract",
  "accessibility",
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
  "checkType",
  "closingElement",
  "closingFragment",
  "computed",
  "consequent",
  "const",
  "constraint",
  "cooked",
  "declarations",
  "declare",
  "default",
  "definite",
  "delegate",
  "discriminant",
  "elements",
  "elementTypes",
  "exprName",
  "expression",
  "expressions",
  "exported",
  "extendsType",
  "falseType",
  "finalizer",
  "flags",
  "generator",
  "handler",
  "id",
  "in",
  "init",
  "initializer",
  "implements",
  "key",
  "kind",
  "label",
  "left",
  "literal",
  "local",
  "members",
  "meta",
  "method",
  "name",
  "namespace",
  "nameType",
  "object",
  "openingElement",
  "openingFragment",
  "operator",
  "optional",
  "out",
  "param",
  "params",
  "pattern",
  "prefix",
  "properties",
  "property",
  "quasi",
  "quasis",
  "raw",
  "readonly",
  "returnType",
  "right",
  "selfClosing",
  "shorthand",
  "source",
  "sourceType",
  "specifiers",
  "superClass",
  "superTypeArguments",
  "tag",
  "tail",
  "test",
  "trueType",
  "typeAnnotation",
  "typeArguments",
  "typeName",
  "typeParameter",
  "typeParameters",
  "types",
  "update",
  "value",
];

/** @type {Map<string, number>} */
const AstProp = new Map();
for (let i = 0; i < AstPropArr.length; i++) {
  AstProp.set(AstPropArr[i], i);
}

const AST_PROP_TYPE = /** @type {number} */ (AstProp.get("type"));
const AST_PROP_PARENT = /** @type {number} */ (AstProp.get("parent"));
const AST_PROP_RANGE = /** @type {number} */ (AstProp.get("range"));

const AstPropName = Array.from(AstProp.keys());

// Keep in sync with Rust
/** @enum {number} */
const PropFlags = {
  Ref: 0,
  RefArr: 1,
  String: 2,
  Bool: 3,
  Null: 4,
  Undefined: 5,
};

/**
 * @typedef {{
 *   buf: Uint8Array,
 *   strTable: Map<number, string>,
 *   strTableOffset: number,
 *   rootId: number,
 *   nodes: Map<number, Node>
 * }} AstContext
 */

/**
 * @typedef {{
 *   plugins: Deno.LintPlugin[],
 *   installedPlugins: Set<string>,
 * }} LintState
 */

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

/**
 * @param {AstContext} ctx
 * @param {number} offset
 * @returns
 */
function getNode(ctx, offset) {
  if (offset === 0) return null;

  const cached = ctx.nodes.get(offset);
  if (cached !== undefined) return cached;

  const node = new Node(ctx, offset);
  ctx.nodes.set(offset, /** @type {*} */ (cached));
  return node;
}

/**
 * @param {Uint8Array} buf
 * @param {number} search
 * @param {number} offset
 * @returns {number}
 */
function findPropOffset(buf, offset, search) {
  // type + parentId + SpanLo + SpanHi
  offset += 1 + 4 + 4 + 4;

  const propCount = buf[offset];
  offset += 1;

  for (let i = 0; i < propCount; i++) {
    const maybe = offset;
    const prop = buf[offset++];
    const kind = buf[offset++];
    if (prop === search) return maybe;

    if (kind === PropFlags.Ref) {
      offset += 4;
    } else if (kind === PropFlags.RefArr) {
      const len = readU32(buf, offset);
      offset += 4 + (len * 4);
    } else if (kind === PropFlags.String) {
      offset += 4;
    } else if (kind === PropFlags.Bool) {
      offset++;
    } else if (kind === PropFlags.Null || kind === PropFlags.Undefined) {
      // No value
    } else {
      offset++;
    }
  }

  return -1;
}

/**
 * @param {AstContext} ctx
 * @param {number} offset
 * @param {number} search
 * @returns {*}
 */
function readValue(ctx, offset, search) {
  const { buf } = ctx;
  const type = buf[offset];

  if (search === AST_PROP_TYPE) {
    return AstTypeName[type];
  } else if (search === AST_PROP_RANGE) {
    const start = readU32(buf, offset + 1 + 4);
    const end = readU32(buf, offset + 1 + 4 + 4);
    return [start, end];
  } else if (search === AST_PROP_PARENT) {
    const pos = readU32(buf, offset + 1);
    return getNode(ctx, pos);
  }

  offset = findPropOffset(ctx.buf, offset, search);
  if (offset === -1) return undefined;

  const kind = buf[offset + 1];

  if (kind === PropFlags.Ref) {
    const value = readU32(buf, offset + 2);
    return getNode(ctx, value);
  } else if (kind === PropFlags.RefArr) {
    const len = readU32(buf, offset);
    offset += 4;

    const nodes = new Array(len);
    for (let i = 0; i < len; i++) {
      nodes[i] = getNode(ctx, readU32(buf, offset));
      offset += 4;
    }
    return nodes;
  } else if (kind === PropFlags.Bool) {
    return buf[offset] === 1;
  } else if (kind === PropFlags.String) {
    const v = readU32(buf, offset);
    return getString(ctx, v);
  } else if (kind === PropFlags.Null) {
    return null;
  } else if (kind === PropFlags.Undefined) {
    return undefined;
  }

  throw new Error(`Unknown prop kind: ${kind}`);
}

/**
 * @param {AstContext} ctx
 * @param {number} offset
 * @returns {*}
 */
function toJsValue(ctx, offset) {
  const { buf } = ctx;

  /** @type {Record<string, any>} */
  const node = {
    type: readValue(ctx, offset, AST_PROP_TYPE),
    range: readValue(ctx, offset, AST_PROP_RANGE),
  };

  // type + parentId + SpanLo + SpanHi
  offset += 1 + 4 + 4 + 4;

  const count = buf[offset++];
  for (let i = 0; i < count; i++) {
    const prop = buf[offset++];
    const kind = buf[offset++];
    const name = AstPropName[prop];

    if (kind === PropFlags.Ref) {
      const v = readU32(buf, offset);
      offset += 4;
      node[name] = v === 0 ? null : toJsValue(ctx, v);
    } else if (kind === PropFlags.RefArr) {
      const len = readU32(buf, offset);
      offset += 4;
      const nodes = new Array(len);
      for (let i = 0; i < len; i++) {
        const v = readU32(buf, offset);
        if (v === 0) continue;
        nodes[i] = toJsValue(ctx, v);
        offset += 4;
      }
      node[name] = nodes;
    } else if (kind === PropFlags.Bool) {
      const v = buf[offset++];
      node[name] = v === 1;
    } else if (kind === PropFlags.String) {
      const v = readU32(buf, offset);
      offset += 4;
      node[name] = getString(ctx, v);
    } else if (kind === PropFlags.Null) {
      node[name] = null;
    } else if (kind === PropFlags.Undefined) {
      node[name] = undefined;
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
    const json = toJsValue(this[INTERNAL_CTX], this[INTERNAL_OFFSET]);
    return Deno.inspect(json, options);
  }
}

for (let i = 0; i < AstPropName.length; i++) {
  const name = AstPropName[i];
  Object.defineProperty(Node.prototype, name, {
    get() {
      return readValue(this[INTERNAL_CTX], this[INTERNAL_OFFSET], i);
    },
  });
}

const DECODER = new TextDecoder();

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

  const strTableOffset = readU32(buf, buf.length - 8);
  const rootId = readU32(buf, buf.length - 4);
  // console.log({ strTableOffset, rootId });

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

  // console.log({ stringCount, strTable, rootId });

  if (strTable.size !== stringCount) {
    throw new Error(
      `Could not deserialize string table. Expected ${stringCount} items, but got ${strTable.size}`,
    );
  }

  /** @type {AstContext} */
  const ctx = { buf, strTable, rootId, nodes: new Map(), strTableOffset };

  // dump(ctx);

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
    ctx.nodes.clear();

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
    const id = AstType.get(name);
    visitTypes.set(id, name);
  }

  // console.log("buffer len", ctx.buf.length, ctx.buf.byteLength);
  console.log("merged visitor", visitor);
  console.log("visiting types", visitTypes);

  traverseInner(ctx, visitTypes, visitor, ctx.rootId);
}

/**
 * @param {AstContext} ctx
 * @param {Map<number, string>} visitTypes
 * @param {Record<string, (x: any) => void>} visitor
 * @param {number} offset
 */
function traverseInner(ctx, visitTypes, visitor, offset) {
  // console.log("traversing offset", offset);

  // Empty id
  if (offset === 0) return;
  const { buf } = ctx;
  const type = buf[offset];

  const name = visitTypes.get(type);
  if (name !== undefined) {
    // console.log("--> invoking visitor");
    const node = new Node(ctx, offset);
    visitor[name](node);
  }

  // type + parentId + SpanLo + SpanHi
  offset += 1 + 4 + 4 + 4;

  const propCount = buf[offset];
  offset += 1;
  // console.log({ propCount });

  for (let i = 0; i < propCount; i++) {
    const kind = buf[offset + 1];
    offset += 2; // propId + propFlags

    if (kind === PropFlags.Ref) {
      const next = readU32(buf, offset);
      offset += 4;
      traverseInner(ctx, visitTypes, visitor, next);
    } else if (kind === PropFlags.RefArr) {
      const len = readU32(buf, offset);
      offset += 4;

      for (let j = 0; j < len; j++) {
        const chiild = readU32(buf, offset);
        offset += 4;
        traverseInner(ctx, visitTypes, visitor, chiild);
      }
    } else if (kind === PropFlags.String) {
      offset += 4;
    } else if (kind === PropFlags.Bool) {
      offset += 1;
    } else if (kind === PropFlags.Null || kind === PropFlags.Undefined) {
      // No value
    }
  }
}

/**
 * @param {AstContext} ctx
 */
function _dump(ctx) {
  const { buf, strTableOffset } = ctx;

  let offset = 0;

  while (offset < strTableOffset) {
    const type = buf[offset];
    const name = AstTypeName[type];
    // @ts-ignore dump fn
    console.log(`${name}, offset: ${offset}, type: ${type}`);
    offset += 1;

    const parent = readU32(buf, offset);
    offset += 4;
    // @ts-ignore dump fn
    console.log(`  parent: ${parent}`);

    const start = readU32(buf, offset);
    offset += 4;
    const end = readU32(buf, offset);
    offset += 4;
    // @ts-ignore dump fn
    console.log(`  range: ${start} -> ${end}}`);

    const count = buf[offset++];
    // @ts-ignore dump fn
    console.log(`  prop count: ${count}`);

    for (let i = 0; i < count; i++) {
      const prop = buf[offset++];
      const kind = buf[offset++];
      const name = AstPropName[prop];

      let kindName = "unknown";
      for (const k in PropFlags) {
        // @ts-ignore dump fn
        if (kind === PropFlags[k]) {
          kindName = k;
        }
      }

      if (kind === PropFlags.Ref) {
        const v = readU32(buf, offset);
        offset += 4;
        // @ts-ignore dump fn
        console.log(`    ${name}: ${v} (${kindName})`);
      } else if (kind === PropFlags.RefArr) {
        const len = readU32(buf, offset);
        offset += 4;
        // @ts-ignore dump fn
        console.log(`    ${name}: Array(${len}) (${kindName})`);

        for (let j = 0; j < len; j++) {
          const v = readU32(buf, offset);
          offset += 4;
          // @ts-ignore dump fn
          console.log(`      - ${v}`);
        }
      } else if (kind === PropFlags.Bool) {
        const v = buf[offset];
        offset += 1;
        // @ts-ignore dump fn
        console.log(`    ${name}: ${v} (${kindName})`);
      } else if (kind === PropFlags.String) {
        const v = readU32(buf, offset);
        offset += 4;
        // @ts-ignore dump fn
        console.log(`    ${name}: ${getString(ctx, v)} (${kindName})`);
      } else if (kind === PropFlags.Null) {
        // @ts-ignore dump fn
        console.log(`    ${name}: null (${kindName})`);
      } else if (kind === PropFlags.Undefined) {
        // @ts-ignore dump fn
        console.log(`    ${name}: undefined (${kindName})`);
      }
    }
  }
}
