// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check

import {
  compileSelector,
  parseSelector,
  splitSelectors,
} from "ext:cli/40_lint_selector.js";
import { core, internals } from "ext:core/mod.js";

const {
  op_lint_get_source,
  op_lint_report,
  op_lint_create_serialized_ast,
  op_is_cancelled,
} = core.ops;

/** @type {(id: string, message: string, hint: string | undefined, start: number, end: number, fix: Deno.lint.Fix[]) => void} */
let doReport = op_lint_report;
/** @type {() => string} */
let doGetSource = op_lint_get_source;

// Keep these in sync with Rust
const AST_IDX_INVALID = 0;
const AST_GROUP_TYPE = 1;
/// <type u8>
/// <prop offset u32>
/// <child idx u32>
/// <next idx u32>
/// <parent idx u32>
const NODE_SIZE = 1 + 4 + 4 + 4 + 4;
const PROP_OFFSET = 1;
const CHILD_OFFSET = 1 + 4;
const NEXT_OFFSET = 1 + 4 + 4;
const PARENT_OFFSET = 1 + 4 + 4 + 4;
// Span size in buffer: u32 + u32
const SPAN_SIZE = 4 + 4;

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
  /**
   * A numnber field. Numbers are represented as strings internally.
   */
  Number: 3,
  /** This value is either 0 = false, or 1 = true */
  Bool: 4,
  /** No value, it's null */
  Null: 5,
  /** No value, it's undefined */
  Undefined: 6,
  /** An object */
  Obj: 7,
  Regex: 8,
  BigInt: 9,
  Array: 10,
};

/** @typedef {import("./40_lint_types.d.ts").AstContext} AstContext */
/** @typedef {import("./40_lint_types.d.ts").VisitorFn} VisitorFn */
/** @typedef {import("./40_lint_types.d.ts").CompiledVisitor} CompiledVisitor */
/** @typedef {import("./40_lint_types.d.ts").LintState} LintState */
/** @typedef {import("./40_lint_types.d.ts").TransformFn} TransformFn */
/** @typedef {import("./40_lint_types.d.ts").MatchContext} MatchContext */
/** @typedef {import("./40_lint_types.d.ts").MatcherFn} MatcherFn */

/** @type {LintState} */
const state = {
  plugins: [],
  installedPlugins: new Set(),
  ignoredRules: new Set(),
};

function resetState() {
  state.plugins = [];
  state.installedPlugins.clear();
  state.ignoredRules.clear();
}

/**
 * This implementation calls into Rust to check if Tokio's cancellation token
 * has already been canceled.
 */
class CancellationToken {
  isCancellationRequested() {
    return op_is_cancelled();
  }
}

/** @implements {Deno.lint.Fixer} */
class Fixer {
  /**
   * @param {Deno.lint.Node} node
   * @param {string} text
   */
  insertTextAfter(node, text) {
    return {
      range: /** @type {[number, number]} */ ([node.range[1], node.range[1]]),
      text,
    };
  }

  /**
   * @param {Deno.lint.Node["range"]} range
   * @param {string} text
   */
  insertTextAfterRange(range, text) {
    return {
      range: /** @type {[number, number]} */ ([range[1], range[1]]),
      text,
    };
  }

  /**
   * @param {Deno.lint.Node} node
   * @param {string} text
   */
  insertTextBefore(node, text) {
    return {
      range: /** @type {[number, number]} */ ([node.range[0], node.range[0]]),
      text,
    };
  }

  /**
   * @param {Deno.lint.Node["range"]} range
   * @param {string} text
   */
  insertTextBeforeRange(range, text) {
    return {
      range: /** @type {[number, number]} */ ([range[0], range[0]]),
      text,
    };
  }

  /**
   * @param {Deno.lint.Node} node
   */
  remove(node) {
    return {
      range: node.range,
      text: "",
    };
  }

  /**
   * @param {Deno.lint.Node["range"]} range
   */
  removeRange(range) {
    return {
      range,
      text: "",
    };
  }

  /**
   * @param {Deno.lint.Node} node
   * @param {string} text
   */
  replaceText(node, text) {
    return {
      range: node.range,
      text,
    };
  }

  /**
   * @param {Deno.lint.Node["range"]} range
   * @param {string} text
   */
  replaceTextRange(range, text) {
    return {
      range,
      text,
    };
  }
}

/**
 * @implements {Deno.lint.SourceCode}
 */
export class SourceCode {
  /** @type {string | null} */
  #source = null;

  /** @type {AstContext} */
  #ctx;

  /**
   * @param {AstContext} ctx
   */
  constructor(ctx) {
    this.#ctx = ctx;
  }

  get text() {
    return this.#getSource();
  }

  get ast() {
    const program = /** @type {*} */ (getNode(
      this.#ctx,
      this.#ctx.rootOffset,
    ));

    return program;
  }

  /**
   * @param {Deno.lint.Node} [node]
   * @returns {string}
   */
  getText(node) {
    const source = this.#getSource();
    if (node === undefined) {
      return source;
    }

    return source.slice(node.range[0], node.range[1]);
  }

  /**
   * @param {Deno.lint.Node} node
   */
  getAncestors(node) {
    const { buf } = this.#ctx;

    /** @type {Deno.lint.Node[]} */
    const ancestors = [];

    let parent = /** @type {*} */ (node)[INTERNAL_IDX];
    while ((parent = readParent(buf, parent)) > AST_IDX_INVALID) {
      if (readType(buf, parent) === AST_GROUP_TYPE) continue;

      const parentNode = /** @type {*} */ (getNode(this.#ctx, parent));
      if (parentNode !== null) {
        ancestors.push(parentNode);
      }
    }

    ancestors.reverse();

    return ancestors;
  }

  /**
   * @returns {string}
   */
  #getSource() {
    if (this.#source === null) {
      this.#source = doGetSource();
    }
    return /** @type {string} */ (this.#source);
  }
}

/**
 * Every rule gets their own instance of this class. This is the main
 * API lint rules interact with.
 * @implements {Deno.lint.RuleContext}
 */
export class Context {
  id;
  // ESLint uses lowercase
  filename;
  sourceCode;

  /**
   * @param {AstContext} ctx
   * @param {string} id
   * @param {string} fileName
   */
  constructor(ctx, id, fileName) {
    this.id = id;
    this.filename = fileName;
    this.sourceCode = new SourceCode(ctx);
  }

  getFilename() {
    return this.filename;
  }

  getSourceCode() {
    return this.sourceCode;
  }

  /**
   * @param {Deno.lint.ReportData} data
   */
  report(data) {
    const range = data.node ? data.node.range : data.range ? data.range : null;
    if (range == null) {
      throw new Error(
        "Either `node` or `range` must be provided when reporting an error",
      );
    }

    const start = range[0];
    const end = range[1];

    /** @type {Deno.lint.Fix[]} */
    const fixes = [];

    if (typeof data.fix === "function") {
      const fixer = new Fixer();
      const result = data.fix(fixer);

      if (Symbol.iterator in result) {
        for (const fix of result) {
          fixes.push(fix);
        }
      } else {
        fixes.push(result);
      }
    }

    doReport(
      this.id,
      data.message,
      data.hint,
      start,
      end,
      fixes,
    );
  }
}

/**
 * @param {Deno.lint.Plugin[]} plugins
 * @param {string[]} exclude
 */
export function installPlugins(plugins, exclude) {
  if (Array.isArray(exclude)) {
    for (let i = 0; i < exclude.length; i++) {
      state.ignoredRules.add(exclude[i]);
    }
  }

  return plugins.map((plugin) => installPlugin(plugin));
}

/**
 * @param {Deno.lint.Plugin} plugin
 */
function installPlugin(plugin) {
  if (typeof plugin !== "object") {
    throw new Error("Linter plugin must be an object");
  }
  if (typeof plugin.name !== "string") {
    throw new Error("Linter plugin name must be a string");
  }
  if (!/^[a-z-]+$/.test(plugin.name)) {
    throw new Error(
      "Linter plugin name must only contain lowercase letters (a-z) or hyphens (-).",
    );
  }
  if (plugin.name.startsWith("-") || plugin.name.endsWith("-")) {
    throw new Error(
      "Linter plugin name must start and end with a lowercase letter.",
    );
  }
  if (plugin.name.includes("--")) {
    throw new Error(
      "Linter plugin name must not have consequtive hyphens.",
    );
  }
  if (typeof plugin.rules !== "object") {
    throw new Error("Linter plugin rules must be an object");
  }
  if (state.installedPlugins.has(plugin.name)) {
    throw new Error(`Linter plugin ${plugin.name} has already been registered`);
  }
  state.plugins.push(plugin);
  state.installedPlugins.add(plugin.name);

  return {
    name: plugin.name,
    ruleNames: Object.keys(plugin.rules),
  };
}

/**
 * @param {AstContext} ctx
 * @param {number} idx
 * @returns {FacadeNode | null}
 */
function getNode(ctx, idx) {
  if (idx === AST_IDX_INVALID) return null;
  const cached = ctx.nodes.get(idx);
  if (cached !== undefined) return /** @type {*} */ (cached);

  const node = new FacadeNode(ctx, idx);
  ctx.nodes.set(idx, /** @type {*} */ (node));
  return /** @type {*} */ (node);
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
  const count = buf[offset];
  offset += 1;

  for (let i = 0; i < count; i++) {
    const maybe = offset;
    const prop = buf[offset++];
    const kind = buf[offset++];
    if (prop === search) return maybe;

    if (kind === PropFlags.Obj) {
      const len = readU32(buf, offset);
      offset += 4;
      // prop + kind + value
      offset += len * (1 + 1 + 4);
    } else {
      offset += 4;
    }
  }

  return -1;
}

const INTERNAL_CTX = Symbol("ctx");
const INTERNAL_IDX = Symbol("offset");

// This class is a facade for all materialized nodes. Instead of creating a
// unique class per AST node, we have one class with getters for every
// possible node property. This allows us to lazily materialize child node
// only when they are needed.
class FacadeNode {
  [INTERNAL_CTX];
  [INTERNAL_IDX];

  /**
   * @param {AstContext} ctx
   * @param {number} idx
   */
  constructor(ctx, idx) {
    this[INTERNAL_CTX] = ctx;
    this[INTERNAL_IDX] = idx;
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
    const json = nodeToJson(this[INTERNAL_CTX], this[INTERNAL_IDX]);
    return Deno.inspect(json, options);
  }

  [Symbol.for("Deno.lint.toJsValue")]() {
    return nodeToJson(this[INTERNAL_CTX], this[INTERNAL_IDX]);
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

    Object.defineProperty(FacadeNode.prototype, name, {
      get() {
        return readValue(
          this[INTERNAL_CTX],
          this[INTERNAL_IDX],
          i,
          getNode,
        );
      },
    });
  }
}

/**
 * @param {AstContext} ctx
 * @param {number} idx
 */
function nodeToJson(ctx, idx) {
  /** @type {Record<string, any>} */
  const node = {
    type: readValue(ctx, idx, AST_PROP_TYPE, nodeToJson),
    range: readValue(ctx, idx, AST_PROP_RANGE, nodeToJson),
  };

  const { buf } = ctx;
  let offset = readPropOffset(ctx, idx);

  const count = buf[offset++];

  for (let i = 0; i < count; i++) {
    const prop = buf[offset];
    const _kind = buf[offset + 1];

    const name = getString(ctx.strTable, ctx.strByProp[prop]);
    node[name] = readProperty(ctx, offset, nodeToJson);

    // prop + type + value
    offset += 1 + 1 + 4;
  }

  return node;
}

/**
 * @param {AstContext["buf"]} buf
 * @param {number} idx
 * @returns {number}
 */
function readType(buf, idx) {
  return buf[idx * NODE_SIZE];
}

/**
 * @param {AstContext} ctx
 * @param {number} idx
 * @returns {Deno.lint.Node["range"]}
 */
function readSpan(ctx, idx) {
  let offset = ctx.spansOffset + (idx * SPAN_SIZE);
  const start = readU32(ctx.buf, offset);
  offset += 4;
  const end = readU32(ctx.buf, offset);

  return [start, end];
}

/**
 * @param {AstContext["buf"]} buf
 * @param {number} idx
 * @returns {number}
 */
function readRawPropOffset(buf, idx) {
  const offset = (idx * NODE_SIZE) + PROP_OFFSET;
  return readU32(buf, offset);
}

/**
 * @param {AstContext} ctx
 * @param {number} idx
 * @returns {number}
 */
function readPropOffset(ctx, idx) {
  return readRawPropOffset(ctx.buf, idx) + ctx.propsOffset;
}

/**
 * @param {AstContext["buf"]} buf
 * @param {number} idx
 * @returns {number}
 */
function readChild(buf, idx) {
  const offset = (idx * NODE_SIZE) + CHILD_OFFSET;
  return readU32(buf, offset);
}
/**
 * @param {AstContext["buf"]} buf
 * @param {number} idx
 * @returns {number}
 */
function readNext(buf, idx) {
  const offset = (idx * NODE_SIZE) + NEXT_OFFSET;
  return readU32(buf, offset);
}

/**
 * @param {AstContext["buf"]} buf
 * @param {number} idx
 * @returns {number}
 */
function readParent(buf, idx) {
  const offset = (idx * NODE_SIZE) + PARENT_OFFSET;
  return readU32(buf, offset);
}

/**
 * @param {AstContext["strTable"]} strTable
 * @param {number} strId
 * @returns  {RegExp}
 */
function readRegex(strTable, strId) {
  const raw = getString(strTable, strId);
  const idx = raw.lastIndexOf("/");
  const pattern = raw.slice(1, idx);
  const flags = idx < raw.length - 1 ? raw.slice(idx + 1) : undefined;

  return new RegExp(pattern, flags);
}

/**
 * @param {AstContext} ctx
 * @param {number} offset
 * @param {(ctx: AstContext, idx: number) => any} parseNode
 * @returns {Record<string, any>}
 */
function readObject(ctx, offset, parseNode) {
  const { buf, strTable, strByProp } = ctx;

  /** @type {Record<string, any>} */
  const obj = {};

  const count = readU32(buf, offset);
  offset += 4;

  for (let i = 0; i < count; i++) {
    const prop = buf[offset];
    const name = getString(strTable, strByProp[prop]);
    obj[name] = readProperty(ctx, offset, parseNode);
    // name + kind + value
    offset += 1 + 1 + 4;
  }

  return obj;
}

/**
 * @param {AstContext} ctx
 * @param {number} offset
 * @param {(ctx: AstContext, idx: number) => any} parseNode
 * @returns {any}
 */
function readProperty(ctx, offset, parseNode) {
  const { buf } = ctx;

  // skip over name
  const _name = buf[offset++];
  const kind = buf[offset++];

  if (kind === PropFlags.Ref) {
    const value = readU32(buf, offset);
    return parseNode(ctx, value);
  } else if (kind === PropFlags.RefArr) {
    const groupId = readU32(buf, offset);

    const nodes = [];
    let next = readChild(buf, groupId);
    while (next > AST_IDX_INVALID) {
      nodes.push(parseNode(ctx, next));
      next = readNext(buf, next);
    }

    return nodes;
  } else if (kind === PropFlags.Bool) {
    const v = readU32(buf, offset);
    return v === 1;
  } else if (kind === PropFlags.String) {
    const v = readU32(buf, offset);
    return getString(ctx.strTable, v);
  } else if (kind === PropFlags.Number) {
    const v = readU32(buf, offset);
    return Number(getString(ctx.strTable, v));
  } else if (kind === PropFlags.BigInt) {
    const v = readU32(buf, offset);
    return BigInt(getString(ctx.strTable, v));
  } else if (kind === PropFlags.Regex) {
    const v = readU32(buf, offset);
    return readRegex(ctx.strTable, v);
  } else if (kind === PropFlags.Null) {
    return null;
  } else if (kind === PropFlags.Undefined) {
    return undefined;
  } else if (kind === PropFlags.Obj) {
    const objOffset = readU32(buf, offset) + ctx.propsOffset;
    return readObject(ctx, objOffset, parseNode);
  }

  throw new Error(`Unknown prop kind: ${kind}`);
}

/**
 * Read a specific property from a node
 * @param {AstContext} ctx
 * @param {number} idx
 * @param {number} search
 * @param {(ctx: AstContext, idx: number) => any} parseNode
 * @returns {*}
 */
function readValue(ctx, idx, search, parseNode) {
  const { buf } = ctx;

  if (search === AST_PROP_TYPE) {
    const type = readType(buf, idx);
    return getString(ctx.strTable, ctx.strByType[type]);
  } else if (search === AST_PROP_RANGE) {
    return readSpan(ctx, idx);
  } else if (search === AST_PROP_PARENT) {
    let parent = readParent(buf, idx);

    const parentType = readType(buf, parent);
    if (parentType === AST_GROUP_TYPE) {
      parent = readParent(buf, parent);
    }
    return getNode(ctx, parent);
  }

  const propOffset = readPropOffset(ctx, idx);

  const offset = findPropOffset(ctx.buf, propOffset, search);
  if (offset === -1) return undefined;

  return readProperty(ctx, offset, parseNode);
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

/** @implements {MatchContext} */
class MatchCtx {
  parentLimitIdx = 0;

  /**
   * @param {AstContext} ctx
   * @param {CancellationToken} cancellationToken
   */
  constructor(ctx, cancellationToken) {
    this.ctx = ctx;
    this.cancellationToken = cancellationToken;
  }

  /**
   * @param {number} idx
   * @returns {number}
   */
  getParent(idx) {
    if (idx === this.parentLimitIdx) return AST_IDX_INVALID;
    const parent = readParent(this.ctx.buf, idx);

    const parentType = readType(this.ctx.buf, parent);
    if (parentType === AST_GROUP_TYPE) {
      return readParent(this.ctx.buf, parent);
    }

    return parent;
  }

  /**
   * @param {number} idx
   * @returns {number}
   */
  getType(idx) {
    return readType(this.ctx.buf, idx);
  }

  /**
   * @param {number} idx
   * @param {number} propId
   * @returns {number}
   */
  getField(idx, propId) {
    if (idx === AST_IDX_INVALID) return -1;

    // Bail out on fields that can never point to another node
    switch (propId) {
      case AST_PROP_TYPE:
      case AST_PROP_PARENT:
      case AST_PROP_RANGE:
        return -1;
    }

    const { buf } = this.ctx;
    let offset = readPropOffset(this.ctx, idx);
    offset = findPropOffset(buf, offset, propId);

    if (offset === -1) return -1;
    const _prop = buf[offset++];
    const kind = buf[offset++];

    if (kind === PropFlags.Ref) {
      return readU32(buf, offset);
    }

    return -1;
  }

  /**
   * @param {number} idx - Node idx
   * @param {number[]} propIds
   * @param {number} propIdx
   * @returns {unknown}
   */
  getAttrPathValue(idx, propIds, propIdx) {
    if (idx === AST_IDX_INVALID) throw -1;

    const { buf, strTable, strByType } = this.ctx;

    const propId = propIds[propIdx];

    switch (propId) {
      case AST_PROP_TYPE: {
        const type = readType(buf, idx);
        return getString(strTable, strByType[type]);
      }
      case AST_PROP_PARENT:
      case AST_PROP_RANGE:
        throw -1;
    }

    let offset = readPropOffset(this.ctx, idx);

    offset = findPropOffset(buf, offset, propId);
    if (offset === -1) throw -1;
    const _prop = buf[offset++];
    const kind = buf[offset++];

    if (kind === PropFlags.Ref) {
      const value = readU32(buf, offset);
      // Checks need to end with a value, not a node
      if (propIdx === propIds.length - 1) throw -1;
      return this.getAttrPathValue(value, propIds, propIdx + 1);
    } else if (kind === PropFlags.RefArr) {
      const arrIdx = readU32(buf, offset);
      offset += 4;

      let count = 0;
      let child = readChild(buf, arrIdx);
      while (child > AST_IDX_INVALID) {
        count++;
        child = readNext(buf, child);
      }

      if (
        propIdx < propIds.length - 1 && propIds[propIdx + 1] === AST_PROP_LENGTH
      ) {
        return count;
      }

      // TODO(@marvinhagemeister): Allow traversing into array children?
      throw -1;
    } else if (kind === PropFlags.Obj) {
      // TODO(@marvinhagemeister)
    }

    // Cannot traverse into primitives further
    if (propIdx < propIds.length - 1) throw -1;

    if (kind === PropFlags.String) {
      const s = readU32(buf, offset);
      return getString(strTable, s);
    } else if (kind === PropFlags.Number) {
      const s = readU32(buf, offset);
      return Number(getString(strTable, s));
    } else if (kind === PropFlags.Regex) {
      const v = readU32(buf, offset);
      return readRegex(strTable, v);
    } else if (kind === PropFlags.Bool) {
      return readU32(buf, offset) === 1;
    } else if (kind === PropFlags.Null) {
      return null;
    } else if (kind === PropFlags.Undefined) {
      return undefined;
    }

    throw -1;
  }

  /**
   * @param {number} idx
   * @returns {number}
   */
  getFirstChild(idx) {
    const siblings = this.getSiblings(idx);
    return siblings[0] ?? -1;
  }

  /**
   * @param {number} idx
   * @returns {number}
   */
  getLastChild(idx) {
    const siblings = this.getSiblings(idx);
    return siblings.at(-1) ?? -1;
  }

  /**
   * @param {number} idx
   * @returns {number[]}
   */
  getSiblings(idx) {
    const { buf } = this.ctx;
    const parent = readParent(buf, idx);

    // Only RefArrays have siblings
    const parentType = readType(buf, parent);
    if (parentType !== AST_GROUP_TYPE) {
      return [];
    }

    const out = [];
    let child = readChild(buf, parent);
    while (child > AST_IDX_INVALID) {
      out.push(child);
      child = readNext(buf, child);
    }

    return out;
  }

  /**
   * Used for `:has()` and `:not()`
   * @param {MatcherFn[]} selectors
   * @param {number} idx
   * @returns {boolean}
   */
  subSelect(selectors, idx) {
    const prevLimit = this.parentLimitIdx;
    this.parentLimitIdx = idx;

    try {
      return subTraverse(this.ctx, selectors, idx, idx, this.cancellationToken);
    } finally {
      this.parentLimitIdx = prevLimit;
    }
  }
}

/**
 * @param {Uint8Array} buf
 * @param {CancellationToken} token
 * @returns {AstContext}
 */
function createAstContext(buf, token) {
  /** @type {Map<number, string>} */
  const strTable = new Map();

  // The buffer has a few offsets at the end which allows us to easily
  // jump to the relevant sections of the message.
  const propsOffset = readU32(buf, buf.length - 24);
  const spansOffset = readU32(buf, buf.length - 20);
  const typeMapOffset = readU32(buf, buf.length - 16);
  const propMapOffset = readU32(buf, buf.length - 12);
  const strTableOffset = readU32(buf, buf.length - 8);

  // Offset of the topmost node in the AST Tree.
  const rootOffset = readU32(buf, buf.length - 4);

  let offset = strTableOffset;
  const stringCount = readU32(buf, offset);
  offset += 4;

  let strId = 0;
  for (let i = 0; i < stringCount; i++) {
    const len = readU32(buf, offset);
    offset += 4;

    const strBytes = buf.slice(offset, offset + len);
    offset += len;
    const s = DECODER.decode(strBytes);
    strTable.set(strId, s);
    strId++;
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
    spansOffset,
    propsOffset,
    nodes: new Map(),
    strTableOffset,
    strByProp,
    strByType,
    typeByStr,
    propByStr,
    matcher: /** @type {*} */ (null),
  };
  ctx.matcher = new MatchCtx(ctx, token);

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
  const token = new CancellationToken();
  const ctx = createAstContext(serializedAst, token);

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

      // Check if this rule is excluded
      if (state.ignoredRules.has(id)) {
        continue;
      }

      const ruleCtx = new Context(ctx, id, fileName);
      const visitor = rule.create(ruleCtx);

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
            destroyFn(ruleCtx);
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
    traverse(ctx, visitors, ctx.rootOffset, token);
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
 * @param {number} idx
 * @param {CancellationToken} cancellationToken
 */
function traverse(ctx, visitors, idx, cancellationToken) {
  const { buf } = ctx;

  while (idx !== AST_IDX_INVALID) {
    if (cancellationToken.isCancellationRequested()) return;

    const nodeType = readType(buf, idx);

    /** @type {VisitorFn[] | null} */
    let exits = null;

    // Only visit if it's an actual node
    if (nodeType !== AST_GROUP_TYPE) {
      // Loop over visitors and check if any selector matches
      for (let i = 0; i < visitors.length; i++) {
        const v = visitors[i];
        if (v.matcher(ctx.matcher, idx)) {
          if (v.info.exit !== NOOP) {
            if (exits === null) {
              exits = [v.info.exit];
            } else {
              exits.push(v.info.exit);
            }
          }

          if (v.info.enter !== NOOP) {
            const node = /** @type {*} */ (getNode(ctx, idx));
            v.info.enter(node);
          }
        }
      }
    }

    try {
      const childIdx = readChild(buf, idx);
      if (childIdx > AST_IDX_INVALID) {
        traverse(ctx, visitors, childIdx, cancellationToken);
      }
    } finally {
      if (exits !== null) {
        for (let i = 0; i < exits.length; i++) {
          const node = /** @type {*} */ (getNode(ctx, idx));
          exits[i](node);
        }
      }
    }

    idx = readNext(buf, idx);
  }
}

/**
 * Used for subqueries in `:has()` and `:not()`
 * @param {AstContext} ctx
 * @param {MatcherFn[]} selectors
 * @param {number} rootIdx
 * @param {number} idx
 * @param {CancellationToken} cancellationToken
 * @returns {boolean}
 */
function subTraverse(ctx, selectors, rootIdx, idx, cancellationToken) {
  const { buf } = ctx;

  while (idx > AST_IDX_INVALID) {
    if (cancellationToken.isCancellationRequested()) return false;

    const nodeType = readType(buf, idx);

    if (nodeType !== AST_GROUP_TYPE) {
      for (let i = 0; i < selectors.length; i++) {
        const sel = selectors[i];

        if (sel(ctx.matcher, idx)) {
          return true;
        }
      }
    }

    const childIdx = readChild(buf, idx);
    if (
      childIdx > AST_IDX_INVALID &&
      subTraverse(ctx, selectors, rootIdx, childIdx, cancellationToken)
    ) {
      return true;
    }

    if (idx === rootIdx) {
      break;
    }

    idx = readNext(buf, idx);
  }

  return false;
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

  // @ts-ignore dump fn
  // deno-lint-ignore no-console
  console.log();

  let idx = 0;
  while (idx < (strTableOffset / NODE_SIZE)) {
    const type = readType(buf, idx);
    const child = readChild(buf, idx);
    const next = readNext(buf, idx);
    const parent = readParent(buf, idx);
    const range = readSpan(ctx, idx);

    const name = type === AST_IDX_INVALID
      ? "<invalid>"
      : type === AST_GROUP_TYPE
      ? "<group>"
      : getString(ctx.strTable, ctx.strByType[type]);
    // @ts-ignore dump fn
    // deno-lint-ignore no-console
    console.log(`${name}, idx: ${idx}, type: ${type}`);

    // @ts-ignore dump fn
    // deno-lint-ignore no-console
    console.log(`  child: ${child}, next: ${next}, parent: ${parent}`);
    // @ts-ignore dump fn
    // deno-lint-ignore no-console
    console.log(`  range: ${range[0]}, ${range[1]}`);

    const rawOffset = readRawPropOffset(ctx.buf, idx);
    let propOffset = readPropOffset(ctx, idx);
    const count = buf[propOffset++];
    // @ts-ignore dump fn
    // deno-lint-ignore no-console
    console.log(
      `  prop count: ${count}, prop offset: ${propOffset} raw offset: ${rawOffset}`,
    );

    for (let i = 0; i < count; i++) {
      const prop = buf[propOffset++];
      const kind = buf[propOffset++];
      const name = getString(ctx.strTable, ctx.strByProp[prop]);

      let kindName = "unknown";
      for (const k in PropFlags) {
        // @ts-ignore dump fn
        if (kind === PropFlags[k]) {
          kindName = k;
        }
      }

      const v = readU32(buf, propOffset);
      propOffset += 4;

      if (kind === PropFlags.Ref) {
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(`    ${name}: ${v} (${kindName}, ${prop})`);
      } else if (kind === PropFlags.RefArr) {
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(`    ${name}: RefArray: ${v}, (${kindName}, ${prop})`);
      } else if (kind === PropFlags.Bool) {
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(`    ${name}: ${v} (${kindName}, ${prop})`);
      } else if (kind === PropFlags.String) {
        const raw = getString(ctx.strTable, v);
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(`    ${name}: ${raw} (${kindName}, ${prop})`);
      } else if (kind === PropFlags.Number) {
        const raw = getString(ctx.strTable, v);
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(`    ${name}: ${raw} (${kindName}, ${prop})`);
      } else if (kind === PropFlags.Regex) {
        const raw = getString(ctx.strTable, v);
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(`    ${name}: ${raw} (${kindName}, ${prop})`);
      } else if (kind === PropFlags.Null) {
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(`    ${name}: null (${kindName}, ${prop})`);
      } else if (kind === PropFlags.Undefined) {
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(`    ${name}: undefined (${kindName}, ${prop})`);
      } else if (kind === PropFlags.BigInt) {
        const raw = getString(ctx.strTable, v);
        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(`    ${name}: ${raw} (${kindName}, ${prop})`);
      } else if (kind === PropFlags.Obj) {
        let offset = v + ctx.propsOffset;
        const count = readU32(ctx.buf, offset);
        offset += 4;

        // @ts-ignore dump fn
        // deno-lint-ignore no-console
        console.log(
          `    ${name}: Object (${count}) (${kindName}, ${prop}), raw offset ${v}`,
        );

        // TODO(@marvinhagemeister): Show object
      }
    }

    idx++;
  }
}

// These are captured by Rust and called when plugins need to be loaded
// or run.
internals.installPlugins = installPlugins;
internals.runPluginsForFile = runPluginsForFile;
internals.resetState = resetState;

/**
 * @param {Deno.lint.Plugin} plugin
 * @param {string} fileName
 * @param {string} sourceText
 */
function runLintPlugin(plugin, fileName, sourceText) {
  installPlugin(plugin);

  /** @type {Deno.lint.Diagnostic[]} */
  const diagnostics = [];
  doReport = (id, message, hint, start, end, fix) => {
    diagnostics.push({
      id,
      message,
      hint,
      range: [start, end],
      fix,
    });
  };
  doGetSource = () => {
    return sourceText;
  };
  try {
    const serializedAst = op_lint_create_serialized_ast(fileName, sourceText);

    runPluginsForFile(fileName, serializedAst);
  } finally {
    resetState();
  }
  doReport = op_lint_report;
  doGetSource = op_lint_get_source;
  return diagnostics;
}

Deno.lint.runPlugin = runLintPlugin;
