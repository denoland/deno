const encoder = new TextEncoder();
const pending = [
  Deno.stdout.write(encoder.encode("Hello, ")),
  Deno.stdout.write(encoder.encode("world!")),
];

await Promise.all(pending);
await Deno.stdout.write(encoder.encode("\n"));
