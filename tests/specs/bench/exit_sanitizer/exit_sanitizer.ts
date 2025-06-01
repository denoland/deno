Deno.bench("exit(0)", function () {
  Deno.exit(0);
});

Deno.bench("exit(1)", function () {
  Deno.exit(1);
});

Deno.bench("exit(2)", function () {
  Deno.exit(2);
});
