Deno.test("step output", async (t) => {
  await t.step("step 1", () => {});
  await t.step("step 2", () => {});
  await t.step("step 3", () => {
    console.log("Hello, world! (from step 3)");
  });
  await t.step("step 4", () => {
    console.log("Hello, world! (from step 4)");
  });
});

Deno.test("step failures", async (t) => {
  await t.step("step 1", () => {});
  await t.step("step 2", () => {
    throw new Error("Fail.");
  });
  await t.step("step 3", () => Promise.reject(new Error("Fail.")));
});

Deno.test("step nested failure", async (t) => {
  await t.step("step 1", async (t) => {
    await t.step("inner 1", () => {});
    await t.step("inner 2", () => {
      throw new Error("Failed.");
    });
  });
});
