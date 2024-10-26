Deno.test("parent 1", async (t) => {
  await t.step("child 1", () => {});
  await t.step("child 2", () => {
    throw new Error("Fail.");
  });
});

Deno.test("parent 2", async (t) => {
  await t.step("child 1", async (t) => {
    await t.step("grandchild 1", () => {});
    await t.step("grandchild 2", () => {
      throw new Error("Fail.");
    });
  });
  await t.step("child 2", () => {
    throw new Error("Fail.");
  });
});

Deno.test("parent 3", async (t) => {
  await t.step("child 1", () => {});
  await t.step("child 2", () => {});
});
