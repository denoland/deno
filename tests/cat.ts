const { stdout, open, copy, args } = Deno;

async function main() {
  for (let i = 1; i < args.length; i++) {
    const filename = args[i];
    const file = await open(filename);
    await copy(stdout, file);
  }
}

main();
