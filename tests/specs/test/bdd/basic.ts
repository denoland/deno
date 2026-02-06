Deno.describe("math", () => {
  Deno.it("adds numbers", () => {
    if (1 + 1 !== 2) throw new Error("fail");
  });

  Deno.it("subtracts numbers", () => {
    if (2 - 1 !== 1) throw new Error("fail");
  });
});
