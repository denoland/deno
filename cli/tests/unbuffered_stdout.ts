const { stdout } = Deno;

stdout.writeSync(new TextEncoder().encode("a"));
