Deno.test("no-op", function () {});
Deno.test({
  name: "leak interval",
  // regression test for sanitizer errors being swallowed with permissions.
  // https://github.com/denoland/deno/pull/18550
  permissions: {},
  fn() {
    setInterval(function () {}, 100000);
  },
});
