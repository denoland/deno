// A `only` test that passes should still report the "only" notice, since the
// run only "failed" because the `only` filter was applied.
Deno.test.only("passes", () => {});
