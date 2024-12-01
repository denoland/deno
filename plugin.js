const NAME = "test-plugin";

Deno.core.ops.op_register_lint_plugin(
  NAME,
  function create(context) {
    console.log("Hello from test plugin");
  },
);

console.log("Loaded plugin", NAME);
