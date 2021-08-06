import { copy } from "../../../test_util/std/io/util.ts";
async function main() {
  for (let i = 1; i < Deno.args.length; i++) {
    const filename = Deno.args[i];
    const file = await Deno.open(filename);
    await copy(file, Deno.stdout);
  }
}

main();
