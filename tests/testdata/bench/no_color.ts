Deno.bench({
  name: "success",
  fn() {},
});

Deno.bench({
  name: "success long",
  fn() {
    1024n ** 10000n;
  },
});

Deno.bench({
  name: "success but longer",
  fn() {
    1024n ** 1000000n;
  },
});

Deno.bench({
  name: "success long and the longest name",
  async fn() {
    await new Promise((resolve) => setTimeout(resolve, 100));
  },
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
