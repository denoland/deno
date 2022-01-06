Deno.test("leak", function () {
  Deno.open("001_hello.js");
  Deno.stdin.close();
});
