// A codemod is just a `Deno.lint.Plugin` whose rules report fixes.
// `deno codemod` applies every reported fix to your source files.
//
// This one rewrites `var` declarations to `let`.
export default {
  name: "var-to-let",
  rules: {
    "var-to-let": {
      create(context) {
        return {
          VariableDeclaration(node) {
            if (node.kind !== "var") {
              return;
            }
            const start = node.range[0];
            context.report({
              node,
              message: "`var` should be `let`",
              fix(fixer) {
                // Replace just the `var` keyword at the start of the node.
                return fixer.replaceTextRange([start, start + 3], "let");
              },
            });
          },
        };
      },
    },
  },
} satisfies Deno.lint.Plugin;
