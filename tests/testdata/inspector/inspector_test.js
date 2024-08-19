Deno.test("basic test", () => {
  const value = 1 + 1;
  if (value !== 2) {
    throw new Error("failed");
  }
});
