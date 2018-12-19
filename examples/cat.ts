import * as deno from "deno";

async function cat(filenames: string[]): Promise<void> {
  for (let filename of filenames) {
    let file = await deno.open(filename);
    await deno.copy(deno.stdout, file);
    file.close();
  }
}

cat(deno.args.slice(1));
