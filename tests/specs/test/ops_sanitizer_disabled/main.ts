Deno.test.disableSanitizers();

Deno.test("no-op", function () {});
Deno.test({
  name: "leak interval",
  fn() {
    setInterval(function () {}, 100000);
  },
});
