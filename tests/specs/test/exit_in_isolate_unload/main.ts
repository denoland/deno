// Regression test for https://github.com/denoland/deno/issues/12888
// `Deno.exit()` called from outside any test function (here, an `unload`
// listener) used to terminate the whole `deno test` process with the
// requested code. The failed test would silently disappear behind a zero
// exit code. The expected behavior is for the isolate to be torn down while
// the existing test results stand.
Deno.test("fail", () => {
  throw new Error("fail");
});

self.onunload = () => {
  Deno.exit(0);
};
