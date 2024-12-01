import { op_lint_get_rule } from "ext:core/ops";

export class Context {
  id;

  fileName;

  constructor(id, fileName) {
    this.id = id;
    this.fileName = fileName;
  }

  report() {
    console.log("Not implemented report");
  }
}

export function runPluginRule(fileName, pluginName, ruleName) {
  const id = `${pluginName}/${ruleName}`;

  const ctx = new Context(id, fileName);
  const rule = op_lint_get_rule(pluginName, ruleName);

  console.log(ctx, typeof rule);
}
