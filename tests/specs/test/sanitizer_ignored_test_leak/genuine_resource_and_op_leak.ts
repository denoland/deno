// Verify the ignorelist for sanitizer-ignoring tests does NOT suppress genuine
// resource and async op leaks introduced by a later, sanitized test.

Deno.test(
  { sanitizeOps: false, sanitizeResources: false },
  function ignoresSanitizersAndLeaksActivities() {
    const listener = Deno.listen({ hostname: "127.0.0.1", port: 0 });
    listener.accept().catch(() => {});
    setTimeout(() => {}, 100000);
  },
);

Deno.test(function laterTestLeaksItsOwnResourceAndOp() {
  const listener = Deno.listen({ hostname: "127.0.0.1", port: 0 });
  listener.accept().catch(() => {});
});
