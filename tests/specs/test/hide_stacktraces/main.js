Deno.test("assert a b", () => {
  class AssertionError extends Error {
    name = "AssertionError";
  }
  throw new AssertionError(
    "Values are not equal.\n\n\n    [Diff] Actual / Expected\n\n\n-   foo\n+   bar\n\n",
  );
});
