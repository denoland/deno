export default {
  name: "ast_plugin",
  rules: {
    ast: {
      create() {
        return {
          BreakStatement(node) {
            console.log(node);
          },
          ContinueStatement(node) {
            console.log(node);
          },
          ReturnStatement(node) {
            console.log(node);
          },
        };
      },
    },
  },
} satisfies Deno.LintPlugin;
