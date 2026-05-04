// Realistic async hang: a pending op (setTimeout) that won't fire within
// the test's 100ms deadline. The timeout should mark this test FAILED and
// the next test in the same file must still run.
//
// Note: a bare `new Promise(() => {})` would instead trip deno_core's
// "Promise resolution is still pending but the event loop has already
// resolved" deadlock detector before the timer can fire, which routes
// through a different (specifier-level) error path. That edge of #13146
// is left to a follow-up PR.
Deno.test({
  name: "hangs forever",
  timeout: 100,
  async fn() {
    await new Promise((resolve) => setTimeout(resolve, 1_000_000));
  },
});

Deno.test("runs after the hang", () => {});
