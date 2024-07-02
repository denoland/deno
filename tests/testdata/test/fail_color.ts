Deno.test("fail color", () => {
  throw new Error(`RedMessage: \x1b[31mThis should be red text\x1b[39m`);
});

Deno.test("step fail color", async (t) => {
  await t.step("step", () => {
    throw new Error(`RedMessage: \x1b[31mThis should be red text\x1b[39m`);
  });
});
