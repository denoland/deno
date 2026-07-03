Deno.test("basic object", async (t) => {
  await t.assertSnapshot({ hello: "world!", example: 123 });
});

Deno.test("multiple snapshots", async (t) => {
  await t.assertSnapshot("first");
  await t.assertSnapshot([1, 2, 3]);
});

Deno.test("steps", async (t) => {
  await t.step("child", async (t) => {
    await t.assertSnapshot("in step");
  });
});

Deno.test("custom name and serializer", async (t) => {
  await t.assertSnapshot("raw value", {
    name: "my custom name",
    serializer: (v) => `serialized: ${v}`,
  });
});
