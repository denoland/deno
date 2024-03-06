Deno.bench({
  name: "before",
  fn() {},
});

Deno.bench({
  only: true,
  name: "only",
  fn() {},
});

Deno.bench({
  name: "after",
  fn() {},
});
