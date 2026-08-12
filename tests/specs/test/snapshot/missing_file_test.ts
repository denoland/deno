Deno.test("missing file", async (t) => {
  await t.assertSnapshot(123);
});
