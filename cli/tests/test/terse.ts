Deno.test("ok", function () {});

Deno.test("fail", function () {
  throw new Error("fail");
});

Deno.test({
  name: "ignore",
  fn() {},
  ignore: true,
});
