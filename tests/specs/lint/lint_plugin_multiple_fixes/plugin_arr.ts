export default {
  name: "test-plugin",
  rules: {
    "my-rule": {
      create(context) {
        return {
          VariableDeclarator(node) {
            if (
              node.init?.type === "Literal" && node.init.value === "unfixed"
            ) {
              context.report({
                node: node.init!,
                message: 'should be "bar" + have string type',
                fix(fixer) {
                  return [
                    fixer.insertTextAfter(node.id, ": string"),
                    fixer.replaceText(node.init!, '"bar"'),
                  ];
                },
              });
            }
          },
        };
      },
    },
  },
} satisfies Deno.lint.Plugin;
