// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// TODO(bartlomieju): enable linter
/* eslint-disable @typescript-eslint/explicit-function-return-type */

import "./unit_tests.ts";

import { permissionCombinations } from "./test_util.ts";

async function main(): Promise<void> {
  console.log(
    "discovered permission combinations:",
    permissionCombinations.size
  );

  for (let comb of permissionCombinations) {
    comb = JSON.parse(comb);
    console.log("perm comb", comb);

    const permFlags = Object.keys(comb)
      .map(permName => {
        if (comb[permName]) {
          const snakeCase = permName.replace(
            /\.?([A-Z])/g,
            (x, y) => `-${y.toLowerCase()}`
          );
          return `--allow-${snakeCase}`;
        }
      })
      .filter(el => !!el);

    console.log("permFlags", permFlags);

    const args = [Deno.execPath, "run", ...permFlags, "js/unit_tests.ts"];

    console.log("args", args);

    const p = Deno.run({
      args,
      stdout: "inherit"
    });
    const status = await p.status();
    console.log("status ", status);
  }
}

main();
