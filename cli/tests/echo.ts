function echo(args: string[]): void {
  const msg = args.join(", ");
  Deno.stdout.writeSync(new TextEncoder().encode(msg));
}

echo(Deno.args);
