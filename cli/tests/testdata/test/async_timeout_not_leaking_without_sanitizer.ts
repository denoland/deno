Deno.test({
  name: "will not rely on sanitizeOps to sanitize the test's timeout",
  fn: async () => {},
  sanitizeOps: false,
});

Deno.test({
  name:
    "will pass because the previous test can still clear its own timeout without sanitizeOps",
  fn: async () => {},
});
