Deno.test("leak", function () {
  Deno.openSync("run/001_hello.js");
  Deno.stdin.close();
});
