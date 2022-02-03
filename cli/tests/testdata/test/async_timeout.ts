Deno.test("test 1", () => {});

Deno.test("test 2", async () => {
  await new Promise(() => {});
});

Deno.test("test 3", () => {});
