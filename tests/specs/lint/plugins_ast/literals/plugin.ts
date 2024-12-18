export default {
  name: "ast_plugin",
  rules: {
    ast: {
      create() {
        return {
          ArrayExpression(node) {
            console.log(node);
          },
          BooleanLiteral(node) {
            console.log(node);
          },
          BigIntLiteral(node) {
            console.log(node);
          },
          NullLiteral(node) {
            console.log(node);
          },
          NumericLiteral(node) {
            console.log(node);
          },
          ObjectExpression(node) {
            console.log(node);
          },
          RegExpLiteral(node) {
            console.log(node);
          },
          StringLiteral(node) {
            console.log(node);
          },
        };
      },
    },
  },
} satisfies Deno.LintPlugin;
