Deno.test("passing test", () => {
  // pass
});

Deno.test("ignored test", { ignore: true }, () => {
  // ignore
});

Deno.test("failing test", () => {
  throw new Error("fail");
});

Deno.test("steps", async (t) => {
  await t.step("passing step", () => {});
  await t.step("failing step", () => {
    throw new Error("fail");
  });
});
