const { stderr } = Deno;

await stderr.write(new TextEncoder().encode("x"));
await stderr.flush();
