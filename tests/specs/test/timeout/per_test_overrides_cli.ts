// Per-test `timeout` takes precedence over --timeout=10000 — the test
// below uses 100ms and times out, even though --timeout would allow 10s.
Deno.test({
  name: "per-test wins",
  timeout: 100,
  async fn() {
    await new Promise((resolve) => setTimeout(resolve, 1000));
  },
});
