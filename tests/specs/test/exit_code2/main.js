Deno.test("Deno.exitCode", () => {
  Deno.exitCode = 5;
  throw new Error("");
});

Deno.test("success", () => {
});
