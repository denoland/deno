export default {
  name: "ast_plugin",
  rules: {
    ast: {
      create() {
        return {
          JSXAttribute(node) {
            console.log(node);
          },
          JSXClosingElement(node) {
            console.log(node);
          },
          JSXClosingFragment(node) {
            console.log(node);
          },
          JSXExpressionContainer(node) {
            console.log(node);
          },
          JSXElement(node) {
            console.log(node);
          },
          JSXFragment(node) {
            console.log(node);
          },
          JSXIdentifier(node) {
            console.log(node);
          },
          JSXMemberExpression(node) {
            console.log(node);
          },
          JSXNamespacedName(node) {
            console.log(node);
          },
          JSXOpeningElement(node) {
            console.log(node);
          },
          JSXOpeningFragment(node) {
            console.log(node);
          },
          JSXSpreadAttribute(node) {
            console.log(node);
          },

          // Ignored: This is part of the JSX spec but unused. No parser
          // properly supports spread children.
          // JSXSpreadChild(node) {
          //   console.log(node);
          // },
          JSXText(node) {
            console.log(node);
          },
        };
      },
    },
  },
} satisfies Deno.LintPlugin;
