Deno.test("ok", function () {
  console.log("ok");
});

Deno.test("fail", function () {
  throw new Error("fail");
});

Deno.test({
  name: "ignore",
  fn() {},
  ignore: true,
});
