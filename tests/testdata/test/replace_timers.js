Deno.test("foo", async (t) => {
  globalThis.setTimeout = () => {};
  globalThis.clearTimeout = () => {};
  globalThis.setInterval = () => {};
  globalThis.clearInterval = () => {};
  await t.step("bar", () => {});
});
