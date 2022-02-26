Deno.test("no prompt", () => {
  Deno.readTextFile("./some_file.txt");
});
