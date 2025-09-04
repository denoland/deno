export default {
  name: "test-plugin",
  rules: {
    "my-rule": {
      create(ctx) {
        console.log(`create: ${ctx.id}`);
        return {};
      },
      destroy(ctx) {
        console.log(`destroy: ${ctx.id}`);
      },
    },
    "my-rule-2": {
      create(ctx) {
        console.log(`create: ${ctx.id}`);
        return {};
      },
      destroy(ctx) {
        console.log(`destroy: ${ctx.id}`);
      },
    },
  },
};
