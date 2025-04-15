Deno.test("capturing", async (t) => {
  let capturedContext!: Deno.TestContext;
  await t.step("some step", (t) => {
    capturedContext = t;
  });
  // this should error because the scope of the tester has already completed
  await capturedContext.step("next step", () => {});
});

Deno.test("top level missing await", (t) => {
  t.step("step", () => {
    return new Promise(() => {});
  });
});

Deno.test({
  name: "inner missing await",
  fn: async (t) => {
    await t.step("step", (t) => {
      t.step("inner", () => {
        return new Promise((resolve) => setTimeout(resolve, 10));
      });
    });
    await new Promise((resolve) => setTimeout(resolve, 10));
  },
  sanitizeResources: false,
  sanitizeOps: false,
  sanitizeExit: false,
});

Deno.test("parallel steps with sanitizers", async (t) => {
  // not allowed because steps with sanitizers cannot be run in parallel
  const step1Entered = Promise.withResolvers<void>();
  const testFinished = Promise.withResolvers<void>();
  t.step("step 1", async () => {
    step1Entered.resolve();
    await testFinished.promise;
  });
  await step1Entered.promise;
  await t.step("step 2", () => {});
});

Deno.test("parallel steps when first has sanitizer", async (t) => {
  const step1Entered = Promise.withResolvers<void>();
  const step2Finished = Promise.withResolvers<void>();
  const step1 = t.step({
    name: "step 1",
    fn: async () => {
      step1Entered.resolve();
      await step2Finished.promise;
    },
  });
  await step1Entered.promise;
  await t.step({
    name: "step 2",
    fn: () => {},
    sanitizeOps: false,
    sanitizeResources: false,
    sanitizeExit: false,
  });
  step2Finished.resolve();
  await step1;
});

Deno.test("parallel steps when second has sanitizer", async (t) => {
  const step1Entered = Promise.withResolvers<void>();
  const step2Finished = Promise.withResolvers<void>();
  const step1 = t.step({
    name: "step 1",
    fn: async () => {
      step1Entered.resolve();
      await step2Finished.promise;
    },
    sanitizeOps: false,
    sanitizeResources: false,
    sanitizeExit: false,
  });
  await step1Entered.promise;
  await t.step({
    name: "step 2",
    fn: async () => {
      await new Promise((resolve) => setTimeout(resolve, 100));
    },
  });
  step2Finished.resolve();
  await step1;
});

Deno.test({
  name: "parallel steps where only inner tests have sanitizers",
  fn: async (t) => {
    const step1Entered = Promise.withResolvers<void>();
    const step2Finished = Promise.withResolvers<void>();
    const step1 = t.step("step 1", async (t) => {
      await t.step({
        name: "step inner",
        fn: async () => {
          step1Entered.resolve();
          await step2Finished.promise;
        },
        sanitizeOps: true,
      });
    });
    await step1Entered.promise;
    await t.step("step 2", async (t) => {
      await t.step({
        name: "step inner",
        fn: () => {},
        sanitizeOps: true,
      });
    });
    step2Finished.resolve();
    await step1;
  },
  sanitizeResources: false,
  sanitizeOps: false,
  sanitizeExit: false,
});
