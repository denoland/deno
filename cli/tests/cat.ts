async function main(): Promise<void> {
  for (let i = 1; i < Deno.args.length; i++) {
    const filename = Deno.args[i];
    const file = await Deno.open(filename);
    await Deno.copy(file, Deno.stdout);
  }
}

main();
