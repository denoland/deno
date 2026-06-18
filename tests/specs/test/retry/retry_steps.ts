// A test using steps is re-invoked on retry; its steps must run cleanly again.
let attempts = 0;
Deno.test({
  name: "steps with retry",
  retry: 2,
  fn: async (t) => {
    attempts++;
    await t.step("first", () => {});
    await t.step("second", () => {
      if (attempts < 2) {
        throw new Error("step fails first time");
      }
    });
  },
});
