// Regression for denoland/deno#33852: a pending-promise deadlock in a test
// lifecycle hook (here `afterAll`) surfaces *after* every test has already
// passed, so it bubbles up as a bare top-level error instead of a per-test
// failure. The error message should still include actionable guidance.
Deno.test("passes", () => {});

Deno.test.afterAll(() => {
  // Never resolves, and nothing else keeps the event loop alive, so the
  // deadlock detector trips once all tests have finished.
  return new Promise(() => {});
});
