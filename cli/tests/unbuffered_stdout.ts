const { stdout } = Deno;

await stdout.write(new TextEncoder().encode("a"));
await stdout.flush();
