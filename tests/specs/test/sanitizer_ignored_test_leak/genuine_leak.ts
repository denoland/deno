// Verify the ignorelist for sanitizer-ignoring tests does NOT suppress a
// genuine leak introduced by a later, sanitized test.

Deno.test(
  { sanitizeOps: false, sanitizeResources: false },
  function ignoresSanitizersAndLeaksTimer() {
    const timer = setTimeout(() => {}, 100000);
    Deno.unrefTimer(timer);
  },
);

Deno.test(function laterTestLeaksItsOwnTimer() {
  // This timer is started (and leaked) by this test, which has sanitizers
  // enabled, so it must be reported as a leak.
  const timer = setTimeout(() => {}, 100000);
  Deno.unrefTimer(timer);
});
