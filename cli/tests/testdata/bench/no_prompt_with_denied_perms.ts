Deno.bench("no prompt", { permissions: { read: false } }, () => {
  Deno.readTextFile("./some_file.txt");
});
