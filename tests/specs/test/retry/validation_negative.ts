// Negative retry/repeats values must be rejected at registration time.
Deno.test({
  name: "should never run",
  retry: -1,
  fn() {},
});
