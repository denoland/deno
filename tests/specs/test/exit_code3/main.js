Deno.test("Deno.exitCode", () => {
  Deno.exitCode = 42;
});

Deno.test("success", () => {
});
