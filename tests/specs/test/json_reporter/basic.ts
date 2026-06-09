Deno.test("alpha", () => {});
Deno.test("beta fails", () => {
  throw new Error("boom");
});
Deno.test({ name: "gamma ignored", ignore: true, fn() {} });
