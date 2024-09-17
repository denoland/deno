Deno.test("nested failure", async (t) => {
  const success = await t.step("step 1", async (t) => {
    let success = await t.step("inner 1", () => {
      throw new Error("Failed.");
    });
    if (success) throw new Error("Expected failure");

    success = await t.step("inner 2", () => {});
    if (!success) throw new Error("Expected success");
  });

  if (success) throw new Error("Expected failure");
});

Deno.test("multiple test step failures", async (t) => {
  await t.step("step 1", () => {
    throw new Error("Fail.");
  });
  await t.step("step 2", () => Promise.reject(new Error("Fail.")));
});

Deno.test("failing step in failing test", async (t) => {
  await t.step("step 1", () => {
    throw new Error("Fail.");
  });
  throw new Error("Fail test.");
});
