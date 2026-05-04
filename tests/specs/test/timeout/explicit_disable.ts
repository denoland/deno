// Per-test `timeout: 0` overrides --timeout=50 to disable the deadline.
Deno.test({
  name: "would otherwise time out",
  timeout: 0,
  async fn() {
    await new Promise((resolve) => setTimeout(resolve, 200));
  },
});
