export default {
  name: "test-plugin",
  rules: {
    "my-rule": {
      create(context) {
        return {
          VariableDeclarator(node) {
            console.log(`Source:`);
            console.log(context.sourceCode.getText());

            console.log(`Source VariableDeclarator:`);
            console.log(context.sourceCode.getText(node));
            console.log();

            console.log(`Ancestors:`);
            console.log(
              context.sourceCode.getAncestors(node).map((node) => node.type),
            );
            console.log();

            console.log(`Ast:`);
            console.log(context.sourceCode.ast.type);
          },
        };
      },
    },
  },
} satisfies Deno.lint.Plugin;
