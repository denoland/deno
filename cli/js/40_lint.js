// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core } from "ext:core/mod.js";
const {
  op_lint_get_rule,
  op_lint_get_source,
  op_lint_report,
} = core.ops;

const state = {
  plugins: {},
};

export class Context {
  id;

  fileName;

  constructor(id, fileName) {
    this.id = id;
    this.fileName = fileName;
  }

  source() {
    // TODO(bartlomieju): cache it on the state - it won't change between files, but callers can mutate it.
    return op_lint_get_source();
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

// TODO(bartlomieju): remove
export function runPluginRule() {}

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

export function runPluginsForFile(fileName, serializedAst) {
  const ast = JSON.parse(serializedAst, (key, value) => {
    if (key === "ctxt") {
      return undefined;
    }
    return value;
  });

  for (const pluginName of Object.keys(state.plugins)) {
    runRulesFromPlugin(pluginName, state.plugins[pluginName], fileName, ast);
  }
}

function runRulesFromPlugin(pluginName, plugin, fileName, ast) {
  for (const ruleName of Object.keys(plugin)) {
    const rule = plugin[ruleName];

    if (typeof rule.create !== "function") {
      throw new Error("Rule's `create` property must be a function");
    }

    // TODO(bartlomieju): can context be created less often, maybe once per plugin or even once per `runRulesForFile` invocation?
    const id = `${pluginName}/${ruleName}`;
    const ctx = new Context(id, fileName);
    const visitor = rule.create(ctx);
    traverse(ast, visitor);

    if (typeof rule.destroy === "function") {
      rule.destroy(ctx);
    }
  }
}

function traverse(ast, visitor, parent = null) {
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
  if (visitor[nodeType] && typeof visitor[nodeType] === "function") {
    visitor[nodeType](ast);
  }

  // Traverse child nodes
  for (const key in ast) {
    if (key === "parent" || key === "type") {
      continue;
    }

    const child = ast[key];

    if (Array.isArray(child)) {
      child.forEach((item) => traverse(item, visitor, ast));
    } else if (child && typeof child === "object") {
      traverse(child, visitor, ast);
    }
  }
}
