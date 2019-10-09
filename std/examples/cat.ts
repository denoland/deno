// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
async function cat(filenames: string[]): Promise<void> {
  for (const filename of filenames) {
    const file = await Deno.open(filename);
    await Deno.copy(Deno.stdout, file);
    file.close();
  }
}

cat(Deno.args.slice(1));
