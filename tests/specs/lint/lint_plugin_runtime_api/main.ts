const plugin = {
  name: "test-plugin",
  rules: {
    testRule: {
      create() {
        return {};
      },
    },
  },
};

Deno.lint.runPlugin(plugin, "source.ts", "export default {}");
