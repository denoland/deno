await Deno.writeTextFile("target.txt", "x");
try {
  await Deno.symlink("target.txt", "link.txt");
  console.log("ok");
} catch (e) {
  console.log(`${(e as Error).constructor.name}: ${(e as Error).message}`);
}
