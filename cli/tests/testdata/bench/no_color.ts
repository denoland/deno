Deno.bench({
  name: "success",
  fn() {},
});

Deno.bench({
  name: "fail",
  fn() {
    throw new Error("fail");
  },
});

Deno.bench({
  name: "ignored",
  ignore: true,
  fn() {},
});
