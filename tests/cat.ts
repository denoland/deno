import { stdout, open, copy, args } from "deno";

async function main() {
  for (let i = 1; i < args.length; i++) {
    let filename = args[i];
    let file = await open(filename);
    await copy(stdout, file);
  }
}

main();
