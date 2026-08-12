// Sanity check: a regular test file that runs alongside the isolate-exit
// files should still run and produce normal output. This guarantees that the
// isolate-exit in a sibling file did not kill the entire test run.
Deno.test("passes", () => {});
