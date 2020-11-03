// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { parse } from "../../yaml.ts";

(() => {
  const yml = Deno.readFileSync(`${Deno.cwd()}/example/sample_document.yml`);

  const document = new TextDecoder().decode(yml);
  // deno-lint-ignore no-explicit-any
  const obj = parse(document) as Record<string, any>;
  console.log(obj);

  let i = 0;
  for (const o of Object.values(obj)) {
    console.log(`======${i}`);
    for (const [key, value] of Object.entries(o)) {
      console.log(key, value);
    }
    i++;
  }
})();
