Deno.test({
  name: "match only",
  only: true,
  fn() {},
});
Deno.test("other", () => {});
