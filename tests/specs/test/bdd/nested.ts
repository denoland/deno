Deno.describe("outer", () => {
  Deno.it("outer test", () => {
    if (true !== true) throw new Error("fail");
  });

  Deno.describe("inner", () => {
    Deno.it("inner test", () => {
      if (1 + 1 !== 2) throw new Error("fail");
    });
  });
});
