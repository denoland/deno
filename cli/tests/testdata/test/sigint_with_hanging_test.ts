setInterval(() => {}, 10000);

Deno.test({
  name: "test",
  sanitizeOps: false,
  sanitizeExit: false,
  sanitizeResources: false,
  async fn(t) {
    await t.step("step 1", async (t) => {
      await t.step("step 2", async () => {
        await new Promise(() => {});
      });
    });
  },
});
