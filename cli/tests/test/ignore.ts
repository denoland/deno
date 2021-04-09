Deno.test({
  name: "ignore1",
  fn() {},
  ignore: true,
});

Deno.test("ignore2", function () {
}, { ignore: true });
