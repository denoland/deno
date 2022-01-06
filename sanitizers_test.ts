Deno.test("foo", () => {
  Deno.openSync("README.md");
  Deno.stdin.close();
});
