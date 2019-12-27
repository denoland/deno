const { stderr } = Deno;

stderr.writeSync(new TextEncoder().encode("x"));
