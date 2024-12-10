export default {
  name: "ast_plugin",
  rules: {
    ast: {
      create() {
        return {
          CallExpression(node) {
            console.log(node);
          },
          MemberExpression(node) {
            console.log(node);
          },
        };
      },
    },
  },
} satisfies Deno.LintPlugin;
