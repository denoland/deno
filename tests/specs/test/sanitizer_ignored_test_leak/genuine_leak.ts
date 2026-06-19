// Verify the ignorelist for sanitizer-ignoring tests does NOT suppress a
// genuine leak introduced by a later, sanitized test.

let ignoredTimerId: ReturnType<typeof setTimeout>;

Deno.test(
  { sanitizeOps: false, sanitizeResources: false },
  function ignoresSanitizersAndLeaksTimer() {
    ignoredTimerId = setTimeout(() => {}, 100000);
  },
);

Deno.test(function laterTestLeaksItsOwnTimer() {
  clearTimeout(ignoredTimerId);
  // This timer is started (and leaked) by this test, which has sanitizers
  // enabled, so it must be reported as a leak.
  setTimeout(() => {}, 100000);
});
