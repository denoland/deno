// Negative `repeats` values must be rejected at registration time, just like
// `retry`.
Deno.test({
  name: "should never run",
  repeats: -1,
  fn() {},
});
