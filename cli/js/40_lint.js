// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check

import {
  compileSelector,
  parseSelector,
  splitSelectors,
} from "ext:cli/40_lint_selector.js";
import { core, internals } from "ext:core/mod.js";
const {
  op_lint_create_serialized_ast,
} = core.ops;

// Keep in sync with Rust
// These types are expected to be present on every node. Note that this
// isn't set in stone. We could revise this at a future point.
const AST_PROP_TYPE = 1;
const AST_PROP_PARENT = 2;
const AST_PROP_RANGE = 3;
const AST_PROP_LENGTH = 4;

// Keep in sync with Rust
// Each node property is tagged with this enum to denote
// what kind of value it holds.
/** @enum {number} */
const PropFlags = {
  /** This is an offset to another node */
  Ref: 0,
  /** This is an array of offsets to other nodes (like children of a BlockStatement) */
  RefArr: 1,
  /**
   * This is a string id. The actual string needs to be looked up in
   * the string table that was included in the message.
   */
  String: 2,
  /** This value is either 0 = false, or 1 = true */
  Bool: 3,
  /** No value, it's null */
  Null: 4,
  /** No value, it's undefined */
  Undefined: 5,
};

/** @typedef {import("./40_lint_types.d.ts").AstContext} AstContext */
/** @typedef {import("./40_lint_types.d.ts").VisitorFn} VisitorFn */
/** @typedef {import("./40_lint_types.d.ts").CompiledVisitor} CompiledVisitor */
/** @typedef {import("./40_lint_types.d.ts").LintState} LintState */
/** @typedef {import("./40_lint_types.d.ts").RuleContext} RuleContext */
/** @typedef {import("./40_lint_types.d.ts").NodeFacade} NodeFacade */
/** @typedef {import("./40_lint_types.d.ts").LintPlugin} LintPlugin */
/** @typedef {import("./40_lint_types.d.ts").TransformFn} TransformFn */
/** @typedef {import("./40_lint_types.d.ts").MatchContext} MatchContext */

/** @type {LintState} */
const state = {
  plugins: [],
  installedPlugins: new Set(),
};

/**
 * Every rule gets their own instance of this class. This is the main
 * API lint rules interact with.
 * @implements {RuleContext}
 */
export class Context {
  id;

  fileName;

  /**
   * @param {string} id
   * @param {string} fileName
   */
  constructor(id, fileName) {
    this.id = id;
    this.fileName = fileName;
  }
}

/**
 * @param {LintPlugin} plugin
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
 * Find the offset of a specific property of a specific node. This will
 * be used later a lot more for selectors.
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

const INTERNAL_CTX = Symbol("ctx");
const INTERNAL_OFFSET = Symbol("offset");

// This class is a facade for all materialized nodes. Instead of creating a
// unique class per AST node, we have one class with getters for every
// possible node property. This allows us to lazily materialize child node
// only when they are needed.
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
   * Logging a class with only getters prints just the class name. This
   * makes debugging difficult because you don't see any of the properties.
   * For that reason we'll intercept inspection and serialize the node to
   * a plain JSON structure which can be logged and allows users to see all
   * properties and their values.
   *
   * This is only expected to be used during development of a rule.
   * @param {*} _
   * @param {Deno.InspectOptions} options
   * @returns {string}
   */
  [Symbol.for("Deno.customInspect")](_, options) {
    const json = toJsValue(this[INTERNAL_CTX], this[INTERNAL_OFFSET]);
    return Deno.inspect(json, options);
  }

  [Symbol.for("Deno.lint.toJsValue")]() {
    return toJsValue(this[INTERNAL_CTX], this[INTERNAL_OFFSET]);
  }
}

/** @type {Set<number>} */
const appliedGetters = new Set();

/**
 * Add getters for all potential properties found in the message.
 * @param {AstContext} ctx
 */
function setNodeGetters(ctx) {
  if (appliedGetters.size === ctx.strByProp.length) return;

  for (let i = 0; i < ctx.strByProp.length; i++) {
    const id = ctx.strByProp[i];
    if (id === 0 || appliedGetters.has(i)) continue;
    appliedGetters.add(i);

    const name = getString(ctx.strTable, id);

    Object.defineProperty(Node.prototype, name, {
      get() {
        return readValue(this[INTERNAL_CTX], this[INTERNAL_OFFSET], i);
      },
    });
  }
}

/**
 * Serialize a node recursively to plain JSON
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
    const name = getString(ctx.strTable, ctx.strByProp[prop]);

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
      node[name] = getString(ctx.strTable, v);
    } else if (kind === PropFlags.Null) {
      node[name] = null;
    } else if (kind === PropFlags.Undefined) {
      node[name] = undefined;
    }
  }

  return node;
}

/**
 * Read a specific property from a node
 * @param {AstContext} ctx
 * @param {number} offset
 * @param {number} search
 * @returns {*}
 */
function readValue(ctx, offset, search) {
  const { buf } = ctx;
  const type = buf[offset];

  if (search === AST_PROP_TYPE) {
    return getString(ctx.strTable, ctx.strByType[type]);
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
  offset += 2;

  if (kind === PropFlags.Ref) {
    const value = readU32(buf, offset);
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
    return getString(ctx.strTable, v);
  } else if (kind === PropFlags.Null) {
    return null;
  } else if (kind === PropFlags.Undefined) {
    return undefined;
  }

  throw new Error(`Unknown prop kind: ${kind}`);
}

const DECODER = new TextDecoder();

/**
 * TODO: Check if it's faster to use the `ArrayView` API instead.
 * @param {Uint8Array} buf
 * @param {number} i
 * @returns {number}
 */
function readU32(buf, i) {
  return (buf[i] << 24) + (buf[i + 1] << 16) + (buf[i + 2] << 8) +
    buf[i + 3];
}

/**
 * Get a string by id and error if it wasn't found
 * @param {AstContext["strTable"]} strTable
 * @param {number} id
 * @returns {string}
 */
function getString(strTable, id) {
  const name = strTable.get(id);
  if (name === undefined) {
    throw new Error(`Missing string id: ${id}`);
  }

  return name;
}

/**
 * @param {AstContext["buf"]} buf
 * @param {number} child
 * @returns {null | [number, number]}
 */
function findChildOffset(buf, child) {
  let offset = readU32(buf, child + 1);

  // type + parentId + SpanLo + SpanHi
  offset += 1 + 4 + 4 + 4;

  const propCount = buf[offset++];
  for (let i = 0; i < propCount; i++) {
    const _prop = buf[offset++];
    const kind = buf[offset++];

    switch (kind) {
      case PropFlags.Ref: {
        const start = offset;
        const value = readU32(buf, offset);
        offset += 4;
        if (value === child) {
          return [start, -1];
        }
        break;
      }
      case PropFlags.RefArr: {
        const start = offset;

        const len = readU32(buf, offset);
        offset += 4;

        for (let j = 0; j < len; j++) {
          const value = readU32(buf, offset);
          offset += 4;
          if (value === child) {
            return [start, j];
          }
        }

        break;
      }
      case PropFlags.String:
        offset += 4;
        break;
      case PropFlags.Bool:
        offset++;
        break;
      case PropFlags.Null:
      case PropFlags.Undefined:
        break;
    }
  }

  return null;
}

/** @implements {MatchContext} */
class MatchCtx {
  /**
   * @param {AstContext["buf"]} buf
   * @param {AstContext["strTable"]} strTable
   * @param {AstContext["strByType"]} strByType
   */
  constructor(buf, strTable, strByType) {
    this.buf = buf;
    this.strTable = strTable;
    this.strByType = strByType;
  }

  /**
   * @param {number} offset
   * @returns {number}
   */
  getParent(offset) {
    return readU32(this.buf, offset + 1);
  }

  /**
   * @param {number} offset
   * @returns {number}
   */
  getType(offset) {
    return this.buf[offset];
  }

  /**
   * @param {number} offset
   * @param {number[]} propIds
   * @param {number} idx
   * @returns {unknown}
   */
  getAttrPathValue(offset, propIds, idx) {
    const { buf } = this;

    const propId = propIds[idx];

    switch (propId) {
      case AST_PROP_TYPE: {
        const type = this.getType(offset);
        return getString(this.strTable, this.strByType[type]);
      }
      case AST_PROP_PARENT:
      case AST_PROP_RANGE:
        throw new Error(`Not supported`);
    }

    offset = findPropOffset(buf, offset, propId);
    if (offset === -1) return undefined;
    const _prop = buf[offset++];
    const kind = buf[offset++];

    if (kind === PropFlags.Ref) {
      const value = readU32(buf, offset);
      // Checks need to end with a value, not a node
      if (idx === propIds.length - 1) return undefined;
      return this.getAttrPathValue(value, propIds, idx + 1);
    } else if (kind === PropFlags.RefArr) {
      const count = readU32(buf, offset);
      offset += 4;

      if (idx < propIds.length - 1 && propIds[idx + 1] === AST_PROP_LENGTH) {
        return count;
      }

      // TODO(@marvinhagemeister): Allow traversing into array children?
    }

    // Cannot traverse into primitives further
    if (idx < propIds.length - 1) return undefined;

    if (kind === PropFlags.String) {
      const s = readU32(buf, offset);
      return getString(this.strTable, s);
    } else if (kind === PropFlags.Bool) {
      return buf[offset] === 1;
    } else if (kind === PropFlags.Null) {
      return null;
    } else if (kind === PropFlags.Undefined) {
      return undefined;
    }

    return undefined;
  }

  /**
   * @param {number} offset
   * @param {number[]} propIds
   * @param {number} idx
   * @returns {boolean}
   */
  hasAttrPath(offset, propIds, idx) {
    const { buf } = this;

    const propId = propIds[idx];
    // If propId is 0 then the property doesn't exist in the AST
    if (propId === 0) return false;

    switch (propId) {
      case AST_PROP_TYPE:
      case AST_PROP_PARENT:
      case AST_PROP_RANGE:
        return true;
    }

    offset = findPropOffset(buf, offset, propId);
    if (offset === -1) return false;
    if (idx === propIds.length - 1) return true;

    const _prop = buf[offset++];
    const kind = buf[offset++];
    if (kind === PropFlags.Ref) {
      const value = readU32(buf, offset);
      return this.hasAttrPath(value, propIds, idx + 1);
    } else if (kind === PropFlags.RefArr) {
      const _count = readU32(buf, offset);
      offset += 4;

      if (idx < propIds.length - 1 && propIds[idx + 1] === AST_PROP_LENGTH) {
        return true;
      }

      // TODO(@marvinhagemeister): Allow traversing into array children?
    }

    // Primitives cannot be traversed further. This means we
    // didn't found the attribute.
    if (idx < propIds.length - 1) return false;

    return true;
  }

  /**
   * @param {number} offset
   * @returns {number}
   */
  getFirstChild(offset) {
    const { buf } = this;

    // type + parentId + SpanLo + SpanHi
    offset += 1 + 4 + 4 + 4;

    const count = buf[offset++];
    for (let i = 0; i < count; i++) {
      const _prop = buf[offset++];
      const kind = buf[offset++];

      switch (kind) {
        case PropFlags.Ref: {
          const v = readU32(buf, offset);
          offset += 4;
          return v;
        }
        case PropFlags.RefArr: {
          const len = readU32(buf, offset);
          offset += 4;
          for (let j = 0; j < len; j++) {
            const v = readU32(buf, offset);
            offset += 4;
            return v;
          }

          return len;
        }

        case PropFlags.String:
          offset += 4;
          break;
        case PropFlags.Bool:
          offset++;
          break;
        case PropFlags.Null:
        case PropFlags.Undefined:
          break;
      }
    }

    return -1;
  }

  /**
   * @param {number} offset
   * @returns {number}
   */
  getLastChild(offset) {
    const { buf } = this;

    // type + parentId + SpanLo + SpanHi
    offset += 1 + 4 + 4 + 4;

    let last = -1;

    const count = buf[offset++];
    for (let i = 0; i < count; i++) {
      const _prop = buf[offset++];
      const kind = buf[offset++];

      switch (kind) {
        case PropFlags.Ref: {
          const v = readU32(buf, offset);
          offset += 4;
          last = v;
          break;
        }
        case PropFlags.RefArr: {
          const len = readU32(buf, offset);
          offset += 4;
          for (let j = 0; j < len; j++) {
            const v = readU32(buf, offset);
            last = v;
            offset += 4;
          }

          break;
        }

        case PropFlags.String:
          offset += 4;
          break;
        case PropFlags.Bool:
          offset++;
          break;
        case PropFlags.Null:
        case PropFlags.Undefined:
          break;
      }
    }

    return last;
  }

  /**
   * @param {number} id
   * @returns {number[]}
   */
  getSiblings(id) {
    const { buf } = this;

    const result = findChildOffset(buf, id);
    // Happens for program nodes
    if (result === null) return [];

    if (result[1] === -1) {
      return [id];
    }

    let offset = result[0];
    const count = readU32(buf, offset);
    offset += 4;

    /** @type {number[]} */
    const out = [];
    for (let i = 0; i < count; i++) {
      const v = readU32(buf, offset);
      offset += 4;
      out.push(v);
    }

    return out;
  }
}

/**
 * @param {Uint8Array} buf
 * @param {AstContext} buf
 */
function createAstContext(buf) {
  /** @type {Map<number, string>} */
  const strTable = new Map();

  // The buffer has a few offsets at the end which allows us to easily
  // jump to the relevant sections of the message.
  const typeMapOffset = readU32(buf, buf.length - 16);
  const propMapOffset = readU32(buf, buf.length - 12);
  const strTableOffset = readU32(buf, buf.length - 8);

  // Offset of the topmost node in the AST Tree.
  const rootOffset = readU32(buf, buf.length - 4);

  let offset = strTableOffset;
  const stringCount = readU32(buf, offset);
  offset += 4;

  // TODO(@marvinhagemeister): We could lazily decode the strings on an as needed basis.
  // Not sure if this matters much in practice though.
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

  if (strTable.size !== stringCount) {
    throw new Error(
      `Could not deserialize string table. Expected ${stringCount} items, but got ${strTable.size}`,
    );
  }

  offset = typeMapOffset;
  const typeCount = readU32(buf, offset);
  offset += 4;

  const typeByStr = new Map();
  const strByType = new Array(typeCount).fill(0);
  for (let i = 0; i < typeCount; i++) {
    const v = readU32(buf, offset);
    offset += 4;

    strByType[i] = v;
    typeByStr.set(strTable.get(v), i);
  }

  offset = propMapOffset;
  const propCount = readU32(buf, offset);
  offset += 4;

  const propByStr = new Map();
  const strByProp = new Array(propCount).fill(0);
  for (let i = 0; i < propCount; i++) {
    const v = readU32(buf, offset);
    offset += 4;

    strByProp[i] = v;
    propByStr.set(strTable.get(v), i);
  }

  /** @type {AstContext} */
  const ctx = {
    buf,
    strTable,
    rootOffset,
    nodes: new Map(),
    strTableOffset,
    strByProp,
    strByType,
    typeByStr,
    propByStr,
    matcher: new MatchCtx(buf, strTable, strByType),
  };

  setNodeGetters(ctx);

  // DEV ONLY: Enable this to inspect the buffer message
  // _dump(ctx);

  return ctx;
}

/**
 * @param {*} _node
 */
const NOOP = (_node) => {};

/**
 * Kick off the actual linting process of JS plugins.
 * @param {string} fileName
 * @param {Uint8Array} serializedAst
 */
export function runPluginsForFile(fileName, serializedAst) {
  const ctx = createAstContext(serializedAst);

  /** @type {Map<string, CompiledVisitor["info"]>}>} */
  const bySelector = new Map();

  const destroyFns = [];

  // Instantiate and merge visitors. This allows us to only traverse
  // the AST once instead of per plugin. When ever we enter or exit a
  // node we'll call all visitors that match.
  for (let i = 0; i < state.plugins.length; i++) {
    const plugin = state.plugins[i];

    for (const name of Object.keys(plugin.rules)) {
      const rule = plugin.rules[name];
      const id = `${plugin.name}/${name}`;
      const ctx = new Context(id, fileName);
      const visitor = rule.create(ctx);

      // deno-lint-ignore guard-for-in
      for (let key in visitor) {
        const fn = visitor[key];
        if (fn === undefined) continue;

        // Support enter and exit callbacks on a visitor.
        // Exit callbacks are marked by having `:exit` at the end.
        let isExit = false;
        if (key.endsWith(":exit")) {
          isExit = true;
          key = key.slice(0, -":exit".length);
        }

        const selectors = splitSelectors(key);

        for (let j = 0; j < selectors.length; j++) {
          const key = selectors[j];

          let info = bySelector.get(key);
          if (info === undefined) {
            info = { enter: NOOP, exit: NOOP };
            bySelector.set(key, info);
          }
          const prevFn = isExit ? info.exit : info.enter;

          /**
           * @param {*} node
           */
          const wrapped = (node) => {
            prevFn(node);

            try {
              fn(node);
            } catch (err) {
              throw new Error(`Visitor "${name}" of plugin "${id}" errored`, {
                cause: err,
              });
            }
          };

          if (isExit) {
            info.exit = wrapped;
          } else {
            info.enter = wrapped;
          }
        }
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

  // Create selectors
  /** @type {TransformFn} */
  const toElem = (str) => {
    const id = ctx.typeByStr.get(str);
    return id === undefined ? 0 : id;
  };
  /** @type {TransformFn} */
  const toAttr = (str) => {
    const id = ctx.propByStr.get(str);
    return id === undefined ? 0 : id;
  };

  /** @type {CompiledVisitor[]} */
  const visitors = [];
  for (const [sel, info] of bySelector.entries()) {
    // Selectors are already split here.
    // TODO(@marvinhagemeister): Avoid array allocation (not sure if that matters)
    const parsed = parseSelector(sel, toElem, toAttr)[0];
    const matcher = compileSelector(parsed);

    visitors.push({ info, matcher });
  }

  // Traverse ast with all visitors at the same time to avoid traversing
  // multiple times.
  try {
    traverse(ctx, visitors, ctx.rootOffset);
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
 * @param {CompiledVisitor[]} visitors
 * @param {number} offset
 */
function traverse(ctx, visitors, offset) {
  // The 0 offset is used to denote an empty/placeholder node
  if (offset === 0) return;

  const originalOffset = offset;

  const { buf } = ctx;

  /** @type {VisitorFn[] | null} */
  let exits = null;

  for (let i = 0; i < visitors.length; i++) {
    const v = visitors[i];

    if (v.matcher(ctx.matcher, offset)) {
      if (v.info.exit !== NOOP) {
        if (exits === null) {
          exits = [v.info.exit];
        } else {
          exits.push(v.info.exit);
        }
      }

      if (v.info.enter !== NOOP) {
        const node = /** @type {*} */ (getNode(ctx, offset));
        v.info.enter(node);
      }
    }
  }

  // Search for node references in the properties of the current node. All
  // other properties can be ignored.
  try {
    // type + parentId + SpanLo + SpanHi
    offset += 1 + 4 + 4 + 4;

    const propCount = buf[offset];
    offset += 1;

    for (let i = 0; i < propCount; i++) {
      const kind = buf[offset + 1];
      offset += 2; // propId + propFlags

      if (kind === PropFlags.Ref) {
        const next = readU32(buf, offset);
        offset += 4;
        traverse(ctx, visitors, next);
      } else if (kind === PropFlags.RefArr) {
        const len = readU32(buf, offset);
        offset += 4;

        for (let j = 0; j < len; j++) {
          const child = readU32(buf, offset);
          offset += 4;
          traverse(ctx, visitors, child);
        }
      } else if (kind === PropFlags.String) {
        offset += 4;
      } else if (kind === PropFlags.Bool) {
        offset += 1;
      } else if (kind === PropFlags.Null || kind === PropFlags.Undefined) {
        // No value
      }
    }
  } finally {
    if (exits !== null) {
      for (let i = 0; i < exits.length; i++) {
        const node = /** @type {*} */ (getNode(ctx, originalOffset));
        exits[i](node);
      }
    }
  }
}

/**
 * This is useful debugging helper to display the buffer's contents.
 * @param {AstContext} ctx
 */
function _dump(ctx) {
  const { buf, strTableOffset, strTable, strByType, strByProp } = ctx;

  // @ts-ignore dump fn
  // deno-lint-ignore no-console
  console.log(strTable);

  for (let i = 0; i < strByType.length; i++) {
    const v = strByType[i];
    // @ts-ignore dump fn
    // deno-lint-ignore no-console
    if (v > 0) console.log(" > type:", i, getString(ctx.strTable, v), v);
  }
  // @ts-ignore dump fn
  // deno-lint-ignore no-console
  console.log();
  for (let i = 0; i < strByProp.length; i++) {
    const v = strByProp[i];
    // @ts-ignore dump fn
    // deno-lint-ignore no-console
    if (v > 0) console.log(" > prop:", i, getString(ctx.strTable, v), v);
  }
  // @ts-ignore dump fn
  // deno-lint-ignore no-console
  console.log();

  let offset = 0;

  while (offset < strTableOffset) {
    const type = buf[offset];
    const name = getString(ctx.strTable, ctx.strByType[type]);
    // @ts-ignore dump fn
    // deno-lint-ignore no-console
    console.log(`${name}, offset: ${offset}, type: ${type}`);
    offset += 1;

    const parent = readU32(buf, offset);
    offset += 4;
    // @ts-ignore dump fn
    // deno-lint-ignore no-console
    console.log(`  parent: ${parent}`);

    const start = readU32(buf, offset);
    offset += 4;
    const end = readU32(buf, offset);
    offset += 4;
    // @ts-ignore dump fn
    // deno-lint-ignore no-console
    console.log(`  range: ${start} -> ${end}`);

    const count = buf[offset++];
    // @ts-ignore dump fn
    // deno-lint-ignore no-console
    console.log(`  prop count: ${count}`);

    for (let i = 0; i < count; i++) {
      const prop = buf[offset++];
      const kind = buf[offset++];
      const name = getString(ctx.strTable, ctx.strByProp[prop]);

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
        // deno-lint-ignore no-console
        console.log(`    ${name}: ${v} (${kindName}, ${prop})`);
      } else if (kind === PropFlags.RefArr) {
        const len = readU32(buf, offset);
        offset += 4;
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(`    ${name}: Array(${len}) (${kindName}, ${prop})`);

        for (let j = 0; j < len; j++) {
          const v = readU32(buf, offset);
          offset += 4;
          // @ts-ignore dump fn
          // deno-lint-ignore no-console
          console.log(`      - ${v} (${prop})`);
        }
      } else if (kind === PropFlags.Bool) {
        const v = buf[offset];
        offset += 1;
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(`    ${name}: ${v} (${kindName}, ${prop})`);
      } else if (kind === PropFlags.String) {
        const v = readU32(buf, offset);
        offset += 4;
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(
          `    ${name}: ${getString(ctx.strTable, v)} (${kindName}, ${prop})`,
        );
      } else if (kind === PropFlags.Null) {
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(`    ${name}: null (${kindName}, ${prop})`);
      } else if (kind === PropFlags.Undefined) {
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(`    ${name}: undefined (${kindName}, ${prop})`);
      }
    }
  }
}

// TODO(bartlomieju): this is temporary, until we get plugins plumbed through
// the CLI linter
/**
 * @param {LintPlugin} plugin
 * @param {string} fileName
 * @param {string} sourceText
 */
function runLintPlugin(plugin, fileName, sourceText) {
  installPlugin(plugin);
  const serializedAst = op_lint_create_serialized_ast(fileName, sourceText);

  try {
    runPluginsForFile(fileName, serializedAst);
  } finally {
    // During testing we don't want to keep plugins around
    state.installedPlugins.clear();
  }
}

// TODO(bartlomieju): this is temporary, until we get plugins plumbed through
// the CLI linter
internals.runLintPlugin = runLintPlugin;
