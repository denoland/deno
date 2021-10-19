Deno.test("description", async (t) => {
  // deno-lint-ignore no-explicit-any
  await (t as any).step("step", () => {});
});
