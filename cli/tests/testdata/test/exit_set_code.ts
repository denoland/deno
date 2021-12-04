Deno.test("ok", function () {
  // pass
});

Deno.core.opSync("op_set_exit_code", 42);