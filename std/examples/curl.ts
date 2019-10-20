// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
async function curl(url: string): Promise<void> {
  const res = await fetch(url);
  await Deno.copy(Deno.stdout, res.body);
}

await curl(Deno.args[1]);
Deno.exit(0);
