// Regression test for https://github.com/denoland/deno/issues/21282
// `deno test --coverage` should still report a dangling rejected
// promise inside a test as an uncaught error, just like a plain
// `deno test` run does.

async function throwsAsync() {
  throw new Error("oh no");
}

Deno.test("test", () => {
  throwsAsync();
});
