function args(args: string[]) {
  const map = {};
  for (let i = 0; i < args.length; i++) {
    map[i] = args[i];
  }
  Deno.stdout.write(new TextEncoder().encode(JSON.stringify(map)));
}

args(Deno.args.slice(1));
