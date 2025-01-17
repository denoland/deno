// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file

const PLUGIN_NAME = "test-plugin-local";
const RULE1_NAME = "first-rule";

const rule = {
  create(context) {
    return {
      VariableDeclarator(node) {
        // console.log("variable declarator", node);
        // Check if a `const` variable declaration
        if (node.parent.kind === "const") {
          // Check if variable name is `foo`
          if (node.id.type === "Identifier" && node.id.name === "foo") {
            // Check if value of variable is "bar"
            if (
              node.init && node.init.type === "StringLiteral" &&
              node.init.value !== "bar"
            ) {
              /*
               * Report error to ESLint. Error message uses
               * a message placeholder to include the incorrect value
               * in the error message.
               * Also includes a `fix(fixer)` function that replaces
               * any values assigned to `const foo` with "bar".
               */
              context.report({
                node,
                message:
                  'Value other than "bar" assigned to `const foo`. Unexpected value: {{ notBar }}.',
                data: {
                  notBar: node.init.value,
                },
                fix(fixer) {
                  return fixer.replaceText(node.init, '"bar"');
                },
              });
            }
          }
        }
      },
    };
  },
};

export default {
  name: PLUGIN_NAME,
  rules: {
    [RULE1_NAME]: rule,
    "jsx-style-string": {
      create(context) {
        console.log("context", context);
        return {
          VariableDeclaration(node) {
            console.log("node", node);
            // console.log("INTERFAcE", { ...node, parent: null });
          },
          JSXAttribute(node) {
            console.log("JSX attr node", node);
            if (
              node.name.type === "JSXIdentifier" &&
              node.name.name === "style" &&
              node.value.type !== "StringLiteral"
            ) {
              context.report({
                node: node.value,
                message: "Use a string literal for 'style'",
              });
            }
          },
        };
      },
    },
    "jest/no-identical-title": {
      create(context) {
        // console.log(context.source());
        const seen = new Set();
        return {
          CallExpression(node) {
            if (
              node.callee.type === "Identifier" &&
              node.callee.value === "describe" && node.arguments.length > 0 &&
              node.arguments[0].expression.type === "StringLiteral"
            ) {
              const name = node.arguments[0].expression.value;
              if (seen.has(name)) {
                context.report({
                  node,
                  message: `Duplicate describe title found`,
                });
              }

              seen.add(name);
            }
          },
        };
      },
    },
    "vue/multi-word-component-names": {
      create(context) {
        return {
          CallExpression(node) {
            // Check for component name in `Vue.component("<name>", ...)`
            if (
              node.callee.type === "MemberExpression" &&
              node.callee.object.type === "Identifier" &&
              node.callee.object.value === "Vue" &&
              node.callee.property.type === "Identifier" &&
              node.callee.property.value === "component" &&
              node.arguments.length > 0 &&
              node.arguments[0].expression.type === "StringLiteral"
            ) {
              const name = node.arguments[0].expression.value;

              const numUpper = name.length - name.replace(/[A-Z]/g, "").length;
              if (!name.includes("-") || numUpper.length < 2) {
                context.report({
                  node: node.arguments[0].expression,
                  message:
                    `Component names must be composed of multiple words, but got "${name}"`,
                });
              }
            }
          },
        };
      },
    },
  },
};

console.log("Loaded plugin", PLUGIN_NAME);
