// Negative timeout values must be rejected at registration time.
Deno.test({
  name: "should never run",
  timeout: -1,
  fn() {},
});
