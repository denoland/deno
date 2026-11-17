export default {
  name: "test-plugin",
  rules: {
    "no-debugger": {
      create(context) {
        return {
          DebuggerStatement(node) {
            context.report({ node, message: "debugger reported by plugin" });
          },
        };
      },
    },
  },
};
