// The --repeats flag provides a default repeats count for tests that don't set
// their own `repeats` option. With --repeats=2 the test runs 3 times; this one
// fails on the 2nd run, so the whole test fails (a single run would not).
let runs = 0;
Deno.test("repeated via flag", () => {
  runs++;
  if (runs === 2) {
    throw new Error("second run fails");
  }
});
