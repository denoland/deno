const encoder = new TextEncoder();
const pending = [
  Deno.stdout.write(encoder.encode("done\n")),
  Deno.stdout.write(encoder.encode("done\n")),
];

await Promise.all(pending);
await Deno.stdout.write(encoder.encode("complete\n"));
