Deno.test("missing entry", async (t) => {
  await t.assertSnapshot(1);
});
