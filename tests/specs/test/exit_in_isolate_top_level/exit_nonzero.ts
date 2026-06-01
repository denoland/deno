// `Deno.exit(N)` for a non-zero `N` at module top level should fail this
// test specifier (since user code asked to exit unsuccessfully) but other
// specifiers in the run should still execute and report normally.
Deno.exit(7);

Deno.test("never registers", () => {
  console.log("should not run");
});
