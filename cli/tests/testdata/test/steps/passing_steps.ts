Deno.test("description", async (t) => {
  const success = await t.step("step 1", async (t) => {
    await t.step("inner 1", () => {});
    await t.step("inner 2", () => {});
  });

  if (!success) throw new Error("Expected the step to return true.");
});

Deno.test("parallel steps without sanitizers", async (t) => {
  // allowed
  await Promise.all([
    t.step({
      name: "step 1",
      fn: async () => {
        await new Promise((resolve) => setTimeout(resolve, 10));
      },
      sanitizeOps: false,
      sanitizeResources: false,
      sanitizeExit: false,
    }),
    t.step({
      name: "step 2",
      fn: async () => {
        await new Promise((resolve) => setTimeout(resolve, 10));
      },
      sanitizeOps: false,
      sanitizeResources: false,
      sanitizeExit: false,
    }),
  ]);
});

Deno.test({
  name: "parallel steps without sanitizers due to parent",
  fn: async (t) => {
    // allowed because parent disabled the sanitizers
    await Promise.all([
      t.step("step 1", async () => {
        await new Promise((resolve) => setTimeout(resolve, 10));
      }),
      t.step("step 2", async () => {
        await new Promise((resolve) => setTimeout(resolve, 10));
      }),
    ]);
  },
  sanitizeResources: false,
  sanitizeOps: false,
  sanitizeExit: false,
});

Deno.test({
  name: "steps with disabled sanitizers, then enabled, then parallel disabled",
  fn: async (t) => {
    await t.step("step 1", async (t) => {
      await t.step({
        name: "step 1",
        fn: async (t) => {
          await Promise.all([
            t.step({
              name: "step 1",
              fn: async (t) => {
                await new Promise((resolve) => setTimeout(resolve, 10));
                await Promise.all([
                  t.step("step 1", () => {}),
                  t.step("step 1", () => {}),
                ]);
              },
              sanitizeExit: false,
              sanitizeResources: false,
              sanitizeOps: false,
            }),
            t.step({
              name: "step 2",
              fn: () => {},
              sanitizeResources: false,
              sanitizeOps: false,
              sanitizeExit: false,
            }),
          ]);
        },
        sanitizeResources: true,
        sanitizeOps: true,
        sanitizeExit: true,
      });
    });
  },
  sanitizeResources: false,
  sanitizeOps: false,
  sanitizeExit: false,
});
