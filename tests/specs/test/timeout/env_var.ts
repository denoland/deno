// DENO_TEST_TIMEOUT=100 in the env should be honored when no flag is set.
Deno.test("honors env var", async () => {
  await new Promise((resolve) => setTimeout(resolve, 1000));
});
