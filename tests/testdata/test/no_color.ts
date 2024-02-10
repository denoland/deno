Deno.test({
  name: "success",
  fn() {},
});

Deno.test({
  name: "fail",
  fn() {
    throw new Error("fail");
  },
});

Deno.test({
  name: "ignored",
  ignore: true,
  fn() {},
});
