Deno.test("timeout", async (t) => {
  const timer = setTimeout(() => {
    console.log("timeout");
  }, 10000);
  clearTimeout(timer);
  await t.step("step", async () => {
    await new Promise<void>((resolve) => setTimeout(() => resolve(), 10));
  });
  console.log("done");
});
