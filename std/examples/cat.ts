// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const filenames = Deno.args.slice(1);
for (const filename of filenames) {
  const file = await Deno.open(filename);
  await Deno.copy(Deno.stdout, file);
  file.close();
}
