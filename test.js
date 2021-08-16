Deno.test("test 1", async (t) => {
  await t.step("step 1", async (t) => {
    await t.step("step 1.1", async (t) => {
      await t.step("step 1.1.1", () => {
        throw new Error("Oh no");
      });
      await t.step("step 1.1.2", () => {});
      await t.step("step 1.1.3", () => {});
    });

    await t.step("step 1.2", () => {});
    await t.step("step 1.3", () => {});
  });

  await t.step("step 2", async (t) => {
    await t.step("step 2.1", () => {});
    await t.step("step 2.2", () => {});
    await t.step("step 2.3", () => {});
  });
});

Deno.test("addition (sequential)", async (t) => {
  const cases = [
    [1, 1, 2],
    [1, 2, 3],
    [-1, 2, 1],
  ];

  for (const [a, b, expected] of cases) {
    await t.step(`${a}+${b}=${expected}`, () => {
      const actual = a + b;
      console.assert(actual === expected);
    });
  }
});
