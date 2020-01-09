async function echox(args: string[]) {
  for (const arg of args) {
    await Deno.stdout.write(new TextEncoder().encode(arg));
  }
  Deno.exit(0);
}

echox(Deno.args);
