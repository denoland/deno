Deno.test({
  name: "abc",
  fn() {},
});

Deno.test({
  only: true,
  name: "def",
  fn() {},
});

Deno.test({
  name: "ghi",
  fn() {},
});
