Deno.test("top level missing await", (t) => {
  t.step("step", () => {
    return new Promise((resolve) => setTimeout(resolve, 10));
  });
});

Deno.test({
  name: "inner missing await",
  fn: async (t) => {
    await t.step("step", (t) => {
      t.step("inner", () => {
        return new Promise((resolve) => setTimeout(resolve, 10));
      });
    });
    await new Promise((resolve) => setTimeout(resolve, 10));
  },
  sanitizeResources: false,
  sanitizeOps: false,
  sanitizeExit: false,
});
