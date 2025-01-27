Deno.test.disableSanitizers();

Deno.test("no-op", function () {});
Deno.test({
  name: "leak interval",
  sanitizeOps: true,
  fn() {
    setInterval(function () {}, 100000);
  },
});
