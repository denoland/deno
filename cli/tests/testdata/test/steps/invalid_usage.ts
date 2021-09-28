Deno.test("capturing", async (t) => {
  let capturedTester!: Deno.Tester;
  await t.step("some step", (t) => {
    capturedTester = t;
  });
  // this should error because the scope of the tester has already completed
  await capturedTester.step("next step", () => {});
});

Deno.test("top level missing await", (t) => {
  t.step("step", () => {
    return new Promise((resolve) => setTimeout(resolve, 10));
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
