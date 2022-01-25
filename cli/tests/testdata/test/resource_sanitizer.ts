Deno.test("leak", function () {
  Deno.openSync("001_hello.js");
  Deno.stdin.close();
});
