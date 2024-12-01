// TODO(bartlomieju): this should be rule name, not plugin name
const PLUGIN_NAME = "test-plugin";
const RULE1_NAME = "first-rule";

Deno.core.ops.op_lint_register_lint_plugin(PLUGIN_NAME);

Deno.core.ops.op_lint_register_lint_plugin_rule(
  PLUGIN_NAME,
  RULE1_NAME,
  function create(context) {
    console.log("Hello from", `${PLUGIN_NAME}/${RULE1_NAME}`);
    context.report({
      message: "Error from " + `${PLUGIN_NAME}/${RULE1_NAME}`,
      data: {
        some: "Data",
      },
    });
    return {};
  },
);

console.log("Loaded plugin", PLUGIN_NAME);
