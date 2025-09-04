Deno.test("SourceCode.text", () => {
  const plugin: Deno.lint.Plugin = {
    name: "sample",
    rules: {
      "test": {
        create: (ctx) => {
          ctx.sourceCode.text;
          return {};
        },
      },
    },
  };
  Deno.lint.runPlugin(
    plugin,
    "./test.js",
    `function add(a, b) {
  return a + b;
}

add(1, 2);`,
  );
});
