// Fractional ms values are rejected at registration so users see a clear
// error rather than the previous misleading "out of range" after silent
// flooring to 0.
Deno.test({
  name: "should never run",
  timeout: 0.5,
  fn() {},
});
