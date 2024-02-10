Deno.test("exit(0)", function () {
  Deno.exit(0);
});

Deno.test("exit(1)", function () {
  Deno.exit(1);
});

Deno.test("exit(2)", function () {
  Deno.exit(2);
});
