Deno.test("ignored step", async (t) => {
  let result = await t.step({
    name: "step 1",
    ignore: true,
    fn: () => {
      throw new Error("Fail.");
    },
  });
  if (result !== false) throw new Error("Expected false.");
  result = await t.step({
    name: "step 2",
    ignore: false,
    fn: () => {},
  });
  if (result !== true) throw new Error("Expected true.");
});
