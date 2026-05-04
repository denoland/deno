// --no-timeout disables the default timeout entirely.
Deno.test("runs longer than the built-in default", async () => {
  await new Promise((resolve) => setTimeout(resolve, 200));
});
