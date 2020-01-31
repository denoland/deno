function echo(args: string[]): void {
  const msg = args.join(", ");
  Deno.stdout.write(new TextEncoder().encode(msg));
}

echo(Deno.args);
