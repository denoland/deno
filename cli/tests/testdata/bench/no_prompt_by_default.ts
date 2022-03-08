Deno.bench("no prompt", async () => {
  await Deno.readTextFile("./some_file.txt");
});
