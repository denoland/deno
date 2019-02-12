const { stderr } = Deno;

stderr.write(new TextEncoder().encode("x"));
