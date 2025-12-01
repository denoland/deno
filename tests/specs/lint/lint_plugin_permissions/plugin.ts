const envVar = Deno.env.get("MY_ENV_VAR");
console.log(`Environment Variable MY_ENV_VAR: ${envVar}`);

export default {
  name: "test-plugin",
  rules: {
    "my-rule": {
      create(context) {
        const readmeContent = Deno.readTextFileSync("./README.md");
        console.log(`Content of README.md: ${readmeContent}`);

        return {
          Identifier(node) {
            if (node.name === "_a") {
              context.report({
                node,
                message: "should be _b",
                fix(fixer) {
                  return fixer.replaceText(node, "_b");
                },
              });
            }
          },
        };
      },
    },
  },
};
