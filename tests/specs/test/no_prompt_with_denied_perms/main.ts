Deno.test("no prompt", { permissions: { read: false } }, async () => {
  await Deno.readTextFile("./some_file.txt");
});
