Deno.test("custom dir", async (t) => {
  await t.assertSnapshot({ a: 1 }, { dir: "custom_snapshots" });
});
