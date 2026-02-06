Deno.describe("math", () => {
  Deno.test("adds numbers", () => {
    if (1 + 1 !== 2) throw new Error("fail");
  });

  Deno.test("subtracts numbers", () => {
    if (2 - 1 !== 1) throw new Error("fail");
  });
});
