Deno.test("ignored step", async (t) => {
  await t.step({
    name: "step 1",
    ignore: true,
    fn: () => {
      throw new Error("Fail.");
    },
  });
  await t.step({
    name: "step 2",
    ignore: false,
    fn: () => {},
  });
});
