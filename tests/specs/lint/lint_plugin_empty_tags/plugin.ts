export default {
  name: "test-plugin",
  rules: {
    "my-rule": {
      create(_context) {
        return {
          Identifier(node) {
            console.log("Plugin:", node.type);
          },
        };
      },
    },
  },
} satisfies Deno.lint.Plugin;
