// Regression coverage for #13146: when an async test never resolves, it
// must time out and remaining tests in the same file must still run.
Deno.test({
  name: "hangs forever",
  timeout: 100,
  fn() {
    return new Promise(() => {});
  },
});

Deno.test("runs after the hang", () => {});
