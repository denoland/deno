Deno.test("exit", { sanitizeExit: false }, function () {
  console.log("output before exit");
  Deno.exit(42);
});

Deno.test("never runs", function () {
  console.log("this test should never run");
});
