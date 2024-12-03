import { op_lint_get_rule, op_lint_report } from "ext:core/ops";

export class Context {
  id;

  fileName;

  constructor(id, fileName) {
    this.id = id;
    this.fileName = fileName;
  }

  report(data) {
    // TODO(bartlomieju): if there's `node` then convert position automatically
    // otherwise lookup `location`
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

export function runPluginRule(fileName, pluginName, ruleName, serializedAst) {
  const id = `${pluginName}/${ruleName}`;

  const ctx = new Context(id, fileName);
  const rule = op_lint_get_rule(pluginName, ruleName);

  const visitor = rule(ctx);
  const ast = JSON.parse(serializedAst, (key, value) => {
    if (key === "ctxt") {
      return undefined;
    }
    return value;
  });

  // console.log("ast", ast);
  traverse(ast, visitor);
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
