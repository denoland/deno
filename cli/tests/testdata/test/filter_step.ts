Deno.test({
  name: "group 1",
  async fn(t) {
    await t.step("step 1", async () => {
      await t.step("sub step 1", () => {});
    });
    await t.step("step 2", () => {});
  },
});

Deno.test("top level test", () => {});
