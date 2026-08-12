Deno.test("mismatch", async (t) => {
  await t.assertSnapshot({ value: 2 });
});
