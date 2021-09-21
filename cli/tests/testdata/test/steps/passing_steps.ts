Deno.test("description", async (t) => {
  const success = await t.step("step 1", async (t) => {
    await t.step("inner 1", () => {});
    await t.step("inner 2", () => {});
  });

  if (!success) throw new Error("Expected the step to return true.");
});
