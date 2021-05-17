Deno.test({
  name: "unresolved promise",
  fn() {
    return new Promise((_resolve, _reject) => {});
  },
});

Deno.test({
  name: "ok",
  fn() {},
});
