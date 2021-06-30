Deno.test({
  name: "before",
  fn() {},
});

Deno.test({
  only: true,
  name: "only",
  fn() {},
});

Deno.test({
  name: "after",
  fn() {},
});
