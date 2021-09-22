Deno.test("no-op", function () {});
Deno.test("leak interval", function () {
  setInterval(function () {});
});
