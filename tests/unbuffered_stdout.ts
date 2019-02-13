const { stdout } = Deno;

stdout.write(new TextEncoder().encode("a"));
