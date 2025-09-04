export default {
  name: "test-plugin",
  rules: {
    "my-rule": {
      create(context) {
        return {
          VariableDeclarator(node) {
            if (node.init.type !== "Literal" || node.init.value !== "1") {
              context.report({
                node: node.init,
                message: 'should be equal to string "1"',
                fix(fixer) {
                  return fixer.replaceText(node.init, '"1"');
                },
              });
            }
          },
        };
      },
    },
  },
};
