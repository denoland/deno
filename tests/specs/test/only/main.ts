Deno.test({
  name: "before",
  fn() {},
});

Deno.test({
  only: true,
  name: "only",
  fn() {},
});

Deno.test.only({
  name: "only2",
  fn() {},
});

Deno.test({
  name: "after",
  fn() {},
});
