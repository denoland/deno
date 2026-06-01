Deno.test("resource leak", () => {
  Deno.openSync("deno_no_sanitizers.json");
});
