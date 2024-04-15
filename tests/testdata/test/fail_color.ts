Deno.test("fail color", () => {
  throw new Error(`RedMessage: \x1b[31mThis should be red text\x1b[39m`);
});

// deno-lint-ignore no-explicit-any
Deno.test("step fail color", async (t: any) => {
  await t.step("step", () => {
    throw new Error(`RedMessage: \x1b[31mThis should be red text\x1b[39m`);
  });
});
