// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { evaluate, instantiate, load } from "./utils.ts";

async function main(args: string[]): Promise<void> {
  const text = await load(args);
  const result = evaluate(text);
  instantiate(...result);
}

main(Deno.args);
