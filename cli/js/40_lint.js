// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check

import { core, internals } from "ext:core/mod.js";

const {
  op_lint_get_rule,
  op_lint_get_source,
  op_lint_report,
  op_lint_create_serialized_ast,
} = core.ops;

// Keep in sync with Rust
// These types are expected to be present on every node. Note that this
// isn't set in stone. We could revise this at a future point.
const AST_PROP_TYPE = 0;
const AST_PROP_PARENT = 1;
const AST_PROP_RANGE = 2;

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
/** @typedef {import("./40_lint_types.d.ts").LintReportData} LintReportData */
/** @typedef {import("./40_lint_types.d.ts").TestReportData} TestReportData */

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

  /** @type {Map<string, { enter: VisitorFn, exit: VisitorFn}>} */
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
            throw new Error(
              `Visitor "${name}" of plugin "${id}" errored, ${err}`,
              {
                cause: err,
              },
            );
          }
        };

        if (isExit) {
          info.exit = wrapped;
        } else {
          info.enter = wrapped;
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

  /** @type {CompiledVisitor[]} */
  const visitors = [];
  for (const [sel, info] of bySelector.entries()) {
    // This will make more sense once selectors land as it's faster
    // to precompile them once upfront.

    // Convert the visiting element name to a number. This number
    // is part of the serialized buffer and comparing a single number
    // is quicker than strings.
    const elemId = ctx.typeByStr.get(sel) ?? -1;

    visitors.push({
      info,
      // Check if we should call this visitor
      matcher: (offset) => {
        const type = ctx.buf[offset];
        return type === elemId;
      },
    });
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

  const { buf } = ctx;

  /** @type {VisitorFn[] | null} */
  let exits = null;

  for (let i = 0; i < visitors.length; i++) {
    const v = visitors[i];

    if (v.matcher(offset)) {
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
        const node = /** @type {*} */ (getNode(ctx, offset));
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
internals.installPlugin = installPlugin;
internals.runPluginsForFile = runPluginsForFile;
