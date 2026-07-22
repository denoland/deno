Deno.test("description", async (t) => {
  const success = await t.step("step 1", async (t) => {
    await t.step("inner 1", () => {});
    await t.step("inner 2", () => {});
  });

  if (!success) throw new Error("Expected the step to return true.");
});

Deno.test("description function as first arg", async (t) => {
  const success = await t.step(async function step1(t) {
    await t.step(function inner1() {});
    await t.step(function inner1() {});
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

Deno.test("steps buffered then streaming reporting", async (t) => {
  // no sanitizers so this will be buffered
  await t.step({
    name: "step 1",
    fn: async (t) => {
      // also ensure the buffered tests display in order regardless of the second one finishing first
      const step2Finished = Promise.withResolvers<void>();
      const step1 = t.step("step 1 - 1", async () => {
        await step2Finished.promise;
      });
      const step2 = t.step("step 1 - 2", async (t) => {
        await t.step("step 1 - 2 - 1", () => {});
      });
      await step2;
      step2Finished.resolve();
      await step1;
    },
    sanitizeResources: false,
    sanitizeOps: false,
    sanitizeExit: false,
  });

  // now this will start streaming and we want to
  // ensure it flushes the buffer of the last test
  await t.step("step 2", async () => {});
});
