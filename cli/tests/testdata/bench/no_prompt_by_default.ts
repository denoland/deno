Deno.bench("no prompt", () => {
  Deno.readTextFile("./some_file.txt");
});
