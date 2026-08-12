// The --retry flag provides a default retry count for tests that don't set
// their own `retry` option.
let attempts = 0;
Deno.test("flaky", () => {
  if (++attempts < 2) {
    throw new Error("not yet");
  }
});
