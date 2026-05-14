console.log((await Deno.readTextFile("assets/included.txt")).trim());
await Deno.remove("assets/excluded.txt");
try {
  await Deno.readTextFile("assets/excluded.txt");
  console.log("excluded file was embedded");
} catch (err) {
  console.log(err instanceof Deno.errors.NotFound ? "excluded" : err.name);
}
