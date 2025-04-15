Deno.test("no prompt", async () => {
  await Deno.readTextFile("./some_file.txt");
});
