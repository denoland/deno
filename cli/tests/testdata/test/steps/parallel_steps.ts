Deno.test("parallel steps with sanitizers", async (t) => {
  // not allowed because steps with sanitizers cannot be run
  await Promise.all([
    t.step("step 1", async () => {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }),
    t.step("step 2", async () => {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }),
  ]);
});

Deno.test("parallel steps when first has sanitizer", async (t) => {
  // not allowed
  await Promise.all([
    t.step({
      name: "step 1",
      fn: async () => {
        await new Promise((resolve) => setTimeout(resolve, 100));
      },
    }),
    t.step({
      name: "step 2",
      fn: async () => {
        await new Promise((resolve) => setTimeout(resolve, 100));
      },
      sanitizeOps: false,
      sanitizeResources: false,
      sanitizeExit: false,
    }),
  ]);
});

Deno.test("parallel steps when second has sanitizer", async (t) => {
  // not allowed
  await Promise.all([
    t.step({
      name: "step 1",
      fn: async () => {
        await new Promise((resolve) => setTimeout(resolve, 100));
      },
      sanitizeOps: false,
      sanitizeResources: false,
      sanitizeExit: false,
    }),
    t.step({
      name: "step 2",
      fn: async () => {
        await new Promise((resolve) => setTimeout(resolve, 100));
      },
    }),
  ]);
});

Deno.test({
  name: "parallel steps where only inner tests have sanitizers",
  fn: async (t) => {
    await Promise.all([
      t.step("step 1", async (t) => {
        await t.step({
          name: "step inner",
          fn: () => new Promise((resolve) => setTimeout(resolve, 10)),
          sanitizeOps: true,
        });
      }),
      t.step("step 2", async (t) => {
        await t.step({
          name: "step inner",
          fn: () => new Promise((resolve) => setTimeout(resolve, 10)),
          sanitizeOps: true,
        });
      }),
    ]);
  },
  sanitizeResources: false,
  sanitizeOps: false,
  sanitizeExit: false,
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
