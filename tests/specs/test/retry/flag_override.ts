// A per-test `retry` option takes precedence over the `--retry` flag default.
// The test sets `retry: 1` (2 attempts) and always fails, so it FAILS after
// the second attempt. If the `--retry=5` flag default had applied instead,
// there would be six attempts and five "retrying" lines.
Deno.test({
  name: "override",
  retry: 1,
  fn() {
    throw new Error("always");
  },
});
