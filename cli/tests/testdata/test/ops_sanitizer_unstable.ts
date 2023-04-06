Deno.test("no-op", function () {});
Deno.test({
  name: "leak interval",
  // regression test for sanitizer errors being swallowed with permissions.
  permissions: {},
  fn() {
    setInterval(function () {}, 100000);
  },
});
