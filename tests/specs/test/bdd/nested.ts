Deno.describe("outer", () => {
  Deno.test("outer test", () => {
    // deno-lint-ignore no-constant-condition
    if (true !== true) throw new Error("fail");
  });

  Deno.describe("inner", () => {
    Deno.test("inner test", () => {
      // deno-lint-ignore no-constant-condition
      if (1 + 1 !== 2) throw new Error("fail");
    });
  });
});
