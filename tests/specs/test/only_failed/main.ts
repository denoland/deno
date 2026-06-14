// When a test marked with `only` fails on its own, the run should report the
// actual failure rather than the misleading "only" notice.
Deno.test.only("fails on its own", () => {
  throw new Error("boom");
});
