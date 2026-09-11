const content = await Deno.readTextFile(
  new URL("./main.js.snapshot", import.meta.url),
);
await Deno.stdout.write(new TextEncoder().encode(content));
