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

const PROGRAM = 1;
const Program = 1;

const ast = {
  Program() {
    return {
      type: "Program",
      moduleOrScript: null,
    };
  },
};

/**
 * @param {Uint8Array} ast
 */
function buildAstFromBinary(ast) {
  console.log(ast);
  const stack = [];
  for (let i = 0; i < ast.length; i += 5) {
    const kind = ast[i];
    if (kind === 0) {
      throw new Error("FAIL");
    }
  }
}

export function runPluginsForFile(fileName, serializedAst, binary) {
  const binaryAst = buildAstFromBinary(binary);
  const ast = JSON.parse(serializedAst, (key, value) => {
    if (key === "ctxt") {
      return undefined;
    }
    return value;
  });

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
