Deno.test("leak", function () {
  Deno.openSync("../../../testdata/run/001_hello.js");
  Deno.stdin.close();
});
