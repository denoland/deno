// TODO(bartlomieju): this should be rule name, not plugin name
const PLUGIN_NAME = "test-plugin";
const RULE1_NAME = "first-rule";

const rule = {
  create(context) {
    console.log("GOGO", context);
    console.log("Hello from", `${PLUGIN_NAME}/${RULE1_NAME}`);
    context.report({
      span: {
        start: 7,
        end: 10,
      },
      message: "Error from " + `${PLUGIN_NAME}/${RULE1_NAME}`,
      data: {
        some: "Data",
      },
    });
    return {
      // Performs action in the function on every variable declarator
      StringLiteral(node) {
        // console.log("string literal", node);
      },
      VariableDeclarator(node) {
        // console.log("variable declarator", node);
        // Check if a `const` variable declaration
        console.log("node.parent.kind", node.parent.kind);
        if (node.parent.kind === "const") {
          // Check if variable name is `foo`
          console.log("node.id.type", node.id.type, node.id.value);
          if (node.id.type === "Identifier" && node.id.value === "foo") {
            // Check if value of variable is "bar"
            console.log("node.init", node.init);
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
