function echo(args: string[]) {
  const msg = args.join(", ");
  Deno.stdout.write(new TextEncoder().encode(msg));
}

echo(Deno.args);
