Deno.test.disableSanitizers();

Deno.test("leak", { sanitizeResources: true }, function () {
  Deno.openSync("../../../testdata/run/001_hello.js");
  Deno.stdin.close();
});
