export default {
  name: "ast_plugin",
  rules: {
    ast: {
      create() {
        return {
          Program(node) {
            console.log(node);
          },
        };
      },
    },
  },
} satisfies Deno.LintPlugin;
